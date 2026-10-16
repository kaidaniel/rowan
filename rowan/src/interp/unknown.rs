//! Unknown calls: what code rowan cannot see may do with what it is given. It may change
//! everything it reaches, at any depth (`Havoc`): records get the call as a mutator, slots may
//! hold anything (the one shared unknown object, `any_object`), lists and sets any items and
//! any length; the program's functions among it may run (`follow_code`). Calls of the
//! program's code past their budget are unknown calls too (`skipped_call`, `effects.rs`).

use indexmap::IndexMap;
use rustc_hash::{FxHashMap, FxHashSet};

use super::{Analyzer, Heap, Payload, MAX_ARG_JOIN, NOEFFECT_METHOD_NAMES};
use crate::domain::{join_rows, KeyVal, Ns, ObjId, Row, Source, SourceKind, Value, DATA};
use crate::frontend::Node;
use crate::intern::{self, s, KeyId, KeySet, StrId};
use crate::state::{Kind, State};

/// havoc walks remembered (`Havoc::walk_from`); past this many the table starts over
pub const HAVOC_MEMOS: usize = 1024;

/// What a havoc walk met and the heap it left (`Havoc::walk_from`).
pub struct HavocMemo {
    pub roots: Vec<ObjId>,
    pub rows: crate::layered::LayerMap<(ObjId, Ns), Row>,
    pub slots: crate::layered::TwoLevel<ObjId, crate::state::SlotKey, Value>,
    pub elems: crate::layered::LayerMap<ObjId, KeyVal>,
    pub lens: crate::layered::LayerMap<ObjId, (Option<usize>, usize, u16)>,
    pub visited: FxHashSet<ObjId>,
    pub code: Vec<ObjId>,
    /// the skipped method's receivers and names they were walked by, and how often each was met
    pub guard: Vec<(ObjId, std::rc::Rc<[KeyId]>, u32)>,
}

/// The havoc walks of unknown code: their marks, a memo of recent walks (a cache: a walk
/// from it gives what a full walk gives), the code the last walk met, and a skipped method's
/// receiver walked only by the names its code uses.
#[derive(Default)]
pub struct Havoc {
    walks: u32,
    /// per object, the last walk that met it
    marks: Vec<u32>,
    /// per (unknown code, values given): what its last walk met, and the heap it left
    memo: FxHashMap<(StrId, u64), HavocMemo>,
    /// the code objects the last walk met (functions, classes, bound methods)
    pub code: Vec<ObjId>,
    /// the receivers of a skipped method call (`skipped_call`), how often a walk met each, and
    /// the names its code uses on it
    pub guard: Vec<(ObjId, u32, std::rc::Rc<[KeyId]>)>,
    /// objects visited by walks: part of the work measure
    pub visited: usize,
}

/// The object standing for any value unknown code may have stored: nothing known about it.
pub fn any_object(heap: &mut Heap) -> ObjId {
    let o = heap.alloc_raw(Kind::Unknown, "any", "", false, "?", Payload::None);
    crate::state::set_any(o);
    o
}

impl Havoc {
    /// A new havoc walk: the objects it meets are marked with its number (`walk_from`).
    pub(in crate::interp) fn new_walk(&mut self) -> u32 {
        self.walks += 1;
        self.walks
    }

