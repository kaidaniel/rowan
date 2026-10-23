//! Laws of the persistent maps (`layered.rs`), property-tested against a `BTreeMap` model.
//!
//! Each case runs a random program of operations on a growing set of maps (copies share
//! structure) and its model, and checks after every step:
//! * contents: every map equals its model (`get`, `items` in key order, `len`);
//! * shape: the trie is a well-formed big-endian Patricia trie (`check_shape`): each leaf's
//!   image is its key's, each branch bit a power of two above its children's, every key under a
//!   branch agrees with its prefix and goes left exactly when the bit is clear. Such a trie is
//!   canonical (one shape per key set), which `same` and `diff` rely on;
//! * the operations' results: `join` (equal values kept, `f` otherwise), `join_sides`,
//!   `merge_with` in all four modes, `remove_block`, `same`, `diff_keys` / `diff_upto`, and
//!   `TwoLevel`'s markers, `drop_key` / `drop_where`, `join_with[_keep]`, `copy_changes`;
//! * sharing: a write of the value already there, a removal of a missing key and the join of a
//!   map with its copy change nothing (the same root); a join allocates at most a path per key
//!   either side wrote since a common ancestor (the cost "what two maps differ in").
//!
//! `PROPTEST_CASES=N` runs more cases (default 64 here; CI runs the default).

use super::*;
use crate::laws::config;
use proptest::prelude::*;
use proptest::sample::Index;
use std::collections::{BTreeMap, BTreeSet};

// ------------------------------------------------------------------ shape and sharing

/// The trie's invariants (module doc); Err names the first broken one.
fn check_node<K: PKey, V>(t: &Tree<K, V>, above: Option<(u64, u64, bool)>, keys: &mut Vec<u64>) -> Result<(), String> {
    let fits = |h: u64| match above {
        None => true,
        Some((p, m, right)) => match_prefix(h, p, m) && zero_bit(h, m) != right,
    };
    match &**t {
        Node::Leaf { h, k, .. } => {
            if *h != k.key64() {
                return Err(format!("leaf image {h:#x} is not its key's {:#x}", k.key64()));
            }
            if !fits(*h) {
                return Err(format!("leaf {h:#x} outside its branch {above:x?}"));
            }
            keys.push(*h);
            Ok(())
        }
        Node::Branch { p, m, l, r } => {
            if m.count_ones() != 1 {
                return Err(format!("branch bit {m:#x} is not a power of two"));
            }
            if mask(*p, *m) != *p {
                return Err(format!("prefix {p:#x} has bits at or below {m:#x}"));
            }
            if let Some((q, n, _)) = above {
                if m >= &n || !match_prefix(*p, q, n) {
                    return Err(format!("branch {p:#x}/{m:#x} not below {q:#x}/{n:#x}"));
                }
            }
            check_node(l, Some((*p, *m, false)), keys)?;
            check_node(r, Some((*p, *m, true)), keys)
        }
    }
}

impl<K: PKey, V: MapValue> LayerMap<K, V> {
    /// Err when the trie is not a well-formed Patricia trie with keys in ascending order.
    pub(crate) fn check_shape(&self) -> Result<(), String> {
        let mut keys = Vec::new();
        if let Some(t) = &self.root {
            check_node(t, None, &mut keys)?;
        }
        if !keys.windows(2).all(|w| w[0] < w[1]) {
            return Err("keys out of order".into());
        }
        Ok(())
    }

    fn root_ptr(&self) -> Option<*const Node<K, V>> {
        self.root.as_ref().map(Arc::as_ptr)
    }

    /// The addresses of every node.
    fn node_ptrs(&self, out: &mut BTreeSet<usize>) {
        fn go<K, V>(t: &Tree<K, V>, out: &mut BTreeSet<usize>) {
            out.insert(Arc::as_ptr(t) as usize);
            if let Node::Branch { l, r, .. } = &**t {
                go(l, out);
                go(r, out);
            }
        }
        if let Some(t) = &self.root {
            go(t, out);
        }
    }
}

/// Nodes of `j` that are in neither `a` nor `b` (what a join allocated).
fn new_nodes(j: &LayerMap<u32, u32>, a: &LayerMap<u32, u32>, b: &LayerMap<u32, u32>) -> usize {
    let (mut old, mut got) = (BTreeSet::new(), BTreeSet::new());
    a.node_ptrs(&mut old);
    b.node_ptrs(&mut old);
    j.node_ptrs(&mut got);
    got.difference(&old).count()
}

