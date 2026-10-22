//! Liveness and garbage collection: which objects the program can still reach (the state's
//! names, the callers' names, the frames' modules and cells, and what the analyzer's own side
//! tables hold: `gc_pinned`), and the collectors at returns, loop exits and try exits that drop
//! the facts of objects made since a mark that nothing reaches.

use rustc_hash::{FxHashMap, FxHashSet};

use super::heap::HeapMark;
use super::{Analyzer, Payload};
use crate::domain::{kv_join, KeyVal, KeysOf, Ns, ObjId, Value};
use super::FrameKind;
use crate::intern::{KeyId, StrId};
use crate::state::{view_of, Kind, State};

impl Analyzer {
    /// May the program still hold `oid` on this path? Every allocation records the object's
    /// length entry in the state, and only the collector (at returns and loop exits) forgets
    /// an object's facts: an object without them was never made on this path or was
    /// collected, and needs no reachability walk.
    pub fn has_facts(&self, oid: ObjId, state: &State) -> bool {
        state.lens.get(&oid).is_some() || state.has_row(oid, Ns::Item) || state.has_row(oid, Ns::Attr) || state.slots.has_key(&oid)
    }

    /// The roots of a reachability walk: the names, narrowed values and result of `state`, the
    /// names of the first `envs` callers, and the modules and cells of the first `frames`
    /// frames.
    pub(in crate::interp) fn state_roots(&self, state: &State, envs: usize, frames: usize, work: &mut Vec<ObjId>) {
        // (a walk takes the last pushed first: the running code's names before the modules'
        // globals, where an object made again is most often still held)
        for fr in self.cx.frames.iter().take(frames) {
            work.push(fr.module);
            work.extend(fr.cells.values().copied());
        }
        let callers = self.cx.env_stack.iter().take(envs).flat_map(|e| e.values());
        for v in callers.chain(state.ret.iter()).chain(state.narrowed.values()).chain(state.env.values()) {
            v.push_objs(work);
        }
    }

    /// Which of `targets` the program can still reach (one walk from the roots for all; it
    /// stops when every one is found).
    pub(in crate::interp) fn live_among(&self, targets: &[ObjId], state: &State) -> FxHashSet<ObjId> {
        let mut work: Vec<ObjId> = Vec::new();
        self.state_roots(state, usize::MAX, usize::MAX, &mut work);
        // (a walk over a large heap is bounded: past the budget the targets not found yet are
        // taken as live, which is always sound: their site makes a recent / variant object; a
        // walk that proves an object dead on a large heap visits all of it, and on email's 20k
        // visits per walk gave the same verdicts as 500)
        const WALK_BUDGET: usize = 200;
        let mut sorted = targets.to_vec();
        sorted.sort_unstable();
        self.find_among(work, &sorted, WALK_BUDGET, true, &mut |o, work| self.obj_edges(o, state, work)).unwrap_or_else(|_| targets.iter().copied().collect())
    }

    /// Which of `targets` (sorted) a walk from `work` reaches, `edges` pushing what each object
    /// refers to (one walk for all; it stops when every one is found); Err (what it found by
    /// then) past `limit` objects. `count`: its visits are walk visits (`Budget::walk_visit`).
    pub(in crate::interp) fn find_among(&self, mut work: Vec<ObjId>, targets: &[ObjId], limit: usize, count: bool, edges: &mut dyn FnMut(ObjId, &mut Vec<ObjId>)) -> Result<FxHashSet<ObjId>, FxHashSet<ObjId>> {
        let mut found: FxHashSet<ObjId> = FxHashSet::default();
        let mut seen: FxHashSet<ObjId> = FxHashSet::default();
        while let Some(o) = work.pop() {
            if !seen.insert(o) {
                continue;
            }
            if seen.len() > limit {
                return Err(found);
            }
            if count {
                self.budget.walk_visit();
            }
            if targets.binary_search(&o).is_ok() && found.insert(o) && found.len() == targets.len() {
                break;
            }
            edges(o, &mut work);
        }
        Ok(found)
    }