    /// Unknown code ran on `vals`, past the objects walk `walk` met already (marking those it
    /// meets; the code met is added to `code`).
    pub(in crate::interp) fn walk_from(&mut self, heap: &mut Heap, vals: &[Value], rid: ObjId, site: StrId, state: &mut State, walk: u32) {
        let mut roots: Vec<ObjId> = vals.iter().flat_map(|v| v.pts.iter()).collect();
        roots.sort_unstable();
        roots.dedup();
        // (the same code havocked the same values before, and few writes happened since: the
        // objects that walk met and no write changed since are as it left them — every slot
        // holds `any`, every row has the mutator —, and what they hold is met through them
        // unchanged, or changed and walked here: only the changed ones are walked again)
        // (a skipped method's receiver walked by names: by the same names)
        let guard: Vec<(ObjId, std::rc::Rc<[KeyId]>)> = self.guard.iter().map(|(g, _, used)| (*g, used.clone())).collect();
        let key = (site, {
            let mut h = rustc_hash::FxHasher::default();
            std::hash::Hash::hash(&roots, &mut h);
            std::hash::Hash::hash(&guard, &mut h);
            std::hash::Hasher::finish(&h)
        });
        let memo = self.memo.remove(&key).filter(|m| m.roots == roots && m.guard.iter().map(|(g, u, _)| (*g, u.clone())).eq(guard.iter().cloned()));
        // (met through something else then: still, as what met it is as it was)
        let met: Vec<u32> = memo.as_ref().map(|m| m.guard.iter().map(|(_, _, n)| *n).collect()).unwrap_or_default();
        let mut work = roots.clone();
        let (mut visited, mut code, changed) = self.memo_restart(memo, state, &mut work);
        let replay = changed.is_some();
        let unchanged = |o: ObjId, visited: &FxHashSet<ObjId>| replay && visited.contains(&o) && !changed.as_ref().is_some_and(|c| c.contains(&o));
        // (one object stands for "anything unknown code stored": a value per call would pile
        // up in every slot it reaches)
        let any_id = any_object(heap);
        let any = Value::obj(any_id);
        while let Some(o) = work.pop() {
            // (a skipped method's receiver: what it holds is havocked, it is not, unless met
            // again through something else)
            if let Some((_, n, used)) = self.guard.iter_mut().find(|(g, _, _)| *g == o) {
                *n += 1;
                if *n == 1 {
                    let used = used.clone();
                    receiver_by_names(heap, state, o, &used, &mut work);
                }
                continue;
            }
            let i = o as usize;
            if i >= self.marks.len() {
                self.marks.resize(heap.len().max(i + 1), 0);
            }
            if o == rid || o == any_id || self.marks[i] == walk {
                continue;
            }
            self.marks[i] = walk;
            if unchanged(o, &visited) {
                continue;
            }
            visited.insert(o);
            self.visited += 1;
            let k = heap.kind(o);
            match (&k, &heap.obj(o).payload) {
                // (the code it meets, for `skipped_call`; a stored method may change its receiver)
                (Kind::Class | Kind::Function | Kind::Bound | Kind::Partial, _) => {
                    self.code.push(o);
                    code.push(o);
                    continue;
                }
                (Kind::Method, Payload::Method(_, recv)) => {
                    work.extend(recv.pts.iter());
                    continue;
                }
                (Kind::Instance, Payload::Cls(c)) => {
                    let c = *c;
                    self.code.push(c);
                    code.push(c);
                    // (an instance of a NamedTuple class is a tuple: no instance dict, its
                    // fields read-only; what they hold is walked)
                    if heap.class_info_opt(c).is_some_and(|ci| ci.lib.named_tuple) {
                        havoc_object(heap, state, o, Kind::Tuple, site, any_id, &any, &mut work);
                        continue;
                    }
                }
                (Kind::Module | Kind::External | Kind::Method, _) => continue,
                // (a compiled pattern, a match, a path, a frozenset: nothing to change)
                (Kind::Namespace, Payload::Method(q, _)) if matches!(q.as_str(), "stdobj.repattern" | "stdobj.rematch" | "stdobj.path") => {
                    havoc_object(heap, state, o, Kind::Tuple, site, any_id, &any, &mut work);
                    continue;
                }
                (Kind::Set, _) if heap.obj(o).label.starts_with("frozenset@") => {
                    havoc_object(heap, state, o, Kind::Tuple, site, any_id, &any, &mut work);
                    continue;
                }
                _ => {}
            }
            havoc_object(heap, state, o, k, site, any_id, &any, &mut work);
        }
        for ((_, n, _), m) in self.guard.iter_mut().zip(met) {
            *n = (*n).max(m);
        }
        if self.memo.len() >= HAVOC_MEMOS {
            self.memo.clear();
        }
        code.sort_unstable();
        code.dedup();
        let guard = self.guard.iter().map(|(g, n, used)| (*g, used.clone(), *n)).collect();
        self.memo.insert(key, HavocMemo { roots, rows: state.rows.clone(), slots: state.slots.clone(), elems: state.elems.clone(), lens: state.lens.clone(), visited, code, guard });
    }

