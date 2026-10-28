//! Laws of the whole state's join (`State::join`, `State::join_from`), property-tested on states
//! a program of small edits makes (names, rows, slots, elements, lengths of five objects, two of
//! them *found*: made where a path first reads them, `mark_found`; requirements on three inputs,
//! one nested in another; the facts of tests, `CondFact`).
//!
//! The reference model is the per-key join: what `State::join` assembles from the joins of its
//! parts (`Row::join`, `Value::join`, `kv_join`, `join_len`, `join_env`) must be the map whose
//! keys are either side's and whose values are both sides' joined, or the one side's: a found
//! object's joined with what it starts with (a path that has not read it holds it as it starts),
//! its elements and length not known where one side has no row of it. Requirements: both sides'
//! joined, one side's demoted to *sometimes*, a nested input's kept where the other side has no
//! requirement on that input at all. Facts: only those both sides hold, each with the rows and
//! guards the two sides agree on.
//! Checked:
//! * the join is that map, for every part;
//! * commutative (`same`) and idempotent; an unreachable side adds nothing but its returns;
//! * the incremental join (`join_from`, what loops and calls' heaps use): into a state that is at
//!   least `prev`, of a state made from `prev` by edits, it gives what the whole join gives on
//!   every key the edits wrote, and keeps the other keys as they are.

use super::*;
use crate::domain::{kv_join, KeyVal, ReqInfo, Row, Value};
use crate::intern::{key_int, key_str_lit, s, IdSet, KeyId, KeySet};
use crate::laws::config;
use crate::source::{source_extend, Source, SourceId, SourceKind};
use proptest::prelude::*;
use std::collections::BTreeMap;

/// Objects the programs use (ids no table of the analyzer marks); the last two are found ones.
const OBJS: [ObjId; 5] = [1001, 1002, 1003, 1004, 1005];
const FOUND_OBJS: [ObjId; 2] = [1004, 1005];

fn key_of(i: u8) -> KeyId {
    match i % 4 {
        0 => key_str_lit("a"),
        1 => key_str_lit("b"),
        2 => key_int(1),
        _ => key_str_lit("c"),
    }
}

/// A value of literals (or not known).
fn value_of(i: u8) -> Value {
    match i % 6 {
        0 => Value::lit(key_int(1)),
        1 => Value::lit(key_int(2)),
        2 => Value::lit(key_str_lit("x")),
        3 => Value::kv_only(KeyVal::lits(KeySet::from_iter([key_int(1), key_int(3)]))),
        4 => Value::none(),
        _ => Value::data("unknown"),
    }
}

/// What a found object starts with: an input's part, open, with key `a`; a placeholder under `b`.
fn found_init() -> (Row, Value) {
    (Row::with_source(Source::new("law:input", SourceKind::Call)).store(key_of(0), true), Value::data("placeholder"))
}

/// The inputs requirements are recorded on (the last nested in the first).
fn sources() -> [SourceId; 3] {
    let a = Source::new("law:a", SourceKind::Call);
    [a, Source::new("law:b", SourceKind::Call), source_extend(a, key_of(0))]
}

/// The thread's table of found objects: only the law's.
fn found_start() {
    reset_thread_state();
    let (row, ph) = found_init();
    for o in FOUND_OBJS {
        mark_found(o, Ns::Item, &row);
        mark_found_slot(o, (Ns::Item, key_of(1)), &ph);
    }
}

/// One edit of a state.
#[derive(Clone, Debug)]
enum Edit {
    Name(u8, u8),
    Store(u8, bool, u8, bool),
    Delete(u8, bool, u8),
    Slot(u8, bool, u8, u8),
    Elems(u8, u8),
    Len(u8, u8),
    Forget(u8),
    /// a path reads a found object: it has what the object starts with
    Read(u8),
    /// a read requires key `.1` of input `.0`, under the tests `.2` (bits) the path passed
    Req(u8, u8, u8),
    /// a test's fact on object `.0`'s row, guarded by object `.1`'s
    Fact(u8, u8),
}

fn edit() -> impl Strategy<Value = Edit> {
    prop_oneof![
        (0u8..4, any::<u8>()).prop_map(|(n, v)| Edit::Name(n, v)),
        (0u8..5, any::<bool>(), any::<u8>(), any::<bool>()).prop_map(|(o, a, k, sure)| Edit::Store(o, a, k, sure)),
        (0u8..5, any::<bool>(), any::<u8>()).prop_map(|(o, a, k)| Edit::Delete(o, a, k)),
        (0u8..5, any::<bool>(), any::<u8>(), any::<u8>()).prop_map(|(o, a, k, v)| Edit::Slot(o, a, k, v)),
        (0u8..5, any::<u8>()).prop_map(|(o, v)| Edit::Elems(o, v)),
        (0u8..5, 0u8..20).prop_map(|(o, n)| Edit::Len(o, n)),
        (0u8..5).prop_map(Edit::Forget),
        (0u8..2).prop_map(Edit::Read),
        (0u8..3, any::<u8>(), 0u8..4).prop_map(|(src, k, t)| Edit::Req(src, k, t)),
        (0u8..5, 0u8..5).prop_map(|(o, g)| Edit::Fact(o, g)),
    ]
}

