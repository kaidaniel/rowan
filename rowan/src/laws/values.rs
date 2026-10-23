//! Laws of the scalar domain (`KeyVal`, `Value`), property-tested against a concretization.
//!
//! The reference model: a scalar abstract value stands for a set of concrete Python scalars
//! (`gamma`), here over a universe of interned literals (ints past `MAX_LITS` of them, bools,
//! None, str, float, bytes, a tuple) and a heap giving each object a key set (for `KeysOf`):
//! * `Lit(k)` / `Lits(ks)`: those literals;
//! * `Data(label, hint)`: by the label alone (the hint is literals it *may* be, never a limit):
//!   a typed label (`int(?)`) its type's values (an int's: bools too), an optional one also None,
//!   a dim a number, any other label anything;
//! * `KeysOf`: the keys the objects hold, but the excluded ones;
//! * `Bot`: nothing.
//!
//! Checked: `kv_join` is an upper bound (soundness: it covers both sides), commutative and
//! idempotent; associative up to `gamma`; its literals and hints saturate as documented
//! (`MAX_LITS`, `HINT_MAX`) with a hint holding every literal joined in until it saturates;
//! ascending chains of joins stabilise within `CHAIN_BOUND` changes, absorbing every value
//! joined in (up to the hint); `Value::join` and `join_values` agree; narrowing (`kv_exclude`, `kv_restrict`,
//! `kv_not_none`, `kv_of_type`) keeps every concrete value that passes the test.

use super::*;
use crate::intern::{key, key_bool, key_float, key_int, key_none, key_str_lit, key_tuple, Key};
use crate::laws::config;
use proptest::prelude::*;
use std::collections::BTreeSet;

/// Literals in the universe the specs pick from (`lit_of`).
const POOL: u8 = 90;

/// The literal of pool index `i`: ints 0..79 (more than `MAX_LITS`), then other kinds.
fn lit_of(i: u8) -> KeyId {
    match i {
        0..=79 => key_int(i as i64),
        80 => key_bool(true),
        81 => key_bool(false),
        82 => key_none(),
        83 => key_str_lit("a"),
        84 => key_str_lit("b"),
        85 => key_str_lit("c"),
        86 => key_float(0.5),
        87 => key_float(1.0),
        88 => key(Key::Bytes(b"x".to_vec())),
        _ => key_tuple(vec![key_int(1), key_int(2)]),
    }
}

/// Every concrete scalar checked: the pool and values outside it.
fn universe() -> Vec<KeyId> {
    let mut u: Vec<KeyId> = (0..POOL).map(lit_of).collect();
    u.extend([key_int(1000), key_int(-3), key_str_lit("zz"), key_float(2.5), key(Key::Bytes(b"q".to_vec())), key(Key::Ellipsis)]);
    u
}

/// The labels the specs pick from: unnamed, named, an operator's, typed, optional, type names,
/// a text's that says what it holds, symbolic dims.
fn label_of(i: u8) -> String {
    use crate::models::shape;
    match i {
        0 => String::new(),
        1 => "x".into(),
        2 => "expr:3:x + 1".into(),
        3 => typed_label("int", ""),
        4 => typed_label("int", "n"),
        5 => typed_label("str", "s"),
        6 => typed_label("float", ""),
        7 => typed_label("bool", ""),
        8 => typed_label("bytes", ""),
        9 => optional_label("int", "y"),
        10 => optional_label("str", ""),
        11 => "str".into(),
        12 => "int".into(),
        13 => typed_label("int", "expr:4:a * 2"),
        14 => "pyjsontext:t".into(),
        15 => shape::dim_label(shape::unknown()),
        _ => shape::dim_label(shape::lit(3)),
    }
}
const LABELS: u8 = 17;

/// A scalar value to build (interning happens in the test's thread).
#[derive(Debug, Clone)]
pub(crate) enum KvSpec {
    Bot,
    Lits(BTreeSet<u8>),
    Data(u8, BTreeSet<u8>, bool),
    KeysOf(BTreeSet<u8>, BTreeSet<u8>),
}

fn kv_spec() -> impl Strategy<Value = KvSpec> {
    let few = prop::collection::btree_set(0u8..POOL, 0..4);
    let lits = prop_oneof![
        3 => prop::collection::btree_set(prop_oneof![0u8..6, 78u8..POOL], 1..5),
        1 => prop::sample::subsequence((0..POOL).collect::<Vec<u8>>(), 50..80).prop_map(|v| v.into_iter().collect()),
    ];
    prop_oneof![
        1 => Just(KvSpec::Bot),
        4 => lits.prop_map(KvSpec::Lits),
        4 => (0u8..LABELS, prop::collection::btree_set(0u8..POOL, 0..10), prop::bool::weighted(0.1)).prop_map(|(l, h, s)| KvSpec::Data(l, h, s)),
        1 => (prop::collection::btree_set(1u8..5, 1..3), few).prop_map(|(o, x)| KvSpec::KeysOf(o, x)),
    ]
}

