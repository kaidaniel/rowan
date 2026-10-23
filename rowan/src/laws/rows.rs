//! Laws of the key-set domain (`Row`), property-tested against concrete dicts.
//!
//! The reference model is the collecting semantics: next to each abstract row the test keeps
//! the concrete dicts (keys in insertion order, as CPython iterates them) that the paths it
//! joins may hold, and runs every transfer on both: a strong store appends a missing key, a
//! weak one (one of several objects) may or may not; a delete; `update` (`copy_from`, with
//! removed keys); a rename (`df.rename(columns=…)`; two keys may become one, here as
//! a comprehension would merge them); a test that found a key (`narrow_in`); keys stored
//! under unknown names (`data_store`); code that may remove keys; unknown code (`mutate`:
//! anything); a join is the union. Two concrete sets are kept: what may really happen (unknown
//! code may add or remove any key), and the optimistic one the `likely` tiers assume (unknown
//! code removes and adds nothing).
//!
//! Checked after every step:
//! * invariants: `present ⊇ likely ⊇ definite`, `absent_likely ⊇ absent`, `absent` disjoint
//!   from `present` and `absent_likely` from `likely`, the insertion order and `stored` keys
//!   among the present ones, without repeats; closed exactly when no source, mutator or top;
//! * soundness, for every concrete dict: it holds the `definite` keys and none `absent`; a closed
//!   row holds no key but `present`; `classify` and `lacks` say `proven` only of keys it holds
//!   and `missing` only of keys it lacks; a closed row's known order (`ordered_keys`) is its
//!   order (restricted to the keys it holds);
//! * optimism, for every optimistic concrete dict: it holds the `likely` keys and none
//!   `absent_likely`, as the `likely` and `likely-missing` verdicts say;
//! * join: commutative, idempotent, an upper bound (above via the concrete sets).

use super::*;
use crate::intern::{key_int, key_str_lit};
use crate::laws::config;
use crate::source::{Source, SourceKind};
use proptest::prelude::*;
use proptest::sample::Index;
use std::collections::BTreeSet;

/// Keys the programs use: five strs, two ints, two rename targets.
fn key_of(i: u8) -> KeyId {
    match i {
        0 => key_str_lit("a"),
        1 => key_str_lit("b"),
        2 => key_str_lit("c"),
        3 => key_str_lit("d"),
        4 => key_str_lit("e"),
        5 => key_int(1),
        6 => key_int(2),
        7 => key_str_lit("A"),
        _ => key_str_lit("B"),
    }
}
const KEYS: u8 = 9;
/// Keys a program writes (the rename targets only by renames).
const WRITTEN: u8 = 7;

type Dict = Vec<KeyId>;
/// Concrete dicts a path may hold, at most `CAP` (fewer only weakens the check).
type Dicts = BTreeSet<Dict>;
const CAP: usize = 48;

fn cap(mut d: Dicts) -> Dicts {
    while d.len() > CAP {
        // (every other one: spread over the set, deterministic)
        d = d.into_iter().step_by(2).collect();
    }
    d
}

fn with_key(d: &Dict, k: KeyId) -> Dict {
    let mut d = d.clone();
    if !d.contains(&k) {
        d.push(k);
    }
    d
}

fn without(d: &Dict, k: KeyId) -> Dict {
    d.iter().copied().filter(|x| *x != k).collect()
}

/// One abstract row and the concrete dicts it stands for.
#[derive(Clone, Debug)]
struct Slot {
    row: Row,
    real: Dicts,
    optimistic: Dicts,
}

#[derive(Debug, Clone)]
enum Op {
    /// a fresh row: empty, open through input 0 or 1, of an object of unknown origin
    New(u8),
    Store(Index, u8, bool),
    StoreMany(Index, BTreeSet<u8>),
    Delete(Index, u8, bool),
    Copy(Index),
    Join(Index, Index),
    CopyFrom(Index, Index, bool, BTreeSet<u8>),
    /// a swap of two keys, or two keys renamed to the targets (which the dict may hold)
    Rename(Index, u8, u8, bool),
    NarrowIn(Index, u8, bool),
    DataStore(Index),
    Mutate(Index, u8),
    MutateLike(Index, u8, u8),
    MayRemove(Index, BTreeSet<u8>),
    MayRemoveAny(Index),
}

fn written() -> impl Strategy<Value = u8> {
    0u8..WRITTEN
}