    /// Code rowan does not follow changed the containers `objs` in place (`Flags::edits`; with
    /// `change` false, only iterated them): their items may be anything, what they held is as it
    /// was. The code their items hold or are instances of is met (dunders: `__hash__`, `__lt__`).
    pub(in crate::interp) fn edit(&mut self, heap: &mut Heap, objs: &[ObjId], change: bool, site: StrId, state: &mut State) {
        let any_id = any_object(heap);
        let any = Value::obj(any_id);
        let mut met: Vec<ObjId> = Vec::new();
        for &o in objs {
            if change {
                let k = heap.kind(o);
                havoc_object(heap, state, o, k, site, any_id, &any, &mut met);
            } else {
                state.slots.each(&o, &mut |_, v| met.extend(v.pts.iter()));
            }
        }
        self.code.extend(met.into_iter().filter_map(|o| code_of(heap, o)));
    }

    /// Where a walk from the same values by the same code left off (`memo`): the objects it
    /// met, the code it met, and what changed since (none: walk all again). The changed objects
    /// it met are walked again.
    #[allow(clippy::type_complexity)]
    fn memo_restart(&mut self, memo: Option<HavocMemo>, state: &State, work: &mut Vec<ObjId>) -> (FxHashSet<ObjId>, Vec<ObjId>, Option<FxHashSet<ObjId>>) {
        match memo {
            Some(m) => {
                let limit = (m.visited.len() / 4).max(32);
                let changed: Option<FxHashSet<ObjId>> = (|| {
                    let mut ch: FxHashSet<ObjId> = state.rows.diff_upto(&m.rows, limit)?.into_iter().map(|(o, _)| o).collect();
                    ch.extend(state.slots.diff_upto(&m.slots, limit)?.into_iter().map(|(o, _)| o));
                    ch.extend(state.elems.diff_upto(&m.elems, limit)?);
                    ch.extend(state.lens.diff_upto(&m.lens, limit)?);
                    Some(ch)
                })();
                match changed {
                    Some(ch) => {
                        work.extend(ch.iter().copied().filter(|o| m.visited.contains(o)));
                        self.code.extend(m.code.iter().copied());
                        (m.visited, m.code, Some(ch))
                    }
                    None => (FxHashSet::default(), Vec::new(), None),
                }
            }
            None => (FxHashSet::default(), Vec::new(), None),
        }
    }
}

