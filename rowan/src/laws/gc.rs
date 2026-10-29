//! The collector's law (`collect_since`, as a loop's exit runs it, `loop_gc_end`): it never
//! drops the state of an object the program can still reach, property-tested on heaps a
//! program of small edits makes (a few objects made before the collector's mark, more after,
//! references between them through slots, elements, payloads, names, narrowed names, the
//! models' side tables (`side_edges`) and what the analyzer holds (`gc_pinned`: an exception
//! in flight, a caller's names)).
//!
//! The reference model is a plain reachability walk over the state and tables as written,
//! independent of `obj_edges`: from the names, narrowed names and result, every side table's
//! entry and what the analyzer holds; through slots, element keys, payloads and the side
//! tables keyed by a reached object. Checked:
//! * sound: every reached object keeps its facts (lengths, slots, elements) as they were;
//! * precise: an object made since the mark that the walk does not reach, nor one made before
//!   it (never collected after it), loses them.

use super::*;
use crate::domain::{KeyVal, KeysOf, ObjSet, Value};
use crate::intern::{key_int, s, KeySet};
use crate::laws::config;
use proptest::prelude::*;
use std::collections::BTreeSet;

/// Objects made before the mark, then after it.
const OLD: usize = 3;
const YOUNG: usize = 6;
const ALL: usize = OLD + YOUNG;

/// One edit after the mark.
#[derive(Clone, Debug)]
enum Edit {
    /// a slot of object `.0` (key `.1`) holds object `.2`
    Slot(u8, u8, u8),
    /// object `.0`'s elements are the keys of object `.1`
    Elems(u8, u8),
    /// a name holds an object
    Name(u8, u8),
    /// a narrowed name holds an object
    Narrowed(u8, u8),
    /// a model's side table keyed by object `.0` holds object `.1` (a defaultdict's factory)
    Side(u8, u8),
    /// a side table maps object `.0` to object `.1` (`sql_cursors`)
    Mirror(u8, u8),
    /// an exception in flight is the object
    Exc(u8),
    /// a caller's name holds the object
    Caller(u8),
    /// a name is rebound to a literal (what it held may be garbage now)
    Unbind(u8),
    /// object `.0` is made again (same site and context) with a payload holding object `.1`
    Repay(u8, u8),
}

fn edit() -> impl Strategy<Value = Edit> {
    let o = || 0u8..ALL as u8;
    prop_oneof![
        4 => (o(), 0u8..3, o()).prop_map(|(a, k, b)| Edit::Slot(a, k, b)),
        1 => (o(), o()).prop_map(|(a, b)| Edit::Elems(a, b)),
        3 => (0u8..3, o()).prop_map(|(n, b)| Edit::Name(n, b)),
        1 => (0u8..2, o()).prop_map(|(n, b)| Edit::Narrowed(n, b)),
        1 => (o(), o()).prop_map(|(a, b)| Edit::Side(a, b)),
        1 => (o(), o()).prop_map(|(a, b)| Edit::Mirror(a, b)),
        1 => o().prop_map(Edit::Exc),
        1 => o().prop_map(Edit::Caller),
        2 => (0u8..3).prop_map(Edit::Unbind),
        1 => (o(), o()).prop_map(|(a, b)| Edit::Repay(a, b)),
    ]
}

fn site(i: usize) -> String {
    format!("law:gc:{i}")
}

fn kind(i: usize) -> Kind {
    [Kind::Dict, Kind::List, Kind::Instance][i % 3]
}

