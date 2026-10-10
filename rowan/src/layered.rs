//! Persistent-style maps for the fork/join pattern (port of `rowan/layered.py`).
//!
//! `LayerMap` is a chain of frozen layers with a private mutable top: `copy()` is O(1),
//! `join` / `same` / `diff_keys` cost O(#keys written since the common ancestor), reads
//! cost O(depth) (the chain is flattened every `FLATTEN_EVERY` layers).  `TwoLevel` adds a
//! copy-on-write inner map per key (object slots, source requirements).

use std::hash::Hash;
use std::sync::Arc;

use rustc_hash::FxHashMap;

pub const FLATTEN_EVERY: u32 = 10;

/// A layer: `None` values are tombstones (DELETED).
struct Node<K, V> {
    parent: Option<Arc<Node<K, V>>>,
    data: FxHashMap<K, Option<V>>,
    depth: u32,
    /// materialised contents; only on flattened (root) nodes, which have no parent
    flat: Option<FxHashMap<K, V>>,
}

pub struct LayerMap<K, V> {
    node: Option<Arc<Node<K, V>>>,
    data: FxHashMap<K, Option<V>>,
}

impl<K: Clone + Eq + Hash, V: Clone + PartialEq> Default for LayerMap<K, V> {
    fn default() -> Self {
        LayerMap { node: None, data: FxHashMap::default() }
    }
}

enum Found<V> {
    Missing,
    Deleted,
    Value(V),
}

impl<K: Clone + Eq + Hash, V: Clone + PartialEq> LayerMap<K, V> {
    pub fn new() -> Self {
        Self::default()
    }

    fn get_from(mut n: Option<&Arc<Node<K, V>>>, k: &K) -> Found<V> {
        while let Some(node) = n {
            if let Some(flat) = &node.flat {
                return match flat.get(k) {
                    Some(v) => Found::Value(v.clone()),
                    None => Found::Missing,
                };
            }
            if let Some(v) = node.data.get(k) {
                return match v {
                    Some(v) => Found::Value(v.clone()),
                    None => Found::Deleted,
                };
            }
            n = node.parent.as_ref();
        }
        Found::Missing
    }

    fn lookup(&self, k: &K) -> Found<V> {
        if let Some(v) = self.data.get(k) {
            return match v {
                Some(v) => Found::Value(v.clone()),
                None => Found::Deleted,
            };
        }
        Self::get_from(self.node.as_ref(), k)
    }

    pub fn get(&self, k: &K) -> Option<V> {
        match self.lookup(k) {
            Found::Value(v) => Some(v),
            _ => None,
        }
    }

    pub fn contains_key(&self, k: &K) -> bool {
        matches!(self.lookup(k), Found::Value(_))
    }

    /// A plain map with the full contents (O(n)).
    pub fn materialise(&self) -> FxHashMap<K, V> {
        let mut chain: Vec<&Arc<Node<K, V>>> = Vec::new();
        let mut n = self.node.as_ref();
        let mut out: FxHashMap<K, V> = FxHashMap::default();
        while let Some(node) = n {
            if let Some(flat) = &node.flat {
                out = flat.clone();
                break;
            }
            chain.push(node);
            n = node.parent.as_ref();
        }
        for node in chain.iter().rev() {
            for (k, v) in node.data.iter() {
                match v {
                    Some(v) => {
                        out.insert(k.clone(), v.clone());
                    }
                    None => {
                        out.remove(k);
                    }
                }
            }
        }
        for (k, v) in self.data.iter() {
            match v {
                Some(v) => {
                    out.insert(k.clone(), v.clone());
                }
                None => {
                    out.remove(k);
                }
            }
        }
        out
    }

    pub fn items(&self) -> Vec<(K, V)> {
        self.materialise().into_iter().collect()
    }

    pub fn keys(&self) -> Vec<K> {
        self.materialise().into_keys().collect()
    }

    pub fn len(&self) -> usize {
        self.materialise().len()
    }

    // ----------------------------------------------------------------- writes
    pub fn insert(&mut self, k: K, v: V) {
        self.data.insert(k, Some(v));
    }

    pub fn remove(&mut self, k: &K) -> Option<V> {
        match self.lookup(k) {
            Found::Value(v) => {
                self.data.insert(k.clone(), None);
                Some(v)
            }
            _ => None,
        }
    }