// ------------------------------------------------------------------ LayerMap vs BTreeMap

type Model = BTreeMap<u32, u32>;

/// Keys: small and dense, clustered on high bits, or anywhere (deep tries, every merge case).
fn key() -> impl Strategy<Value = u32> {
    prop_oneof![0u32..12, (0u32..8).prop_map(|x| x.wrapping_mul(0x1000_0001)), any::<u32>()]
}

#[derive(Debug, Clone)]
enum MapOp {
    Insert(Index, u32, u32),
    Remove(Index, u32),
    Copy(Index),
    Join(Index, Index),
    JoinSides(Index, Index),
    /// `merge_with`: (one_side_identity, equal_identity)
    Merge(Index, Index, bool, bool),
    RemoveBlock(Index, u32, u32),
    Compare(Index, Index, usize),
}

fn map_op() -> impl Strategy<Value = MapOp> {
    prop_oneof![
        4 => (any::<Index>(), key(), 0u32..6).prop_map(|(i, k, v)| MapOp::Insert(i, k, v)),
        2 => (any::<Index>(), key()).prop_map(|(i, k)| MapOp::Remove(i, k)),
        2 => any::<Index>().prop_map(MapOp::Copy),
        2 => (any::<Index>(), any::<Index>()).prop_map(|(i, j)| MapOp::Join(i, j)),
        1 => (any::<Index>(), any::<Index>()).prop_map(|(i, j)| MapOp::JoinSides(i, j)),
        1 => (any::<Index>(), any::<Index>(), any::<bool>(), any::<bool>()).prop_map(|(i, j, o, e)| MapOp::Merge(i, j, o, e)),
        1 => (any::<Index>(), key(), prop::sample::select(vec![0u32, 3, 8, 31, 32])).prop_map(|(i, k, b)| MapOp::RemoveBlock(i, k, b)),
        2 => (any::<Index>(), any::<Index>(), 0usize..4).prop_map(|(i, j, l)| MapOp::Compare(i, j, l)),
    ]
}

/// The join's value of two unequal ones (not a lattice: the maps must call it, not assume it).
fn f(a: &u32, b: &u32) -> u32 {
    a.max(b) * 2 + 1
}

/// A one-sided entry under `join_sides` (None: dropped).
fn side(k: &u32, v: &u32) -> Option<u32> {
    (k % 3 != 0).then_some(v + 100)
}

/// `merge_with`'s result by the model; `drops`: its `both` drops a pair summing to 7 (`join`'s
/// never drops).
fn model_merge(a: &Model, b: &Model, one_side_identity: bool, equal_identity: bool, drops: bool) -> Model {
    let mut out = Model::new();
    for k in a.keys().chain(b.keys()) {
        let v = match (a.get(k), b.get(k)) {
            (Some(x), Some(y)) if equal_identity && x == y => Some(*x),
            (Some(x), Some(y)) => (!drops || x + y != 7).then(|| f(x, y)),
            (Some(x), None) if one_side_identity => Some(*x),
            (Some(x), None) => (x % 2 == 0).then_some(x + 1),
            (None, Some(y)) if one_side_identity => Some(*y),
            (None, Some(y)) => (y % 3 != 0).then_some(y + 2),
            (None, None) => unreachable!(),
        };
        if let Some(v) = v {
            out.insert(*k, v);
        }
    }
    out
}

fn diff_model(a: &Model, b: &Model) -> Vec<u32> {
    let ks: BTreeSet<u32> = a.keys().chain(b.keys()).copied().filter(|k| a.get(k) != b.get(k)).collect();
    ks.into_iter().collect()
}

fn agrees(m: &LayerMap<u32, u32>, model: &Model) -> Result<(), String> {
    m.check_shape()?;
    let items: Model = m.items().into_iter().collect();
    if &items != model {
        return Err(format!("contents {items:?}, model {model:?}"));
    }
    if m.len() != model.len() || m.keys() != model.keys().copied().collect::<Vec<_>>() {
        return Err("len / keys".into());
    }
    Ok(())
}