pub(crate) fn build(s: &KvSpec) -> KeyVal {
    match s {
        KvSpec::Bot => BOT,
        KvSpec::Lits(ks) => KeyVal::lits(ks.iter().map(|i| lit_of(*i)).collect()),
        KvSpec::Data(l, h, sat) => {
            let hint = if *sat { hint_sat() } else { h.iter().take(HINT_MAX).map(|i| lit_of(*i)).collect() };
            let label = label_of(*l);
            KeyVal::Data(intern::s(&label), if hintable(&label) { hint } else { KeySet::new() })
        }
        KvSpec::KeysOf(objs, excl) => {
            let mut ko = KeysOf::new(objs.iter().map(|o| *o as ObjId).collect(), Ns::Item, KeySet::new(), false);
            ko.exclude = excl.iter().map(|i| lit_of(*i)).collect();
            KeyVal::KeysOf(Box::new(ko))
        }
    }
}

/// Each object's keys, for `KeysOf` (objects 1..4).
type Heap = [BTreeSet<u8>; 5];

fn heap() -> impl Strategy<Value = Heap> {
    prop::array::uniform5(prop::collection::btree_set(prop_oneof![0u8..6, 78u8..POOL], 0..6))
}

/// The concretization: does `kv` stand for the concrete scalar `v`?
pub(crate) fn gamma(kv: &KeyVal, v: KeyId, heap: &Heap) -> bool {
    match kv {
        KeyVal::Bot => false,
        KeyVal::Lit(k) => *k == v,
        KeyVal::Lits(ks) => ks.contains(v),
        KeyVal::KeysOf(ko) => {
            !ko.exclude.contains(v) && ko.chain == EMPTY_LENS && ko.objs.iter().any(|o| heap.get(o as usize).is_some_and(|ks| ks.iter().any(|i| lit_of(*i) == v)))
        }
        KeyVal::Data(l, _) => {
            let l = str_get(*l);
            if crate::models::shape::parse_dim_label(&l).is_some() {
                return key_is_number(v);
            }
            match label_base(&l) {
                Some((t, opt)) => key_type(v, t) || (opt && v == key_none()),
                None => true,
            }
        }
    }
}

fn same_gamma(a: &KeyVal, b: &KeyVal, heap: &Heap) -> Option<KeyId> {
    universe().into_iter().find(|v| gamma(a, *v, heap) != gamma(b, *v, heap))
}

/// Changes an ascending chain of joins may make: literals one at a time up to `MAX_LITS`, an
/// unknown's hint up to `HINT_MAX` and saturated, its label from a name to its type to none, the
/// objects' keys one object at a time.
const CHAIN_BOUND: usize = MAX_LITS + HINT_MAX + 8;

