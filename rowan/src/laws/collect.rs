//! Laws of the other collectors, property-tested on heaps a program of small edits makes:
//! * `drop_unreached` (a raise leaves an instance half-built): the instance keeps its facts in
//!   the raise state wherever the program can still reach it there, at every raise that looks;
//! * `fold_recent_in` (a call's or a loop's *recent* objects folded into their summaries,
//!   `call_recent`, `rename_recent`): every recent object the program still reaches has its
//!   facts (rows, slots, elements, lengths) covered by its summary's, and no place the program
//!   reaches refers to it any more (names, slots, elements, the result, payloads, side tables).
//!
//! The reference model is the plain reachability walk of `laws/gc.rs`: from the names, narrowed
//! names, the result, the lists' last items, every side table's entry and what the analyzer
//! holds; through slots, element keys, payloads and the side tables keyed by a reached object.

use super::*;
use crate::domain::{KeyVal, KeysOf, ObjSet, Row, Value};
use crate::intern::{key_int, s, KeySet};
use crate::laws::config;
use crate::state::{join_len, mark_merged};
use proptest::prelude::*;
use std::collections::BTreeSet;

/// One edit of the heap after a mark (objects by index into the law's objects).
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
    /// an attribute mirror of object `.0` is object `.1`
    Mirror(u8, u8),
    /// an exception in flight is the object
    Exc(u8),
    /// a name is rebound to a literal
    Unbind(u8),
    /// object `.0` is made again (same site and context) with a payload holding object `.1`
    Repay(u8, u8),
    /// a list's last item (`lasts`) of object `.0` is object `.1`
    Last(u8, u8),
    /// a raise looks at the half-built object (`drop_unreached` only)
    Look,
}

fn edit(n: usize) -> impl Strategy<Value = Edit> {
    let o = move || 0u8..n as u8;
    prop_oneof![
        4 => (o(), 0u8..3, o()).prop_map(|(a, k, b)| Edit::Slot(a, k, b)),
        1 => (o(), o()).prop_map(|(a, b)| Edit::Elems(a, b)),
        3 => (0u8..3, o()).prop_map(|(n, b)| Edit::Name(n, b)),
        1 => (0u8..2, o()).prop_map(|(n, b)| Edit::Narrowed(n, b)),
        1 => (o(), o()).prop_map(|(a, b)| Edit::Side(a, b)),
        1 => (o(), o()).prop_map(|(a, b)| Edit::Mirror(a, b)),
        1 => o().prop_map(Edit::Exc),
        2 => (0u8..3).prop_map(Edit::Unbind),
        2 => (o(), o()).prop_map(|(a, b)| Edit::Repay(a, b)),
        1 => (o(), o()).prop_map(|(a, b)| Edit::Last(a, b)),
        2 => Just(Edit::Look),
    ]
}

const NAMES: [&str; 3] = ["x", "y", "z"];

/// Applies a heap edit (not `Look`) on `objs`.
fn apply(an: &mut Analyzer, st: &mut State, objs: &[ObjId], kinds: &[Kind], sites: &[(String, String)], e: &Edit) {
    let ob = |i: u8| objs[i as usize];
    match *e {
        Edit::Slot(a, k, b) => st.set_slot(ob(a), Ns::Attr, key_int(k as i64), Value::obj(ob(b)), true),
        Edit::Elems(a, b) => st.elems.insert(ob(a), KeyVal::KeysOf(Box::new(KeysOf::new(ObjSet::one(ob(b)), Ns::Item, KeySet::default(), false)))),
        Edit::Name(n, b) => {
            st.env.insert(s(NAMES[n as usize]), Value::obj(ob(b)));
        }
        Edit::Narrowed(n, b) => {
            st.narrowed.insert(s(["x.a", "y.a"][n as usize]), Value::obj(ob(b)));
        }
        Edit::Side(a, b) => {
            an.m.dd_factories.insert(ob(a), Value::obj(ob(b)));
        }
        Edit::Mirror(a, b) => {
            an.m.attr_mirrors.insert(ob(a), ob(b));
        }
        Edit::Exc(b) => an.unwind.cur_exc = Some(Value::obj(ob(b))),
        Edit::Unbind(n) => {
            st.env.insert(s(NAMES[n as usize]), Value::lit(key_int(0)));
        }
        Edit::Repay(a, b) => {
            let (site, ctx) = &sites[a as usize];
            an.heap.alloc_raw(kinds[a as usize], site, ctx, true, "again", Payload::Pair(Value::obj(ob(b)), Value::none()));
        }
        Edit::Last(a, b) => st.set_last(ob(a), Value::obj(ob(b))),
        Edit::Look => {}
    }
}