impl Analyzer {
    pub fn unknown_call(&mut self, qual: &str, recv: Option<&Value>, args: &[Value], kwargs: &IndexMap<StrId, Value>, star_args: &[Value], star_kwargs: &[Value], state: &mut State, node: &Node) -> Value {
        // (a call of the result of a call of the result of … : past two levels the result is
        // the call node's own object, as access paths below an input are bounded; otherwise
        // recursion over unknown values makes new objects without end)
        let deep = qual.matches("result of").count() >= 2;
        let site = if deep {
            let text: String = crate::frontend::text_at(node).chars().take(40).collect();
            format!("call {} at M{}:L{}:{}", text, node.module, node.line, node.col)
        } else {
            format!("{}@L{}", qual, node.line)
        };
        // unknown code may raise
        if crate::interp::debug_level() == 2 {
            eprintln!("  unknown call {} in {} (may raise)", site, self.cx.frames.last().map(|f| f.label.clone()).unwrap_or_default());
        }
        // unknown code may remove keys stored through names
        state.drop_name_keys(None);
        // unknown code may change what an earlier test read: facts about it no longer hold
        if !state.facts.is_empty() {
            let mut objs: Vec<u32> = recv.map(|r| r.pts.to_vec()).unwrap_or_default();
            for v in args.iter().chain(kwargs.values()).chain(star_args.iter()).chain(star_kwargs.iter()) {
                objs.extend(v.pts.iter());
            }
            self.kill_facts_on(&objs, state);
        }
        // (the method's name: the qualified name's last part, or for a method of an unknown
        // value the attribute it was read as, `result of …['equals']`)
        let read_as = qual.rsplit_once("['").and_then(|(_, r)| r.split_once("']")).map(|(n, _)| n);
        let name = read_as.unwrap_or_else(|| qual.rsplit('.').next().unwrap_or(qual));
        // (a library object's logging / writing / comparing method — the receiver is not one
        // of the program's objects — changes nothing it is given)
        if NOEFFECT_METHOD_NAMES.contains(&name) && recv.map(|r| r.pts.iter().all(|o| matches!(self.heap.kind(o), Kind::Unknown | Kind::External) && !self.is_any(o))).unwrap_or(true) {
            self.flag_may_raise(state);
            return Value::data(&format!("L{}", node.line));
        }
        if !state.narrowed.is_empty() {
            state.narrowed.clear();
        }
        let mut vals: Vec<Value> = args.to_vec();
        vals.extend(kwargs.values().cloned());
        vals.extend(star_args.iter().cloned());
        vals.extend(star_kwargs.iter().cloned());
        if let Some(r) = recv {
            vals.push(r.clone());
        }
        let touched = self.reachable_records(&vals, state, 0);
        let site_id = s(&site);
        let arg_recs: Vec<(u32, Ns)> = touched.iter().copied().filter(|(oid, _)| vals.iter().any(|v| v.pts.contains(*oid))).collect();
        // (the result's object is keyed by the last call site of the context only: with the
        // whole call string every context made its own result and its own tree of parts — a
        // recursive walk over unknown values, rich's pretty printer)
        let rctx: String = self.frame().ctx.rsplit('/').next().unwrap_or("").to_string();
        let rid = self.alloc(Kind::Unknown, &format!("ret:{}", site), state, &format!("result of {}", site), Payload::None, None, Ns::Item, Some(true), Some(&rctx));
        // everything it can reach through what it is given may have changed: keys, values,
        // items, lengths; and it may call the program's functions it is given
        self.havoc_reachable(&vals, rid, site_id, state);
        let found = std::mem::take(&mut self.havoc.code);
        let callables: Vec<ObjId> = found.iter().copied().filter(|o| matches!(self.heap.kind(*o), Kind::Function | Kind::Bound | Kind::Partial)).collect();
        if !callables.is_empty() {
            self.follow_code(callables, rid, site_id, state);
        }
        self.havoc.code = found;
        self.name_unknown_result(rid, site_id, &arg_recs, node);
        unknown_result_rows(state, rid, &site, site_id, &arg_recs);
        self.findings.note(format!("unknown call {}", site));
        // (it may raise anything, after any part of its effects: the state with all of them
        // stands for those with some)
        self.flag_may_raise(state);
        Value::obj(rid)
    }

    /// For people: the call as written names its result (and its site, for sources and
    /// mutators), with the known objects it was given (unknown code may return one of them;
    /// given unknowns add nothing to `?`).
    fn name_unknown_result(&mut self, rid: ObjId, site_id: StrId, arg_recs: &[(u32, Ns)], node: &Node) {
        let text = crate::frontend::text_at(node);
        let mut given: Vec<String> = Vec::new();
        for (o, _) in arg_recs {
            if matches!(self.heap.kind(*o), Kind::Unknown) {
                continue;
            }
            // (an object named by where it was made, `dict@L4`; one named by how it was made
            // from others — a copy of an unknown call's result — is not shown)
            let g = self.heap.obj(*o).label.to_string();
            let g = if g.contains(char::is_whitespace) { "…".to_string() } else { g };
            if !given.contains(&g) {
                given.push(g);
            }
            // (a few: a call given a whole structure names its first objects only)
            if given.len() >= 4 {
                break;
            }
        }
        if !text.is_empty() {
            let t = text.split_whitespace().collect::<Vec<_>>().join(" ");
            self.names.site_names.entry(site_id).or_insert_with(|| (t.clone(), node.module, node.line));
            let scope = self.frame().func.unwrap_or(0);
            self.names.set(rid, t.clone(), scope);
            self.names.origins.insert(rid, (t, node.line, given));
        }
    }

    /// The shared unknown object (`any_object`).
    pub(in crate::interp) fn is_any(&self, o: ObjId) -> bool {
        crate::state::is_any(o)
    }