fn ns_of(a: bool) -> Ns {
    if a {
        Ns::Attr
    } else {
        Ns::Item
    }
}

/// A path's first use of a found object reads it: it holds it as it starts.
fn read_found(s: &mut State, o: ObjId) {
    if FOUND_OBJS.contains(&o) && s.rows.get(&(o, Ns::Item)).is_none() {
        let (row, ph) = found_init();
        s.rows.insert((o, Ns::Item), row);
        s.slots.set(&o, (Ns::Item, key_of(1)), ph);
    }
}

fn apply(s: &mut State, e: &Edit) {
    if let Edit::Store(o, ..) | Edit::Delete(o, ..) | Edit::Slot(o, ..) | Edit::Elems(o, _) | Edit::Len(o, _) | Edit::Forget(o) = *e {
        read_found(s, OBJS[o as usize]);
    }
    match *e {
        Edit::Name(n, v) => {
            s.env.insert(s_name(n), value_of(v));
        }
        Edit::Store(o, a, k, sure) => {
            let key = (OBJS[o as usize], ns_of(a));
            let r = s.rows.get(&key).unwrap_or_else(Row::new).store(key_of(k), sure);
            s.rows.insert(key, r);
        }
        Edit::Delete(o, a, k) => {
            let key = (OBJS[o as usize], ns_of(a));
            let r = s.rows.get(&key).unwrap_or_else(Row::new).delete(key_of(k), true);
            s.rows.insert(key, r);
        }
        Edit::Slot(o, a, k, v) => s.slots.set(&OBJS[o as usize], (ns_of(a), key_of(k)), value_of(v)),
        Edit::Elems(o, v) => s.elems.insert(OBJS[o as usize], value_of(v).kv),
        Edit::Len(o, n) => s.lens.insert(OBJS[o as usize], if n < 16 { (Some(n as usize), n as usize, 0) } else { (None, (n - 16) as usize, 0) }),
        Edit::Forget(o) => {
            s.lens.remove(&OBJS[o as usize]);
        }
        Edit::Read(i) => read_found(s, FOUND_OBJS[i as usize]),
        Edit::Req(src, k, t) => {
            let tests = IdSet::from_iter([(t & 1 != 0).then(|| s_test("x > 0")), (t & 2 != 0).then(|| s_test("y"))].into_iter().flatten());
            let info = ReqInfo { always: true, only_if: tests, ..ReqInfo::default() };
            let src = sources()[src as usize];
            let nv = match s.reqs.get(&src, &key_of(k)) {
                Some(cur) => cur.seq(&info),
                None => info,
            };
            s.reqs.set(&src, key_of(k), nv);
        }
        Edit::Fact(o, g) => {
            let (k, gk) = ((OBJS[o as usize], Ns::Item), (OBJS[g as usize], Ns::Item));
            let joined = s.rows.get(&k).unwrap_or_else(Row::new);
            let guard = s.rows.get(&gk).unwrap_or_else(Row::new);
            let fact = CondFact { cond: s_test(&format!("t{o}{g}")), truth: true, deps: Vec::new(), rows: vec![(k, joined.store(key_of(3), true), joined)], guards: vec![(gk, guard)], unbound: None };
            let mut fs: Vec<Arc<CondFact>> = s.facts.iter().cloned().collect();
            fs.push(Arc::new(fact));
            s.facts = Arc::new(fs);
        }
    }
}

fn s_name(n: u8) -> crate::intern::StrId {
    s(["x", "y", "z", "w"][n as usize % 4])
}

fn s_test(t: &str) -> crate::intern::StrId {
    s(t)
}

fn made(base: &State, edits: &[Edit]) -> State {
    let mut st = base.copy();
    for e in edits {
        apply(&mut st, e);
    }
    st
}

/// The parts of a state as ordered maps.
type Parts = (BTreeMap<(ObjId, u8), Row>, BTreeMap<(ObjId, u8, KeyId), Value>, BTreeMap<ObjId, KeyVal>, BTreeMap<ObjId, LenFact>);

fn ns_code(ns: Ns) -> u8 {
    if ns == Ns::Attr {
        1
    } else {
        0
    }
}

