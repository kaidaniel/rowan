//! Abstract objects and the flow-sensitive analysis state (port of `rowan/state.py`).

use std::sync::Arc;

use indexmap::IndexMap;
use rustc_hash::FxHashMap;

use crate::domain::{kv_join, rho, KeyVal, Ns, ObjId, ReqInfo, Row, Value};
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
    Ph,
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
            Kind::Ph => "ph",
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
        matches!(self, Kind::Dict | Kind::Frame | Kind::Instance | Kind::Unknown | Kind::Module | Kind::Class | Kind::Namespace | Kind::Ph)
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

thread_local! {
    /// `State::same` also compares `narrowed` (Options::narrowed_same); set by the analyser.
    static NARROWED_SAME: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

pub fn set_narrowed_same(on: bool) {
    NARROWED_SAME.with(|c| c.set(on));
}

/// Mutable analysis state at one program point.
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
    /// Source -> {key term -> ReqInfo}
    pub reqs: TwoLevel<SourceId, KeyId, ReqInfo>,
    pub ret: Option<Value>,
    pub reachable: bool,
    pub exits: Option<Arc<State>>,
    /// rows of untouched objects are entry rows (summary analysis)
    pub symbolic: bool,
    /// what a test that was joined over said about rows (see `CondFact`)
    pub facts: Arc<Vec<Arc<CondFact>>>,
    /// (name, key): the object the name is bound to has the key, whichever of its possible
    /// objects that is (a store or setdefault through the name)
    pub name_keys: Arc<Vec<(StrId, KeyId)>>,
    /// lists whose last element is known on this path (an `append` since any other change of
    /// their elements): `lst[-1]` after appends of unknown count
    pub lasts: Arc<Vec<(ObjId, Value)>>,
    /// tensor-shape facts on this path: ranks and dim equalities (shape.rs)
    pub tf: Arc<crate::shape::TFacts>,
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
}