    /// The objects `o` refers to: its slots and element kinds in `state`, its payload, and what
    /// side tables keyed by it hold (a ChainMap's maps, a defaultdict's factory, …).
    pub fn obj_edges(&self, o: ObjId, state: &State, work: &mut Vec<ObjId>) {
        // (the slot table in place: a walk visits the whole reachable heap)
        state.slots.each(&o, &mut |_, v| v.push_objs(work));
        match state.elems.get_ref(&o) {
            Some(KeyVal::KeysOf(ko)) => work.extend(ko.objs.iter()),
            _ => {}
        }
        for (lo, v) in state.lasts.iter() {
            if *lo == o {
                v.push_objs(work);
            }
        }
        if !self.heap.has_obj(o) {
            return;
        }
        // (a loop's item is one of the records its summary stands for: the summary's facts
        // take its changes)
        work.extend(view_of(o));
        match &self.heap.obj(o).payload {
            Payload::Func(fi) => {
                work.extend(fi.closure.values().copied());
                // (an `lru_cache` function's cache: the results it keeps)
                if fi.is_memo.get() {
                    work.extend(self.heap.find(Kind::Dict, &format!("memo:{}", o), ""));
                }
            }
            Payload::Cls(c) => work.push(*c),
            Payload::Method(_, v) | Payload::View(_, v) | Payload::Enumerate(v, _) => v.push_objs(work),
            Payload::Bound(f, v) | Payload::Super(f, v) => {
                work.push(*f);
                v.push_objs(work);
            }
            Payload::Zip(a, b) | Payload::Pair(a, b) => {
                a.push_objs(work);
                b.push_objs(work);
            }
            Payload::Partial(f, args, kw) => {
                f.push_objs(work);
                for a in args {
                    a.push_objs(work);
                }
                for v in kw.values() {
                    v.push_objs(work);
                }
            }
            Payload::Groupby(fs, _, _, _) => work.extend(fs.iter().copied()),
            Payload::Locidx(_, f) => work.push(*f),
            Payload::Deferred(d) => {
                d.f.push_objs(work);
                work.extend(d.apps.iter().map(|a| a.0));
                d.names.iter().for_each(|(_, v)| v.push_objs(work));
            }
            _ => {}
        }
        self.side_edges(Some(o), work);
    }