fn parts(st: &State) -> Parts {
    let mut rows = BTreeMap::new();
    let mut slots = BTreeMap::new();
    let mut elems = BTreeMap::new();
    let mut lens = BTreeMap::new();
    for o in OBJS {
        for ns in [Ns::Item, Ns::Attr] {
            if let Some(r) = st.rows.get(&(o, ns)) {
                rows.insert((o, ns_code(ns)), r);
            }
            for k in 0..4u8 {
                if let Some(v) = st.slots.get(&o, &(ns, key_of(k))) {
                    slots.insert((o, ns_code(ns), key_of(k)), v);
                }
            }
        }
        if let Some(e) = st.elems.get(&o) {
            elems.insert(o, e);
        }
        if let Some(l) = st.lens.get(&o) {
            lens.insert(o, l);
        }
    }
    (rows, slots, elems, lens)
}

/// The reference join of two maps: either side's keys, both sides' values joined (`one`: what
/// a key on one side only gives).
fn join_map<K: Ord + Clone, V: Clone>(a: &BTreeMap<K, V>, b: &BTreeMap<K, V>, j: impl Fn(&V, &V) -> V, one: impl Fn(&K, &V) -> V) -> BTreeMap<K, V> {
    let mut out: BTreeMap<K, V> = a.iter().map(|(k, v)| (k.clone(), if b.contains_key(k) { v.clone() } else { one(k, v) })).collect();
    for (k, v) in b {
        let nv = match a.get(k) {
            Some(x) => j(x, v),
            None => one(k, v),
        };
        out.insert(k.clone(), nv);
    }
    out
}

fn found(o: ObjId) -> bool {
    FOUND_OBJS.contains(&o)
}

fn reference(a: &State, b: &State) -> Parts {
    let (ra, sa, ea, la) = parts(a);
    let (rb, sb, eb, lb) = parts(b);
    let (init, ph) = found_init();
    let rows = join_map(&ra, &rb, |x, y| x.join(y), |k, r| if found(k.0) && k.1 == 0 { r.join(&init) } else { r.clone() });
    let slots = join_map(&sa, &sb, |x, y| x.join(y), |k, v| if found(k.0) && k.1 == 0 && k.2 == key_of(1) { v.join(&ph) } else { v.clone() });
    // (a found object's on one side only: not known on the other, which holds it as it starts)
    let mut elems = join_map(&ea, &eb, kv_join, |_, v| v.clone());
    let mut lens = join_map(&la, &lb, join_len, |_, v| *v);
    for o in FOUND_OBJS {
        if ea.contains_key(&o) != eb.contains_key(&o) {
            elems.remove(&o);
        }
        if la.contains_key(&o) != lb.contains_key(&o) {
            lens.remove(&o);
        }
    }
    (rows, slots, elems, lens)
}

/// Requirements as maps.
fn reqs(st: &State) -> BTreeMap<(SourceId, KeyId), ReqInfo> {
    let mut out = BTreeMap::new();
    for src in sources() {
        for k in 0..4u8 {
            if let Some(r) = st.reqs.get(&src, &key_of(k)) {
                out.insert((src, key_of(k)), r);
            }
        }
    }
    out
}

/// The reference join of requirements.
fn reqs_reference(a: &State, b: &State) -> BTreeMap<(SourceId, KeyId), ReqInfo> {
    let (ra, rb) = (reqs(a), reqs(b));
    let on = |m: &BTreeMap<(SourceId, KeyId), ReqInfo>, src: SourceId| m.keys().any(|(s, _)| *s == src);
    let one = |other: &BTreeMap<(SourceId, KeyId), ReqInfo>, k: &(SourceId, KeyId), v: &ReqInfo| if crate::source::source_is_nested(k.0) && !on(other, k.0) { v.clone() } else { v.demote() };
    let mut out: BTreeMap<(SourceId, KeyId), ReqInfo> = ra.iter().map(|(k, v)| (*k, if rb.contains_key(k) { v.clone() } else { one(&rb, k, v) })).collect();
    for (k, v) in &rb {
        out.insert(*k, match ra.get(k) {
            Some(x) => x.join(v),
            None => one(&ra, k, v),
        });
    }
    out
}