    // ------------------------------------------------------------ fork / freeze
    /// Turn the private top into a frozen node; returns the node to fork from.
    fn freeze(&mut self) -> Option<Arc<Node<K, V>>> {
        if self.data.is_empty() {
            return self.node.clone();
        }
        let depth = self.node.as_ref().map(|n| n.depth + 1).unwrap_or(1);
        let data = std::mem::take(&mut self.data);
        let node = if depth % FLATTEN_EVERY == 0 {
            // materialise the contents and make this node a root
            let tmp = LayerMap { node: self.node.clone(), data };
            let flat = tmp.materialise();
            Arc::new(Node { parent: None, data: FxHashMap::default(), depth: 0, flat: Some(flat) })
        } else {
            Arc::new(Node { parent: self.node.clone(), data, depth, flat: None })
        };
        self.node = Some(node.clone());
        Some(node)
    }

    pub fn copy(&mut self) -> LayerMap<K, V> {
        LayerMap { node: self.freeze(), data: FxHashMap::default() }
    }

    /// A copy made from a shared reference: the private top is cloned into a frozen node
    /// of the copy (self keeps writing into its own top).
    pub fn copy_ref(&self) -> LayerMap<K, V> {
        if self.data.is_empty() {
            return LayerMap { node: self.node.clone(), data: FxHashMap::default() };
        }
        let depth = self.node.as_ref().map(|n| n.depth + 1).unwrap_or(1);
        let node = Arc::new(Node { parent: self.node.clone(), data: self.data.clone(), depth, flat: None });
        LayerMap { node: Some(node), data: FxHashMap::default() }
    }

    /// Freeze the private top (so that values referenced from here on are shared).
    pub fn share(&mut self) {
        self.freeze();
    }

    // ----------------------------------------------------------- join / compare
    /// Common ancestor node of two chains; `(None, true)` means both chains are empty,
    /// `(None, false)` means there is no common ancestor.
    fn ancestor(a: Option<&Arc<Node<K, V>>>, b: Option<&Arc<Node<K, V>>>) -> (Option<Arc<Node<K, V>>>, bool) {
        if a.is_none() && b.is_none() {
            return (None, true);
        }
        let mut ids: Vec<*const Node<K, V>> = Vec::new();
        let mut n = a;
        while let Some(node) = n {
            ids.push(Arc::as_ptr(node));
            n = node.parent.as_ref();
        }
        let mut n = b;
        while let Some(node) = n {
            if ids.contains(&Arc::as_ptr(node)) {
                return (Some(node.clone()), true);
            }
            n = node.parent.as_ref();
        }
        (None, false)
    }

    /// {key: value} for every key written above `anc` (topmost write wins; tombstones kept).
    fn values_above(&self, anc: Option<&Arc<Node<K, V>>>) -> FxHashMap<K, Option<V>> {
        let mut layers: Vec<&FxHashMap<K, Option<V>>> = vec![&self.data];
        let mut n = self.node.as_ref();
        while let Some(node) = n {
            if let Some(a) = anc {
                if Arc::ptr_eq(node, a) {
                    break;
                }
            }
            layers.push(&node.data);
            n = node.parent.as_ref();
        }
        if layers.len() == 1 {
            return self.data.clone();
        }
        let mut out: FxHashMap<K, Option<V>> = FxHashMap::default();
        for d in layers.iter().rev() {
            for (k, v) in d.iter() {
                out.insert(k.clone(), v.clone());
            }
        }
        out
    }

    fn resolve(va: Option<&Option<V>>, anc: Option<&Arc<Node<K, V>>>, k: &K) -> Option<V> {
        match va {
            Some(Some(v)) => Some(v.clone()),
            Some(None) => None,
            None => match Self::get_from(anc, k) {
                Found::Value(v) => Some(v),
                _ => None,
            },
        }
    }

