//! Unknown calls: what code rowan cannot see may do with what it is given. It may change
//! everything it reaches, at any depth (`Havoc`): records get the call as a mutator, slots may
//! hold anything (the one shared unknown object, `any_object`), lists and sets any items and
//! any length; the program's functions among it may run (`follow_code`). Calls of the
//! program's code past their budget are unknown calls too (`skipped_call`, `effects.rs`).

use rustc_hash::{FxHashMap, FxHashSet};

use super::flows::Removes;
use super::{Analyzer, Args, Heap, NOEFFECT_METHOD_NAMES, Payload};
use crate::domain::{join_rows, kv_join, DATA, KeyVal, Ns, ObjId, Row, TYPE_NAMES, Value};
use crate::frontend::Node;
use crate::intern::{self, s, KeyId, KeySet, StrId};
use crate::source::{Source, SourceId, SourceKind};
use crate::state::{set_any, Kind, SlotKey, State};

/// Records an unknown call's result joins at most (past that: an open row).
pub const MAX_ARG_JOIN: usize = 64;
/// Havoc walks remembered (`Havoc::walk_from`); past this many the table starts over.
pub const HAVOC_MEMOS: usize = 1024;

/// The parts of a state unknown code changes (rows, slots, elements, lengths), kept to tell
/// later which objects a state changed since (O(1) to keep: the maps are persistent).
pub struct Snapshot {
    rows: crate::layered::LayerMap<(ObjId, Ns), Row>,
    slots: crate::layered::TwoLevel<ObjId, SlotKey, Value>,
    elems: crate::layered::LayerMap<ObjId, KeyVal>,
    lens: crate::layered::LayerMap<ObjId, crate::state::LenFact>,
}

impl Snapshot {
    /// The facts of `state` as they are now (O(1): the maps are shared).
    pub fn of(state: &State) -> Snapshot {
        Snapshot { rows: state.rows.clone(), slots: state.slots.clone(), elems: state.elems.clone(), lens: state.lens.clone() }
    }

    /// The objects whose rows, slots, elements or lengths differ in `state` (at a cost of what
    /// differs); None past `limit` of them.
    pub fn changed(&self, state: &State, limit: usize) -> Option<FxHashSet<ObjId>> {
        let (mut ch, slots) = self.changed_parts(state, limit)?;
        ch.extend(slots.into_iter().map(|(o, _)| o));
        Some(ch)
    }

    /// `changed`, the objects whose rows, elements or lengths differ apart from the slots that
    /// differ.
    pub fn changed_parts(&self, state: &State, limit: usize) -> Option<(FxHashSet<ObjId>, Vec<(ObjId, SlotKey)>)> {
        let mut ch: FxHashSet<ObjId> = state.rows.diff_upto(&self.rows, limit)?.into_iter().map(|(o, _)| o).collect();
        ch.extend(state.elems.diff_upto(&self.elems, limit)?);
        ch.extend(state.lens.diff_upto(&self.lens, limit)?);
        Some((ch, state.slots.diff_upto(&self.slots, limit)?))
    }
}

/// What a havoc walk met and the heap it left (`Havoc::walk_from`).
pub struct HavocMemo {
    pub roots: Vec<ObjId>,
    pub left: Snapshot,
    pub visited: FxHashSet<ObjId>,
    pub code: Vec<ObjId>,
    /// the skipped method's receivers and names they were walked by, and how often each was met
    pub guard: Vec<(ObjId, std::rc::Rc<[KeyId]>, u32)>,
}

/// The havoc walks of unknown code: their marks, a memo of recent walks (a cache: a walk
/// from it gives what a full walk gives; a walk from the same values, by any unknown code,
/// redoes only what changed, and the tests of what it met no longer hold), the code the last
/// walk met, and a skipped method's receiver walked only by the names its code uses.
#[derive(Default)]
pub struct Havoc {
    walks: u32,
    /// per object, the last walk that met it, and the last that queued it (`Queue`)
    marks: Vec<u32>,
    queued: Vec<u32>,
    /// per values given (and receivers walked by names): what the last walk from them met, and
    /// the heap it left
    memo: FxHashMap<u64, HavocMemo>,
    /// the code objects the last walk met (functions, classes, bound methods)
    pub code: Vec<ObjId>,
    /// the receivers of a skipped method call (`skipped_call`), how often a walk met each, and
    /// the names its code uses on it
    pub guard: Vec<(ObjId, u32, std::rc::Rc<[KeyId]>)>,
    /// objects visited by walks: part of the work measure
    pub visited: usize,
    /// the next unknown call is a skipped call's, which follows the classes it meets itself
    /// (their dunders among their code)
    pub classes_followed: bool,
    /// per code followed (`follow_code`) and receivers walked by names: what it read and
    /// changed, and the heap it left
    follows: FxHashMap<u64, FollowMemo>,
    /// while code is followed: the objects its walks meet
    met: Option<Vec<ObjId>>,
}