/// Each fact of the join is one both sides hold, with rows and guards the two sides agree on;
/// a fact both hold whose rows and guards neither side changed (the same rows) is kept whole.
fn facts_hold(j: &State, a: &State, b: &State) -> Result<(), String> {
    let both: Vec<&Arc<CondFact>> = a.facts.iter().filter(|f| b.facts.iter().any(|g| Arc::ptr_eq(f, g))).collect();
    let agree = |k: &(ObjId, Ns)| a.rows.get(k) == b.rows.get(k);
    for f in j.facts.iter() {
        if !both.iter().any(|g| g.cond == f.cond && g.truth == f.truth && g.guards == f.guards && f.rows.iter().all(|r| g.rows.contains(r))) {
            return Err(format!("fact {:?} not held by both sides", f.cond));
        }
        if f.rows.iter().any(|r| !agree(&r.0)) || f.guards.iter().any(|(k, _)| !agree(k)) {
            return Err(format!("fact {:?} keeps a row the sides do not agree on", f.cond));
        }
    }
    let untouched = |k: &(ObjId, Ns)| match (a.rows.get(k), b.rows.get(k)) {
        (Some(x), Some(y)) => x.same_ptr(&y),
        (None, None) => true,
        _ => false,
    };
    for f in both {
        if f.rows.iter().all(|r| untouched(&r.0)) && f.guards.iter().all(|(k, _)| untouched(k)) && !j.facts.iter().any(|g| Arc::ptr_eq(f, g)) {
            return Err(format!("fact {:?} both sides hold untouched was dropped", f.cond));
        }
    }
    Ok(())
}

proptest! {
    #![proptest_config(config())]

    /// `State::join` is the per-key join of its parts; commutative, idempotent, and an
    /// unreachable side adds nothing.
    #[test]
    fn join_is_the_join_of_its_parts(base in prop::collection::vec(edit(), 0..12), x in prop::collection::vec(edit(), 0..8), y in prop::collection::vec(edit(), 0..8)) {
        found_start();
        let p = made(&State::new(), &base);
        let (a, b) = (made(&p, &x), made(&p, &y));
        let j = a.copy().join(&mut b.copy());
        prop_assert!(parts(&j) == reference(&a, &b), "join of the parts");
        prop_assert!(j.env == join_env(&a.env, &b.env), "names");
        prop_assert!(reqs(&j) == reqs_reference(&a, &b), "requirements");
        let fh = facts_hold(&j, &a, &b);
        prop_assert!(fh.is_ok(), "{}", fh.unwrap_err());
        prop_assert!(j.same(&b.copy().join(&mut a.copy())), "commutative");
        prop_assert!(a.copy().join(&mut a.copy()).same(&a), "idempotent");
        prop_assert!(a.copy().join(&mut State::bottom()).same(&a), "unreachable side");
    }

    /// `join_from(prev, o)` into a state at least `prev` gives the whole join on every key `o`
    /// changed since `prev`, and leaves the others as they are.
    #[test]
    fn incremental_join_is_the_join_where_written(base in prop::collection::vec(edit(), 0..12), x in prop::collection::vec(edit(), 0..8), y in prop::collection::vec(edit(), 0..8)) {
        found_start();
        let prev = made(&State::new(), &base);
        let acc = prev.copy().join(&mut made(&prev, &x));
        let o = made(&prev, &y);
        let mut inc = acc.copy();
        inc.join_from(&prev, &o, false);
        let (whole, mine, before, op) = (reference(&acc, &o), parts(&inc), parts(&acc), parts(&o));
        let pp = parts(&prev);
        // (keys `o` wrote: its value differs from prev's)
        for (k, v) in &op.0 { if pp.0.get(k) != Some(v) { prop_assert!(mine.0.get(k) == whole.0.get(k), "row {:?}", k); } }
        for (k, v) in &op.1 { if pp.1.get(k) != Some(v) { prop_assert!(mine.1.get(k) == whole.1.get(k), "slot {:?}", k); } }
        for (k, v) in &op.2 { if pp.2.get(k) != Some(v) { prop_assert!(mine.2.get(k) == whole.2.get(k), "elems {:?}", k); } }
        for (k, v) in &op.3 { if pp.3.get(k) != Some(v) { prop_assert!(mine.3.get(k) == whole.3.get(k), "len {:?}", k); } }
        // (the others: as the accumulated state had them)
        for (k, v) in &before.0 { if op.0.get(k).is_none_or(|w| pp.0.get(k) == Some(w)) { prop_assert!(mine.0.get(k) == Some(v), "row kept {:?}", k); } }
        for (k, v) in &before.1 { if op.1.get(k).is_none_or(|w| pp.1.get(k) == Some(w)) { prop_assert!(mine.1.get(k) == Some(v), "slot kept {:?}", k); } }
        for (k, v) in &before.2 { if op.2.get(k).is_none_or(|w| pp.2.get(k) == Some(w)) { prop_assert!(mine.2.get(k) == Some(v), "elems kept {:?}", k); } }
        for (k, v) in &before.3 { if op.3.get(k).is_none_or(|w| pp.3.get(k) == Some(w)) { prop_assert!(mine.3.get(k) == Some(v), "len kept {:?}", k); } }
    }
}