    /// Keys on one side only are kept; keys on both sides are combined with f (unless identical).
    pub fn join(&self, o: &LayerMap<K, V>, f: &mut dyn FnMut(&V, &V) -> V) -> LayerMap<K, V> {
        let (anc, ok) = Self::ancestor(self.node.as_ref(), o.node.as_ref());
        if !ok {
            let mut out = self.materialise();
            for (k, v) in o.materialise() {
                match out.get(&k) {
                    None => {
                        out.insert(k, v);
                    }
                    Some(cur) => {
                        if *cur != v {
                            let nv = f(cur, &v);
                            out.insert(k, nv);
                        }
                    }
                }
            }
            return LayerMap { node: None, data: out.into_iter().map(|(k, v)| (k, Some(v))).collect() };
        }
        let va_map = self.values_above(anc.as_ref());
        let vb_map = o.values_above(anc.as_ref());
        let mut d: FxHashMap<K, Option<V>> = FxHashMap::default();
        let keys: Vec<K> = va_map.keys().chain(vb_map.keys().filter(|k| !va_map.contains_key(*k))).cloned().collect();
        for k in keys {
            let va = Self::resolve(va_map.get(&k), anc.as_ref(), &k);
            let vb = Self::resolve(vb_map.get(&k), anc.as_ref(), &k);
            let nv = match (va, vb) {
                (None, None) => None,
                (None, Some(b)) => Some(b),
                (Some(a), None) => Some(a),
                (Some(a), Some(b)) => {
                    if a == b {
                        Some(a)
                    } else {
                        Some(f(&a, &b))
                    }
                }
            };
            d.insert(k, nv);
        }
        LayerMap { node: anc, data: d }
    }

    pub fn same(&self, o: &LayerMap<K, V>) -> bool {
        let (anc, ok) = Self::ancestor(self.node.as_ref(), o.node.as_ref());
        if !ok {
            return self.materialise() == o.materialise();
        }
        let va_map = self.values_above(anc.as_ref());
        let vb_map = o.values_above(anc.as_ref());
        for k in va_map.keys().chain(vb_map.keys().filter(|k| !va_map.contains_key(*k))) {
            let va = Self::resolve(va_map.get(k), anc.as_ref(), k);
            let vb = Self::resolve(vb_map.get(k), anc.as_ref(), k);
            if va != vb {
                return false;
            }
        }
        true
    }

    /// Keys whose value differs between self and base.
    pub fn diff_keys(&self, base: &LayerMap<K, V>) -> Vec<K> {
        let (anc, ok) = Self::ancestor(self.node.as_ref(), base.node.as_ref());
        if !ok {
            let a = self.materialise();
            let b = base.materialise();
            let mut out = Vec::new();
            for k in a.keys().chain(b.keys().filter(|k| !a.contains_key(*k))) {
                if a.get(k) != b.get(k) {
                    out.push(k.clone());
                }
            }
            return out;
        }
        let va_map = self.values_above(anc.as_ref());
        let vb_map = base.values_above(anc.as_ref());
        let mut out = Vec::new();
        for k in va_map.keys().chain(vb_map.keys().filter(|k| !va_map.contains_key(*k))) {
            let va = Self::resolve(va_map.get(k), anc.as_ref(), k);
            let vb = Self::resolve(vb_map.get(k), anc.as_ref(), k);
            if va != vb {
                out.push(k.clone());
            }
        }
        out
    }
}

/// key -> {inner key -> value}, with copy-on-write inner maps over a LayerMap.
pub struct TwoLevel<K, IK, V> {
    pub m: LayerMap<K, Arc<FxHashMap<IK, V>>>,
}

impl<K: Clone + Eq + Hash, IK: Clone + Eq + Hash, V: Clone + PartialEq> Default for TwoLevel<K, IK, V> {
    fn default() -> Self {
        TwoLevel { m: LayerMap::new() }
    }
}

