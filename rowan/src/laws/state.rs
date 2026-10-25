//! Laws of the whole state's join (`State::join`, `State::join_from`), property-tested on states
//! a program of small edits makes (names, rows, slots, elements, lengths of five objects).
//!
//! The reference model is the per-key join: what `State::join` assembles from the joins of its
//! parts (`Row::join`, `Value::join`, `kv_join`, `join_len`, `join_env`) must be the map whose
//! keys are either side's and whose values are both sides' joined (or the one side's).
//! Checked:
//! * the join is that map, for every part;
//! * commutative (`same`) and idempotent; an unreachable side adds nothing but its returns;
//! * the incremental join (`join_from`, what loops and calls' heaps use): into a state that is at
//!   least `prev`, of a state made from `prev` by edits, it gives what the whole join gives on
//!   every key the edits wrote, and keeps the other keys as they are.

use super::*;
use crate::domain::{kv_join, KeyVal, Row, Value};
use crate::intern::{key_int, key_str_lit, s, KeyId, KeySet};
use crate::laws::config;
use proptest::prelude::*;
use std::collections::BTreeMap;

/// Objects the programs use (ids no table of the analyzer marks).
const OBJS: [ObjId; 5] = [1001, 1002, 1003, 1004, 1005];

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
    ]
}

fn ns_of(a: bool) -> Ns {
    if a {
        Ns::Attr
    } else {
        Ns::Item
    }
}

fn apply(s: &mut State, e: &Edit) {
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
    }
}

fn s_name(n: u8) -> crate::intern::StrId {
    s(["x", "y", "z", "w"][n as usize % 4])
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

/// The reference join of two maps: either side's keys, both sides' values joined.
fn join_map<K: Ord + Clone, V: Clone>(a: &BTreeMap<K, V>, b: &BTreeMap<K, V>, j: impl Fn(&V, &V) -> V) -> BTreeMap<K, V> {
    let mut out = a.clone();
    for (k, v) in b {
        let nv = match a.get(k) {
            Some(x) => j(x, v),
            None => v.clone(),
        };
        out.insert(k.clone(), nv);
    }
    out
}

fn reference(a: &State, b: &State) -> Parts {
    let (ra, sa, ea, la) = parts(a);
    let (rb, sb, eb, lb) = parts(b);
    (join_map(&ra, &rb, |x, y| x.join(y)), join_map(&sa, &sb, |x, y| x.join(y)), join_map(&ea, &eb, kv_join), join_map(&la, &lb, join_len))
}

proptest! {
    #![proptest_config(config())]

    /// `State::join` is the per-key join of its parts; commutative, idempotent, and an
    /// unreachable side adds nothing.
    #[test]
    fn join_is_the_join_of_its_parts(base in prop::collection::vec(edit(), 0..12), x in prop::collection::vec(edit(), 0..8), y in prop::collection::vec(edit(), 0..8)) {
        let p = made(&State::new(), &base);
        let (a, b) = (made(&p, &x), made(&p, &y));
        let j = a.copy().join(&mut b.copy());
        prop_assert!(parts(&j) == reference(&a, &b), "join of the parts");
        prop_assert!(j.env == join_env(&a.env, &b.env), "names");
        prop_assert!(j.same(&b.copy().join(&mut a.copy())), "commutative");
        prop_assert!(a.copy().join(&mut a.copy()).same(&a), "idempotent");
        prop_assert!(a.copy().join(&mut State::bottom()).same(&a), "unreachable side");
    }

    /// `join_from(prev, o)` into a state at least `prev` gives the whole join on every key `o`
    /// changed since `prev`, and leaves the others as they are.
    #[test]
    fn incremental_join_is_the_join_where_written(base in prop::collection::vec(edit(), 0..12), x in prop::collection::vec(edit(), 0..8), y in prop::collection::vec(edit(), 0..8)) {
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
