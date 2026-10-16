//! Abstract objects and the flow-sensitive analysis state (port of `rowan/state.py`).

use std::sync::Arc;

use indexmap::IndexMap;
use rustc_hash::FxHashMap;

use crate::domain::{kv_join, KeyVal, Ns, ObjId, ReqInfo, Row, Value};
use crate::intern::{IdSet, KeyId, SourceId, StrId, STAR};
use crate::layered::{LayerMap, TwoLevel};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Kind {
    Dict,
    Frame,
    Instance,
    Unknown,
    Module,
    Class,
    Namespace,
    List,
    Tuple,
    Set,
    Function,
    Bound,
    Method,
    External,
    Cell,
    View,
    Zip,
    Enumerate,
    Pair,
    Partial,
    Groupby,
    Locidx,
    Super,
    Response,
    Series,
    Parser,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Dict => "dict",
            Kind::Frame => "frame",
            Kind::Instance => "instance",
            Kind::Unknown => "unknown",
            Kind::Module => "module",
            Kind::Class => "class",
            Kind::Namespace => "namespace",
            Kind::List => "list",
            Kind::Tuple => "tuple",
            Kind::Set => "set",
            Kind::Function => "function",
            Kind::Bound => "bound",
            Kind::Method => "method",
            Kind::External => "external",
            Kind::Cell => "cell",
            Kind::View => "view",
            Kind::Zip => "zip",
            Kind::Enumerate => "enumerate",
            Kind::Pair => "pair",
            Kind::Partial => "partial",
            Kind::Groupby => "groupby",
            Kind::Locidx => "locidx",
            Kind::Super => "super",
            Kind::Response => "response",
            Kind::Series => "series",
            Kind::Parser => "parser",
        }
    }
    pub fn is_record(self) -> bool {
        matches!(self, Kind::Dict | Kind::Frame | Kind::Instance | Kind::Unknown | Kind::Module | Kind::Class | Kind::Namespace)
    }
    pub fn is_container(self) -> bool {
        matches!(self, Kind::List | Kind::Tuple | Kind::Set)
    }
    /// RECORD_KINDS or frame (the common "record-like" test of the prototype)
    pub fn is_record_like(self) -> bool {
        self.is_record() || self == Kind::Frame
    }
}

pub type SlotKey = (Ns, KeyId);

/// Mutable analysis state at one program point.
#[derive(Clone)]
pub struct State {
    /// current frame's plain locals (small; copied eagerly)
    pub env: IndexMap<StrId, Value>,
    /// path-sensitive facts about attribute chains (`isinstance(a.b, C)` → `a.b` on this path)
    pub narrowed: FxHashMap<StrId, Value>,
    pub rows: LayerMap<(ObjId, Ns), Row>,
    /// (oid) -> {(ns, key) -> Value}; key '*' = stored under a non-literal key
    pub slots: TwoLevel<ObjId, SlotKey, Value>,
    /// literal element keys of containers
    pub elems: LayerMap<ObjId, KeyVal>,
    /// lists / tuples: (the length when known on this path, the number of leading positions
    /// known: all of them when the length is known); written at birth, so a key on one side
    /// only of a join is an object born there
    pub lens: LayerMap<ObjId, (Option<usize>, usize, u16)>,
    /// Source -> {key term -> ReqInfo}
    pub reqs: TwoLevel<SourceId, KeyId, ReqInfo>,
    pub ret: Option<Value>,
    pub reachable: bool,
    pub exits: Option<Arc<State>>,
    /// what a test that was joined over said about rows (see `CondFact`)
    pub facts: Arc<Vec<Arc<CondFact>>>,
    /// (name, key): the object the name is bound to has the key, whichever of its possible
    /// objects that is (a store or setdefault through the name)
    pub name_keys: Arc<Vec<(StrId, KeyId)>>,
    /// lists whose last element is known on this path (an `append` since any other change of
    /// their elements): `lst[-1]` after appends of unknown count
    pub lasts: Arc<Vec<(ObjId, Value)>>,
    /// tensor-shape facts on this path: ranks and dim equalities (shape.rs)
    pub tf: Arc<crate::models::shape::TFacts>,
    /// locals bound on some paths only (a join of a path that bound the name with one that did
    /// not): reading one may raise UnboundLocalError
    pub unbound: Arc<Vec<StrId>>,
    /// tests of the running functions' parameters every path here passed, as Python text true
    /// at the call (`interp/guards.rs`): (`param F.` of the function, the test)
    pub path: Arc<Vec<(StrId, StrId)>>,
    /// lists whose length is a tensor dim (`shape.rs` label): a loop of `T` rounds appended once
    /// per round (`loops.rs` `counted_appends`); any other change of the list ends it
    pub sym_lens: Arc<Vec<(ObjId, StrId)>>,
}

/// At the join of a branch on a pure test: when `cond` (a canonical test text) is `truth`,
/// rows were as in `rows` (the branch row, and the joined row it was folded into). A later
/// test with the same text, while the names it reads still hold `deps`, restores the branch
/// row of every object whose row is still the joined one (unchanged since).
#[derive(Debug)]
pub struct CondFact {
    pub cond: StrId,
    pub truth: bool,
    pub deps: Vec<(StrId, Value)>,
    pub rows: Vec<((ObjId, Ns), Row, Row)>,
    /// rows of the objects the test reads, as at the join: the fact holds only while they are
    /// unchanged (a test like `"x" in cfg` reads cfg's keys)
    pub guards: Vec<((ObjId, Ns), Row)>,
    /// the possibly-unbound locals of the branch and of the join (`State::unbound`), when they
    /// differ: a name bound in this branch only is bound again when the test recurs
    pub unbound: Option<(Arc<Vec<StrId>>, Arc<Vec<StrId>>)>,
}