    /// `v` passes to unknown code without a call (stored into an object only unknown code
    /// holds): what it reaches may change, and the code it holds may run.
    pub(in crate::interp) fn escape(&mut self, v: &Value, site: &str, state: &mut State) {
        if v.pts.is_empty() {
            return;
        }
        let any = crate::interp::any_object(&mut self.heap);
        let site_id = s(site);
        self.havoc_reachable(std::slice::from_ref(v), any, site_id, state);
        let found = std::mem::take(&mut self.havoc.code);
        let callables: Vec<ObjId> = found.into_iter().filter(|o| matches!(self.heap.kind(*o), Kind::Function | Kind::Bound | Kind::Partial)).collect();
        if !callables.is_empty() {
            self.follow_code(callables, any, site_id, state);
        }
    }

    pub(in crate::interp) fn reachable_records(&self, vals: &[Value], state: &State, depth: u32) -> Vec<(u32, Ns)> {
        let mut out: Vec<(u32, Ns)> = Vec::new();
        let mut seen: rustc_hash::FxHashSet<u32> = rustc_hash::FxHashSet::default();
        let mut work: Vec<(u32, u32)> = vals.iter().flat_map(|v| v.pts.iter().map(|o| (o, 0u32)).collect::<Vec<_>>()).collect();
        while let Some((oid, d)) = work.pop() {
            if !seen.insert(oid) {
                continue;
            }
            let k = self.heap.kind(oid);
            if matches!(k, Kind::Dict | Kind::Frame | Kind::Unknown) {
                out.push((oid, Ns::Item));
            }
            if matches!(k, Kind::Instance | Kind::Unknown) {
                out.push((oid, Ns::Attr));
            }
            if d < depth {
                for (_, v) in state.slots_of(oid) {
                    work.extend(v.pts.iter().map(|x| (x, d + 1)));
                }
            }
        }
        out
    }

    /// Unknown code ran on `vals`: every object they reach (through slots, items, elements)
    /// may have changed. Records get the call as a mutator (any key may have been added or
    /// removed), every slot of a mutable object may now hold the call's result `rid` (any value),
    /// lists and sets any items and any length. Modules, classes and functions are not entered.
    pub(in crate::interp) fn havoc_reachable(&mut self, vals: &[Value], rid: ObjId, site: StrId, state: &mut State) {
        self.havoc.code.clear();
        let walk = self.havoc.new_walk();
        self.havoc.walk_from(&mut self.heap, vals, rid, site, state, walk);
    }

    /// What the code `roots` hold may change when it runs where rowan does not run it: the
    /// values at the slots its closure may change are havocked, the slots it rebinds may hold
    /// anything, and the code the data they hold is followed too.
    pub(super) fn follow_code(&mut self, mut roots: Vec<ObjId>, rid: ObjId, site: StrId, state: &mut State) {
        let any = Value::obj(crate::interp::any_object(&mut self.heap));
        let walk = self.havoc.new_walk();
        self.havoc.code.clear();
        loop {
            roots.append(&mut self.havoc.code);
            roots.sort_unstable();
            roots.dedup();
            let cl = self.code_closure(&roots, state);
            let mut vals: Vec<Value> = cl.locs.iter().map(|(o, ns, k)| state.slot(*o, *ns, *k)).collect();
            vals.extend(cl.values.iter().cloned());
            // (values whose parts may change, not themselves: their parts; whose parts' parts
            // may: those)
            let mut parts: Vec<ObjId> = Vec::new();
            for v in cl.held_locs.iter().map(|(o, ns, k)| state.slot(*o, *ns, *k)).chain(cl.held_values.iter().cloned()) {
                for o in v.pts.iter() {
                    self.obj_edges(o, state, &mut parts);
                }
            }
            let mut mid: Vec<ObjId> = Vec::new();
            for v in cl.held2_locs.iter().map(|(o, ns, k)| state.slot(*o, *ns, *k)) {
                for o in v.pts.iter() {
                    self.obj_edges(o, state, &mut mid);
                }
            }
            for o in mid {
                self.havoc.code.extend(code_of(&self.heap, o));
                self.obj_edges(o, state, &mut parts);
            }
            parts.sort_unstable();
            parts.dedup();
            vals.extend(parts.into_iter().map(Value::obj));
            self.havoc.walk_from(&mut self.heap, &vals, rid, site, state, walk);
            for (locs, values, change) in [(&cl.edited_locs, &cl.edited_values, true), (&cl.iterated_locs, &cl.iterated_values, false)] {
                let objs: Vec<ObjId> = locs.iter().map(|(o, ns, k)| state.slot(*o, *ns, *k)).chain(values.iter().cloned()).flat_map(|v| v.pts.to_vec()).collect();
                self.havoc.edit(&mut self.heap, &objs, change, site, state);
            }
            for (o, ns, k) in &cl.rebound {
                let v = state.slot(*o, *ns, *k).join(&any);
                state.set_slot(*o, *ns, *k, v, false);
            }
            self.havoc.code.retain(|c| !cl.code.contains(c));
            if self.havoc.code.is_empty() {
                return;
            }
        }
    }