/// Runs one program of operations; Err says which law broke.
fn run_map_ops(ops: &[MapOp]) -> Result<(), String> {
    let mut maps: Vec<LayerMap<u32, u32>> = vec![LayerMap::new()];
    let mut models: Vec<Model> = vec![Model::new()];
    // (per map: the map it was copied from first, and the keys written on the way since)
    let mut lineage: Vec<(usize, BTreeSet<u32>)> = vec![(0, BTreeSet::new())];
    for op in ops {
        let n = maps.len();
        match op {
            MapOp::Insert(i, k, v) => {
                let i = i.index(n);
                let before = maps[i].root_ptr();
                let had = maps[i].get(k);
                maps[i].insert(*k, *v);
                models[i].insert(*k, *v);
                if had == Some(*v) && maps[i].root_ptr() != before {
                    return Err(format!("writing the value already there under {k} copied the map"));
                }
                if had != Some(*v) {
                    lineage[i].1.insert(*k);
                }
            }
            MapOp::Remove(i, k) => {
                let i = i.index(n);
                let before = maps[i].root_ptr();
                let got = maps[i].remove(k);
                if got != models[i].remove(k) {
                    return Err(format!("remove({k}) gave {got:?}"));
                }
                if got.is_none() && maps[i].root_ptr() != before {
                    return Err(format!("removing missing {k} changed the map"));
                }
                if got.is_some() {
                    lineage[i].1.insert(*k);
                }
            }
            MapOp::Copy(i) => {
                let i = i.index(n);
                maps.push(maps[i].clone());
                models.push(models[i].clone());
                lineage.push((i, BTreeSet::new()));
            }
            MapOp::Join(i, j) => {
                let (i, j) = (i.index(n), j.index(n));
                let joined = maps[i].join(&maps[j], &mut f);
                let exp = model_merge(&models[i], &models[j], true, true, false);
                // a join with an unchanged copy is the map itself
                let copy = maps[i].clone();
                if maps[i].join(&copy, &mut f).root_ptr() != maps[i].root_ptr() {
                    return Err("a map joined with its copy is a new map".into());
                }
                // sibling copies written since they shared: at most a path per written key
                if lineage[j].0 == i && i != j {
                    let wrote = lineage[j].1.len() + lineage[i].1.len();
                    let made = new_nodes(&joined, &maps[i], &maps[j]);
                    if made > 33 * wrote {
                        return Err(format!("join allocated {made} nodes for {wrote} written keys"));
                    }
                }
                maps.push(joined);
                models.push(exp);
                lineage.push((maps.len() - 1, BTreeSet::new()));
            }
            MapOp::JoinSides(i, j) => {
                let (i, j) = (i.index(n), j.index(n));
                let joined = maps[i].join_sides(&maps[j], &mut f, &mut side);
                let mut exp = Model::new();
                for k in models[i].keys().chain(models[j].keys()) {
                    let v = match (models[i].get(k), models[j].get(k)) {
                        (Some(x), Some(y)) => Some(if x == y { *x } else { f(x, y) }),
                        (Some(x), None) | (None, Some(x)) => side(k, x),
                        _ => unreachable!(),
                    };
                    if let Some(v) = v {
                        exp.insert(*k, v);
                    }
                }
                maps.push(joined);
                models.push(exp);
                lineage.push((maps.len() - 1, BTreeSet::new()));
            }
            MapOp::Merge(i, j, one, eq) => {
                let (i, j) = (i.index(n), j.index(n));
                let mut both = |_: &u32, x: &u32, y: &u32| (x + y != 7).then(|| f(x, y));
                let mut left = |_: &u32, x: &u32| (x % 2 == 0).then_some(x + 1);
                let mut right = |_: &u32, y: &u32| (y % 3 != 0).then_some(y + 2);
                let mut g = Merge { both: &mut both, left: &mut left, right: &mut right, one_side_identity: *one, equal_identity: *eq };
                let joined = maps[i].merge_with(&maps[j], &mut g);
                maps.push(joined);
                models.push(model_merge(&models[i], &models[j], *one, *eq, true));
                lineage.push((maps.len() - 1, BTreeSet::new()));
            }
            MapOp::RemoveBlock(i, k, bits) => {
                let i = i.index(n);
                maps[i].remove_block(k, *bits);
                let gone: Vec<u32> = models[i].keys().copied().filter(|x| (*x as u64) >> bits == (*k as u64) >> bits).collect();
                for x in gone {
                    models[i].remove(&x);
                    lineage[i].1.insert(x);
                }
            }
            MapOp::Compare(i, j, limit) => {
                let (i, j) = (i.index(n), j.index(n));
                if maps[i].same(&maps[j]) != (models[i] == models[j]) {
                    return Err("same disagrees".into());
                }
                let exp = diff_model(&models[i], &models[j]);
                let mut got = maps[i].diff_keys(&maps[j]);
                got.sort();
                if got != exp {
                    return Err(format!("diff_keys {got:?}, model {exp:?}"));
                }
                let upto = maps[i].diff_upto(&maps[j], *limit).map(|mut v| {
                    v.sort();
                    v
                });
                if upto != (exp.len() <= *limit).then_some(exp.clone()) {
                    return Err(format!("diff_upto({limit}) {upto:?}, model {exp:?}"));
                }
                if maps[i].written_upto(&maps[j], *limit).map(|v| v.len()) != (exp.len() <= *limit).then_some(exp.len()) {
                    return Err("written_upto of equal-means-same values differs from diff_upto".into());
                }
            }
        }
        for (m, model) in maps.iter().zip(&models) {
            agrees(m, model)?;
        }
    }
    Ok(())
}

