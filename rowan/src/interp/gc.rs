//! Liveness and garbage collection: which objects the program can still reach (the state's
//! names, the callers' names, the frames' modules and cells, and what the analyzer's own side
//! tables hold: `gc_pinned`), and the collectors at returns, loop exits and try exits that drop
//! the facts of objects made since a mark that nothing reaches.

use rustc_hash::{FxHashMap, FxHashSet};

use super::{Analyzer, Payload};
use crate::domain::{KeyVal, KeysOf, Ns, ObjId, Value};
use super::FrameKind;
use crate::intern::{KeyId, StrId};
use crate::state::{Kind, State};

impl Analyzer {
    /// May the program still hold `oid` on this path? Every allocation records the object's
    /// length entry in the state, and only the collector (at returns and loop exits) forgets
    /// an object's facts: an object without them was never made on this path or was
    /// collected, and needs no reachability walk.
    pub(in crate::interp) fn has_facts(&self, oid: ObjId, state: &State) -> bool {
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
        let mut found: FxHashSet<ObjId> = FxHashSet::default();
        let mut work: Vec<ObjId> = Vec::new();
        self.state_roots(state, usize::MAX, usize::MAX, &mut work);
        // (a walk over a large heap is bounded: past the budget the targets not found yet are
        // taken as live, which is always sound: their site makes a recent / variant object; a
        // walk that proves an object dead on a large heap visits all of it, and on email's 20k
        // visits per walk gave the same verdicts as 500)
        const WALK_BUDGET: usize = 200;
        let budget = WALK_BUDGET;
        let mut seen: FxHashSet<ObjId> = FxHashSet::default();
        while let Some(o) = work.pop() {
            if !seen.insert(o) {
                continue;
            }
            self.budget.walk_visit();
            if seen.len() > budget {
                return targets.iter().copied().collect();
            }
            if targets.contains(&o) {
                found.insert(o);
                if found.len() == targets.len() {
                    return found;
                }
            }
            self.obj_edges(o, state, &mut work);
        }
        found
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
        match &self.heap.obj(o).payload {
            Payload::Func(fi) => work.extend(fi.closure.values().copied()),
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
        for m in [&self.m.attr_mirrors, &self.heap.copy_root, &self.m.sql_cursors] {
            work.extend(each(m, o).copied());
        }
        work.extend(each(&self.entry.instances, o).flatten());
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
    }

    /// Objects the analyzer itself holds beyond the state: what side tables keyed by other
    /// objects refer to, exceptions in flight, the callers' names, loop recency objects.
    /// `unwind`: for a raise state, only the first `unwind` callers' names, and no loop's recent
    /// objects (those still in use are reachable from the names).
    pub(in crate::interp) fn gc_pinned(&self, work: &mut Vec<ObjId>, unwind: Option<usize>) {
        // (the side tables' entries: their keys themselves may be dead)
        self.side_edges(None, work);
        for v in self.unwind.cur_exc.iter().chain(self.unwind.tries.iter().flat_map(|t| t.recs.iter().map(|r| &r.2))).chain(self.unwind.handling_values.iter()).chain(self.m.stdout_redirect.iter()).chain(std::iter::once(&self.m.fa_status)).chain(self.m.fa_gen_ret.iter()) {
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
        for (_, env, _) in &self.gens.cm_frames {
            env.values().for_each(|v| v.push_objs(work));
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
    pub(in crate::interp) fn rename_recent(&mut self, back: &mut State, head: &State, recs: Vec<(ObjId, Kind, String, String)>, mark: ObjId) {
        if recs.is_empty() {
            return;
        }
        let (map, keep_pos) = self.summary_objects(back, &recs);
        fold_recent_facts(back, &map, &keep_pos);
        rename_references(back, head, &map);
        // 3. objects allocated during the iteration that hold references in their payloads
        self.heap.rename_payloads(mark, &|v: &Value| rename_value(v, &map), &map);
        // 4. flow-insensitive class attributes
        let ups: Vec<((ObjId, Ns, KeyId), Value)> = self.heap.cls_slots.iter().filter_map(|(k, v)| rename_value(v, &map).map(|nv| (*k, nv))).collect();
        for (k, nv) in ups {
            self.heap.cls_slots.insert(k, nv);
        }
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
            let (label, payload, iter_elem, derived, depth) = {
                let o = self.heap.obj(*r);
                (o.label.clone(), o.payload.clone(), o.iter_elem, o.derived_from, o.depth)
            };
            let sid = self.heap.alloc_raw(*kind, site, ctx, false, &label, payload);
            {
                let so = self.heap.obj_mut(sid);
                so.single = false;
                so.literal_len = None;
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
                    back.lens.insert(sid, (Some(n), n, 0));
                    keep_pos.insert(*r);
                }
                None => {
                    back.lens.insert(sid, (None, 0, 0));
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
        let Some(crate::interp::PreCall(mark, pre)) = self.frame().pre_call.clone() else { return };
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
        self.gc_pinned(&mut work, unwind);
        // (unknown objects stand for objects outside the call — a parameter's record, an
        // input's part: what they hold is reachable from there)
        work.extend(fresh.iter().copied().filter(|o| matches!(self.heap.kind(*o), Kind::Unknown)));
        // (older objects reach young ones only through those roots: a young one was made
        // since, so whatever refers to it was written since; only young ones expand)
        let mut reached: rustc_hash::FxHashSet<u32> = Default::default();
        while let Some(o) = work.pop() {
            if !reached.insert(o) || !young(self, o) {
                continue;
            }
            self.budget.walk_visit();
            self.obj_edges(o, state, &mut work);
        }
        // (a part of an input — an unknown object, a placeholder — is found again by its path
        // whenever that path is read: its rows belong to the input, never garbage)
        for o in fresh {
            if !reached.contains(&o) && !matches!(self.heap.kind(o), Kind::Unknown) {
                state.forget_obj(o);
            }
        }
    }

    /// Garbage collection at a loop's exit (in a function): objects the loop allocated that the
    /// frame's names, the enclosing frames and the analyzer no longer reach lose their state.
    pub(super) fn loop_gc_begin(&mut self, state: &mut State) -> Option<(u64, State)> {
        if !matches!(self.frame().kind, FrameKind::Function) || self.frame().pre_call.is_none() {
            return None;
        }
        Some((self.heap.serial(), state.copy()))
    }

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
    let pts = v.pts.map(|o| *map.get(&o).unwrap_or(&o));
    let kv = match &v.kv {
        KeyVal::KeysOf(ko) if hit_kv => KeyVal::KeysOf(KeysOf { objs: ko.objs.map(|o| *map.get(&o).unwrap_or(&o)), ..ko.clone() }),
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
                Some(cur) => crate::domain::kv_join(&cur, &kv),
                None => kv,
            };
            back.elems.insert(sid, nv);
            back.elems.remove(&r);
        }
    }
}