/// A `follow_code` remembered: its code, the skipped method's receivers walked by names (and
/// how often its walks met each), the objects whose rows, slots, elements or lengths it read or
/// changed, and the heap it left.
pub struct FollowMemo {
    roots: Vec<ObjId>,
    guard: Vec<(ObjId, std::rc::Rc<[KeyId]>)>,
    met: Vec<u32>,
    deps: FxHashSet<ObjId>,
    /// the slots it read or rebound, of objects it did not walk (globals, cells, class
    /// attributes)
    slots: FxHashSet<(ObjId, SlotKey)>,
    left: Snapshot,
}

/// The object standing for any value unknown code may have stored: nothing known about it.
pub fn any_object(heap: &mut Heap) -> ObjId {
    let o = heap.alloc_raw(Kind::Unknown, "any", "", false, "?", Payload::None);
    set_any(o);
    o
}

impl Havoc {
    /// A new havoc walk: the objects it meets are marked with its number (`walk_from`).
    pub(in crate::interp) fn new_walk(&mut self) -> u32 {
        self.walks += 1;
        self.walks
    }

    /// The key of the code `roots` followed with the receivers walked by names now.
    fn follow_key(&self, roots: &[ObjId]) -> u64 {
        let mut h = rustc_hash::FxHasher::default();
        std::hash::Hash::hash(roots, &mut h);
        for (g, _, used) in &self.guard {
            std::hash::Hash::hash(&(g, &**used), &mut h);
        }
        std::hash::Hasher::finish(&h)
    }

    /// The code `roots` was followed before (with the same receivers walked by names), and
    /// nothing it read or changed has changed since: running it again changes nothing (each of
    /// its writes would write what is there). Its walks' meetings of the receivers count again.
    fn followed_unchanged(&mut self, key: u64, roots: &[ObjId], state: &State) -> bool {
        let Some(m) = self.follows.get(&key) else { return false };
        if m.roots != roots || !m.guard.iter().map(|(g, u)| (*g, u)).eq(self.guard.iter().map(|(g, _, u)| (*g, u))) {
            return false;
        }
        // (a diff past what re-applying it would read costs more than it saves)
        match m.left.changed_parts(state, (m.deps.len() + m.slots.len()).max(64)) {
            Some((ch, slots)) if !ch.iter().any(|o| m.deps.contains(o)) && !slots.iter().any(|(o, k)| m.deps.contains(o) || m.slots.contains(&(*o, *k))) => {
                for ((_, n, _), k) in self.guard.iter_mut().zip(&m.met) {
                    *n += k;
                }
                true
            }
            _ => false,
        }
    }

    fn remember_follow(&mut self, key: u64, roots: Vec<ObjId>, met: Vec<u32>, deps: Vec<ObjId>, slots: FxHashSet<(ObjId, SlotKey)>, state: &State) {
        if self.follows.len() >= HAVOC_MEMOS {
            self.follows.clear();
        }
        let guard = self.guard.iter().map(|(g, _, u)| (*g, u.clone())).collect();
        self.follows.insert(key, FollowMemo { roots, guard, met, deps: deps.into_iter().collect(), slots, left: Snapshot::of(state) });
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
        // (keyed by what is walked, not by the code that walks it: any unknown code may do
        // the same to what it reaches; a row the memo's walk left names that walk's call, for
        // people only — any mutator makes a row open)
        let key = {
            let mut h = rustc_hash::FxHasher::default();
            std::hash::Hash::hash(&roots, &mut h);
            std::hash::Hash::hash(&guard, &mut h);
            std::hash::Hasher::finish(&h)
        };
        let memo = self.memo.remove(&key).filter(|m| m.roots == roots && m.guard.iter().map(|(g, u, _)| (*g, u.clone())).eq(guard.iter().cloned()));
        // (met through something else then: still, as what met it is as it was)
        let met: Vec<u32> = memo.as_ref().map(|m| m.guard.iter().map(|(_, _, n)| *n).collect()).unwrap_or_default();
        let mut work = roots.clone();
        let (mut visited, mut code, changed) = self.memo_restart(memo, state, &mut work);
        self.walk_objects(heap, work, &mut visited, changed.as_ref(), &mut code, rid, site, state, walk);
        for ((_, n, _), m) in self.guard.iter_mut().zip(met) {
            *n = (*n).max(m);
        }
        // (what it met, the remembered walk's too: what a `follow_code` read and changed)
        if let Some(m) = &mut self.met {
            m.extend(visited.iter().copied());
        }
        // (the tests of what it met a path passed since no longer hold: a walk replayed left
        // the objects unchanged since as they were, rows and all, and one that gives a row a
        // mutator it has already leaves it too)
        drop_facts_on(state, |o| visited.contains(&o));
        if self.memo.len() >= HAVOC_MEMOS {
            self.memo.clear();
        }
        code.sort_unstable();
        code.dedup();
        let guard = self.guard.iter().map(|(g, n, used)| (*g, used.clone(), *n)).collect();
        self.memo.insert(key, HavocMemo { roots, left: Snapshot::of(state), visited, code, guard });
    }