/// What the program reaches in `st` from its names (`names`: the state's names count), `extra`,
/// the side tables' entries and the exception in flight, through every edge of every object.
fn reached(an: &Analyzer, st: &State, extra: &[ObjId], names: bool) -> BTreeSet<ObjId> {
    let mut work: Vec<ObjId> = extra.to_vec();
    if names {
        for v in st.env.values().chain(st.narrowed.values()).chain(st.ret.iter()) {
            v.push_objs(&mut work);
        }
    }
    an.m.dd_factories.values().for_each(|v| v.push_objs(&mut work));
    work.extend(an.m.attr_mirrors.values().copied());
    an.unwind.cur_exc.iter().for_each(|v| v.push_objs(&mut work));
    let mut seen = BTreeSet::new();
    while let Some(o) = work.pop() {
        if !seen.insert(o) || !an.heap.has_obj(o) {
            continue;
        }
        for (_, v) in st.slots_of(o) {
            v.push_objs(&mut work);
        }
        if let Some(kv) = st.elems.get(&o) {
            Value::kv_only(kv).push_objs(&mut work);
        }
        if let Some(v) = st.last_of(o) {
            v.push_objs(&mut work);
        }
        if let Payload::Pair(a, b) = &an.heap.obj(o).payload {
            a.push_objs(&mut work);
            b.push_objs(&mut work);
        }
        if let Some(v) = an.m.dd_factories.get(&o) {
            v.push_objs(&mut work);
        }
        work.extend(an.m.attr_mirrors.get(&o).copied());
    }
    seen
}

// ---------------------------------------------------------------------------
// drop_unreached
// ---------------------------------------------------------------------------

/// Objects made before the construction began, then the instance, then objects made since.
const D_OLD: usize = 3;
const D_YOUNG: usize = 5;
const D_ALL: usize = D_OLD + 1 + D_YOUNG;

fn d_kind(i: usize) -> Kind {
    [Kind::Dict, Kind::List, Kind::Instance][i % 3]
}

/// The constructor's run: edits, and at each `Look` (and at the end) a raise state that
/// `drop_unreached` sees; Err with the first raise state where it dropped a reached instance.
fn d_run(young_payloads: &[Option<u8>], edits: &[Edit]) -> Result<(), String> {
    let mut an = Analyzer::new(String::new(), "law_drop.py", Vec::new(), false, false).expect("an analyzer");
    let mut st = State::new();
    let mut objs: Vec<ObjId> = Vec::new();
    let mut sites: Vec<(String, String)> = Vec::new();
    let kinds: Vec<Kind> = (0..D_ALL).map(|i| if i == D_OLD { Kind::Instance } else { d_kind(i) }).collect();
    for (i, kind) in kinds.iter().enumerate().take(D_OLD + 1) {
        let site = format!("law:drop:{i}");
        let o = an.heap.alloc_raw(*kind, &site, "", true, "old", Payload::None);
        an.heap.renew(o);
        st.set_lens(o, (Some(0), 0, 0));
        objs.push(o);
        sites.push((site, String::new()));
    }
    st.env.insert(s("old"), Value::obj(objs[0]));
    // (the instance: made, then its constructor begins; `self` is among the frame's names)
    let target = objs[D_OLD];
    st.set_slot(target, Ns::Attr, key_int(7), Value::lit(key_int(1)), true);
    st.env.insert(s("self"), Value::obj(target));
    an.cx.fresh_init.insert(target, crate::interp::context::Building { mark: an.heap.serial(), seen: an.heap.mark(), repaid: false, at: st.copy(), roots: Vec::new(), holders: Vec::new(), looks: 0, held: false });
    for i in D_OLD + 1..D_ALL {
        let payload = match young_payloads[i - D_OLD - 1] {
            Some(j) if (j as usize) < i => Payload::Pair(Value::obj(objs[j as usize]), Value::none()),
            _ => Payload::None,
        };
        let site = format!("law:drop:{i}");
        let o = an.heap.alloc_raw(kinds[i], &site, "", true, "young", payload);
        an.heap.renew(o);
        st.set_lens(o, (Some(0), 0, 0));
        st.set_slot(o, Ns::Attr, key_int(9), Value::lit(key_int(1)), true);
        objs.push(o);
        sites.push((site, String::new()));
    }
    let look = |an: &mut Analyzer, st: &State| -> Result<(), String> {
        // (the raise leaves the constructor: `self` is no root any more)
        let mut here = st.copy();
        here.env.shift_remove(&s("self"));
        let live = reached(an, &here, &[], true).contains(&target);
        let before = here.copy();
        let mut roots: Vec<ObjId> = Vec::new();
        an.state_roots(&here, usize::MAX, usize::MAX, &mut roots);
        for (x, v) in here.lasts.iter() {
            roots.push(*x);
            v.push_objs(&mut roots);
        }
        an.drop_unreached(target, roots, &mut here, usize::MAX);
        if live && (!an.has_facts(target, &here) || before.slots_of(target) != here.slots_of(target)) {
            return Err("the instance is reached, yet lost its facts".into());
        }
        Ok(())
    };
    for e in edits {
        match e {
            Edit::Look => look(&mut an, &st)?,
            e => apply(&mut an, &mut st, &objs, &kinds, &sites, e),
        }
    }
    look(&mut an, &st)
}