    /// Drop the facts whose test read one of `objs`.
    pub(in crate::interp) fn kill_facts_on(&mut self, objs: &[u32], state: &mut State) {
        if objs.is_empty() || state.facts.is_empty() {
            return;
        }
        let kept: Vec<_> = state.facts.iter().filter(|f| !f.deps.iter().any(|(_, v)| v.pts.iter().any(|o| objs.contains(&o)))).cloned().collect();
        if kept.len() != state.facts.len() {
            state.facts = std::sync::Arc::new(kept);
        }
    }
}

/// The code object `o` is or is an instance of (what running or comparing it may run).
pub(in crate::interp) fn code_of(heap: &Heap, o: ObjId) -> Option<ObjId> {
    match (heap.kind(o), &heap.obj(o).payload) {
        (Kind::Class | Kind::Function | Kind::Bound | Kind::Partial, _) => Some(o),
        (Kind::Instance, Payload::Cls(c)) => Some(*c),
        _ => None,
    }
}

/// A skipped method's receiver met by a walk: its code reaches the receiver's attributes only
/// by the names it uses (its own, or its class's data); `self.__class__` the class.
fn receiver_by_names(heap: &Heap, state: &State, o: ObjId, used: &[KeyId], work: &mut Vec<ObjId>) {
    // (its code reaches the receiver's attributes only by the names it uses: its own, or its
    // class's data)
    let mro = heap.instance_class(o).map(|c| heap.mro(c)).unwrap_or_default();
    for k in used.iter() {
        // (`self.__class__` / `type(self)`: the class, and all it reaches)
        if intern::key_as_str(*k).as_deref() == Some("__class__") {
            work.extend(mro.first().copied());
        }
        work.extend(state.slot(o, Ns::Attr, *k).pts.iter());
        for c in &mro {
            let v = if state.has_slot(*c, Ns::Attr, *k) { state.slot(*c, Ns::Attr, *k) } else { heap.cls_slots.get(&(*c, Ns::Attr, *k)).cloned().unwrap_or_default() };
            work.extend(v.pts.iter().filter(|x| heap.kind(*x) != Kind::Function));
        }
    }
}