    /// The havoc walk `walk` from the objects `work` holds (`walk_from`), adding those it meets
    /// to `visited` and the code it meets to `code` (and `self.code`); with `changed`, a walk
    /// resumed: the objects `visited` holds that `changed` does not are as an earlier walk left
    /// them, and are not walked again.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::interp) fn walk_objects(&mut self, heap: &mut Heap, mut work: Vec<ObjId>, visited: &mut FxHashSet<ObjId>, changed: Option<&FxHashSet<ObjId>>, code: &mut Vec<ObjId>, rid: ObjId, site: StrId, state: &mut State, walk: u32) {
        let unchanged = |o: ObjId, visited: &FxHashSet<ObjId>| changed.is_some_and(|c| visited.contains(&o) && !c.contains(&o));
        // (one object stands for "anything unknown code stored": a value per call would pile
        // up in every slot it reaches)
        let any_id = any_object(heap);
        let any = Value::obj(any_id);
        // (each object is queued once per walk: objects whose slots share large object sets
        // pushed them again and again, `Queue`)
        if self.marks.len() < heap.len() {
            self.marks.resize(heap.len(), 0);
            self.queued.resize(heap.len(), 0);
        }
        let guard: Vec<ObjId> = self.guard.iter().map(|(g, _, _)| *g).collect();
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
                // (a module of the program's: its globals' code, what they hold, `walk_closure`)
                (Kind::Class | Kind::Function | Kind::Bound | Kind::Partial | Kind::Module, _) => {
                    self.code.push(o);
                    code.push(o);
                    // (a function: unknown code may set any attribute on it)
                    if k == Kind::Function {
                        heap.escaped_fns.insert(o);
                    }
                    continue;
                }
                (Kind::Method, Payload::Method(_, recv)) => {
                    work.extend(recv.pts.iter());
                    continue;
                }
                // (a deferred definition: unknown code may run its decorators' code, the function's)
                (Kind::Unknown, Payload::Deferred(_)) => {
                    let c = super::deferred::deferred_code(heap, o).unwrap_or_default();
                    self.code.extend(c.iter().copied());
                    code.extend(c);
                    continue;
                }
                (Kind::Instance, Payload::Cls(c)) => {
                    let c = *c;
                    self.code.push(c);
                    code.push(c);
                    // (an instance of a NamedTuple class is a tuple: no instance dict, its
                    // fields read-only; what they hold is walked)
                    if heap.class_info_opt(c).is_some_and(|ci| ci.lib.named_tuple) {
                        havoc_object(heap, state, o, Kind::Tuple, site, any_id, &any, &mut Queue { work: &mut work, marks: &self.marks, queued: &mut self.queued, walk, guard: &guard });
                        continue;
                    }
                }
                (Kind::External | Kind::Method, _) => continue,
                // (a compiled pattern, a match, a path, a frozenset: nothing to change)
                (Kind::Namespace, Payload::Method(q, _)) if matches!(q.as_str(), "stdobj.repattern" | "stdobj.rematch" | "stdobj.path") => {
                    havoc_object(heap, state, o, Kind::Tuple, site, any_id, &any, &mut Queue { work: &mut work, marks: &self.marks, queued: &mut self.queued, walk, guard: &guard });
                    continue;
                }
                (Kind::Set, _) if heap.obj(o).label.starts_with("frozenset@") => {
                    havoc_object(heap, state, o, Kind::Tuple, site, any_id, &any, &mut Queue { work: &mut work, marks: &self.marks, queued: &mut self.queued, walk, guard: &guard });
                    continue;
                }
                _ => {}
            }
            havoc_object(heap, state, o, k, site, any_id, &any, &mut Queue { work: &mut work, marks: &self.marks, queued: &mut self.queued, walk, guard: &guard });
        }
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
                havoc_object(heap, state, o, k, site, any_id, &any, &mut Queue { work: &mut met, marks: &[], queued: &mut [], walk: 0, guard: &[] });
            } else {
                state.slots.each(&o, &mut |_, v| met.extend(v.pts.iter()));
            }
        }
        self.code.extend(met.into_iter().filter_map(|o| compared_code(heap, o)));
    }

    /// Where a walk from the same values by the same code left off (`memo`): the objects it
    /// met, the code it met, and what changed since (none: walk all again). The changed objects
    /// it met are walked again.
    #[allow(clippy::type_complexity)]
    fn memo_restart(&mut self, memo: Option<HavocMemo>, state: &State, work: &mut Vec<ObjId>) -> (FxHashSet<ObjId>, Vec<ObjId>, Option<FxHashSet<ObjId>>) {
        match memo {
            Some(m) => {
                let limit = (m.visited.len() / 4).max(32);
                match m.left.changed(state, limit) {
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
    /// A call of code rowan cannot see (`qual`: its name; `recv`: a method's receiver): it may
    /// raise; what it is given, and everything that reaches, may change (`havoc_reachable`); the
    /// program's functions it is given may run (`follow_code`); it returns an unknown named by
    /// the call, which may be one of the known objects it was given.
    pub fn unknown_call(&mut self, qual: &str, recv: Option<&Value>, a: Args, state: &mut State, node: &Node) -> Value {
        let Args { pos: args, kw: kwargs, star: star_args, star_kw: star_kwargs } = a;
        crate::models::csvio::stdout_may_write(self, state, node.line);
        let classes_followed = std::mem::take(&mut self.havoc.classes_followed);
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
        // (what it is given, the receiver apart; `*xs` / `**d`: their items, in a new tuple / dict)
        let mut vals: Vec<Value> = args.iter().chain(kwargs.values()).cloned().collect();
        vals.extend(star_args.iter().chain(star_kwargs.iter()).map(|v| self.unpacked(v, state)));
        // unknown code may change what an earlier test read: facts about it no longer hold
        if !state.facts.is_empty() {
            let objs: Vec<u32> = recv.into_iter().chain(vals.iter()).flat_map(|v| v.pts.iter()).collect();
            self.kill_facts_on(&objs, state);
        }
        // (the method's name: the qualified name's last part, or for a method of an unknown
        // value the attribute it was read as, `result of …['equals']`)
        let read_as = qual.rsplit_once("['").and_then(|(_, r)| r.split_once("']")).map(|(n, _)| n);
        let name = read_as.unwrap_or_else(|| qual.rsplit('.').next().unwrap_or(qual));
        // (a library object's logging / writing / comparing method — the receiver is not one
        // of the program's objects — changes nothing it is given)
        if NOEFFECT_METHOD_NAMES.contains(&name) && recv.map(|r| r.pts.iter().all(|o| matches!(self.heap.kind(o), Kind::Unknown | Kind::External) && !self.is_any(o))).unwrap_or(true) {
            // (`log.info("%s", obj)`: it may run the dunders of the instances it is given)
            self.library_dunders(&vals, state, node);
            self.flag_may_raise(state);
            return Value::data(&format!("L{}", node.line));
        }
        if !state.narrowed.is_empty() {
            state.narrowed.clear();
        }
        vals.extend(recv.cloned());
        let touched = self.reachable_records(&vals, state, 0);
        let site_id = s(&site);
        let arg_recs: Vec<(u32, Ns)> = touched.iter().copied().filter(|(oid, _)| vals.iter().any(|v| v.pts.contains(*oid))).collect();
        // (the result's object is keyed by the last call site of the context only: with the
        // whole call string every context made its own result and its own tree of parts — a
        // recursive walk over unknown values, rich's pretty printer)
        let rctx: String = self.frame().ctx.rsplit('/').next().unwrap_or("").to_string();
        let rid = self.alloc(Kind::Unknown, &format!("ret:{}", site), state, &format!("result of {}", site), Payload::None, None, Ns::Item, Some(true), Some(&rctx));
        // (a library's object: `ContextVar(…).get()` is no dict's `get`; a skipped call of the
        // program's code gives back the program's values)
        self.heap.obj_mut(rid).opaque |= !classes_followed;
        // everything it can reach through what it is given may have changed: keys, values,
        // items, lengths; and it may call the program's functions it is given
        let (found, met) = self.havoc_given(&vals, rid, site_id, state);
        let callables: Vec<ObjId> = found.iter().copied().filter(|o| matches!(self.heap.kind(*o), Kind::Function | Kind::Bound | Kind::Partial | Kind::Module)).collect();
        // (a skipped call follows them itself, with its own code: `classes_followed`)
        if !callables.is_empty() && !classes_followed {
            self.follow_code(callables, rid, site_id, state);
        }
        // (the dunders of the instances it met may run: what they change, their receivers
        // changed already; followed apart, as the same dunders recur; `xs.append(o)` /
        // `setattr` store it, running none of its code)
        if !classes_followed && !matches!(name, "append" | "insert" | "__setattr__" | "setattr") {
            let dunders = self.met_dunders(&found, &met, state);
            if !dunders.is_empty() {
                self.follow_code(dunders, rid, site_id, state);
            }
        }
        self.havoc.code = found;
        self.name_unknown_result(rid, site_id, &arg_recs, node);
        // (the mappings it was given itself: `f(**d)` gives the code d's values, not d; an
        // unknown callable called, `cls(x)`, is not given its own items; an `int` has none)
        let callee = recv.and_then(|r| r.single()).is_some_and(|o| qual == format!("call of {}", self.heap.obj(o).label));
        let recv_given = recv.filter(|_| !callee);
        let scalar = |o: ObjId| self.heap.obj(o).ty.is_some_and(|t| TYPE_NAMES.contains(&t));
        let given: Vec<ObjId> = arg_recs.iter().filter(|(o, ns)| *ns == Ns::Item && !scalar(*o) && args.iter().chain(kwargs.values()).chain(recv_given).any(|v| v.pts.contains(*o))).map(|(o, _)| *o).collect();
        unknown_result_rows(state, rid, &site, site_id, &arg_recs, &given, callee);
        self.findings.note(format!("unknown call {}", site));
        // (it may raise anything, after any part of its effects: the state with all of them
        // stands for those with some)
        self.flag_may_raise(state);
        Value::obj(rid)
    }

    /// What `*v` / `**v` give the code called: a list's, tuple's, set's or dict's items, in a
    /// new tuple or dict (the container is not given: its keys and length stay as they are; an
    /// open one's items are anything); anything else itself (an iterator is consumed, an
    /// instance's `keys` / `__iter__` run).
    pub(in crate::interp) fn unpacked(&self, v: &Value, state: &State) -> Value {
        let mut out = Value::with_kv(crate::domain::ObjSet::new(), v.kv.clone());
        for o in v.pts.iter() {
            let plain = matches!(self.heap.kind(o), Kind::Dict | Kind::List | Kind::Tuple | Kind::Set) && !self.heap.obj(o).site.starts_with("gen:") && self.has_facts(o, state);
            if !plain {
                out = out.join(&Value::obj(o));
                continue;
            }
            state.slots.each(&o, &mut |sk, sv| {
                if matches!(sk.0, Ns::Item | Ns::Elem | Ns::Pos) {
                    out = out.join(sv);
                }
            });
            if let Some(kv) = state.elems.get(&o) {
                out = out.join(&Value::with_kv(crate::domain::ObjSet::new(), kv));
            }
            if state.row(o, Ns::Item).open() || state.row(o, Ns::Attr).open() {
                out = out.join(&Value::data("unpacked"));
            }
        }
        out
    }

    /// The program's dunders of the classes among `found` that unknown code may run (`met`: the
    /// objects it met; a class met itself may be called).
    fn met_dunders(&mut self, found: &[ObjId], met: &FxHashSet<ObjId>, state: &State) -> Vec<ObjId> {
        let mut dunders: Vec<ObjId> = Vec::new();
        let classes: Vec<ObjId> = found.iter().copied().filter(|o| self.heap.kind(*o) == Kind::Class).collect();
        for c in classes {
            dunders.extend(self.changing_dunders(c, state).iter().copied());
            // (it may call the class it met, or an instance of it: what their code reaches
            // besides the instance may change, `Box.made += 1` in `__init__`)
            dunders.extend(self.construct_dunders(c, met.contains(&c), state));
        }
        dunders.sort_unstable();
        dunders.dedup();
        dunders
    }

    /// Unknown code ran on `vals` (`havoc_reachable`), and the generators it met may run on
    /// (`unknown_runs_generators`): the code it met, and the objects.
    fn havoc_given(&mut self, vals: &[Value], rid: ObjId, site_id: StrId, state: &mut State) -> (Vec<ObjId>, FxHashSet<ObjId>) {
        let outer = self.havoc.met.replace(Vec::new());
        self.havoc_reachable(vals, rid, site_id, state);
        let mut found = std::mem::take(&mut self.havoc.code);
        let met = std::mem::replace(&mut self.havoc.met, outer).unwrap_or_default();
        self.unknown_runs_generators(&met, rid, site_id, &mut found, state);
        let set: FxHashSet<ObjId> = met.iter().copied().collect();
        if let Some(m) = &mut self.havoc.met {
            m.extend(met);
        }
        (found, set)
    }

    /// The generators among the objects unknown code met (`met`) may run on (it may consume
    /// them): what they were given may change, as unknown code changes what it is given, and
    /// their functions' code is met (`found`), so what it changes is followed.
    fn unknown_runs_generators(&mut self, met: &[ObjId], rid: ObjId, site_id: StrId, found: &mut Vec<ObjId>, state: &mut State) {
        let gens: Vec<std::rc::Rc<super::gens::LazyGen>> = met.iter().filter_map(|o| self.gens.lazy.get(o).cloned()).collect();
        if gens.is_empty() {
            return;
        }
        let given: Vec<Value> = gens.iter().flat_map(|lg| lg.args.iter().chain(lg.kwargs.values()).chain(lg.self_v.iter()).cloned().collect::<Vec<_>>()).collect();
        let walk = self.havoc.new_walk();
        self.havoc.walk_from(&mut self.heap, &given, rid, site_id, state, walk);
        found.append(&mut self.havoc.code);
        found.extend(gens.iter().map(|lg| lg.foid));
        found.sort_unstable();
        found.dedup();
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
            // (an object as Python writes one, `{…}`, `Settings(…)`; one named by how it was made
            // from others — a copy of an unknown call's result — is not shown)
            let g = crate::interp::render::given_text(self, *o);
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
            self.names.origins.insert(rid, (t, (node.module, node.line), given));
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
        let callables: Vec<ObjId> = found.into_iter().filter(|o| matches!(self.heap.kind(*o), Kind::Function | Kind::Bound | Kind::Partial | Kind::Module)).collect();
        if !callables.is_empty() {
            self.follow_code(callables, any, site_id, state);
        }
    }

    /// The records (and namespaces) `vals` reach through slots, at most `depth` steps below them.
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
    /// lists and sets any items and any length. Modules, classes and functions are not entered (their
    /// code is met: `follow_code`).
    pub(in crate::interp) fn havoc_reachable(&mut self, vals: &[Value], rid: ObjId, site: StrId, state: &mut State) {
        self.havoc.code.clear();
        let walk = self.havoc.new_walk();
        self.havoc.walk_from(&mut self.heap, vals, rid, site, state, walk);
    }

    /// The code followed again (`Havoc::follows`, key `key`) changed nothing, but what it may
    /// change may have changed again: as its walk would, it drops the tests of those objects
    /// (their rows got this call's mutator) and the last items of the lists among them.
    fn followed_again(&mut self, key: u64, state: &mut State) {
        // (what it may change may have changed again: the tests of it no longer hold, as
        // when its walk gives their rows this call's mutator)
        if let Some(m) = self.havoc.follows.get(&key) {
            drop_facts_on(state, |o| m.deps.contains(&o));
        }
        // (a list given an item since, as a walk would: its last item, any length)
        let lasts: Vec<ObjId> = state.lasts.iter().map(|(o, _)| *o).filter(|o| self.havoc.follows.get(&key).is_some_and(|m| m.deps.contains(o))).collect();
        for o in lasts {
            self.heap.set_prefix(o, state, 0);
        }
    }

    /// What the code `roots` hold may change when it runs where rowan does not run it: the
    /// values at the slots its closure may change are havocked, the slots it rebinds may hold
    /// anything, and the code the data they hold is followed too, in rounds (each applies what
    /// the closure gained; the rounds resume one walk, and a class attribute's flags apply again
    /// as they grow). The closure (`code_closure`) is cached per set of roots until a name it
    /// read unbound is bound or a module it walked gains a name. The same code followed before,
    /// with nothing it read or changed changed since, changes nothing again (`Havoc::follows`).
    pub(super) fn follow_code(&mut self, mut roots: Vec<ObjId>, rid: ObjId, site: StrId, state: &mut State) {
        roots.sort_unstable();
        roots.dedup();
        // (a closure is the program's own while it is followed whole: not remembered)
        let memo = self.effects.whole.is_none();
        // (a library's code reaching much of it: the closure the library's calls share)
        if memo && self.shares_lib_closure(&roots, state) {
            return self.follow_shared(roots, rid, site, state);
        }
        let key = self.havoc.follow_key(&roots);
        if memo && self.havoc.followed_unchanged(key, &roots, state) {
            self.followed_again(key, state);
            return;
        }
        let first = roots.clone();
        let counts0: Vec<u32> = self.havoc.guard.iter().map(|(_, n, _)| *n).collect();
        let mut deps: Vec<ObjId> = self.havoc.guard.iter().map(|(g, _, _)| *g).collect();
        let met0 = self.havoc.met.replace(Vec::new());
        let mut slots: FxHashSet<(ObjId, SlotKey)> = FxHashSet::default();
        let any = Value::obj(crate::interp::any_object(&mut self.heap));
        let walk = self.havoc.new_walk();
        self.havoc.code.clear();
        let mut walk_so_far = None;
        let mut prev: Option<std::rc::Rc<super::effects::Closure>> = None;
        let mut shared = false;
        loop {
            roots.append(&mut self.havoc.code);
            roots.sort_unstable();
            roots.dedup();
            // (a later round, with the code the data changed holds: past as much as a library's
            // code shares, the rest is the library's closure, `follow_shared`)
            if memo && prev.is_some() && self.any_library_code(&roots) && !self.closure_within(&roots, super::libclosure::SHARED_PAST, false, state) {
                self.follow_shared(roots, rid, site, state);
                shared = true;
                break;
            }
            // (resumed: the walk goes on from the last round's, so its lists go on from the last
            // closure's; a closure cached is another walk's)
            let resumed = walk_so_far.is_some() && !self.closure_cached(&roots, state);
            let full = self.code_closure(&roots, &mut walk_so_far, state);
            // (a later round: what the closure gained, the rest was applied)
            let gained;
            let cl: &super::effects::Closure = match &prev {
                Some(p) => {
                    gained = full.since(p, resumed);
                    &gained
                }
                None => &full,
            };
            let before = slots.len();
            slots.extend(cl.slot_locs().map(|(o, ns, k)| (o, (ns, k))));
            // (each slot it reads: part of the work measure)
            (before..slots.len()).for_each(|_| self.budget.walk_visit());
            deps.extend(cl.value_objects());
            // (a module walked as a value: all its names, those it gains too)
            deps.extend(cl.modules.iter().map(|(m, _)| *m));
            let mut vals: Vec<Value> = cl.locs.iter().map(|(o, ns, k)| state.slot(*o, *ns, *k)).collect();
            vals.extend(cl.values.iter().cloned());
            vals.extend(self.held_parts(cl, state, &mut deps).into_iter().map(Value::obj));
            self.havoc.walk_from(&mut self.heap, &vals, rid, site, state, walk);
            let objs = |locs: &[(ObjId, Ns, KeyId)], values: &[Value], state: &State| -> Vec<ObjId> { locs.iter().map(|(o, ns, k)| state.slot(*o, *ns, *k)).chain(values.iter().cloned()).flat_map(|v| v.pts.to_vec()).collect() };
            for (locs, values, change) in [(&cl.edited_locs, &cl.edited_values, true), (&cl.iterated_locs, &cl.iterated_values, false)] {
                let objs = objs(locs, values, state);
                deps.extend_from_slice(&objs);
                self.havoc.edit(&mut self.heap, &objs, change, site, state);
            }
            let removed: Vec<(ObjId, Removes)> = cl.removed_locs.iter().map(|((o, ns, k), r)| (state.slot(*o, *ns, *k), *r)).chain(cl.removed_values.iter().cloned()).flat_map(|(v, r)| v.pts.iter().map(move |o| (o, r)).collect::<Vec<_>>()).collect();
            deps.extend(removed.iter().map(|(o, _)| *o));
            may_have_removed(&self.heap, &removed, state);
            for (o, ns, k) in &cl.rebound {
                let v = state.slot(*o, *ns, *k).join(&any);
                state.set_slot(*o, *ns, *k, v, false);
            }
            self.havoc.code.retain(|c| !full.code.contains(c));
            prev = Some(full);
            if self.havoc.code.is_empty() {
                break;
            }
        }
        deps.extend(std::mem::replace(&mut self.havoc.met, met0).unwrap_or_default());
        if memo && !shared {
            let added: Vec<u32> = self.havoc.guard.iter().zip(counts0).map(|((_, n, _), n0)| n - n0).collect();
            self.havoc.remember_follow(key, first, added, deps, slots, state);
        }
    }

    /// The parts of the values a closure may change the parts of, not themselves (and of those
    /// whose parts' parts it may change, those), the values met added to `deps`.
    fn held_parts(&mut self, cl: &super::effects::Closure, state: &State, deps: &mut Vec<ObjId>) -> Vec<ObjId> {
        let mut parts: Vec<ObjId> = Vec::new();
        for v in cl.held_locs.iter().map(|(o, ns, k)| state.slot(*o, *ns, *k)).chain(cl.held_values.iter().cloned()) {
            for o in v.pts.iter() {
                deps.push(o);
                self.obj_edges(o, state, &mut parts);
            }
        }
        let mut mid: Vec<ObjId> = Vec::new();
        for v in cl.held2_locs.iter().map(|(o, ns, k)| state.slot(*o, *ns, *k)) {
            for o in v.pts.iter() {
                deps.push(o);
                self.obj_edges(o, state, &mut mid);
            }
        }
        for o in mid {
            deps.push(o);
            self.havoc.code.extend(code_of(&self.heap, o));
            self.obj_edges(o, state, &mut parts);
        }
        parts.sort_unstable();
        parts.dedup();
        parts
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

/// Drop the facts of tests (`CondFact`) that read or narrowed an object `on` holds.
pub(in crate::interp) fn drop_facts_on(state: &mut State, on: impl Fn(ObjId) -> bool) {
    if state.facts.is_empty() {
        return;
    }
    let kept: Vec<_> = state.facts.iter().filter(|f| !(f.deps.iter().any(|(_, v)| v.pts.iter().any(&on)) || f.rows.iter().any(|((o, _), _, _)| on(*o)) || f.guards.iter().any(|((o, _), _)| on(*o)))).cloned().collect();
    if kept.len() != state.facts.len() {
        state.facts = std::sync::Arc::new(kept);
    }
}

/// The code comparing or hashing `o` may run (a function's runs none of its code).
pub(in crate::interp) fn compared_code(heap: &Heap, o: ObjId) -> Option<ObjId> {
    code_of(heap, o).filter(|c| heap.kind(*c) != Kind::Function)
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

/// The objects a havoc walk is still to visit: what an object holds is queued unless the walk
/// met or queued it already (an object met again counts only as a skipped method's receiver,
/// `guard`: met through something else, it is walked whole).
struct Queue<'a> {
    work: &'a mut Vec<ObjId>,
    /// per object the last walk that met it, and that queued it (empty: queue everything)
    marks: &'a [u32],
    queued: &'a mut [u32],
    walk: u32,
    guard: &'a [ObjId],
}

impl Queue<'_> {
    fn extend(&mut self, v: &Value) {
        for o in v.pts.iter() {
            let i = o as usize;
            match self.queued.get_mut(i) {
                Some(q) if self.marks[i] == self.walk || *q == self.walk => {
                    if self.guard.contains(&o) {
                        self.work.push(o);
                    }
                }
                Some(q) => {
                    *q = self.walk;
                    self.work.push(o);
                }
                None => self.work.push(o),
            }
        }
    }
}

/// Unknown code may have changed object `o` (of kind `k`): its slots may hold anything
/// (`any_id`), its rows have the call as a mutator, a list or set any items and length (what
/// the slots hold is walked next).
#[allow(clippy::too_many_arguments)]
/// The program's own code not run may have removed items of `objs` (dicts, lists, sets changed
/// in place: `Flags::removes`): no key of theirs is there for sure, nor likely.
pub(super) fn may_have_removed(heap: &Heap, objs: &[(ObjId, Removes)], state: &mut State) {
    for &(o, rm) in objs {
        // (a dict's or set's keys, an instance's attributes, by key; a list's positions shift)
        let keys = rm.keys().filter(|_| matches!(heap.kind(o), Kind::Dict | Kind::Set | Kind::Instance));
        for ns in [Ns::Item, Ns::Attr] {
            if let Some(r0) = state.rows.get(&(o, ns)) {
                let r = match keys {
                    Some(ks) => r0.may_remove(ks),
                    None => r0.may_remove_any(),
                };
                if r != r0 {
                    state.set_row(o, ns, r);
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn havoc_object(heap: &mut Heap, state: &mut State, o: ObjId, k: Kind, site: StrId, any_id: ObjId, any: &Value, work: &mut Queue) {
    // (an enum member's `name` and `value` cannot be set or deleted)
    let member = heap.keys.member_key.contains_key(&o);
    let fixed = |ns: Ns, k: KeyId| member && ns == Ns::Attr && intern::key_as_str(k).is_some_and(|n| matches!(&*n, "name" | "value"));
    let mut writes: Vec<(SlotKey, Value)> = Vec::new();
    state.slots.each(&o, &mut |sk, v| {
        work.extend(v);
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
        state.slot(o, Ns::Elem, intern::STAR).pts.contains(any_id) && state.lens.get(&o) == Some((None, 0, 0)) && !state.lasts.iter().any(|(x, _)| *x == o) && state.elems.get(&o).is_some_and(|kv| kv_join(&kv, &DATA) == kv)
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
        let kv = kv_join(&state.elems.get(&o).unwrap_or(DATA), &DATA);
        state.elems.insert(o, kv);
        state.clear_last(o);
        heap.set_prefix(o, state, 0);
    }
}

/// The rows of an unknown call's result: the items of the mappings it was given itself (up to
/// `MAX_ARG_JOIN`; past that any record; not `**d`'s, which gives it d's values, nor the
/// callable's own, nor an `int`'s), else open through the call (not through a value read under
/// an input's key); changed by the call when it was given records.
fn unknown_result_rows(state: &mut State, rid: ObjId, site: &str, site_id: StrId, arg_recs: &[(u32, Ns)], item_recs: &[ObjId], callee: bool) {
    // (many records given — a value joined over many contexts: the result is any record,
    // an open row, rather than the join of them all and a copy of all their slots)
    // (its items: those of the mappings it was given; an instance's attributes are not
    // items)
    if !item_recs.is_empty() && item_recs.len() <= MAX_ARG_JOIN {
        let row = join_rows(item_recs.iter().map(|o| state.row(*o, Ns::Item)));
        let mut d = row.data();
        d.definite = KeySet::new();
        d.absent = KeySet::new();
        d.mutators = d.mutators.insert(site_id);
        // (an input given whole, or an element of one, may come back with its items: a key
        // read from the result may be one it must have, on that guess (the requirement's caveat
        // is the call). A value read under a key of an input is taken as a value, not a
        // mapping: the result is open through the call, `consume(line["sku"])["picks"]` needs
        // nothing of `line["sku"]`. An unknown callable's result is open through the call.)
        let src = Source::new(site, SourceKind::Call);
        let leaf = |(src, _): (SourceId, _)| intern::with_source(src, |s| s.path.last().is_some_and(|k| *k != intern::STAR));
        if callee {
            // (it may give back anything: no key of what it was given is there unless removed)
            d.likely = KeySet::new();
        }
        if callee || d.sources.iter().any(leaf) {
            d.sources = d.sources.filter(|x| !leaf(x)).insert((src, intern::EMPTY_LENS));
        }
        state.set_row(rid, Ns::Item, Row::from_data(d));
        // as an object the result is unknown: its attributes come from the call
        state.set_row(rid, Ns::Attr, Row::with_source(src).mutate(site_id));
        for oid in item_recs {
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