// ---------------------------------------------------------------------------
// fold_recent_in
// ---------------------------------------------------------------------------

/// Summary objects (merged, made before the call), other older objects, the call's recent
/// objects (each of a summary's site), objects the call made otherwise.
const F_SUM: usize = 2;
const F_OLD: usize = 2;
const F_REC: usize = 3;
const F_YOUNG: usize = 3;
const F_ALL: usize = F_SUM + F_OLD + F_REC + F_YOUNG;

fn f_kind(i: usize) -> Kind {
    if i < F_SUM {
        [Kind::Instance, Kind::List][i]
    } else if i < F_SUM + F_OLD + F_REC && i >= F_SUM + F_OLD {
        [Kind::Instance, Kind::List][(i - F_SUM - F_OLD) % F_SUM]
    } else {
        [Kind::Dict, Kind::List, Kind::Instance][i % 3]
    }
}

/// A fact a recent object or a summary starts with (`facts.0`: a key stored, `.1`: a slot's
/// literal, `.2`: a length, `.3`: elements known).
type Start = (u8, u8, Option<u8>, bool);

fn start(st: &mut State, o: ObjId, kind: Kind, f: &Start) {
    let k = key_int(f.0 as i64 % 3);
    let row = st.rows.get(&(o, Ns::Attr)).unwrap_or_else(Row::new).store(k, true);
    st.rows.insert((o, Ns::Attr), row);
    st.set_slot(o, Ns::Attr, k, Value::lit(key_int(f.1 as i64)), true);
    st.set_lens(o, (f.2.map(|n| n as usize % 4), 0, 0));
    if kind == Kind::List && f.3 {
        st.elems.insert(o, KeyVal::lits(KeySet::from_iter([key_int(f.1 as i64 + 10)])));
    }
}

fn leq(a: &Value, b: &Value) -> bool {
    a.join(b) == *b
}

/// Is `after`'s `(o, ns)` row at least `r` (each set on its side; a key order only where both say it)?
fn row_covers(after: &State, o: ObjId, ns: Ns, r: &Row) -> bool {
    after.rows.get(&(o, ns)).is_some_and(|a| {
        r.present.is_subset(&a.present) && a.likely.is_subset(&r.likely) && a.definite.is_subset(&r.definite) && a.absent.is_subset(&r.absent) && a.absent_likely.is_subset(&r.absent_likely) && r.sources.is_subset(&a.sources) && r.mutators.is_subset(&a.mutators) && (a.top || !r.top) && a.ordered_keys().is_none_or(|k| r.ordered_keys() == Some(k))
    })
}