fn op() -> impl Strategy<Value = Op> {
    let i = any::<Index>;
    prop_oneof![
        2 => (0u8..4).prop_map(Op::New),
        8 => (i(), written(), prop::bool::weighted(0.7)).prop_map(|(i, k, s)| Op::Store(i, k, s)),
        1 => (i(), prop::collection::btree_set(written(), 1..3)).prop_map(|(i, ks)| Op::StoreMany(i, ks)),
        4 => (i(), written(), prop::bool::weighted(0.7)).prop_map(|(i, k, s)| Op::Delete(i, k, s)),
        3 => i().prop_map(Op::Copy),
        5 => (i(), i()).prop_map(|(a, b)| Op::Join(a, b)),
        2 => (i(), i(), any::<bool>(), prop::collection::btree_set(written(), 0..2)).prop_map(|(a, b, s, k)| Op::CopyFrom(a, b, s, k)),
        1 => (i(), written(), written(), any::<bool>()).prop_map(|(a, x, y, swap)| Op::Rename(a, x, y, swap)),
        2 => (i(), written(), prop::bool::weighted(0.7)).prop_map(|(a, k, s)| Op::NarrowIn(a, k, s)),
        1 => i().prop_map(Op::DataStore),
        1 => (i(), 0u8..3).prop_map(|(a, s)| Op::Mutate(a, s)),
        1 => (i(), 0u8..3, 2u8..4).prop_map(|(a, s, m)| Op::MutateLike(a, s, m)),
        1 => (i(), prop::collection::btree_set(written(), 1..3)).prop_map(|(a, ks)| Op::MayRemove(a, ks)),
        1 => i().prop_map(Op::MayRemoveAny),
    ]
}

/// What two inputs hold (each a dict of the written keys, in an order).
fn inputs() -> impl Strategy<Value = [Dict; 2]> {
    let one = || prop::sample::subsequence((0..WRITTEN).collect::<Vec<u8>>(), 0..5).prop_shuffle().prop_map(|v| v.into_iter().map(key_of).collect::<Dict>());
    (one(), one()).prop_map(|(a, b)| [a, b])
}

/// The dicts unknown code may leave of `d`: any one key added or removed (a sample of
/// "anything", which the abstract row must cover).
fn havoc(d: &Dicts, like: &dyn Fn(KeyId) -> bool) -> Dicts {
    let mut out = d.clone();
    for x in d {
        for i in 0..WRITTEN {
            let k = key_of(i);
            if like(k) {
                out.insert(with_key(x, k));
                out.insert(without(x, k));
            }
        }
    }
    cap(out)
}

/// `{f(k): v for k, v in d.items()}`: two keys renamed to one stay where the first was.
fn rename_dict(d: &Dict, f: &dyn Fn(KeyId) -> KeyId) -> Dict {
    d.iter().fold(Dict::new(), |o, k| with_key(&o, f(*k)))
}

/// `self.update(o)` without the `kill` keys: o's keys appended in its order, those there kept
/// in place.
fn update(d: &Dict, o: &Dict, kill: &KeySet) -> Dict {
    let mut d = d.clone();
    for k in o.iter().filter(|k| !kill.contains(**k)) {
        if !d.contains(k) {
            d.push(*k);
        }
    }
    d
}