    /// What the analyzer's side tables keyed by an object refer to: `o`'s entries, or every
    /// entry's (None: the collector's roots, `gc_pinned`).
    pub(super) fn side_edges(&self, o: Option<ObjId>, work: &mut Vec<ObjId>) {
        fn each<'a, V>(m: &'a FxHashMap<ObjId, V>, o: Option<ObjId>) -> Box<dyn Iterator<Item = &'a V> + 'a> {
            match o {
                Some(o) => Box::new(m.get(&o).into_iter()),
                None => Box::new(m.values()),
            }
        }
        each(&self.m.chainmaps, o).flatten().for_each(|m| m.push_objs(work));
        for m in [&self.m.dd_factories, &self.m.csv_readers, &self.m.wrapped] {
            each(m, o).for_each(|v| v.push_objs(work));
        }
        each(&self.m.partial_star, o).flatten().for_each(|v| v.push_objs(work));
        each(&self.m.lazy_src, o).flatten().for_each(|(s, _)| work.push(*s));
        if o.is_some() {
            work.extend(self.heap.copy_root.get(&o.unwrap()).copied());
        }
        for m in [&self.m.attr_mirrors, &self.m.sql_cursors] {
            work.extend(each(m, o).copied());
        }
        work.extend(each(&self.entry.instances, o).flatten());
        work.extend(each(&self.entry.general_instances, o).flatten());
        work.extend(each(&self.entry.any_instances, o).flatten());
        each(&self.m.sorted_items, o).flat_map(|items| items.iter()).for_each(|(_, v, _)| v.push_objs(work));
        for (f, args, kw) in each(&self.m.exit_callbacks, o).flatten() {
            f.push_objs(work);
            args.iter().for_each(|a| a.push_objs(work));
            kw.values().for_each(|v| v.push_objs(work));
        }
        for (v, f) in each(&self.m.sdispatch, o).flatten() {
            v.push_objs(work);
            work.push(*f);
        }
        each(&self.m.fa_app_deps, o).flatten().for_each(|v| v.push_objs(work));
        each(&self.m.argp, o).for_each(|a| crate::models::argparse::push_objs(a, work));
    }

    /// Objects the analyzer itself holds beyond the state: what side tables keyed by other
    /// objects refer to, exceptions in flight, the callers' names, loop recency objects.
    /// `unwind`: for a raise state, only the first `unwind` callers' names, and no loop's recent
    /// objects (those still in use are reachable from the names) nor other raises' exceptions. `since`: a collector after
    /// that mark, which expands only objects born since (older ones may be left out).
    pub(in crate::interp) fn gc_pinned(&self, work: &mut Vec<ObjId>, unwind: Option<usize>, since: Option<u64>) {
        // (the side tables' entries: their keys themselves may be dead)
        self.side_edges(None, work);
        self.heap.push_copy_roots(since, work);
        self.held_by_analyzer(work, unwind);
    }

    /// What the analyzer holds beyond the state and its side tables (`gc_pinned`): exceptions
    /// in flight, the callers' names, the managers of running with statements, …
    pub(in crate::interp) fn held_by_analyzer(&self, work: &mut Vec<ObjId>, unwind: Option<usize>) {
        // (a raise state is one path: the exceptions other raises took to the running tries are
        // other paths')
        let recorded = self.unwind.tries.iter().filter(|_| unwind.is_none()).flat_map(|t| t.recs.iter().map(|r| &r.2));
        for v in self.unwind.cur_exc.iter().chain(recorded).chain(self.unwind.handling_values.iter()).chain(self.m.stdout_redirect.iter()).chain(std::iter::once(&self.m.fa_status)).chain(self.m.fa_gen_ret.iter()) {
            v.push_objs(work);
        }
        for c in &self.m.fa_dep_cache {
            c.values().for_each(|v| v.push_objs(work));
        }
        for (_, _, v, f) in &self.m.fa_hooks {
            v.push_objs(work);
            work.extend(f.iter().copied());
        }
        for env in self.cx.env_stack.iter().take(unwind.unwrap_or(usize::MAX)) {
            env.values().for_each(|v| v.push_objs(work));
        }
        // (the managers of the with statements running: their `__exit__` runs on the way out)
        for (_, fin, _) in &self.unwind.finally_stack {
            if let crate::interp::Finally::Exit(ms) = &**fin {
                ms.iter().for_each(|(v, _)| v.push_objs(work));
            }
        }
        for (_, env, _, _) in &self.gens.cm_frames {
            env.values().for_each(|v| v.push_objs(work));
        }
        for h in &self.gens.stop_hooks {
            h.result.iter().for_each(|v| v.push_objs(work));
        }
        for cells in &self.cx.comp_cells {
            work.extend(cells.values().copied());
        }
        for rs in self.cx.recent_stack.iter().filter(|_| unwind.is_none()) {
            work.extend(rs.iter().map(|(o, _, _, _)| *o));
        }
        for rs in &self.m.fa_out_resp {
            work.extend(rs.iter().copied());
        }
        // (the value unknown code stores: values anywhere may hold it)
        if let Some(any) = self.heap.find(Kind::Unknown, "any", "") {
            work.push(any);
        }
        for fr in &self.cx.frames {
            work.push(fr.module);
            work.extend(fr.cells.values().copied());
        }
    }

    /// Recency abstraction, back-edge step: the recent objects allocated during a loop
    /// iteration are folded into their weak per-site summary objects, and every reference
    /// to them written during the iteration is re-pointed to the summary object.
    pub(in crate::interp) fn rename_recent(&mut self, back: &mut State, head: &State, recs: Vec<(ObjId, Kind, String, String)>, mark: HeapMark) -> FxHashMap<ObjId, ObjId> {
        if recs.is_empty() {
            return FxHashMap::default();
        }
        let (map, keep_pos) = self.summary_objects(back, &recs);
        fold_recent_facts(back, &map, &keep_pos);
        rename_references(back, head, &map);
        // (a list's last item, as the summary)
        if back.lasts.iter().any(|(_, x)| rename_value(x, &map).is_some()) {
            back.lasts = std::sync::Arc::new(back.lasts.iter().map(|(o, x)| (*o, rename_value(x, &map).unwrap_or_else(|| x.clone()))).collect());
        }
        // 3. objects allocated during the iteration that hold references in their payloads
        self.heap.rename_payloads(mark, &|v: &Value| rename_value(v, &map), &map);
        self.m.rename_recent(&map);
        // 4. flow-insensitive class attributes
        let recent: Vec<ObjId> = map.keys().copied().collect();
        let ups: Vec<((ObjId, Ns, KeyId), Value)> = self.heap.cls_slots_with(&recent).into_iter().filter_map(|k| rename_value(self.heap.cls_slots.get(&k)?, &map).map(|nv| (k, nv))).collect();
        for (k, nv) in ups {
            self.heap.set_cls_slot(k, nv);
        }
        map
    }

    /// Recency per call, the return's step (`call_recent`): the call's recent objects fold into
    /// their summaries in its end state `st` (from `head`, its start) and its result `ret`,
    /// which `st` then holds.
    pub(in crate::interp) fn fold_call_recent(&mut self, st: &mut State, head: &State, ret: &mut Value) {
        let recs = std::mem::take(&mut self.frame_mut().recent);
        if let Some(super::PreCall(mark, _)) = self.frame().pre_call.clone() {
            self.fold_recent_in(st, head, mark, recs, ret, false);
        }
        st.ret = Some(ret.clone());
    }

    /// The recent objects `recs` of calls fold into their summaries in `st` (from `head` and the
    /// heap's `mark`, before they were made) and in `v`: at a call's return, and in the state a raise takes out of
    /// the calls. One nothing refers to (a value written since `head`, `v`, a payload, a class
    /// attribute; `names`: the state's names, else they are the returning call's) is garbage:
    /// its facts go without joining its summary.
    pub(in crate::interp) fn fold_recent_in(&mut self, st: &mut State, head: &State, mark: HeapMark, recs: Vec<(ObjId, Kind, String, String)>, v: &mut Value, names: bool) {
        if recs.is_empty() {
            return;
        }
        let rs: Vec<ObjId> = recs.iter().map(|r| r.0).collect();
        let held = self.referred(st, head, &rs, v, mark, names);
        let (recs, garbage): (Vec<_>, Vec<_>) = recs.into_iter().partition(|r| held.contains(&r.0));
        for (r, ..) in garbage {
            for ns in [Ns::Item, Ns::Attr] {
                st.rows.remove(&(r, ns));
            }
            st.drop_obj(r);
            st.elems.remove(&r);
            st.forget_len(&r);
        }
        let map = self.rename_recent(st, head, recs, mark);
        for r in map.keys() {
            st.forget_len(r);
        }
        if let Some(nv) = rename_value(v, &map) {
            *v = nv;
        }
        // (a test's facts on them go: they are other objects' when made again)
        let mentions = |f: &crate::state::CondFact| f.deps.iter().any(|(_, x)| rs.iter().any(|r| x.pts.contains(*r))) || f.rows.iter().any(|((o, _), _, _)| rs.contains(o)) || f.guards.iter().any(|((o, _), _)| rs.contains(o));
        if st.facts.iter().any(|f| mentions(f)) {
            st.facts = std::sync::Arc::new(st.facts.iter().filter(|f| !mentions(f)).cloned().collect());
        }
    }

    /// Which of `rs` (made since `mark`) a value refers to: `v`, the names (`names`), a slot or
    /// elements written since `head`, a payload given since `mark`, a class attribute.
    fn referred(&self, st: &State, head: &State, rs: &[ObjId], v: &Value, mark: HeapMark, names: bool) -> FxHashSet<ObjId> {
        if !self.heap.cls_slots_with(rs).is_empty() {
            return rs.iter().copied().collect();
        }
        let mut work: Vec<ObjId> = Vec::new();
        v.push_objs(&mut work);
        for x in st.env.values().chain(st.narrowed.values()).chain(st.ret.iter()).filter(|_| names) {
            x.push_objs(&mut work);
        }
        for (_, x) in st.lasts.iter() {
            x.push_objs(&mut work);
        }
        let fp = st.footprint(head);
        for (o, sk) in fp.slots.iter() {
            if let Some(x) = st.slots.get(o, sk) {
                x.push_objs(&mut work);
            }
        }
        for o in fp.elems.iter() {
            if let Some(KeyVal::KeysOf(ko)) = st.elems.get(o) {
                work.extend(ko.objs.iter());
            }
        }
        let empty = State::bottom();
        for o in self.heap.payloads_since(mark) {
            self.obj_edges(o, &empty, &mut work);
        }
        work.into_iter().filter(|o| rs.contains(o)).collect()
    }

    /// The weak per-site summary object of each recent object (`recs`), with its length; the
    /// tuples whose positions the summary keeps (one length).
    fn summary_objects(&mut self, back: &mut State, recs: &[(ObjId, Kind, String, String)]) -> (FxHashMap<ObjId, ObjId>, FxHashSet<ObjId>) {
        let mut map: FxHashMap<ObjId, ObjId> = FxHashMap::default();
        // (tuples and lists of one length keep their positions in the summary object: each
        // position the join of theirs — unpacking `a, b = t` then keeps them apart, a summary
        // block's `nn.Sequential` its layers)
        let mut keep_pos: FxHashSet<ObjId> = FxHashSet::default();
        for (r, kind, site, ctx) in recs {
            if let Some(s) = view_of(*r) {
                back.forget_len(r);
                map.insert(*r, s);
                continue;
            }
            let (label, payload, iter_elem, derived, depth) = {
                let o = self.heap.obj(*r);
                (o.label.clone(), o.payload.clone(), o.iter_elem, o.derived_from, o.depth)
            };
            let sid = self.heap.alloc_raw(*kind, site, ctx, false, &label, payload);
            {
                let so = self.heap.obj_mut(sid);
                so.single = false;
                so.iter_elem = iter_elem;
                so.depth = depth;
                if so.derived_from.is_none() {
                    so.derived_from = derived;
                }
            }
            let rlen = back.lens.get(r).and_then(|l| l.0);
            let prev = back.lens.get(&sid);
            let same_len = match (rlen, prev) {
                (Some(n), None) => Some(n),
                (Some(n), Some((Some(m), _, _))) if n == m => Some(n),
                _ => None,
            };
            match same_len.filter(|_| matches!(kind, Kind::Tuple | Kind::List)) {
                Some(n) => {
                    back.set_lens(sid, (Some(n), n, 0));
                    keep_pos.insert(*r);
                }
                None => {
                    back.set_lens(sid, (None, 0, 0));
                    back.drop_obj_ns(sid, Ns::Pos);
                }
            }
            map.insert(*r, sid);
        }
        (map, keep_pos)
    }

    /// A `return` of a value without objects (`return None`) in a call: the call's
    /// own objects that nothing older can reach are garbage on this path; forgetting them
    /// lets the join of the exits keep what the other paths say of them (a record returned
    /// on another path keeps that path's keys).
    pub(super) fn drop_garbage_at_return(&mut self, v: &Value, state: &mut State) {
        let Some(crate::interp::PreCall(hm, pre)) = self.frame().pre_call.clone() else { return };
        let mark = hm.serial;
        if !matches!(self.frame().kind, FrameKind::Function) {
            return;
        }
        let mut roots: Vec<u32> = Vec::new();
        v.push_objs(&mut roots);
        self.collect_since(mark, &pre, roots, state, None);
    }

    /// Objects made since the allocation serial `mark` that no root reaches lose their state
    /// (`drop_garbage_at_return`; at a loop's exit the roots are the frame's live names).
    /// `unwind`: a raise state, which only the first `unwind` callers' names outlive.
    pub(in crate::interp) fn collect_since(&mut self, mark: u64, pre: &State, roots: Vec<u32>, state: &mut State, unwind: Option<usize>) {
        // (nothing made since: nothing to collect)
        if self.heap.serial() <= mark {
            return;
        }
        let young = |an: &Analyzer, o: u32| !an.heap.born_before(o, mark);
        let fp = state.footprint(pre);
        let mut fresh: rustc_hash::FxHashSet<u32> = Default::default();
        for o in fp.rows.iter().map(|(o, _)| *o).chain(fp.slots.iter().map(|(o, _)| *o)).chain(fp.elems.iter().copied()) {
            if young(self, o) {
                fresh.insert(o);
            }
        }
        if fresh.is_empty() || fresh.len() > 4096 {
            return;
        }
        // roots: the returned value, what older objects' changed slots hold (globals,
        // parameters, cells, …), and what the analyzer holds beyond the state
        let mut work: Vec<u32> = roots;
        for (o, sk) in fp.slots.iter() {
            if !young(self, *o) {
                if let Some(val) = state.slots.get(o, sk) {
                    work.extend(val.pts.iter());
                    if let KeyVal::KeysOf(ko) = &val.kv {
                        work.extend(ko.objs.iter());
                    }
                }
            }
        }
        for o in fp.elems.iter().filter(|o| !young(self, **o)) {
            if let Some(KeyVal::KeysOf(ko)) = state.elems.get(o) {
                work.extend(ko.objs.iter());
            }
        }
        self.gc_pinned(&mut work, unwind, Some(mark));
        // (unknown objects stand for objects outside the call — a parameter's record, an
        // input's part: what they hold is reachable from there)
        // (a loop's item is not found again by a path: garbage when nothing holds it)
        let input_part = |an: &Analyzer, o: ObjId| matches!(an.heap.kind(o), Kind::Unknown) && view_of(o).is_none();
        work.extend(fresh.iter().copied().filter(|o| input_part(self, *o)));
        // (older objects reach young ones only through those roots: a young one was made
        // since, so whatever refers to it was written since; only young ones expand)
        let mut reached: rustc_hash::FxHashSet<u32> = Default::default();
        // (only young objects go into `reached`: the walk stops at older ones, and only young
        // ones are looked up after it)
        while let Some(o) = work.pop() {
            if !young(self, o) || !reached.insert(o) {
                continue;
            }
            self.budget.walk_visit();
            self.obj_edges(o, state, &mut work);
        }
        // (a part of an input — an unknown object, a placeholder — is found again by its path
        // whenever that path is read: its rows belong to the input, never garbage)
        for o in fresh {
            if !reached.contains(&o) && !input_part(self, o) {
                // (a suspended generator dropped: closed — in a raise state maybe, as the
                // traceback keeps the frames it leaves until its handler ends)
                if self.gens.closable.contains_key(&o) && self.may_close(o, state) {
                    let how = if unwind.is_some() { super::genclose::Close::Maybe } else { super::genclose::Close::Sure };
                    self.close_gen(o, state, how);
                }
                state.forget_obj(o);
            }
        }
    }

    /// How many objects `drop_unreached` walks before it takes its target as held (a constructor
    /// whose parts are a large graph, in a try around much: timm's models).
    const DROP_WALK: usize = 256;
    /// How many raise points of one construction `drop_unreached` looks at before it takes the
    /// object as held (each looks at what the analyzer holds: timm's constructors raise often).
    const DROP_LOOKS: u32 = 4;

    /// Whether `target`, an instance under construction (`fresh_init`), is held by what
    /// `roots` (and what the analyzer holds, callers' names up to `envs`) reach in `state`: if
    /// not, it loses its state there.
    /// Older objects reach it only through slots written since its construction began (gathered
    /// from look to look); the walk expands only younger objects and ends when it finds the
    /// target, or past `DROP_WALK` objects (then it is held, and stays so).
    pub(in crate::interp) fn drop_unreached(&mut self, target: ObjId, roots: Vec<ObjId>, state: &mut State, envs: usize) {
        let Some(mut b) = self.cx.fresh_init.remove(&target) else { return };
        // (what was written since the last look: holders of the target, older objects' values)
        let fp = state.footprint(&b.at);
        for (o, sk) in fp.slots.iter() {
            let Some(val) = state.slots.get(o, sk) else { continue };
            if val.pts.contains(target) && *o != target {
                b.holders.push(*o);
            }
            if self.heap.born_before(*o, b.mark) {
                val.push_objs(&mut b.roots);
            }
        }
        for o in fp.elems.iter() {
            if let Some(KeyVal::KeysOf(ko)) = state.elems.get(o) {
                if ko.objs.contains(target) {
                    b.holders.push(*o);
                }
                if self.heap.born_before(*o, b.mark) {
                    b.roots.extend(ko.objs.iter());
                }
            }
        }
        b.roots.sort_unstable();
        b.roots.dedup();
        b.at = state.copy();
        b.looks += 1;
        // (the roots without what the analyzer holds first: those are many, so a construction
        // looked at many times is taken as held from then on)
        let mut work: Vec<ObjId> = roots.into_iter().chain(b.roots.iter().copied()).collect();
        b.held = b.looks > Self::DROP_LOOKS || self.reaches_young(&b, target, &mut work, state);
        if !b.held {
            self.gc_pinned(&mut work, Some(envs), None);
            b.held = self.reaches_young(&b, target, &mut work, state);
        }
        if !b.held {
            state.forget_obj(target);
        }
        self.cx.fresh_init.insert(target, b);
    }

    /// Whether the walk from `work` reaches `target` through objects made since `b` began (or
    /// goes past `DROP_WALK` of them: taken as held). Held by nothing written since it began
    /// nor by a root directly: not reached, without a walk.
    fn reaches_young(&self, b: &super::context::Building, target: ObjId, work: &mut Vec<ObjId>, state: &State) -> bool {
        if work.contains(&target) {
            return true;
        }
        if b.holders.is_empty() {
            work.clear();
            return false;
        }
        let mut reached: FxHashSet<ObjId> = FxHashSet::default();
        while let Some(o) = work.pop() {
            if o == target || reached.len() > Self::DROP_WALK {
                return true;
            }
            if self.heap.born_before(o, b.mark) || !reached.insert(o) {
                continue;
            }
            self.budget.walk_visit();
            self.obj_edges(o, state, work);
        }
        false
    }

    /// Garbage collection at a loop's exit (in a function): objects the loop allocated that the
    /// frame's names, the enclosing frames and the analyzer no longer reach lose their state.
    /// The mark at the loop's start (None: not in a function's call); `loop_gc_end` collects.
    pub(super) fn loop_gc_begin(&mut self, state: &mut State) -> Option<(u64, State)> {
        if !matches!(self.frame().kind, FrameKind::Function) || self.frame().pre_call.is_none() {
            return None;
        }
        Some((self.heap.serial(), state.copy()))
    }

    /// The collection at the loop's exit, of what was made since `loop_gc_begin`'s mark.
    pub(super) fn loop_gc_end(&mut self, g: Option<(u64, State)>, state: &mut State) {
        let Some((mark, pre)) = g else { return };
        if !state.reachable {
            return;
        }
        let mut roots: Vec<u32> = Vec::new();
        self.state_roots(state, usize::MAX, usize::MAX, &mut roots);
        for (o, v) in state.lasts.iter() {
            roots.push(*o);
            v.push_objs(&mut roots);
        }
        self.collect_since(mark, &pre, roots, state, None);
    }
}