impl Default for State {
    fn default() -> Self {
        State::new()
    }
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
            reqs: TwoLevel::new(),
            ret: None,
            reachable: true,
            exits: None,
            symbolic: false,
            facts: Arc::new(Vec::new()),
            name_keys: Arc::new(Vec::new()),
            lasts: Arc::new(Vec::new()),
            tf: Arc::new(crate::shape::TFacts::default()),
        }
    }

    pub fn bottom() -> State {
        let mut s = State::new();
        s.reachable = false;
        s
    }

    // -- copying -----------------------------------------------------------
    /// Copy from a shared reference (used for `exits`, which are shared and never mutated).
    pub fn copy_ref(&self) -> State {
        State {
            env: self.env.clone(),
            narrowed: self.narrowed.clone(),
            rows: self.rows.copy_ref(),
            slots: self.slots.copy_ref(),
            elems: self.elems.copy_ref(),
            reqs: self.reqs.copy_ref(),
            ret: self.ret.clone(),
            reachable: self.reachable,
            exits: self.exits.clone(),
            symbolic: self.symbolic,
            facts: self.facts.clone(),
            name_keys: self.name_keys.clone(),
            lasts: self.lasts.clone(),
            tf: self.tf.clone(),
        }
    }

    pub fn copy(&mut self) -> State {
        State {
            env: self.env.clone(),
            narrowed: self.narrowed.clone(),
            rows: self.rows.copy(),
            slots: self.slots.copy(),
            elems: self.elems.copy(),
            reqs: self.reqs.copy(),
            ret: self.ret.clone(),
            reachable: self.reachable,
            exits: self.exits.clone(),
            symbolic: self.symbolic,
            facts: self.facts.clone(),
            name_keys: self.name_keys.clone(),
            lasts: self.lasts.clone(),
            tf: self.tf.clone(),
        }
    }

    /// Snapshot the heap at a `return` so the caller sees it joined over all return points.
    pub fn record_exit(&mut self) {
        let mut snap = self.copy();
        snap.reachable = true;
        snap.ret = None;
        snap.exits = None;
        self.exits = Some(Arc::new(match self.exits.take() {
            None => snap,
            Some(e) => e.copy_ref().join(&mut snap),
        }));
    }

    /// Make self equal to o (o stays usable: both get their own private top layers).
    pub fn set_to(&mut self, o: &mut State) {
        self.env = o.env.clone();
        self.narrowed = o.narrowed.clone();
        self.rows = o.rows.copy();
        self.slots = o.slots.copy();
        self.elems = o.elems.copy();
        self.reqs = o.reqs.copy();
        self.ret = o.ret.clone();
        self.reachable = o.reachable;
        self.exits = o.exits.clone();
        self.symbolic = o.symbolic;
        self.facts = o.facts.clone();
        self.name_keys = o.name_keys.clone();
        self.lasts = o.lasts.clone();
        self.tf = o.tf.clone();
    }

    pub fn clear_last(&mut self, oid: ObjId) {
        if self.lasts.iter().any(|(o, _)| *o == oid) {
            self.lasts = Arc::new(self.lasts.iter().filter(|(o, _)| *o != oid).cloned().collect());
        }
    }

    pub fn set_last(&mut self, oid: ObjId, v: Value) {
        let mut l: Vec<(ObjId, Value)> = self.lasts.iter().filter(|(o, _)| *o != oid).cloned().collect();
        l.push((oid, v));
        self.lasts = Arc::new(l);
    }

    pub fn last_of(&self, oid: ObjId) -> Option<Value> {
        self.lasts.iter().find(|(o, _)| *o == oid).map(|(_, v)| v.clone())
    }

    pub fn replace_with(&mut self, mut o: State) {
        self.set_to(&mut o);
    }

    // -- lattice -----------------------------------------------------------
    pub fn join(&mut self, o: &mut State) -> State {
        let ret = match (&self.ret, &o.ret) {
            (None, r) => r.clone(),
            (r, None) => r.clone(),
            (Some(a), Some(b)) => Some(a.join(b)),
        };
        let exits = match (&self.exits, &o.exits) {
            (None, e) => e.clone(),
            (e, None) => e.clone(),
            (Some(a), Some(b)) if Arc::ptr_eq(a, b) => Some(a.clone()),
            (Some(a), Some(b)) => Some(Arc::new(a.copy_ref().join(&mut b.copy_ref()))),
        };
        if !o.reachable {
            let mut s = self.copy();
            s.ret = ret;
            s.exits = exits;
            return s;
        }
        if !self.reachable {
            let mut s = o.copy();
            s.ret = ret;
            s.exits = exits;
            return s;
        }
        // facts both sides still hold
        let facts: Vec<Arc<CondFact>> = self.facts.iter().filter(|f| o.facts.iter().any(|g| Arc::ptr_eq(f, g))).cloned().collect();
        let mut s = State::new();
        s.env = join_env(&self.env, &o.env);
        if !self.narrowed.is_empty() && !o.narrowed.is_empty() {
            s.narrowed = self.narrowed.iter().filter(|(k, v)| o.narrowed.get(*k) == Some(*v)).map(|(k, v)| (*k, v.clone())).collect();
        }
        s.rows = self.rows.join(&o.rows, &mut |a, b| a.join(b));
        s.slots = self.slots.join(&mut o.slots, &mut |a, b| a.join(b));
        s.elems = self.elems.join(&o.elems, &mut |a, b| kv_join(a, b));
        s.reqs = self.reqs.join_with(&mut o.reqs, &mut |a, b| a.join(b), &mut |a| a.demote());
        s.ret = ret;
        s.exits = exits;
        s.reachable = true;
        s.symbolic = self.symbolic || o.symbolic;
        s.facts = Arc::new(facts);
        s.tf = if Arc::ptr_eq(&self.tf, &o.tf) || self.tf == o.tf { self.tf.clone() } else { Arc::new(self.tf.join(&o.tf)) };
        s.name_keys = if Arc::ptr_eq(&self.name_keys, &o.name_keys) { self.name_keys.clone() } else { Arc::new(self.name_keys.iter().filter(|x| o.name_keys.contains(x)).copied().collect()) };
        s.lasts = join_lasts(&self.lasts, &o.lasts);
        s
    }

    /// The join at a loop head: like `join`, but a requirement recorded only in the loop body
    /// stays "always" (per element of the iteration; spec: in-loop demands are not conditional).
    pub fn join_loop(&mut self, o: &mut State) -> State {
        let demoted = self.reqs.copy();
        let mut s = self.join(o);
        let mut a = demoted;
        let mut b = o.reqs.copy();
        if self.reachable && o.reachable {
            s.reqs = a.join_with(&mut b, &mut |x, y| x.join(y), &mut |x| x.clone());
        }
        s
    }

    /// Join all heap facts (rows/slots/elems/reqs) of `o` into self, in place.
    pub fn join_heap_all(&mut self, o: &mut State) {
        if !o.reachable {
            return;
        }
        self.rows = self.rows.join(&o.rows, &mut |a, b| a.join(b));
        self.slots = self.slots.join(&mut o.slots, &mut |a, b| a.join(b));
        self.elems = self.elems.join(&o.elems, &mut |a, b| kv_join(a, b));
        self.reqs = self.reqs.join_with(&mut o.reqs, &mut |a, b| a.join(b), &mut |a| a.demote());
        self.lasts = join_lasts(&self.lasts, &o.lasts);
    }

    /// Join the heap facts of `o` restricted to a footprint (the objects a callee touched).
    pub fn join_heap_fp(&mut self, o: &State, fp: &Footprint) {
        if !o.reachable {
            return;
        }
        for k in &fp.rows {
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
        for (oid, sk) in &fp.slots {
            if matches!(sk.0, Ns::Pos | Ns::Elem) {
                self.clear_last(*oid);
            }
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
        for k in &fp.elems {
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
        for (src, d) in o.reqs.items() {
            if let Some(cur) = self.reqs.inner(&src) {
                if Arc::ptr_eq(&cur, &d) {
                    continue;
                }
            }
            let tgt = self.reqs.writable(&src);
            for (k, info) in d.iter() {
                let nv = match tgt.get(k) {
                    Some(cur) => cur.join(info),
                    None => info.demote(),
                };
                tgt.insert(*k, nv);
            }
        }
    }

    /// Keys whose facts differ from `base` (what a callee changed or created).
    pub fn footprint(&self, base: &State) -> Footprint {
        Footprint { rows: self.rows.diff_keys(&base.rows), slots: self.slots.diff_keys(&base.slots), elems: self.elems.diff_keys(&base.elems) }
    }

    pub fn same(&self, o: &State) -> bool {
        self.reachable == o.reachable
            && self.env == o.env
            && (!NARROWED_SAME.with(|c| c.get()) || self.narrowed == o.narrowed)
            && self.ret == o.ret
            && (Arc::ptr_eq(&self.tf, &o.tf) || self.tf == o.tf)
            && self.exits.is_none() == o.exits.is_none()
            && self.rows.same(&o.rows)
            && self.slots.same(&o.slots)
            && self.elems.same(&o.elems)
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
        if self.symbolic {
            return Row::entry(rho(oid, ns));
        }
        Row::new()
    }

    pub fn has_row(&self, oid: ObjId, ns: Ns) -> bool {
        self.rows.contains_key(&(oid, ns))
    }

    pub fn set_row(&mut self, oid: ObjId, ns: Ns, row: Row) {
        self.rows.insert((oid, ns), row);
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
        if matches!(ns, Ns::Pos | Ns::Elem) {
            self.clear_last(oid);
        }
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

    pub fn drop_obj(&mut self, oid: ObjId) {
        self.slots.drop_key(&oid);
    }

    pub fn drop_obj_ns(&mut self, oid: ObjId, ns: Ns) {
        if matches!(ns, Ns::Pos | Ns::Elem) {
            self.clear_last(oid);
        }
        self.slots.drop_where(&oid, |(n, _)| *n == ns);
    }

    pub fn require(&mut self, src: SourceId, term: KeyId, site: StrId, alternatives: IdSet<SourceId>, caveats: IdSet<StrId>, alt_keys: IdSet<KeyId>) {
        let info = ReqInfo {
            always: true,
            sites: IdSet::one(site),
            alternatives,
            caveats,
            alt_keys: if alt_keys.len() > 1 { alt_keys } else { IdSet::new() },
        };
        let d = self.reqs.writable(&src);
        let nv = match d.get(&term) {
            Some(cur) => cur.seq(&info),
            None => info,
        };
        d.insert(term, nv);
    }

    pub fn require_simple(&mut self, src: SourceId, term: KeyId, site: StrId) {
        self.require(src, term, site, IdSet::new(), IdSet::new(), IdSet::new());
    }
}

#[derive(Clone, Debug, Default)]
pub struct Footprint {
    pub rows: Vec<(ObjId, Ns)>,
    pub slots: Vec<(ObjId, SlotKey)>,
    pub elems: Vec<ObjId>,
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
        state.require_simple(src1, key_str_lit("k1"), s("L1"));
        let mut st = state.copy();
        let mut then_s = st.copy();
        let mut else_s = st.copy();
        then_s.require_simple(src2, key_str_lit("objs"), s("L21"));
        then_s.record_exit();
        then_s.reachable = false;
        let mut j = then_s.join(&mut else_s);
        assert!(j.reachable);
        let ex = j.exits.take().unwrap();
        let mut exs = ex.copy_ref();
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
            then_s.require_simple(src, key_str_lit(k), s(site));
        }
        then_s.record_exit();
        then_s.reachable = false;
        let j = then_s.join(&mut else_s);
        st.replace_with(j);
    }

    #[test]
    fn reqs_survive_many_returns() {
        let src1 = Source::new("param f.x", SourceKind::Param);
        let src2 = Source::new("param g.a", SourceKind::Param);
        let mut state = State::new();
        state.require_simple(src1, key_str_lit("k1"), s("L1"));
        let mut st = state.copy();
        st.require_simple(src2, key_str_lit("chain"), s("L20"));
        branch_return(&mut st, None);
        branch_return(&mut st, None);
        branch_return(&mut st, None);
        branch_return(&mut st, Some((src2, "objs", "L21")));
        branch_return(&mut st, None);
        branch_return(&mut st, None);
        st.require_simple(src2, key_str_lit("ns"), s("L30"));
        let ex = st.exits.take().unwrap();
        let mut exs = ex.copy_ref();
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