fn apply(slots: &mut Vec<Slot>, op: &Op, ins: &[Dict; 2]) {
    let n = slots.len();
    match op {
        Op::New(kind) => {
            let (row, d) = match kind {
                0 => (Row::new(), Dict::new()),
                1 | 2 => {
                    let s = Source::new(if *kind == 1 { "in0" } else { "in1" }, SourceKind::Call);
                    (Row::with_source(s), ins[*kind as usize - 1].clone())
                }
                _ => (Row::top(), ins[0].clone()),
            };
            slots.push(Slot { row, real: Dicts::from([d.clone()]), optimistic: Dicts::from([d]) });
        }
        Op::Copy(i) => {
            let s = slots[i.index(n)].clone();
            slots.push(s);
        }
        Op::Join(i, j) => {
            let (a, b) = (&slots[i.index(n)], &slots[j.index(n)]);
            let s = Slot {
                row: a.row.join(&b.row),
                real: cap(a.real.union(&b.real).cloned().collect()),
                optimistic: cap(a.optimistic.union(&b.optimistic).cloned().collect()),
            };
            slots.push(s);
        }
        Op::CopyFrom(i, j, strong, kill) => {
            let (i, j) = (i.index(n), j.index(n));
            let kill: KeySet = kill.iter().map(|k| key_of(*k)).collect();
            let o = slots[j].clone();
            let s = &mut slots[i];
            s.row = s.row.copy_from(&o.row, *strong, &kill, EMPTY_LENS);
            let prod = |mine: &Dicts, theirs: &Dicts| -> Dicts {
                let mut out = Dicts::new();
                for d in mine {
                    for x in theirs {
                        out.insert(update(d, x, &kill));
                    }
                    if !strong {
                        out.insert(d.clone());
                    }
                }
                cap(out)
            };
            s.real = prod(&s.real, &o.real);
            s.optimistic = prod(&s.optimistic, &o.optimistic);
        }
        Op::Store(i, k, strong) => each(&mut slots[i.index(n)], &|r| r.store(key_of(*k), *strong), &|d| {
            let s = with_key(d, key_of(*k));
            if *strong { vec![s] } else { vec![s, d.clone()] }
        }),
        Op::StoreMany(i, ks) => each(&mut slots[i.index(n)], &|r| r.store_many(&ks.iter().map(|k| key_of(*k)).collect()), &|d| {
            let mut v: Vec<Dict> = ks.iter().map(|k| with_key(d, key_of(*k))).collect();
            v.push(d.clone());
            v
        }),
        Op::Delete(i, k, strong) => each(&mut slots[i.index(n)], &|r| r.delete(key_of(*k), *strong), &|d| {
            let s = without(d, key_of(*k));
            if *strong { vec![s] } else { vec![s, d.clone()] }
        }),
        Op::Rename(i, x, y, swap) => {
            let (kx, ky) = (key_of(*x), key_of(*y));
            if kx == ky {
                return;
            }
            let pairs = if *swap { vec![(kx, ky), (ky, kx)] } else { vec![(kx, key_of(7)), (ky, key_of(8))] };
            let step = crate::source::Step::RenameMap(Arc::new(pairs));
            let s = &mut slots[i.index(n)];
            let st = step.clone();
            each(s, &|r| r.rename(&st), &|d| vec![rename_dict(d, &|k| step.apply(k))]);
        }
        Op::NarrowIn(i, k, strong) => {
            let k = key_of(*k);
            let s = &mut slots[i.index(n)];
            s.row = s.row.narrow_in(k, *strong);
            if *strong {
                s.real.retain(|d| d.contains(&k));
                s.optimistic.retain(|d| d.contains(&k));
            }
        }
        Op::DataStore(i) => {
            let src = Source::new("keys not known", SourceKind::Data);
            each(&mut slots[i.index(n)], &|r| r.data_store(src), &|d| (0..WRITTEN).map(|j| with_key(d, key_of(j))).chain([d.clone()]).collect());
        }
        Op::Mutate(i, site) => {
            let s = &mut slots[i.index(n)];
            s.row = s.row.mutate(intern::s(&format!("f{site}")));
            s.real = havoc(&s.real, &|_| true);
        }
        Op::MutateLike(i, site, m) => {
            let like = |k: KeyId| intern::key_as_str(k).is_some_and(|s| s.len() as u8 % *m == 1) || k.0 % *m as u32 == 0;
            let s = &mut slots[i.index(n)];
            s.row = s.row.mutate_like(intern::s(&format!("f{site}")), &like);
            s.real = havoc(&s.real, &like);
        }
        Op::MayRemove(i, ks) => {
            let keys: Vec<KeyId> = ks.iter().map(|k| key_of(*k)).collect();
            each(&mut slots[i.index(n)], &|r| r.may_remove(&keys), &|d| {
                let mut v = vec![d.clone()];
                for k in &keys {
                    v.push(without(d, *k));
                }
                v
            });
        }
        Op::MayRemoveAny(i) => each(&mut slots[i.index(n)], &|r| r.may_remove_any(), &|d| {
            let mut v = vec![d.clone()];
            for k in d {
                v.push(without(d, *k));
            }
            v
        }),
    }
}

/// A transfer on the row and on each concrete dict of both sets.
fn each(s: &mut Slot, row: &dyn Fn(&Row) -> Row, dict: &dyn Fn(&Dict) -> Vec<Dict>) {
    s.row = row(&s.row);
    s.real = cap(s.real.iter().flat_map(dict).collect());
    s.optimistic = cap(s.optimistic.iter().flat_map(dict).collect());
}