/// The heap, the state at the mark, the state after the edits, the objects (old first), the mark.
fn run(young_payloads: &[Option<u8>], edits: &[Edit]) -> (Analyzer, State, State, Vec<ObjId>, u64) {
    let mut an = Analyzer::new(String::new(), "law_gc.py", Vec::new(), false, false).expect("an analyzer");
    let mut st = State::new();
    let mut objs: Vec<ObjId> = Vec::new();
    for i in 0..OLD {
        let o = an.heap.alloc_raw(kind(i), &site(i), "", true, "old", Payload::None);
        st.set_lens(o, (Some(0), 0, 0));
        objs.push(o);
    }
    // (an old object a name holds: the young ones reach older ones too)
    st.env.insert(s("old"), Value::obj(objs[0]));
    let pre = st.copy();
    let mark = an.heap.serial();
    for i in OLD..ALL {
        // (a young object's payload refers to an object made before it)
        let payload = match young_payloads[i - OLD] {
            Some(j) if (j as usize) < i => Payload::Pair(Value::obj(objs[j as usize]), Value::none()),
            _ => Payload::None,
        };
        let o = an.heap.alloc_raw(kind(i), &site(i), "", true, "young", payload);
        an.heap.renew(o);
        // (a fact the collector sees as written since the mark: a slot; a length alone is not)
        st.set_lens(o, (Some(0), 0, 0));
        st.set_slot(o, Ns::Attr, key_int(9), Value::lit(key_int(1)), true);
        objs.push(o);
    }
    for e in edits {
        let ob = |i: u8| objs[i as usize];
        match *e {
            Edit::Slot(a, k, b) => st.set_slot(ob(a), Ns::Attr, key_int(k as i64), Value::obj(ob(b)), true),
            Edit::Elems(a, b) => st.elems.insert(ob(a), KeyVal::KeysOf(Box::new(KeysOf::new(ObjSet::one(ob(b)), Ns::Item, KeySet::default(), false)))),
            Edit::Name(n, b) => {
                st.env.insert(s(["x", "y", "z"][n as usize]), Value::obj(ob(b)));
            }
            Edit::Narrowed(n, b) => {
                st.narrowed.insert(s(["x.a", "y.a"][n as usize]), Value::obj(ob(b)));
            }
            Edit::Side(a, b) => {
                an.m.dd_factories.insert(ob(a), Value::obj(ob(b)));
            }
            Edit::Mirror(a, b) => {
                an.m.sql_cursors.insert(ob(a), ob(b));
            }
            Edit::Exc(b) => an.unwind.cur_exc = Some(Value::obj(ob(b))),
            Edit::Caller(b) => {
                let mut env = indexmap::IndexMap::new();
                env.insert(s("caller"), Value::obj(ob(b)));
                an.cx.env_stack.push(env);
            }
            Edit::Unbind(n) => {
                st.env.insert(s(["x", "y", "z"][n as usize]), Value::lit(key_int(0)));
            }
            Edit::Repay(a, b) => {
                let i = a as usize;
                an.heap.alloc_raw(kind(i), &site(i), "", true, "again", Payload::Pair(Value::obj(ob(b)), Value::none()));
            }
        }
    }
    (an, pre, st, objs, mark)
}

/// The reference: what the names, the side tables and the analyzer's holdings reach in `st`
/// (`old`: also what the objects made before the mark reach, as the collector, which never
/// drops those, takes them as live).
fn reached(an: &Analyzer, st: &State, objs: &[ObjId], old: bool) -> BTreeSet<ObjId> {
    let mut work: Vec<ObjId> = if old { objs[..OLD].to_vec() } else { Vec::new() };
    for v in st.env.values().chain(st.narrowed.values()).chain(st.ret.iter()) {
        v.push_objs(&mut work);
    }
    for (_, v) in st.lasts.iter() {
        v.push_objs(&mut work);
    }
    an.m.dd_factories.values().for_each(|v| v.push_objs(&mut work));
    work.extend(an.m.sql_cursors.values().copied());
    an.unwind.cur_exc.iter().for_each(|v| v.push_objs(&mut work));
    an.cx.env_stack.iter().flat_map(|e| e.values()).for_each(|v| v.push_objs(&mut work));
    let mut seen = BTreeSet::new();
    while let Some(o) = work.pop() {
        if !seen.insert(o) || !objs.contains(&o) {
            continue;
        }
        for (_, v) in st.slots_of(o) {
            v.push_objs(&mut work);
        }
        if let Some(kv) = st.elems.get(&o) {
            Value::kv_only(kv).push_objs(&mut work);
        }
        if let Payload::Pair(a, b) = &an.heap.obj(o).payload {
            a.push_objs(&mut work);
            b.push_objs(&mut work);
        }
        if let Some(v) = an.m.dd_factories.get(&o) {
            v.push_objs(&mut work);
        }
        work.extend(an.m.sql_cursors.get(&o).copied());
    }
    seen
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn collector_keeps_what_is_reached(young_payloads in proptest::collection::vec(proptest::option::of(0u8..ALL as u8), YOUNG), edits in proptest::collection::vec(edit(), 0..14)) {
        let (mut an, pre, before, objs, mark) = run(&young_payloads, &edits);
        let live = reached(&an, &before, &objs, false);
        let kept = reached(&an, &before, &objs, true);
        let mut after = before.copy();
        an.loop_gc_end(Some((mark, pre)), &mut after);
        for (i, &o) in objs.iter().enumerate() {
            let young = i >= OLD;
            if live.contains(&o) || !young {
                prop_assert!(an.has_facts(o, &after), "object {} (reached) lost its facts", i);
                prop_assert_eq!(before.slots_of(o), after.slots_of(o), "object {}'s slots changed", i);
                prop_assert_eq!(before.elems.get(&o), after.elems.get(&o), "object {}'s elements changed", i);
                prop_assert_eq!(before.lens.get(&o), after.lens.get(&o), "object {}'s length changed", i);
            } else if !kept.contains(&o) {
                prop_assert!(!an.has_facts(o, &after), "object {} (not reached) kept its facts", i);
            }
        }
    }
}