impl<K: Clone + Eq + Hash, IK: Clone + Eq + Hash, V: Clone + PartialEq> TwoLevel<K, IK, V> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn copy(&mut self) -> Self {
        TwoLevel { m: self.m.copy() }
    }

    pub fn copy_ref(&self) -> Self {
        TwoLevel { m: self.m.copy_ref() }
    }

    pub fn inner(&self, key: &K) -> Option<Arc<FxHashMap<IK, V>>> {
        self.m.get(key)
    }

    /// Mutable access to the inner map of `key`, copying it when it is shared.
    pub fn writable(&mut self, key: &K) -> &mut FxHashMap<IK, V> {
        if !self.m.data.contains_key(key) || matches!(self.m.data.get(key), Some(None)) {
            let cur = self.m.get(key).unwrap_or_else(|| Arc::new(FxHashMap::default()));
            self.m.data.insert(key.clone(), Some(cur));
        }
        let slot = self.m.data.get_mut(key).unwrap().as_mut().unwrap();
        Arc::make_mut(slot)
    }

    pub fn drop_key(&mut self, key: &K) {
        self.m.remove(key);
    }

    pub fn outer_keys(&self) -> Vec<K> {
        self.m.keys()
    }

    pub fn items(&self) -> Vec<(K, Arc<FxHashMap<IK, V>>)> {
        self.m.items().into_iter().filter(|(_, d)| !d.is_empty()).collect()
    }

    pub fn get(&self, key: &K, ik: &IK) -> Option<V> {
        self.m.get(key).and_then(|d| d.get(ik).cloned())
    }

    pub fn contains(&self, key: &K, ik: &IK) -> bool {
        self.m.get(key).map(|d| d.contains_key(ik)).unwrap_or(false)
    }

    pub fn set(&mut self, key: &K, ik: IK, v: V) {
        self.writable(key).insert(ik, v);
    }

    pub fn remove(&mut self, key: &K, ik: &IK) -> Option<V> {
        if !self.contains(key, ik) {
            return None;
        }
        let d = self.writable(key);
        let v = d.remove(ik);
        if d.is_empty() {
            self.drop_key(key);
        }
        v
    }

    /// The inner entries of one key, as a vector (sorted by the caller if needed).
    pub fn for_key(&self, key: &K) -> Vec<(IK, V)> {
        match self.m.get(key) {
            Some(d) => d.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            None => Vec::new(),
        }
    }

    pub fn drop_where(&mut self, key: &K, mut pred: impl FnMut(&IK) -> bool) {
        let Some(d) = self.m.get(key) else { return };
        if !d.keys().any(|ik| pred(ik)) {
            return;
        }
        let w = self.writable(key);
        w.retain(|ik, _| !pred(ik));
        if w.is_empty() {
            self.drop_key(key);
        }
    }

    /// Freeze the private top so that inner maps referenced from a join result are never
    /// mutated in place afterwards.
    fn share(&mut self) {
        self.m.share();
    }

    pub fn join(&mut self, o: &mut Self, f: &mut dyn FnMut(&V, &V) -> V) -> Self {
        self.share();
        o.share();
        let mut merge = |d1: &Arc<FxHashMap<IK, V>>, d2: &Arc<FxHashMap<IK, V>>| -> Arc<FxHashMap<IK, V>> {
            let mut merged: Option<FxHashMap<IK, V>> = None;
            for (nk, v) in d2.iter() {
                match d1.get(nk) {
                    None => merged.get_or_insert_with(|| (**d1).clone()).insert(nk.clone(), v.clone()),
                    Some(cur) => {
                        if cur != v {
                            let nv = f(cur, v);
                            merged.get_or_insert_with(|| (**d1).clone()).insert(nk.clone(), nv)
                        } else {
                            None
                        }
                    }
                };
            }
            match merged {
                Some(m) => Arc::new(m),
                None => d1.clone(),
            }
        };
        TwoLevel { m: self.m.join(&o.m, &mut merge) }
    }

    /// Join where inner entries present on one side only are transformed by `one_side`.
    pub fn join_with(&mut self, o: &mut Self, both: &mut dyn FnMut(&V, &V) -> V, one_side: &mut dyn FnMut(&V) -> V) -> Self {
        self.join_with_keep(o, both, one_side, &|_| false)
    }

    /// `join_with`, but an outer key whose whole inner map one side lacks keeps the other
    /// side's values unchanged when `keep_whole` says so.
    pub fn join_with_keep(&mut self, o: &mut Self, both: &mut dyn FnMut(&V, &V) -> V, one_side: &mut dyn FnMut(&V) -> V, keep_whole: &dyn Fn(&K) -> bool) -> Self {
        self.share();
        o.share();
        let a = &self.m;
        let b = &o.m;
        let (anc, ok) = LayerMap::ancestor(a.node.as_ref(), b.node.as_ref());
        let (va_map, vb_map): (FxHashMap<K, Option<Arc<FxHashMap<IK, V>>>>, FxHashMap<K, Option<Arc<FxHashMap<IK, V>>>>) = if ok {
            (a.values_above(anc.as_ref()), b.values_above(anc.as_ref()))
        } else {
            (
                a.materialise().into_iter().map(|(k, v)| (k, Some(v))).collect(),
                b.materialise().into_iter().map(|(k, v)| (k, Some(v))).collect(),
            )
        };
        let mut new: LayerMap<K, Arc<FxHashMap<IK, V>>> = LayerMap { node: if ok { anc.clone() } else { None }, data: FxHashMap::default() };
        let keys: Vec<K> = va_map.keys().chain(vb_map.keys().filter(|k| !va_map.contains_key(*k))).cloned().collect();
        for k in keys {
            let da = if ok { LayerMap::resolve(va_map.get(&k), anc.as_ref(), &k) } else { va_map.get(&k).cloned().flatten() };
            let db = if ok { LayerMap::resolve(vb_map.get(&k), anc.as_ref(), &k) } else { vb_map.get(&k).cloned().flatten() };
            match (da, db) {
                (None, None) => continue,
                (Some(x), Some(y)) if Arc::ptr_eq(&x, &y) => {
                    // the same inner map on both sides: nothing to merge, but it is only
                    // implicit when the common ancestor itself holds that map (copies made
                    // with `copy_ref` can carry one map in two distinct layers above it)
                    let inherited = ok && matches!(LayerMap::get_from(anc.as_ref(), &k), Found::Value(v) if Arc::ptr_eq(&v, &x));
                    if !inherited {
                        new.data.insert(k, Some(x));
                    }
                }
                (Some(d), None) | (None, Some(d)) if keep_whole(&k) => {
                    new.data.insert(k, Some(d));
                }
                (Some(d), None) | (None, Some(d)) => {
                    let m: FxHashMap<IK, V> = d.iter().map(|(nk, v)| (nk.clone(), one_side(v))).collect();
                    new.data.insert(k, Some(Arc::new(m)));
                }
                (Some(da), Some(db)) => {
                    let mut merged: FxHashMap<IK, V> = FxHashMap::default();
                    for nk in da.keys().chain(db.keys().filter(|nk| !da.contains_key(*nk))) {
                        let v = match (da.get(nk), db.get(nk)) {
                            (Some(va), None) => one_side(va),
                            (None, Some(vb)) => one_side(vb),
                            (Some(va), Some(vb)) => {
                                if va == vb {
                                    va.clone()
                                } else {
                                    both(va, vb)
                                }
                            }
                            (None, None) => unreachable!(),
                        };
                        merged.insert(nk.clone(), v);
                    }
                    new.data.insert(k, Some(Arc::new(merged)));
                }
            }
        }
        TwoLevel { m: new }
    }

    pub fn same(&self, o: &Self) -> bool {
        self.m.same(&o.m)
    }

    pub fn diff_keys(&self, base: &Self) -> Vec<(K, IK)> {
        let mut out = Vec::new();
        for key in self.m.diff_keys(&base.m) {
            let d = self.m.get(&key);
            let b = base.m.get(&key);
            let empty = Arc::new(FxHashMap::default());
            let d = d.as_ref().unwrap_or(&empty);
            let b = b.as_ref().unwrap_or(&empty);
            for nk in d.keys().chain(b.keys().filter(|nk| !d.contains_key(*nk))) {
                if d.get(nk) != b.get(nk) {
                    out.push((key.clone(), nk.clone()));
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_basics() {
        let mut m: LayerMap<u32, u32> = LayerMap::new();
        m.insert(1, 10);
        m.insert(2, 20);
        let mut a = m.copy();
        let mut b = m.copy();
        a.insert(1, 11);
        b.insert(3, 30);
        b.remove(&2);
        let j = a.join(&b, &mut |x, y| x + y);
        assert_eq!(j.get(&1), Some(21));
        assert_eq!(j.get(&2), Some(20));
        assert_eq!(j.get(&3), Some(30));
        assert!(!a.same(&b));
        let mut d = a.diff_keys(&m);
        d.sort();
        assert_eq!(d, vec![1]);
        let mut d = b.diff_keys(&m);
        d.sort();
        assert_eq!(d, vec![2, 3]);
        let ac = a.copy(); assert!(a.same(&ac));
    }

    #[test]
    fn flatten() {
        let mut m: LayerMap<u32, u32> = LayerMap::new();
        for i in 0..40 {
            m.insert(i, i);
            m = m.copy();
        }
        assert_eq!(m.get(&3), Some(3));
        assert_eq!(m.len(), 40);
    }

    #[test]
    fn two_level() {
        let mut t: TwoLevel<u32, u32, u32> = TwoLevel::new();
        t.set(&1, 1, 5);
        let mut c = t.copy();
        c.set(&1, 2, 6);
        assert_eq!(t.get(&1, &2), None);
        assert_eq!(c.get(&1, &2), Some(6));
        let j = t.join(&mut c, &mut |a, b| *a.max(b));
        assert_eq!(j.get(&1, &1), Some(5));
        assert_eq!(j.get(&1, &2), Some(6));
        let d = c.diff_keys(&t);
        assert_eq!(d, vec![(1, 2)]);
    }
}

#[cfg(test)]
mod row_tests {
    use super::*;
    use crate::domain::{Ns, Row};
    use crate::intern::key_str_lit;

    #[test]
    fn join_rows_after_copy() {
        let mut m: LayerMap<(u32, Ns), Row> = LayerMap::new();
        let r0 = Row::new().store(key_str_lit("id"), true);
        m.insert((3, Ns::Item), r0.clone());
        m.insert((1, Ns::Attr), Row::new());
        let mut a = m.copy();
        let mut b = m.copy();
        let r1 = r0.store(key_str_lit("role"), true);
        a.insert((3, Ns::Item), r1.clone());
        assert_ne!(r0, r1);
        let j = a.join(&b, &mut |x, y| x.join(y));
        let got = j.get(&(3, Ns::Item)).unwrap();
        assert!(got.present.contains(key_str_lit("role")), "got {}", got);
        let _ = &mut b;
    }
}

#[cfg(test)]
mod prop_tests {
    //! Randomised differential tests of the layered maps against a naive model.
    use super::*;

    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            self.0 >> 33
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
    }

    type Model = FxHashMap<u32, FxHashMap<u32, u32>>;

    fn model_of(t: &TwoLevel<u32, u32, u32>) -> Model {
        t.m.materialise().into_iter().map(|(k, d)| (k, (*d).clone())).collect()
    }

    fn model_join_with(a: &Model, b: &Model) -> Model {
        let mut out: Model = FxHashMap::default();
        for k in a.keys().chain(b.keys()) {
            let (da, db) = (a.get(k), b.get(k));
            let mut m: FxHashMap<u32, u32> = FxHashMap::default();
            match (da, db) {
                (Some(x), None) | (None, Some(x)) => {
                    for (nk, v) in x {
                        m.insert(*nk, v + 1000);
                    }
                }
                (Some(x), Some(y)) => {
                    for nk in x.keys().chain(y.keys()) {
                        let v = match (x.get(nk), y.get(nk)) {
                            (Some(p), None) | (None, Some(p)) => p + 1000,
                            (Some(p), Some(q)) => {
                                if p == q {
                                    *p
                                } else {
                                    p.max(q) + 1
                                }
                            }
                            _ => unreachable!(),
                        };
                        m.insert(*nk, v);
                    }
                }
                _ => {}
            }
            out.insert(*k, m);
        }
        out
    }

    #[test]
    fn two_level_join_with_matches_model() {
        let mut rng = Lcg(42);
        for round in 0..300 {
            let mut maps: Vec<TwoLevel<u32, u32, u32>> = vec![TwoLevel::new()];
            let mut models: Vec<Model> = vec![FxHashMap::default()];
            let nops = 5 + rng.below(40) as usize;
            for _ in 0..nops {
                let i = rng.below(maps.len() as u64) as usize;
                match rng.below(6) {
                    0 | 1 => {
                        let k = rng.below(4) as u32;
                        let ik = rng.below(4) as u32;
                        let v = rng.below(50) as u32;
                        maps[i].set(&k, ik, v);
                        models[i].entry(k).or_default().insert(ik, v);
                    }
                    2 => {
                        let c = maps[i].copy();
                        let m = models[i].clone();
                        maps.push(c);
                        models.push(m);
                    }
                    3 => {
                        let k = rng.below(4) as u32;
                        let ik = rng.below(4) as u32;
                        maps[i].remove(&k, &ik);
                        if let Some(d) = models[i].get_mut(&k) {
                            d.remove(&ik);
                            if d.is_empty() {
                                models[i].remove(&k);
                            }
                        }
                    }
                    4 => {
                        if maps.len() > 1 {
                            let j = rng.below(maps.len() as u64) as usize;
                            if j != i {
                                let (a, b) = if i < j {
                                    let (l, r) = maps.split_at_mut(j);
                                    (&mut l[i], &mut r[0])
                                } else {
                                    let (l, r) = maps.split_at_mut(i);
                                    (&mut r[0], &mut l[j])
                                };
                                let joined = a.join_with(b, &mut |p, q| p.max(q) + 1, &mut |p| p + 1000);
                                let exp = model_join_with(&models[i], &models[j]);
                                let got = model_of(&joined);
                                let exp2: Model = exp.into_iter().filter(|(_, d)| !d.is_empty()).collect();
                                assert_eq!(got, exp2, "round {} join_with mismatch", round);
                                maps.push(joined);
                                models.push(exp2);
                            }
                        }
                    }
                    _ => {
                        let k = rng.below(4) as u32;
                        let got = maps[i].inner(&k).map(|d| (*d).clone()).unwrap_or_default();
                        let exp = models[i].get(&k).cloned().unwrap_or_default();
                        assert_eq!(got, exp, "round {} get mismatch", round);
                    }
                }
            }
            for (m, model) in maps.iter().zip(models.iter()) {
                let got: Model = model_of(m).into_iter().filter(|(_, d)| !d.is_empty()).collect();
                let exp: Model = model.clone().into_iter().filter(|(_, d)| !d.is_empty()).collect();
                assert_eq!(got, exp, "round {} final mismatch", round);
            }
        }
    }

    #[test]
    fn layer_map_matches_model() {
        let mut rng = Lcg(7);
        for round in 0..300 {
            let mut maps: Vec<LayerMap<u32, u32>> = vec![LayerMap::new()];
            let mut models: Vec<FxHashMap<u32, u32>> = vec![FxHashMap::default()];
            let nops = 5 + rng.below(60) as usize;
            for _ in 0..nops {
                let i = rng.below(maps.len() as u64) as usize;
                match rng.below(7) {
                    0 | 1 => {
                        let k = rng.below(6) as u32;
                        let v = rng.below(50) as u32;
                        maps[i].insert(k, v);
                        models[i].insert(k, v);
                    }
                    2 => {
                        let c = maps[i].copy();
                        maps.push(c);
                        models.push(models[i].clone());
                    }
                    3 => {
                        let k = rng.below(6) as u32;
                        maps[i].remove(&k);
                        models[i].remove(&k);
                    }
                    4 => {
                        if maps.len() > 1 {
                            let j = rng.below(maps.len() as u64) as usize;
                            let joined = maps[i].join(&maps[j], &mut |p, q| p.max(q) + 1);
                            let mut exp = models[i].clone();
                            for (k, v) in &models[j] {
                                match exp.get(k) {
                                    None => {
                                        exp.insert(*k, *v);
                                    }
                                    Some(cur) => {
                                        if cur != v {
                                            let nv = (*cur).max(*v) + 1;
                                            exp.insert(*k, nv);
                                        }
                                    }
                                }
                            }
                            assert_eq!(joined.materialise(), exp, "round {} join mismatch", round);
                            maps.push(joined);
                            models.push(exp);
                        }
                    }
                    5 => {
                        if maps.len() > 1 {
                            let j = rng.below(maps.len() as u64) as usize;
                            let same = maps[i].same(&maps[j]);
                            assert_eq!(same, models[i] == models[j], "round {} same mismatch", round);
                            let mut d = maps[i].diff_keys(&maps[j]);
                            d.sort();
                            let mut exp: Vec<u32> = models[i].keys().chain(models[j].keys()).copied().filter(|k| models[i].get(k) != models[j].get(k)).collect();
                            exp.sort();
                            exp.dedup();
                            assert_eq!(d, exp, "round {} diff mismatch", round);
                        }
                    }
                    _ => {
                        let k = rng.below(6) as u32;
                        assert_eq!(maps[i].get(&k), models[i].get(&k).copied(), "round {} get mismatch", round);
                    }
                }
            }
        }
    }
}