/// `v` with the recent objects of `map` renamed to their summary objects (None: none in it).
fn rename_value(v: &Value, map: &FxHashMap<ObjId, ObjId>) -> Option<Value> {
    let hit_pts = v.pts.iter().any(|o| map.contains_key(&o));
    let hit_kv = matches!(&v.kv, KeyVal::KeysOf(ko) if ko.objs.iter().any(|o| map.contains_key(&o)));
    if !hit_pts && !hit_kv {
        return None;
    }
    let pts = if hit_pts { v.pts.replace(|o| map.get(&o).copied()) } else { v.pts.clone() };
    let kv = match &v.kv {
        KeyVal::KeysOf(ko) if hit_kv => KeyVal::KeysOf(Box::new(KeysOf { objs: ko.objs.replace(|o| map.get(&o).copied()), ..(**ko).clone() })),
        other => other.clone(),
    };
    Some(Value::with_kv(pts, kv))
}

/// References written during a loop iteration (`back` beyond `head`) to the recent objects of
/// `map`: renamed to their summary objects.
fn rename_references(back: &mut State, head: &State, map: &FxHashMap<ObjId, ObjId>) {
    let fp = back.footprint(head);
    for (oid, sk) in fp.slots {
        if map.contains_key(&oid) {
            continue;
        }
        if let Some(v) = back.slots.get(&oid, &sk) {
            if let Some(nv) = rename_value(&v, map) {
                back.slots.set(&oid, sk, nv);
            }
        }
    }
    for oid in fp.elems {
        if map.contains_key(&oid) {
            continue;
        }
        if let Some(kv) = back.elems.get(&oid) {
            if let Some(nv) = rename_value(&Value::kv_only(kv), map) {
                back.elems.insert(oid, nv.kv);
            }
        }
    }
    let env_updates: Vec<(StrId, Value)> = back.env.iter().filter_map(|(k, v)| rename_value(v, map).map(|nv| (*k, nv))).collect();
    for (k, nv) in env_updates {
        back.env.insert(k, nv);
    }
    let nar_updates: Vec<(StrId, Value)> = back.narrowed.iter().filter_map(|(k, v)| rename_value(v, map).map(|nv| (*k, nv))).collect();
    for (k, nv) in nar_updates {
        back.narrowed.insert(k, nv);
    }
    // (not the value returned so far: the returns' states, `exits`, keep the recent objects'
    // own facts, as the iteration that returned left them)
}