/// Unknown code may have changed object `o` (of kind `k`): its slots may hold anything
/// (`any_id`), its rows have the call as a mutator, a list or set any items and length (what
/// the slots hold is walked next).
#[allow(clippy::too_many_arguments)]
fn havoc_object(heap: &mut Heap, state: &mut State, o: ObjId, k: Kind, site: StrId, any_id: ObjId, any: &Value, work: &mut Vec<ObjId>) {
    // (an enum member's `name` and `value` cannot be set or deleted)
    let member = heap.keys.member_key.contains_key(&o);
    let fixed = |ns: Ns, k: KeyId| member && ns == Ns::Attr && intern::key_as_str(k).is_some_and(|n| matches!(&*n, "name" | "value"));
    let mut writes: Vec<(crate::state::SlotKey, Value)> = Vec::new();
    state.slots.each(&o, &mut |sk, v| {
        work.extend(v.pts.iter());
        if matches!(sk.0, Ns::Item | Ns::Attr | Ns::Pos | Ns::Elem) && !v.pts.contains(any_id) && !fixed(sk.0, sk.1) {
            writes.push((*sk, v.join(any)));
        }
    });
    // (a tuple cannot change; an unknown object's content is unknown already)
    if k == Kind::Tuple || (k == Kind::Unknown && !state.has_row(o, Ns::Item) && !state.has_row(o, Ns::Attr)) {
        return;
    }
    // (what an earlier havoc by the same code did already is not written again: code
    // past its budget is often called in a loop)
    for ns in [Ns::Item, Ns::Attr] {
        let r0 = match state.rows.get(&(o, ns)) {
            Some(r) => r,
            None if (ns == Ns::Item && matches!(k, Kind::Dict | Kind::Frame)) || (ns == Ns::Attr && k == Kind::Instance) => Row::new(),
            None => continue,
        };
        let r = if member { r0.mutate_like(site, &|k| !fixed(ns, k)) } else { r0.mutate(site) };
        if r != r0 {
            state.set_row(o, ns, r);
        }
    }
    // (each value holds what the slot held: written as it is)
    for (sk, v) in writes {
        state.set_slot(o, sk.0, sk.1, v, true);
    }
    let havocked = |state: &State| {
        state.slot(o, Ns::Elem, intern::STAR).pts.contains(any_id) && state.lens.get(&o) == Some((None, 0, 0)) && !state.lasts.iter().any(|(x, _)| *x == o) && state.elems.get(&o).is_some_and(|kv| crate::domain::kv_join(&kv, &DATA) == kv)
    };
    // (for people: which unknown code may have changed a list or set, on a row of its own —
    // it has no attributes to read)
    if matches!(k, Kind::List | Kind::Set) && !heap.obj(o).site.starts_with("gen:") {
        let r0 = state.rows.get(&(o, Ns::Attr)).unwrap_or_else(Row::new);
        let r = r0.mutate(site);
        if r != r0 {
            state.set_row(o, Ns::Attr, r);
        }
    }
    if matches!(k, Kind::List | Kind::Set) && !heap.obj(o).site.starts_with("gen:") && !havocked(state) {
        let e = state.slot(o, Ns::Elem, intern::STAR).join(any);
        state.set_slot(o, Ns::Elem, intern::STAR, e, false);
        let kv = crate::domain::kv_join(&state.elems.get(&o).unwrap_or(DATA), &DATA);
        state.elems.insert(o, kv);
        state.clear_last(o);
        heap.set_prefix(o, state, 0);
    }
}

/// The rows of an unknown call's result: the items of the mappings it was given (up to
/// `MAX_ARG_JOIN`; past that any record), else open through the call; changed by the call when
/// it was given records.
fn unknown_result_rows(state: &mut State, rid: ObjId, site: &str, site_id: StrId, arg_recs: &[(u32, Ns)]) {
    // (many records given — a value joined over many contexts: the result is any record,
    // an open row, rather than the join of them all and a copy of all their slots)
    // (its items: those of the mappings it was given; an instance's attributes are not
    // items)
    let item_recs: Vec<ObjId> = arg_recs.iter().filter(|(_, ns)| *ns == Ns::Item).map(|(o, _)| *o).collect();
    if !item_recs.is_empty() && item_recs.len() <= MAX_ARG_JOIN {
        let row = join_rows(item_recs.iter().map(|o| state.row(*o, Ns::Item)));
        let mut d = row.data();
        d.definite = KeySet::new();
        d.absent = KeySet::new();
        d.mutators = d.mutators.insert(site_id);
        state.set_row(rid, Ns::Item, Row::from_data(d));
        // as an object the result is unknown: its attributes come from the call
        let src = Source::new(site, SourceKind::Call);
        state.set_row(rid, Ns::Attr, Row::with_source(src).mutate(site_id));
        for oid in &item_recs {
            for (k, v) in state.obj_slots(*oid, Ns::Item) {
                state.set_slot(rid, Ns::Item, k, v, false);
            }
        }
    } else {
        let src = Source::new(site, SourceKind::Call);
        let mutated = |r: Row| if arg_recs.is_empty() { r } else { r.mutate(site_id) };
        state.set_row(rid, Ns::Item, mutated(Row::with_source(src)));
        state.set_row(rid, Ns::Attr, mutated(Row::with_source(src)));
    }
}