// ------------------------------------------------------------------ TwoLevel vs nested BTreeMaps

/// Outer key -> (inner entries, the marker's value if marked).
type TwoModel = BTreeMap<u32, (BTreeMap<u32, u32>, Option<u32>)>;

#[derive(Debug, Clone)]
enum TwoOp {
    Set(Index, u32, u32, u32),
    Remove(Index, u32, u32),
    Mark(Index, u32, u32),
    DropKey(Index, u32),
    DropWhere(Index, u32, u32),
    Copy(Index),
    Join(Index, Index),
    JoinWith(Index, Index, bool),
    CopyChanges(Index, Index, Index),
    Compare(Index, Index, usize),
}

fn inner_key() -> impl Strategy<Value = u32> {
    prop_oneof![0u32..6, 0u32..(SENTINEL - 1)]
}

fn two_op() -> impl Strategy<Value = TwoOp> {
    prop_oneof![
        5 => (any::<Index>(), key(), inner_key(), 0u32..6).prop_map(|(i, k, ik, v)| TwoOp::Set(i, k, ik, v)),
        2 => (any::<Index>(), key(), inner_key()).prop_map(|(i, k, ik)| TwoOp::Remove(i, k, ik)),
        1 => (any::<Index>(), key(), 0u32..6).prop_map(|(i, k, v)| TwoOp::Mark(i, k, v)),
        1 => (any::<Index>(), key()).prop_map(|(i, k)| TwoOp::DropKey(i, k)),
        1 => (any::<Index>(), key(), 1u32..4).prop_map(|(i, k, m)| TwoOp::DropWhere(i, k, m)),
        2 => any::<Index>().prop_map(TwoOp::Copy),
        1 => (any::<Index>(), any::<Index>()).prop_map(|(i, j)| TwoOp::Join(i, j)),
        2 => (any::<Index>(), any::<Index>(), any::<bool>()).prop_map(|(i, j, keep)| TwoOp::JoinWith(i, j, keep)),
        1 => (any::<Index>(), any::<Index>(), any::<Index>()).prop_map(|(i, j, b)| TwoOp::CopyChanges(i, j, b)),
        2 => (any::<Index>(), any::<Index>(), 0usize..4).prop_map(|(i, j, l)| TwoOp::Compare(i, j, l)),
    ]
}

fn two_agrees(t: &TwoLevel<u32, u32, u32>, model: &TwoModel) -> Result<(), String> {
    t.m.check_shape()?;
    let got: BTreeMap<u32, BTreeMap<u32, u32>> = t.items().into_iter().map(|(k, d)| (k, d.into_iter().collect())).collect();
    let exp: BTreeMap<u32, BTreeMap<u32, u32>> = model.iter().filter(|(_, (d, _))| !d.is_empty()).map(|(k, (d, _))| (*k, d.clone())).collect();
    if got != exp {
        return Err(format!("items {got:?}, model {exp:?}"));
    }
    for (k, (d, mark)) in model {
        if t.has_key(k) != (!d.is_empty() || mark.is_some()) {
            return Err(format!("has_key({k})"));
        }
        let inner: Option<BTreeMap<u32, u32>> = t.inner(k).map(|m| m.into_iter().collect());
        if inner != t.has_key(k).then(|| d.clone()) {
            return Err(format!("inner({k}) {inner:?}"));
        }
        if t.for_key(k) != d.iter().map(|(a, b)| (*a, *b)).collect::<Vec<_>>() {
            return Err(format!("for_key({k})"));
        }
    }
    Ok(())
}