fn f_run(starts: &[Start], young_payloads: &[Option<u8>], edits: &[Edit], ret: (u8, u8), names: bool) -> Result<(), String> {
    let mut an = Analyzer::new(String::new(), "law_fold.py", Vec::new(), false, false).expect("an analyzer");
    let mut st = State::new();
    let mut objs: Vec<ObjId> = Vec::new();
    let mut sites: Vec<(String, String)> = Vec::new();
    let kinds: Vec<Kind> = (0..F_ALL).map(f_kind).collect();
    for i in 0..F_SUM + F_OLD {
        let (site, single) = (format!("law:fold:{i}"), i >= F_SUM);
        let o = an.heap.alloc_raw(kinds[i], &site, "c", single, "old", Payload::None);
        an.heap.renew(o);
        if !single {
            mark_merged(o);
        }
        start(&mut st, o, kinds[i], &starts[i]);
        objs.push(o);
        sites.push((site, "c".into()));
    }
    st.env.insert(s("old"), Value::obj(objs[F_SUM]));
    st.env.insert(s("summary"), Value::obj(objs[0]));
    let head = st.copy();
    let mark = an.heap.mark();
    let mut recs = Vec::new();
    for i in F_SUM + F_OLD..F_ALL {
        let o = if i < F_SUM + F_OLD + F_REC {
            // (a recent object of summary `j`'s site, one per call depth)
            let j = (i - F_SUM - F_OLD) % F_SUM;
            let (site, ctx) = (format!("law:fold:{j}"), "c".to_string());
            let r = an.heap.alloc_raw(kinds[i], &format!("{site}#call"), &format!("{ctx}#{i}"), true, "recent", Payload::None);
            recs.push((r, kinds[i], site, ctx));
            sites.push((format!("law:fold:{j}#call"), format!("c#{i}")));
            r
        } else {
            let p = match young_payloads[i - F_SUM - F_OLD - F_REC] {
                Some(j) if (j as usize) < i => Payload::Pair(Value::obj(objs[j as usize]), Value::none()),
                _ => Payload::None,
            };
            let site = format!("law:fold:{i}");
            sites.push((site.clone(), "c".into()));
            an.heap.alloc_raw(kinds[i], &site, "c", true, "young", p)
        };
        an.heap.renew(o);
        start(&mut st, o, kinds[i], &starts[i]);
        objs.push(o);
    }
    for e in edits {
        apply(&mut an, &mut st, &objs, &kinds, &sites, e);
    }
    let mut v = if ret.0 % 3 == 0 { Value::none() } else { Value::obj(objs[ret.1 as usize % F_ALL]) };
    // (the exception in flight is the value a raise state folds with, `fold_recent_in`'s `v`)
    if let Some(e) = an.unwind.cur_exc.take() {
        v = v.join(&e);
    }
    let before = st.copy();
    let rs: Vec<ObjId> = recs.iter().map(|r| r.0).collect();
    let mut extra = Vec::new();
    v.push_objs(&mut extra);
    let live = reached(&an, &before, &extra, names);
    an.fold_recent_in(&mut st, &head, mark, recs.clone(), &mut v, names);
    let map: rustc_hash::FxHashMap<ObjId, ObjId> = recs.iter().filter_map(|(r, k, site, ctx)| Some((*r, an.heap.find(*k, site, ctx)?))).collect();
    for (r, kind, site, ctx) in &recs {
        if !live.contains(r) {
            continue;
        }
        let sum = an.heap.find(*kind, site, ctx).ok_or("no summary")?;
        for ns in [Ns::Item, Ns::Attr] {
            if let Some(row) = before.rows.get(&(*r, ns)) {
                if !row_covers(&st, sum, ns, &row) {
                    return Err(format!("summary row {ns:?} does not cover the recent object's"));
                }
            }
            if let Some(row) = before.rows.get(&(sum, ns)) {
                if !row_covers(&st, sum, ns, &row) {
                    return Err(format!("summary row {ns:?} lost its own keys"));
                }
            }
        }
        for ((ns, k), x) in before.slots_of(*r).into_iter().chain(before.slots_of(sum)) {
            if ns == Ns::Pos {
                continue;
            }
            let x = super::rename_value(&x, &map).unwrap_or(x);
            if !st.slot_opt(sum, ns, k).is_some_and(|y| leq(&x, &y)) {
                return Err(format!("summary slot {ns:?} {k:?} does not cover {x:?}"));
            }
        }
        // (elements: as `State::join` takes them, an object without them adds nothing)
        for o in [*r, sum] {
            if let Some(e) = before.elems.get(&o) {
                let Some(ae) = st.elems.get(&sum) else { continue }; // (none: not known)
                let e = super::rename_value(&Value::kv_only(e.clone()), &map).map(|v| v.kv).unwrap_or(e);
                if crate::domain::kv_join(&e, &ae) != ae {
                    return Err("summary's elements do not cover".into());
                }
            }
        }
        if let Some(al) = st.lens.get(&sum) {
            for l in [before.lens.get(r), before.lens.get(&sum)].into_iter().flatten() {
                if join_len(&l, &al) != al {
                    return Err(format!("summary's length {al:?} does not cover {l:?}"));
                }
            }
        }
    }
    // (no place the program reaches refers to a recent object it reached)
    let mut extra = Vec::new();
    v.push_objs(&mut extra);
    let now = reached(&an, &st, &extra, names);
    for r in rs.iter().filter(|r| live.contains(r)) {
        if now.contains(r) {
            return Err(format!("recent object {r} still referred to"));
        }
    }
    Ok(())
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn drop_unreached_keeps_what_is_reached(young_payloads in proptest::collection::vec(proptest::option::of(0u8..D_ALL as u8), D_YOUNG), edits in proptest::collection::vec(edit(D_ALL), 0..14)) {
        let r = d_run(&young_payloads, &edits);
        prop_assert!(r.is_ok(), "{}", r.unwrap_err());
    }

    #[test]
    fn folded_recent_objects_are_covered(starts in proptest::collection::vec((any::<u8>(), 0u8..4, proptest::option::of(0u8..4), any::<bool>()), F_ALL), young_payloads in proptest::collection::vec(proptest::option::of(0u8..F_ALL as u8), F_YOUNG), edits in proptest::collection::vec(edit(F_ALL), 0..14), ret in (any::<u8>(), any::<u8>()), names in any::<bool>()) {
        let r = f_run(&starts, &young_payloads, &edits, ret, names);
        prop_assert!(r.is_ok(), "{}", r.unwrap_err());
    }
}