impl Default for State {
    fn default() -> Self {
        State::new()
    }
}

/// A join of states added one after the other, made pairwise as they come (a binary
/// counter: two partial joins of as many states each are joined): the work of `join_all`,
/// holding a logarithmic number of states.
#[derive(Default)]
pub struct Joins(Vec<(State, usize)>);

impl Joins {
    pub fn add(&mut self, s: State) {
        let mut cur = (s, 1);
        while let Some((mut prev, n)) = self.0.pop() {
            if n != cur.1 {
                self.0.push((prev, n));
                break;
            }
            cur = (prev.join(&mut cur.0), n * 2);
        }
        self.0.push(cur);
    }

    pub fn finish(self) -> Option<State> {
        State::join_all(self.0.into_iter().map(|(s, _)| s).collect())
    }
}

thread_local! {
    /// How often all name facts were dropped (a deletion, unknown code): a callee that did
    /// drops its caller's too.
    pub static NAME_KEY_DROPS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

impl State {
    /// The name's object has `k` from here on.
    pub fn add_name_key(&mut self, n: StrId, k: KeyId) {
        if !self.name_keys.contains(&(n, k)) {
            let mut v: Vec<(StrId, KeyId)> = self.name_keys.iter().copied().collect();
            v.push((n, k));
            if v.len() > 64 {
                v.remove(0);
            }
            self.name_keys = Arc::new(v);
        }
    }

    pub fn name_has_key(&self, n: StrId, k: KeyId) -> bool {
        self.name_keys.contains(&(n, k))
    }

    /// Forget name facts (all of them, or those of one name).
    pub fn drop_name_keys(&mut self, n: Option<StrId>) {
        if n.is_none() {
            NAME_KEY_DROPS.with(|c| c.set(c.get() + 1));
        }
        if self.name_keys.is_empty() {
            return;
        }
        self.name_keys = Arc::new(match n {
            None => Vec::new(),
            Some(n) => self.name_keys.iter().filter(|(m, _)| *m != n).copied().collect(),
        });
    }

    pub fn new() -> State {
        State {
            env: IndexMap::new(),
            narrowed: FxHashMap::default(),
            rows: LayerMap::new(),
            slots: TwoLevel::new(),
            elems: LayerMap::new(),
            lens: LayerMap::new(),
            reqs: TwoLevel::new(),
            ret: None,
            reachable: true,
            exits: None,
            facts: Arc::new(Vec::new()),
            name_keys: Arc::new(Vec::new()),
            lasts: Arc::new(Vec::new()),
            tf: Arc::new(crate::models::shape::TFacts::default()),
            unbound: Arc::new(Vec::new()),
            path: Arc::new(Vec::new()),
            sym_lens: Arc::new(Vec::new()),
        }
    }

    pub fn bottom() -> State {
        let mut s = State::new();
        s.reachable = false;
        s
    }

    // -- copying -----------------------------------------------------------
    /// A copy (O(1): the maps are persistent).
    pub fn copy(&self) -> State {
        self.clone()
    }

    /// What this state says of the heap (objects' rows, slots, elements, lengths, tensor facts),
    /// without the running frame's names, path, returns or requirements.
    pub fn heap_only(&self) -> State {
        State { rows: self.rows.clone(), slots: self.slots.clone(), elems: self.elems.clone(), lens: self.lens.clone(), lasts: self.lasts.clone(), tf: self.tf.clone(), sym_lens: self.sym_lens.clone(), reachable: self.reachable, ..State::new() }
    }

    /// Join what `o` says of the objects this state knows (rows, slots, elements, lengths) into
    /// it: an object only `o` has facts of reads as unknown here. Costs what the two differ in.
    pub fn join_known(&mut self, o: &State) {
        if o.reachable {
            let prev = self.copy();
            self.join_from(&prev, o, true);
        }
    }

    /// Join into this state, which is at least `prev` (a join of states up to it), the heap of
    /// `o`: only where `o` differs from `prev` can it add anything (states made one after the
    /// other: what changed between them). `known`: only of the objects this state has facts of.
    pub fn join_from(&mut self, prev: &State, o: &State, known: bool) {
        let mut facts_of: FxHashMap<ObjId, bool> = FxHashMap::default();
        let mut new_ok = |s: &State, oid: ObjId| !known || *facts_of.entry(oid).or_insert_with(|| s.has_facts(oid));
        for k in o.rows.diff_keys(&prev.rows) {
            let Some(v) = o.rows.get(&k) else { continue };
            match self.rows.get(&k) {
                Some(cur) if cur == v => {}
                Some(cur) => self.rows.insert(k, cur.join(&v)),
                None if new_ok(self, k.0) => self.rows.insert(k, v),
                None => {}
            }
        }
        for (oid, sk) in o.slots.diff_keys(&prev.slots) {
            let Some(v) = o.slots.get(&oid, &sk) else { continue };
            match self.slots.get(&oid, &sk) {
                Some(cur) if cur == v => {}
                Some(cur) => self.slots.set(&oid, sk, cur.join(&v)),
                None if new_ok(self, oid) => self.slots.set(&oid, sk, v),
                None => {}
            }
        }
        for k in o.elems.diff_keys(&prev.elems) {
            match (o.elems.get(&k), self.elems.get(&k)) {
                (Some(v), Some(cur)) => self.elems.insert(k, kv_join(&cur, &v)),
                (Some(v), None) if !known => self.elems.insert(k, v),
                _ => {}
            }
        }
        for k in o.lens.diff_keys(&prev.lens) {
            match (o.lens.get(&k), self.lens.get(&k)) {
                (Some(v), Some(cur)) => self.lens.insert(k, join_len(&cur, &v)),
                (Some(v), None) if !known => self.lens.insert(k, v),
                _ => {}
            }
        }
        self.lasts = join_lasts(&self.lasts, &o.lasts);
        if !Arc::ptr_eq(&self.sym_lens, &o.sym_lens) {
            self.sym_lens = Arc::new(self.sym_lens.iter().filter(|x| o.sym_lens.contains(x)).copied().collect());
        }
        if !(Arc::ptr_eq(&self.tf, &o.tf) || self.tf == o.tf) {
            self.tf = Arc::new(self.tf.join(&o.tf));
        }
        self.facts = Arc::new(Vec::new());
        self.name_keys = Arc::new(Vec::new());
    }

    /// Has this state facts of object `oid` (a row, a length, elements, a cell's value)?
    pub fn has_facts(&self, oid: ObjId) -> bool {
        [Ns::Item, Ns::Attr, Ns::Pos, Ns::Elem].iter().any(|ns| self.rows.get(&(oid, *ns)).is_some()) || self.lens.get(&oid).is_some() || self.elems.get(&oid).is_some() || self.slots.get(&oid, &(Ns::Cell, crate::intern::key_str_lit("v"))).is_some()
    }

    /// Snapshot the heap at a `return` so the caller sees it joined over all return points.
    pub fn record_exit(&mut self) {
        let mut snap = self.copy();
        snap.reachable = true;
        snap.ret = None;
        snap.exits = None;
        self.exits = Some(Arc::new(match self.exits.take() {
            None => snap,
            Some(e) => e.copy().join(&mut snap),
        }));
    }

    pub fn clear_last(&mut self, oid: ObjId) {
        // (an element changed: the length alternatives of the last join no longer hold either)
        if let Some((None, p, m)) = self.lens.get(&oid) {
            if m != 0 {
                self.lens.insert(oid, (None, p, 0));
            }
        }
        if self.lasts.iter().any(|(o, _)| *o == oid) {
            self.lasts = Arc::new(self.lasts.iter().filter(|(o, _)| *o != oid).cloned().collect());
        }
        self.drop_sym_len(oid);
    }

    pub fn set_last(&mut self, oid: ObjId, v: Value) {
        let mut l: Vec<(ObjId, Value)> = self.lasts.iter().filter(|(o, _)| *o != oid).cloned().collect();
        l.push((oid, v));
        self.lasts = Arc::new(l);
    }

    /// A list's length on this path (positions, alternatives): any write of it goes through here.
    pub fn set_lens(&mut self, oid: ObjId, v: (Option<usize>, usize, u16)) {
        self.lens.insert(oid, v);
        self.drop_sym_len(oid);
    }

    pub fn forget_len(&mut self, oid: &ObjId) {
        self.lens.remove(oid);
        self.drop_sym_len(*oid);
    }

    pub fn drop_sym_len(&mut self, oid: ObjId) {
        if self.sym_lens.iter().any(|(o, _)| *o == oid) {
            self.sym_lens = Arc::new(self.sym_lens.iter().filter(|(o, _)| *o != oid).copied().collect());
        }
    }

    pub fn set_sym_len(&mut self, oid: ObjId, label: StrId) {
        let mut l: Vec<(ObjId, StrId)> = self.sym_lens.iter().filter(|(o, _)| *o != oid).copied().collect();
        l.push((oid, label));
        self.sym_lens = Arc::new(l);
    }

    /// The length of a list as a tensor dim's label (`sym_lens`).
    pub fn sym_len(&self, oid: ObjId) -> Option<StrId> {
        self.sym_lens.iter().find(|(o, _)| *o == oid).map(|(_, l)| *l)
    }

    pub fn last_of(&self, oid: ObjId) -> Option<Value> {
        self.lasts.iter().find(|(o, _)| *o == oid).map(|(_, v)| v.clone())
    }

    pub fn replace_with(&mut self, o: State) {
        *self = o;
    }

    // -- lattice -----------------------------------------------------------
    pub fn join(&mut self, o: &mut State) -> State {
        let ret = match (&self.ret, &o.ret) {
            (None, r) | (r, None) => r.clone(),
            (Some(a), Some(b)) => Some(a.join(b)),
        };
        let exits = match (&self.exits, &o.exits) {
            (None, e) | (e, None) => e.clone(),
            (Some(a), Some(b)) if Arc::ptr_eq(a, b) => Some(a.clone()),
            (Some(a), Some(b)) => Some(Arc::new(a.copy().join(&mut b.copy()))),
        };
        // (an unreachable side adds nothing but its returns)
        if !o.reachable || !self.reachable {
            let mut s = if o.reachable { o.copy() } else { self.copy() };
            s.ret = ret;
            s.exits = exits;
            return s;
        }
        // facts both sides still hold
        let facts = if self.facts.is_empty() || o.facts.is_empty() { Vec::new() } else { self.fresh_facts(o) };
        let mut s = State::new();
        s.env = join_env(&self.env, &o.env);
        if !self.narrowed.is_empty() && !o.narrowed.is_empty() {
            s.narrowed = self.narrowed.iter().filter(|(k, v)| o.narrowed.get(*k) == Some(*v)).map(|(k, v)| (*k, v.clone())).collect();
        }
        s.rows = self.rows.join(&o.rows, &mut |a, b| a.join(b));
        s.slots = self.slots.join(&o.slots, &mut |a, b| a.join(b));
        s.elems = self.elems.join(&o.elems, &mut |a, b| kv_join(a, b));
        s.lens = self.lens.join(&o.lens, &mut join_len);
        s.reqs = self.reqs.join_with_keep(&o.reqs, &mut |a, b| a.join(b), &mut |a| a.demote(), &nested_keep);
        s.ret = ret;
        s.exits = exits;
        s.reachable = true;
        s.facts = Arc::new(facts);
        s.tf = if Arc::ptr_eq(&self.tf, &o.tf) || self.tf == o.tf { self.tf.clone() } else { Arc::new(self.tf.join(&o.tf)) };
        s.name_keys = if Arc::ptr_eq(&self.name_keys, &o.name_keys) { self.name_keys.clone() } else { Arc::new(self.name_keys.iter().filter(|x| o.name_keys.contains(x)).copied().collect()) };
        s.lasts = join_lasts(&self.lasts, &o.lasts);
        s.unbound = join_unbound(self, o);
        s.sym_lens = if Arc::ptr_eq(&self.sym_lens, &o.sym_lens) { self.sym_lens.clone() } else { Arc::new(self.sym_lens.iter().filter(|x| o.sym_lens.contains(x)).copied().collect()) };
        s.path = if Arc::ptr_eq(&self.path, &o.path) { self.path.clone() } else { Arc::new(self.path.iter().filter(|t| o.path.contains(t)).copied().collect()) };
        s
    }

    /// The facts both `self` and `o` hold, each without the rows a path wrote since (a row the
    /// two differ in: a write on one of them, though the join may give back an equal row).
    fn fresh_facts(&self, o: &State) -> Vec<Arc<CondFact>> {
        let mut out = Vec::new();
        for f in self.facts.iter().filter(|f| o.facts.iter().any(|g| Arc::ptr_eq(f, g))) {
            let same = |k: &(ObjId, Ns)| match (self.rows.get(k), o.rows.get(k)) {
                (Some(a), Some(b)) => a.same_ptr(&b),
                (None, None) => true,
                _ => false,
            };
            if f.rows.iter().all(|(k, _, _)| same(k)) && f.guards.iter().all(|(k, _)| same(k)) {
                out.push(f.clone());
                continue;
            }
            // (a test's objects written since: the test may read otherwise now)
            if !f.guards.iter().all(|(k, _)| same(k)) {
                continue;
            }
            let rows: Vec<((ObjId, Ns), Row, Row)> = f.rows.iter().filter(|(k, _, _)| same(k)).cloned().collect();
            if !rows.is_empty() || f.unbound.is_some() {
                out.push(Arc::new(CondFact { cond: f.cond, truth: f.truth, deps: f.deps.clone(), rows, guards: f.guards.clone(), unbound: f.unbound.clone() }));
            }
        }
        out
    }

    /// The join of several states, pairwise: each round joins what two of them wrote, not a
    /// growing join again (states made one after the other share most of their structure).
    pub fn join_all(mut states: Vec<State>) -> Option<State> {
        while states.len() > 1 {
            let mut next: Vec<State> = Vec::with_capacity(states.len().div_ceil(2));
            let mut it = states.into_iter();
            while let Some(mut a) = it.next() {
                match it.next() {
                    Some(mut b) => next.push(a.join(&mut b)),
                    None => next.push(a),
                }
            }
            states = next;
        }
        states.pop()
    }

    /// The join at a loop head: like `join`, but a requirement recorded only in the loop body
    /// stays "always" (per element of the iteration; spec: in-loop demands are not conditional).
    pub fn join_loop(&mut self, o: &mut State) -> State {
        let mut s = self.join(o);
        if self.reachable && o.reachable {
            s.reqs = self.reqs.join_with(&o.reqs, &mut |x, y| x.join(y), &mut |x| x.clone());
        }
        s
    }

    /// Join the heap facts of `o` restricted to a footprint (the objects a callee touched).
    pub fn join_heap_fp(&mut self, o: &State, fp: &Footprint) {
        if !o.reachable {
            return;
        }
        // Only where `o` differs from `self` can a join change anything: a structural diff of
        // the two (they descend from one another and share structure: the diff costs what
        // differs), restricted to the footprint. A recursive call answered by its running
        // activation joins that activation's footprint at every call: per key it was calls ×
        // footprint lookups.
        let rows: Vec<(ObjId, Ns)> = {
            let s: rustc_hash::FxHashSet<(ObjId, Ns)> = fp.rows.iter().copied().collect();
            o.rows.diff_keys(&self.rows).into_iter().filter(|k| s.contains(k)).collect()
        };
        let slots: Vec<(ObjId, SlotKey)> = {
            let s: rustc_hash::FxHashSet<(ObjId, SlotKey)> = fp.slots.iter().copied().collect();
            o.slots.diff_keys(&self.slots).into_iter().filter(|k| s.contains(k)).collect()
        };
        let elems: Vec<ObjId> = {
            let s: rustc_hash::FxHashSet<ObjId> = fp.elems.iter().copied().collect();
            o.elems.diff_keys(&self.elems).into_iter().filter(|k| s.contains(k)).collect()
        };
        FP_VISITS.with(|c| c.set(c.get() + rows.len() + slots.len() + elems.len()));
        for k in &rows {
            let Some(v) = o.rows.get(k) else { continue };
            match self.rows.get(k) {
                None => self.rows.insert(*k, v),
                Some(cur) => {
                    if cur != v {
                        self.rows.insert(*k, cur.join(&v));
                    }
                }
            }
        }
        // (the callee changed these lists' elements: their last element is not known here)
        if !self.lasts.is_empty() {
            for (oid, sk) in &fp.slots {
                if matches!(sk.0, Ns::Pos | Ns::Elem) {
                    self.clear_last(*oid);
                }
            }
        }
        for (oid, sk) in &slots {
            let Some(v) = o.slots.get(oid, sk) else { continue };
            match self.slots.get(oid, sk) {
                None => self.slots.set(oid, *sk, v),
                Some(cur) => {
                    if cur != v {
                        self.slots.set(oid, *sk, cur.join(&v));
                    }
                }
            }
        }
        // the lengths of the objects the callee touched
        let mut touched: Vec<ObjId> = fp.slots.iter().map(|(o, _)| *o).chain(fp.elems.iter().copied()).collect();
        touched.sort_unstable();
        touched.dedup();
        let touched: Vec<ObjId> = {
            let s: rustc_hash::FxHashSet<ObjId> = touched.into_iter().collect();
            o.lens.diff_keys(&self.lens).into_iter().filter(|k| s.contains(k)).collect()
        };
        for k in touched {
            if let Some(v) = o.lens.get(&k) {
                match self.lens.get(&k) {
                    None => self.set_lens(k, v),
                    Some(cur) => {
                        if cur != v {
                            self.set_lens(k, join_len(&cur, &v));
                        }
                    }
                }
            }
        }
        for k in &elems {
            let Some(v) = o.elems.get(k) else { continue };
            match self.elems.get(k) {
                None => self.elems.insert(*k, v),
                Some(cur) => {
                    if cur != v {
                        self.elems.insert(*k, kv_join(&cur, &v));
                    }
                }
            }
        }
        for (src, k) in o.reqs.diff_keys(&self.reqs) {
            let Some(info) = o.reqs.get(&src, &k) else { continue };
            let nv = match self.reqs.get(&src, &k) {
                Some(cur) => cur.join(&info),
                None => info.demote(),
            };
            self.reqs.set(&src, k, nv);
        }
    }

    /// Keys whose facts differ from `base` (what a callee changed or created).
    pub fn footprint(&self, base: &State) -> Footprint {
        Footprint { rows: self.rows.diff_keys(&base.rows), slots: self.slots.diff_keys(&base.slots), elems: self.elems.diff_keys(&base.elems) }
    }

    pub fn same(&self, o: &State) -> bool {
        self.reachable == o.reachable
            && self.env == o.env
            && self.narrowed == o.narrowed
            && self.ret == o.ret
            && (Arc::ptr_eq(&self.tf, &o.tf) || self.tf == o.tf)
            && (Arc::ptr_eq(&self.lasts, &o.lasts) || self.lasts == o.lasts)
            && self.exits.is_none() == o.exits.is_none()
            && self.rows.same(&o.rows)
            && self.slots.same(&o.slots)
            && self.elems.same(&o.elems)
            && self.lens.same(&o.lens)
            && self.reqs.same(&o.reqs)
            && match (&self.exits, &o.exits) {
                (Some(a), Some(b)) => a.same(b),
                _ => true,
            }
    }

    // -- accessors ---------------------------------------------------------
    pub fn row(&self, oid: ObjId, ns: Ns) -> Row {
        if let Some(r) = self.rows.get(&(oid, ns)) {
            return r;
        }
        if is_any(oid) && matches!(ns, Ns::Item | Ns::Attr) {
            return Row::top();
        }
        Row::new()
    }

    pub fn has_row(&self, oid: ObjId, ns: Ns) -> bool {
        self.rows.contains_key(&(oid, ns)) || (is_any(oid) && matches!(ns, Ns::Item | Ns::Attr))
    }

    pub fn set_row(&mut self, oid: ObjId, ns: Ns, row: Row) {
        self.mirror_row(oid, ns, &row);
        self.set_row_narrowed(oid, ns, row);
    }

    /// Key `k` stored into `oid`'s row: maybe into the others of its family too (a weak store
    /// there, not their rows joined: what else they hold is as it was).
    pub fn store_key_row(&mut self, oid: ObjId, ns: Ns, k: KeyId, strong: bool) {
        for m in family(oid) {
            if let Some(cur) = self.rows.get(&(m, ns)) {
                self.rows.insert((m, ns), cur.store(k, false));
            }
        }
        let r = self.row(oid, ns).store(k, strong);
        self.set_row_narrowed(oid, ns, r);
    }

    /// A row the path learned more about (a test, a read that succeeded), not one the program
    /// changed: the other objects of its family (`link_view`) keep theirs.
    pub fn set_row_narrowed(&mut self, oid: ObjId, ns: Ns, row: Row) {
        // (what unknown code holds is anything, whatever is stored into it)
        if is_any(oid) {
            return;
        }
        // (an object several runtime objects share: what is installed joins what is there)
        if is_merged(oid) {
            if let Some(cur) = self.rows.get(&(oid, ns)) {
                if cur != row {
                    let j = cur.join(&row);
                    self.rows.insert((oid, ns), j);
                }
                return;
            }
        }
        self.rows.insert((oid, ns), row);
    }

    /// A change of `oid`'s row may be a change of each object of its family (a loop's item
    /// and the records it is one of, `link_view`): theirs on this path join it.
    /// (A view's row set to what it is changes nothing: the record it stands for is as it was.
    /// The summary's always may: its row covers the record a change hits and others.)
    fn mirror_row(&mut self, oid: ObjId, ns: Ns, row: &Row) {
        if view_of(oid).is_some() && self.rows.get(&(oid, ns)).as_ref() == Some(row) {
            return;
        }
        for m in family(oid) {
            if let Some(cur) = self.rows.get(&(m, ns)) {
                note_view_hit(m);
                self.rows.insert((m, ns), cur.join(row));
            }
        }
    }

    pub fn slot(&self, oid: ObjId, ns: Ns, key: KeyId) -> Value {
        self.slots.get(&oid, &(ns, key)).unwrap_or_default()
    }

    pub fn slot_opt(&self, oid: ObjId, ns: Ns, key: KeyId) -> Option<Value> {
        self.slots.get(&oid, &(ns, key))
    }

    pub fn has_slot(&self, oid: ObjId, ns: Ns, key: KeyId) -> bool {
        self.slots.contains(&oid, &(ns, key))
    }

    pub fn set_slot(&mut self, oid: ObjId, ns: Ns, key: KeyId, v: Value, strong: bool) {
        // (a value stored into a loop's item or the records it is one of: maybe into the others)
        for m in family(oid) {
            if self.has_row(m, Ns::Item) || self.has_row(m, Ns::Attr) {
                let nv = self.slot(m, ns, key).join(&v);
                self.slots.set(&m, (ns, key), nv);
            }
        }
        self.set_slot_found(oid, ns, key, v, strong);
    }

    /// A value a read found under a key (an input's, a placeholder for it), not one the program
    /// stored: the other objects of its family (`link_view`) keep theirs.
    pub fn set_slot_found(&mut self, oid: ObjId, ns: Ns, key: KeyId, v: Value, strong: bool) {
        if is_any(oid) {
            return;
        }
        if matches!(ns, Ns::Pos | Ns::Elem) {
            self.clear_last(oid);
        }
        let strong = strong && !is_merged(oid);
        if strong {
            self.slots.set(&oid, (ns, key), v);
        } else {
            let nv = self.slot(oid, ns, key).join(&v);
            self.slots.set(&oid, (ns, key), nv);
        }
    }

    pub fn pop_slot(&mut self, oid: ObjId, ns: Ns, key: KeyId) -> Option<Value> {
        if matches!(ns, Ns::Pos | Ns::Elem) {
            self.clear_last(oid);
        }
        self.slots.remove(&oid, &(ns, key))
    }

    pub fn all_slots(&self, oid: ObjId, ns: Ns) -> Value {
        let mut acc = Value::new();
        for ((n, _k), v) in self.slots.for_key(&oid) {
            if n == ns {
                acc = acc.join(&v);
            }
        }
        acc
    }

    pub fn slot_keys(&self, oid: ObjId, ns: Ns) -> Vec<KeyId> {
        self.slots.for_key(&oid).into_iter().filter(|((n, k), _)| *n == ns && *k != STAR).map(|((_, k), _)| k).collect()
    }

    /// [(key, Value)] of one object's namespace, sorted by key id.
    pub fn obj_slots(&self, oid: ObjId, ns: Ns) -> Vec<(KeyId, Value)> {
        let mut v: Vec<(KeyId, Value)> = self.slots.for_key(&oid).into_iter().filter(|((n, _), _)| *n == ns).map(|((_, k), v)| (k, v)).collect();
        v.sort_by_key(|(k, _)| *k);
        v
    }

    /// All slots of one object, sorted by (ns, key).
    pub fn slots_of(&self, oid: ObjId) -> Vec<(SlotKey, Value)> {
        let mut v = self.slots.for_key(&oid);
        v.sort_by_key(|(k, _)| *k);
        v
    }

    /// Forget all of an object (an object that is garbage on this path: a join takes the
    /// other paths' facts about it).
    pub fn forget_obj(&mut self, oid: ObjId) {
        for ns in [Ns::Item, Ns::Attr, Ns::Cell, Ns::Pos, Ns::Elem, Ns::Meta] {
            self.rows.remove(&(oid, ns));
        }
        self.slots.drop_key(&oid);
        self.elems.remove(&oid);
        self.lens.remove(&oid);
        self.drop_sym_len(oid);
    }

    pub fn drop_obj(&mut self, oid: ObjId) {
        self.slots.drop_key(&oid);
    }

    pub fn drop_obj_ns(&mut self, oid: ObjId, ns: Ns) {
        if matches!(ns, Ns::Pos | Ns::Elem) {
            self.clear_last(oid);
        }
        self.slots.drop_where(&oid, |(n, _)| *n == ns);
    }

    pub fn require(&mut self, src: SourceId, term: KeyId, alternatives: IdSet<SourceId>, caveats: IdSet<StrId>, alt_keys: IdSet<KeyId>) {
        let info = ReqInfo {
            always: true,
            alternatives,
            caveats,
            alt_keys: if alt_keys.len() > 1 { alt_keys } else { IdSet::new() },
            attr: false,
            only_if: self.path_tests(src, term),
        };
        let nv = match self.reqs.get(&src, &term) {
            Some(cur) => cur.seq(&info),
            None => info,
        };
        self.reqs.set(&src, term, nv);
    }

    /// The tests of `src`'s function's parameters every path here passed (`param F.x`: F's),
    /// but `"k" in x` for `x["k"]` itself (the read a test of the key guards says nothing more).
    fn path_tests(&self, src: SourceId, term: KeyId) -> IdSet<StrId> {
        if self.path.is_empty() {
            return IdSet::new();
        }
        let (site, top) = crate::intern::with_source(src, |s| (s.site.clone(), s.path.is_empty()));
        let own = |t: StrId| top && site.rsplit_once('.').is_some_and(|(_, x)| *crate::intern::str_get(t).replace('\'', "\"") == format!("{} in {}", crate::domain::key_display(term), x));
        IdSet::from_iter(self.path.iter().filter(|(o, t)| site.starts_with(&*crate::intern::str_get(*o)) && !own(*t)).map(|(_, t)| *t))
    }

    pub fn require_simple(&mut self, src: SourceId, term: KeyId) {
        self.require(src, term, IdSet::new(), IdSet::new(), IdSet::new());
    }
}

#[derive(Clone, Debug, Default)]
pub struct Footprint {
    pub rows: Vec<(ObjId, Ns)>,
    pub slots: Vec<(ObjId, SlotKey)>,
    pub elems: Vec<ObjId>,
}

/// Locals possibly unbound after a join: those of either side, and the names bound on one side
/// only.
fn join_unbound(a: &State, b: &State) -> Arc<Vec<StrId>> {
    let one_sided = a.env.keys().filter(|k| !b.env.contains_key(*k)).chain(b.env.keys().filter(|k| !a.env.contains_key(*k)));
    let mut out: Vec<StrId> = a.unbound.iter().chain(b.unbound.iter()).copied().chain(one_sided.copied()).collect();
    if out.is_empty() {
        return a.unbound.clone();
    }
    out.sort();
    out.dedup();
    if out == *a.unbound {
        return a.unbound.clone();
    }
    Arc::new(out)
}

pub fn join_env(a: &IndexMap<StrId, Value>, b: &IndexMap<StrId, Value>) -> IndexMap<StrId, Value> {
    let mut out = a.clone();
    for (k, v) in b.iter() {
        match out.get(k) {
            None => {
                out.insert(*k, v.clone());
            }
            Some(cur) => {
                if cur != v {
                    let nv = cur.join(v);
                    out.insert(*k, nv);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Source, SourceKind};
    use crate::intern::{key_str_lit, s};

    #[test]
    fn reqs_survive_return_in_branch() {
        let src1 = Source::new("param f.x", SourceKind::Param);
        let src2 = Source::new("param g.a", SourceKind::Param);
        let mut state = State::new();
        state.require_simple(src1, key_str_lit("k1"));
        let mut st = state.copy();
        let mut then_s = st.copy();
        let mut else_s = st.copy();
        then_s.require_simple(src2, key_str_lit("objs"));
        then_s.record_exit();
        then_s.reachable = false;
        let mut j = then_s.join(&mut else_s);
        assert!(j.reachable);
        let ex = j.exits.take().unwrap();
        let mut exs = ex.copy();
        let fin = j.join(&mut exs);
        let d = fin.reqs.inner(&src2).expect("source lost");
        assert!(d.contains_key(&key_str_lit("objs")), "key lost");
        assert!(!d[&key_str_lit("objs")].always);
    }
}

#[cfg(test)]
mod tests2 {
    use super::*;
    use crate::domain::{Source, SourceKind};
    use crate::intern::{key_str_lit, s};

    /// A sequence of `if cond: return` statements, one of which records a requirement.
    fn branch_return(st: &mut State, req: Option<(crate::intern::SourceId, &str, &str)>) {
        let mut then_s = st.copy();
        let mut else_s = st.copy();
        if let Some((src, k, site)) = req {
            then_s.require_simple(src, key_str_lit(k));
        }
        then_s.record_exit();
        then_s.reachable = false;
        let j = then_s.join(&mut else_s);
        st.replace_with(j);
    }

    #[test]
    fn merged_reset_between_analyses() {
        mark_merged(7);
        assert!(is_merged(7));
        reset_thread_state();
        assert!(!is_merged(7));
    }

    #[test]
    fn reqs_survive_many_returns() {
        let src1 = Source::new("param f.x", SourceKind::Param);
        let src2 = Source::new("param g.a", SourceKind::Param);
        let mut state = State::new();
        state.require_simple(src1, key_str_lit("k1"));
        let mut st = state.copy();
        st.require_simple(src2, key_str_lit("chain"));
        branch_return(&mut st, None);
        branch_return(&mut st, None);
        branch_return(&mut st, None);
        branch_return(&mut st, Some((src2, "objs", "L21")));
        branch_return(&mut st, None);
        branch_return(&mut st, None);
        st.require_simple(src2, key_str_lit("ns"));
        let ex = st.exits.take().unwrap();
        let mut exs = ex.copy();
        let fin = st.join(&mut exs);
        let d = fin.reqs.inner(&src2).expect("source lost");
        let mut ks: Vec<String> = d.keys().map(|k| crate::intern::key_repr(*k)).collect();
        ks.sort();
        assert_eq!(ks, vec!["'chain'", "'ns'", "'objs'"]);
    }
}

/// Lists whose last element is known on both paths (the join of the two values).
fn join_lasts(a: &Arc<Vec<(ObjId, Value)>>, b: &Arc<Vec<(ObjId, Value)>>) -> Arc<Vec<(ObjId, Value)>> {
    if Arc::ptr_eq(a, b) || (a.is_empty() && b.is_empty()) {
        return a.clone();
    }
    Arc::new(a.iter().filter_map(|(o, v)| b.iter().find(|(p, _)| p == o).map(|(_, w)| (*o, v.join(w)))).collect())
}

/// Lengths at a join: the length when equal, the shorter known prefix.
/// (requirements on a nested source a path lacks entirely: that path never read the parent's
/// value, so the requirement still holds wherever it was read)
fn nested_keep(s: &crate::domain::SourceId) -> bool {
    crate::domain::source_is_nested(*s)
}

thread_local! {
    static MERGED: std::cell::RefCell<rustc_hash::FxHashSet<ObjId>> = std::cell::RefCell::new(rustc_hash::FxHashSet::default());
}

/// An abstract object that stands for several runtime objects made at different times (a
/// clone collapsed past the clone budget, a site re-allocated while live): no store into it
/// replaces what is there.
pub fn mark_merged(oid: ObjId) {
    MERGED.with(|m| {
        m.borrow_mut().insert(oid);
    });
}

thread_local! {
    /// the shared unknown object (`interp::any_object`): no store changes it
    static ANY: std::cell::Cell<ObjId> = const { std::cell::Cell::new(ObjId::MAX) };
}

pub fn set_any(oid: ObjId) {
    ANY.with(|a| a.set(oid));
}

pub fn is_any(oid: ObjId) -> bool {
    ANY.with(|a| a.get() == oid)
}

thread_local! {
    /// a loop's item (a view) -> the object standing for the records it is one of
    static VIEWS: std::cell::RefCell<rustc_hash::FxHashMap<ObjId, ObjId>> = std::cell::RefCell::new(rustc_hash::FxHashMap::default());
    /// such an object -> its views
    static VIEWED: std::cell::RefCell<rustc_hash::FxHashMap<ObjId, Vec<ObjId>>> = std::cell::RefCell::new(rustc_hash::FxHashMap::default());
    /// changes a view got from another object of its family (`view_hits`)
    static VIEW_HITS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// `i` is one of the runtime objects `s` stands for (a loop's current item): a change of
/// either, or of another view of `s`, may be a change of the others (they join it).
pub fn link_view(i: ObjId, s: ObjId) {
    VIEWS.with(|v| v.borrow_mut().insert(i, s));
    VIEWED.with(|v| {
        let mut v = v.borrow_mut();
        let e = v.entry(s).or_default();
        if !e.contains(&i) {
            e.push(i);
        }
    });
}

/// The object a view is one of.
pub fn view_of(i: ObjId) -> Option<ObjId> {
    if !has_views() {
        return None;
    }
    VIEWS.with(|v| v.borrow().get(&i).copied())
}

fn has_views() -> bool {
    VIEWS.with(|v| !v.borrow().is_empty())
}

/// The other objects of `oid`'s family: its summary and the summary's other views, or its
/// views.
fn family(oid: ObjId) -> Vec<ObjId> {
    if !has_views() {
        return Vec::new();
    }
    let root = view_of(oid).unwrap_or(oid);
    let views = VIEWED.with(|v| v.borrow().get(&root).cloned()).unwrap_or_default();
    std::iter::once(root).chain(views).filter(|m| *m != oid).collect()
}

/// (a view `m` got a change from another object of its family)
fn note_view_hit(m: ObjId) {
    if view_of(m).is_some() {
        VIEW_HITS.with(|c| c.set(c.get() + 1));
    }
}

/// How many changes views got from another object of their family so far.
pub fn view_hits() -> u64 {
    VIEW_HITS.with(|c| c.get())
}

pub fn is_merged(oid: ObjId) -> bool {
    MERGED.with(|m| m.borrow().contains(&oid))
}

/// A new analysis on this thread (`rowan serve` runs every request on one): object ids start
/// again, so the merged set and the cost counters do too.
pub fn reset_thread_state() {
    MERGED.with(|m| m.borrow_mut().clear());
    VIEWS.with(|v| v.borrow_mut().clear());
    VIEWED.with(|v| v.borrow_mut().clear());
    ANY.with(|a| a.set(ObjId::MAX));
    FP_VISITS.with(|c| c.set(0));
    crate::domain::reset_row_joins();
    crate::models::shape::reset();
}

/// Lengths join to the one both sides have, else to unknown with the shorter known prefix and
/// the possible lengths (a bit per length 0..15, 0: no information) when both sides know theirs.
pub fn join_len(a: &(Option<usize>, usize, u16), b: &(Option<usize>, usize, u16)) -> (Option<usize>, usize, u16) {
    let pa = a.0.unwrap_or(a.1);
    let pb = b.0.unwrap_or(b.1);
    if a.0.is_some() && a.0 == b.0 {
        return (a.0, pa.min(pb), 0);
    }
    let mask = |x: &(Option<usize>, usize, u16)| -> u16 {
        match x.0 {
            Some(n) if n < 16 => 1u16 << n,
            Some(_) => 0,
            None => x.2,
        }
    };
    let (ma, mb) = (mask(a), mask(b));
    let m = if ma != 0 && mb != 0 { ma | mb } else { 0 };
    (None, pa.min(pb), m)
}

thread_local! {
    /// footprint entries visited by `join_heap_fp` (a deterministic cost measure: golden
    /// `#@visits <= N`, with the liveness walks' visits)
    static FP_VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Footprint entries visited by `join_heap_fp` so far.
pub fn fp_visits() -> usize {
    FP_VISITS.with(|c| c.get())
}