/// Every (outer, inner) entry, the marker as inner key `SENTINEL`: what the trie holds.
fn flat(m: &TwoModel) -> BTreeMap<(u32, u32), u32> {
    let mut out = BTreeMap::new();
    for (k, (d, mark)) in m {
        for (ik, v) in d {
            out.insert((*k, *ik), *v);
        }
        if let Some(v) = mark {
            out.insert((*k, SENTINEL), *v);
        }
    }
    out
}

fn unflat(f: BTreeMap<(u32, u32), u32>) -> TwoModel {
    let mut out = TwoModel::new();
    for ((k, ik), v) in f {
        let e = out.entry(k).or_default();
        if ik == SENTINEL {
            e.1 = Some(v);
        } else {
            e.0.insert(ik, v);
        }
    }
    out
}

fn tidy(m: &mut TwoModel) {
    m.retain(|_, (d, mark)| !d.is_empty() || mark.is_some());
}

fn run_two_ops(ops: &[TwoOp]) -> Result<(), String> {
    let mut maps: Vec<TwoLevel<u32, u32, u32>> = vec![TwoLevel::new()];
    let mut models: Vec<TwoModel> = vec![TwoModel::new()];
    for op in ops {
        let n = maps.len();
        match op {
            TwoOp::Set(i, k, ik, v) => {
                let i = i.index(n);
                maps[i].set(k, *ik, *v);
                models[i].entry(*k).or_default().0.insert(*ik, *v);
            }
            TwoOp::Remove(i, k, ik) => {
                let i = i.index(n);
                let got = maps[i].remove(k, ik);
                let exp = models[i].get_mut(k).and_then(|e| e.0.remove(ik));
                if got != exp {
                    return Err(format!("remove({k}, {ik}) gave {got:?}"));
                }
            }
            TwoOp::Mark(i, k, v) => {
                let i = i.index(n);
                maps[i].mark(k, *v);
                let e = models[i].entry(*k).or_default();
                if e.0.is_empty() && e.1.is_none() {
                    e.1 = Some(*v);
                }
            }
            TwoOp::DropKey(i, k) => {
                let i = i.index(n);
                maps[i].drop_key(k);
                models[i].remove(k);
            }
            TwoOp::DropWhere(i, k, m) => {
                let i = i.index(n);
                maps[i].drop_where(k, |ik| ik % m == 0);
                if let Some(e) = models[i].get_mut(k) {
                    e.0.retain(|ik, _| ik % m != 0);
                    e.1 = None;
                }
            }
            TwoOp::Copy(i) => {
                let i = i.index(n);
                maps.push(maps[i].clone());
                models.push(models[i].clone());
            }
            TwoOp::Join(i, j) => {
                let (i, j) = (i.index(n), j.index(n));
                maps.push(maps[i].join(&maps[j], &mut f));
                let (a, b) = (flat(&models[i]), flat(&models[j]));
                let mut out = a.clone();
                for (p, y) in b {
                    let v = match a.get(&p) {
                        Some(x) if *x != y => f(x, &y),
                        _ => y,
                    };
                    out.insert(p, v);
                }
                models.push(unflat(out));
            }
            TwoOp::JoinWith(i, j, keep) => {
                let (i, j) = (i.index(n), j.index(n));
                // (keep_whole: outer keys divisible by 2, when `keep`)
                let keep_whole = |k: &u32| *keep && k % 2 == 0;
                let joined = maps[i].join_with_keep(&maps[j], &mut f, &mut |v| v + 1000, &keep_whole);
                let (a, b) = (flat(&models[i]), flat(&models[j]));
                let mut out = BTreeMap::new();
                for p in a.keys().chain(b.keys()) {
                    let other_lacks = |m: &TwoModel| !m.contains_key(&p.0);
                    let v = match (a.get(p), b.get(p)) {
                        (Some(x), Some(y)) => if x == y { *x } else { f(x, y) },
                        (Some(x), None) => if keep_whole(&p.0) && other_lacks(&models[j]) { *x } else { x + 1000 },
                        (None, Some(y)) => if keep_whole(&p.0) && other_lacks(&models[i]) { *y } else { y + 1000 },
                        _ => unreachable!(),
                    };
                    out.insert(*p, v);
                }
                maps.push(joined);
                models.push(unflat(out));
            }
            TwoOp::CopyChanges(i, j, b) => {
                let (i, j, b) = (i.index(n), j.index(n), b.index(n));
                let (o, base) = (maps[j].clone(), maps[b].clone());
                maps[i].copy_changes(&o, &base);
                let (fo, fb) = (flat(&models[j]), flat(&models[b]));
                let mut fi = flat(&models[i]);
                for p in fo.keys().chain(fb.keys()) {
                    if fo.get(p) != fb.get(p) {
                        match fo.get(p) {
                            Some(v) => fi.insert(*p, *v),
                            None => fi.remove(p),
                        };
                    }
                }
                models[i] = unflat(fi);
            }
            TwoOp::Compare(i, j, limit) => {
                let (i, j) = (i.index(n), j.index(n));
                let (a, b) = (flat(&models[i]), flat(&models[j]));
                if maps[i].same(&maps[j]) != (a == b) {
                    return Err("same disagrees".into());
                }
                let all: BTreeSet<(u32, u32)> = a.keys().chain(b.keys()).copied().filter(|p| a.get(p) != b.get(p)).collect();
                let exp: Vec<(u32, u32)> = all.iter().copied().filter(|p| p.1 != SENTINEL).collect();
                let mut got = maps[i].diff_keys(&maps[j]);
                got.sort();
                if got != exp {
                    return Err(format!("diff_keys {got:?}, model {exp:?}"));
                }
                // (the limit counts markers too: a conservative None)
                let upto = maps[i].diff_upto(&maps[j], *limit).map(|mut v| {
                    v.sort();
                    v
                });
                if upto != (all.len() <= *limit).then_some(exp) {
                    return Err(format!("diff_upto({limit}) {upto:?}"));
                }
            }
        }
        for m in models.iter_mut() {
            tidy(m);
        }
        for (t, model) in maps.iter().zip(&models) {
            two_agrees(t, model)?;
        }
    }
    Ok(())
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn layer_map_agrees_with_btreemap(ops in prop::collection::vec(map_op(), 1..80)) {
        if let Err(e) = run_map_ops(&ops) {
            prop_assert!(false, "{}", e);
        }
    }

    #[test]
    fn two_level_agrees_with_nested_btreemaps(ops in prop::collection::vec(two_op(), 1..80)) {
        if let Err(e) = run_two_ops(&ops) {
            prop_assert!(false, "{}", e);
        }
    }

    /// Building a key set in any order gives the same trie (canonical shapes: `same` holds and
    /// a join of the two allocates only where their paths are not shared).
    #[test]
    fn tries_are_canonical(keys in prop::collection::vec(key(), 0..40), seed in any::<u64>()) {
        let mut a: LayerMap<u32, u32> = LayerMap::new();
        for k in &keys {
            a.insert(*k, k % 5);
        }
        let mut shuffled = keys.clone();
        let mut s = seed | 1;
        for i in (1..shuffled.len()).rev() {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            shuffled.swap(i, (s % (i as u64 + 1)) as usize);
        }
        let mut b: LayerMap<u32, u32> = LayerMap::new();
        for k in &shuffled {
            b.insert(*k, k % 5);
        }
        prop_assert!(a.check_shape().is_ok() && b.check_shape().is_ok());
        prop_assert!(a.same(&b));
        fn shape(t: &Option<Tree<u32, u32>>) -> String {
            match t.as_deref() {
                None => "-".into(),
                Some(Node::Leaf { h, .. }) => format!("{h}"),
                Some(Node::Branch { p, m, l, r }) => format!("({p:x}/{m:x} {} {})", shape(&Some(l.clone())), shape(&Some(r.clone()))),
            }
        }
        prop_assert_eq!(shape(&a.root), shape(&b.root));
    }
}