/// A row's facts for a failure message (`details` and what it leaves out).
fn show(r: &Row) -> String {
    let ks = |s: &KeySet| intern::keys_str(s.iter());
    let order: Vec<String> = r.order.iter().map(|k| intern::key_repr(*k)).collect();
    format!("{} absent_likely={} stored={} order=[{}]", r.details(), ks(&r.absent_likely), ks(&r.stored), order.join(", "))
}

fn check(s: &Slot) -> Result<(), String> {
    let r = &s.row;
    let inv = |ok: bool, what: &str| if ok { Ok(()) } else { Err(format!("{what}: {}", show(r))) };
    inv(r.likely.is_subset(&r.present), "likely ⊄ present")?;
    inv(r.definite.is_subset(&r.likely), "definite ⊄ likely")?;
    inv(r.absent.is_subset(&r.absent_likely), "absent ⊄ absent_likely")?;
    inv(!r.absent.intersects(&r.present), "absent ∩ present")?;
    inv(!r.absent_likely.intersects(&r.likely), "absent_likely ∩ likely")?;
    inv(r.stored.is_subset(&r.present), "stored ⊄ present")?;
    inv(r.order.iter().all(|k| r.present.contains(*k)), "order ⊄ present")?;
    inv(r.order.iter().collect::<BTreeSet<_>>().len() == r.order.len(), "order repeats a key")?;
    inv(r.closed() == (r.sources.is_empty() && r.mutators.is_empty() && !r.top), "closed")?;
    if r.top {
        return Ok(());
    }
    let order = r.ordered_keys().filter(|_| r.closed());
    for d in &s.real {
        let has = |k: KeyId| d.contains(&k);
        for i in 0..KEYS {
            let k = key_of(i);
            let tier = classify(r, k).tier;
            if (r.definite.contains(k) || tier == Tier::Proven) && !has(k) {
                return Err(format!("{} is {tier:?} but a run's dict {d:?} lacks it: {}", intern::key_repr(k), show(r)));
            }
            if (r.absent.contains(k) || r.lacks(k) || tier == Tier::Missing) && has(k) {
                return Err(format!("{} is {tier:?} but a run's dict {d:?} holds it: {}", intern::key_repr(k), show(r)));
            }
        }
        if r.closed() && !d.iter().all(|k| r.present.contains(*k)) {
            return Err(format!("closed row {} but a run's dict {d:?}", show(r)));
        }
        if let Some(o) = &order {
            let want: Dict = o.iter().copied().filter(|k| has(*k)).collect();
            if &want != d {
                return Err(format!("order {o:?} but a run's dict iterates {d:?}: {}", show(r)));
            }
        }
    }
    for d in &s.optimistic {
        for i in 0..KEYS {
            let k = key_of(i);
            let tier = classify(r, k).tier;
            if (r.likely.contains(k) || tier == Tier::Likely) && !d.contains(&k) {
                return Err(format!("{} is {tier:?} but a run whose unknown code changed nothing lacks it: {d:?} {}", intern::key_repr(k), show(r)));
            }
            if (r.absent_likely.contains(k) || tier == Tier::LikelyMissing) && d.contains(&k) {
                return Err(format!("{} is {tier:?} but a run whose unknown code changed nothing holds it: {d:?} {}", intern::key_repr(k), show(r)));
            }
        }
    }
    Ok(())
}

fn run(ops: &[Op], ins: &[Dict; 2]) -> Result<(), String> {
    let mut slots: Vec<Slot> = vec![Slot { row: Row::new(), real: Dicts::from([Dict::new()]), optimistic: Dicts::from([Dict::new()]) }];
    for (step, op) in ops.iter().enumerate() {
        apply(&mut slots, op, ins);
        for s in &slots {
            check(s).map_err(|e| format!("after step {step} ({op:?}): {e}"))?;
        }
        if let Op::Join(i, j) = op {
            let (a, b) = (&slots[i.index(slots.len() - 1)].row, &slots[j.index(slots.len() - 1)].row);
            if a.join(b) != b.join(a) {
                return Err(format!("join not commutative: {} ⊔ {}", show(a), show(b)));
            }
            if a.join(a) != *a {
                return Err(format!("join not idempotent: {}", show(a)));
            }
        }
    }
    Ok(())
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn rows_cover_their_runs(ops in prop::collection::vec(op(), 1..40), ins in inputs()) {
        if let Err(e) = run(&ops, &ins) {
            prop_assert!(false, "{}", e);
        }
    }
}