proptest! {
    #![proptest_config(config())]

    #[test]
    fn join_covers_both_sides(a in kv_spec(), b in kv_spec(), h in heap()) {
        let (x, y) = (build(&a), build(&b));
        let j = kv_join(&x, &y);
        for v in universe() {
            prop_assert!(!(gamma(&x, v, &h) || gamma(&y, v, &h)) || gamma(&j, v, &h),
                "{} ⊔ {} = {} loses {}", x, y, j, intern::key_repr(v));
        }
    }

    #[test]
    fn join_is_commutative_and_idempotent(a in kv_spec(), b in kv_spec()) {
        let (x, y) = (build(&a), build(&b));
        prop_assert_eq!(kv_join(&x, &y), kv_join(&y, &x));
        prop_assert_eq!(kv_join(&x, &x), x.clone());
        let v = Value::kv_only(x.clone());
        prop_assert_eq!(v.join(&v), v);
    }

    /// Associative up to what the values stand for (the labels and hints may differ: a name
    /// kept or dropped, literals kept as a hint or not).
    #[test]
    fn join_is_associative_up_to_gamma(a in kv_spec(), b in kv_spec(), c in kv_spec(), h in heap()) {
        let (x, y, z) = (build(&a), build(&b), build(&c));
        let l = kv_join(&kv_join(&x, &y), &z);
        let r = kv_join(&x, &kv_join(&y, &z));
        if let Some(v) = same_gamma(&l, &r, &h) {
            prop_assert!(false, "({} ⊔ {}) ⊔ {} = {} but {} ⊔ ({} ⊔ {}) = {}: differ on {}", x, y, z, l, x, y, z, r, intern::key_repr(v));
        }
    }

    /// Literals saturate past `MAX_LITS` into an unknown; a hintable unknown's hint holds every
    /// literal of both sides until it saturates past `HINT_MAX`, and stays saturated.
    #[test]
    fn saturation(a in kv_spec(), b in kv_spec()) {
        let (x, y) = (build(&a), build(&b));
        let j = kv_join(&x, &y);
        if x.is_literal() && y.is_literal() {
            let n = kv_literals(&x).union(&kv_literals(&y)).len();
            prop_assert_eq!(j.is_literal(), n <= MAX_LITS, "{} literals gave {}", n, j);
        }
        if let KeyVal::Data(l, hj) = &j {
            if hintable(&str_get(*l)) {
                let part = |k: &KeyVal| if k.is_literal() { kv_literals(k) } else { k.hint() };
                let sat = |k: &KeyVal| matches!(k, KeyVal::Data(_, h) if h.contains(intern::STAR));
                if sat(&x) || sat(&y) {
                    prop_assert!(sat(&j), "{} ⊔ {} = {} unsaturated", x, y, j);
                } else if !sat(&j) {
                    prop_assert!(hj.len() <= HINT_MAX);
                    prop_assert!(part(&x).union(&part(&y)).is_subset(hj), "{} ⊔ {} = {} drops a literal from its hint", x, y, j);
                }
            }
        }
    }

    /// A loop head's ascending chain: joining values one after the other changes the head at
    /// most `CHAIN_BOUND` times, and the head it ends with absorbs each value joined in.
    #[test]
    fn ascending_chains_stabilise(specs in prop::collection::vec(kv_spec(), 1..120)) {
        let vals: Vec<KeyVal> = specs.iter().map(build).collect();
        let mut acc = BOT;
        let mut changes = 0;
        for v in &vals {
            let n = kv_join(&acc, v);
            if n != acc {
                changes += 1;
            }
            acc = n;
        }
        prop_assert!(changes <= CHAIN_BOUND, "{} changes", changes);
        // (up to the hint: literals a value may be grow it until it saturates, each a change
        // `CHAIN_BOUND` counts; a dim's or tensor's label keeps none, so a literal joined in
        // before it may come back)
        for v in &vals {
            let again = kv_join(&acc, v);
            prop_assert_eq!(again.unhinted(), acc.unhinted(), "the head {} is not stable under {}", acc, v);
        }
    }

    #[test]
    fn join_values_is_the_fold(specs in prop::collection::vec((kv_spec(), prop::collection::btree_set(0u32..50, 0..4)), 0..12)) {
        let vals: Vec<Value> = specs.iter().map(|(k, p)| Value::with_kv(p.iter().copied().collect(), build(k))).collect();
        let fold = vals.iter().fold(Value::new(), |a, v| a.join(v));
        prop_assert_eq!(join_values(vals.clone()), fold);
    }

    /// Narrowing keeps every concrete value that passes the test (None: none passes).
    #[test]
    fn narrowing_keeps_what_passes(a in kv_spec(), keys in prop::collection::btree_set(0u8..POOL, 0..4), ty in 0usize..5, h in heap()) {
        let x = build(&a);
        let ks: KeySet = keys.iter().map(|i| lit_of(*i)).collect();
        let ex = kv_exclude(&x, &ks);
        let re = kv_restrict(&x, &ks);
        let nn = kv_not_none(&x);
        let t = TYPE_NAMES[ty];
        let ot = kv_of_type(&x, t);
        for v in universe() {
            if !gamma(&x, v, &h) {
                continue;
            }
            if !ks.contains(v) {
                prop_assert!(ex.as_ref().is_some_and(|e| gamma(e, v, &h)), "{} not in {:?}: {:?} loses {}", x, ks, ex, intern::key_repr(v));
            } else {
                prop_assert!(re.as_ref().is_some_and(|e| gamma(e, v, &h)), "{} in {:?}: {:?} loses {}", x, ks, re, intern::key_repr(v));
            }
            if v != key_none() {
                prop_assert!(gamma(&nn, v, &h), "{} not None: {} loses {}", x, nn, intern::key_repr(v));
            }
            if key_type(v, t) {
                prop_assert!(gamma(&ot, v, &h), "{} of type {}: {} loses {}", x, t, ot, intern::key_repr(v));
            }
        }
    }
}