/// The recent objects' own facts move to their summary objects (weakly; positions only for
/// the tuples in `keep_pos`).
fn fold_recent_facts(back: &mut State, map: &FxHashMap<ObjId, ObjId>, keep_pos: &FxHashSet<ObjId>) {
    for (&r, &sid) in map {
        for ns in [Ns::Item, Ns::Attr] {
            if let Some(row) = back.rows.get(&(r, ns)) {
                let nr = match back.rows.get(&(sid, ns)) {
                    Some(cur) => cur.join(&row),
                    None => row,
                };
                back.rows.insert((sid, ns), nr);
                back.rows.remove(&(r, ns));
            }
        }
        for ((ns, k), v) in back.slots_of(r) {
            if ns == Ns::Pos && !keep_pos.contains(&r) {
                continue;
            }
            let v = rename_value(&v, map).unwrap_or(v);
            back.set_slot(sid, ns, k, v, false);
        }
        back.drop_obj(r);
        if let Some(kv) = back.elems.get(&r) {
            let kv = rename_value(&Value::kv_only(kv.clone()), map).map(|v| v.kv).unwrap_or(kv);
            let nv = match back.elems.get(&sid) {
                Some(cur) => kv_join(&cur, &kv),
                None => kv,
            };
            back.elems.insert(sid, nv);
            back.elems.remove(&r);
        }
    }
}
