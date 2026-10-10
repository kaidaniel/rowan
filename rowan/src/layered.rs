//! Persistent maps for the fork/join pattern of abstract states: big-endian Patricia tries
//! on integer keys (Okasaki & Gill, "Fast Mergeable Integer Maps"; the design of SPARTA's
//! `PatriciaTreeMap`). A copy is O(1) (an `Arc`), a write copies the path to its leaf
//! (in place when no copy shares it), and `join` / `same` / `diff_keys` skip subtrees the
//! two maps share, so they cost what the maps differ in, whatever their history. `TwoLevel`
//! adds a copy-on-write inner map per key (object slots, source requirements).

use std::hash::Hash;
use std::sync::Arc;

use rustc_hash::FxHashMap;

/// A key that is a small integer: its `u64` image orders the trie.
pub trait PKey: Clone + Eq + Hash {
    fn key64(&self) -> u64;
}

impl PKey for u32 {
    fn key64(&self) -> u64 {
        *self as u64
    }
}

impl PKey for (u32, crate::domain::Ns) {
    fn key64(&self) -> u64 {
        ((self.0 as u64) << 3) | self.1 as u64
    }
}

impl PKey for crate::intern::SourceId {
    fn key64(&self) -> u64 {
        self.0 as u64
    }
}

enum Node<K, V> {
    Leaf { h: u64, k: K, v: V },
    /// keys agree with `p` above the branching bit `m`; those with `m` clear go left
    Branch { p: u64, m: u64, l: Arc<Node<K, V>>, r: Arc<Node<K, V>> },
}

impl<K: Clone, V: Clone> Clone for Node<K, V> {
    fn clone(&self) -> Self {
        match self {
            Node::Leaf { h, k, v } => Node::Leaf { h: *h, k: k.clone(), v: v.clone() },
            Node::Branch { p, m, l, r } => Node::Branch { p: *p, m: *m, l: l.clone(), r: r.clone() },
        }
    }
}

type Tree<K, V> = Arc<Node<K, V>>;

#[inline]
fn mask(k: u64, m: u64) -> u64 {
    k & !((m << 1).wrapping_sub(1))
}

#[inline]
fn match_prefix(k: u64, p: u64, m: u64) -> bool {
    mask(k, m) == p
}

#[inline]
fn zero_bit(k: u64, m: u64) -> bool {
    k & m == 0
}

#[inline]
fn branching_bit(a: u64, b: u64) -> u64 {
    1u64 << (63 - (a ^ b).leading_zeros())
}

fn prefix_of<K, V>(t: &Node<K, V>) -> u64 {
    match t {
        Node::Leaf { h, .. } => *h,
        Node::Branch { p, .. } => *p,
    }
}

/// Two trees with disjoint prefixes under one branch.
fn link<K, V>(p0: u64, t0: Tree<K, V>, p1: u64, t1: Tree<K, V>) -> Tree<K, V> {
    let m = branching_bit(p0, p1);
    if zero_bit(p0, m) {
        Arc::new(Node::Branch { p: mask(p0, m), m, l: t0, r: t1 })
    } else {
        Arc::new(Node::Branch { p: mask(p0, m), m, l: t1, r: t0 })
    }
}

/// A branch from possibly empty children; `orig` is returned when both children are its own.
fn branch<K, V>(p: u64, m: u64, l: Option<Tree<K, V>>, r: Option<Tree<K, V>>, orig: Option<&Tree<K, V>>) -> Option<Tree<K, V>> {
    match (l, r) {
        (None, None) => None,
        (Some(x), None) | (None, Some(x)) => Some(x),
        (Some(l), Some(r)) => {
            if let Some(o) = orig {
                if let Node::Branch { l: ol, r: or, .. } = &**o {
                    if Arc::ptr_eq(ol, &l) && Arc::ptr_eq(or, &r) {
                        return Some(o.clone());
                    }
                }
            }
            Some(Arc::new(Node::Branch { p, m, l, r }))
        }
    }
}

fn get<'a, K, V>(mut t: &'a Tree<K, V>, h: u64) -> Option<&'a V> {
    loop {
        match &**t {
            Node::Leaf { h: j, v, .. } => return if *j == h { Some(v) } else { None },
            Node::Branch { p, m, l, r } => {
                if !match_prefix(h, *p, *m) {
                    return None;
                }
                t = if zero_bit(h, *m) { l } else { r };
            }
        }
    }
}

fn insert<K: Clone, V: Clone>(t: &mut Tree<K, V>, h: u64, k: K, v: V) {
    let fits = match &**t {
        Node::Leaf { h: j, .. } => *j == h,
        Node::Branch { p, m, .. } => match_prefix(h, *p, *m),
    };
    if !fits {
        let old = t.clone();
        let po = prefix_of(&old);
        *t = link(h, Arc::new(Node::Leaf { h, k, v }), po, old);
        return;
    }
    match Arc::make_mut(t) {
        Node::Leaf { v: val, .. } => *val = v,
        Node::Branch { m, l, r, .. } => {
            if zero_bit(h, *m) {
                insert(l, h, k, v)
            } else {
                insert(r, h, k, v)
            }
        }
    }
}

/// The value of `h` for writing (path copied); None when absent.
fn get_mut<K: Clone, V: Clone>(t: &mut Tree<K, V>, h: u64) -> Option<&mut V> {
    let fits = match &**t {
        Node::Leaf { h: j, .. } => *j == h,
        Node::Branch { p, m, .. } => match_prefix(h, *p, *m),
    };
    if !fits {
        return None;
    }
    match Arc::make_mut(t) {
        Node::Leaf { v, .. } => Some(v),
        Node::Branch { m, l, r, .. } => {
            if zero_bit(h, *m) {
                get_mut(l, h)
            } else {
                get_mut(r, h)
            }
        }
    }
}

/// `t` without `h`: None when nothing changes, Some(rest) otherwise (rest None: empty).
fn remove<K: Clone, V: Clone>(t: &Tree<K, V>, h: u64) -> Option<(Option<Tree<K, V>>, V)> {
    match &**t {
        Node::Leaf { h: j, v, .. } => {
            if *j == h {
                Some((None, v.clone()))
            } else {
                None
            }
        }
        Node::Branch { p, m, l, r } => {
            if !match_prefix(h, *p, *m) {
                return None;
            }
            if zero_bit(h, *m) {
                let (nl, v) = remove(l, h)?;
                Some((branch(*p, *m, nl, Some(r.clone()), None), v))
            } else {
                let (nr, v) = remove(r, h)?;
                Some((branch(*p, *m, Some(l.clone()), nr, None), v))
            }
        }
    }
}

fn for_each<K, V>(t: &Tree<K, V>, f: &mut dyn FnMut(&K, &V)) {
    match &**t {
        Node::Leaf { k, v, .. } => f(k, v),
        Node::Branch { l, r, .. } => {
            for_each(l, f);
            for_each(r, f);
        }
    }
}

/// How a merge treats a key: both sides have it, or one side only (the result's value, or
/// none to drop the key).
pub struct Merge<'a, K, V> {
    pub both: &'a mut dyn FnMut(&K, &V, &V) -> Option<V>,
    pub left: &'a mut dyn FnMut(&K, &V) -> Option<V>,
    pub right: &'a mut dyn FnMut(&K, &V) -> Option<V>,
    /// one-sided keys keep their values: such subtrees are reused whole
    pub one_side_identity: bool,
    /// a key whose two values are equal keeps that value (shared subtrees are reused whole)
    pub equal_identity: bool,
}

fn map_side<K: Clone, V: Clone>(t: &Tree<K, V>, left: bool, g: &mut Merge<K, V>) -> Option<Tree<K, V>> {
    if g.one_side_identity {
        return Some(t.clone());
    }
    match &**t {
        Node::Leaf { h, k, v } => {
            let nv = if left { (g.left)(k, v) } else { (g.right)(k, v) }?;
            Some(Arc::new(Node::Leaf { h: *h, k: k.clone(), v: nv }))
        }
        Node::Branch { p, m, l, r } => {
            let nl = map_side(l, left, g);
            let nr = map_side(r, left, g);
            branch(*p, *m, nl, nr, None)
        }
    }
}

fn both_leaf<K: Clone, V: Clone + PartialEq>(h: u64, k: &K, a: &V, b: &V, g: &mut Merge<K, V>) -> Option<Tree<K, V>> {
    if g.equal_identity && a == b {
        return Some(Arc::new(Node::Leaf { h, k: k.clone(), v: a.clone() }));
    }
    let v = (g.both)(k, a, b)?;
    Some(Arc::new(Node::Leaf { h, k: k.clone(), v }))
}

/// A leaf of the left map merged with a tree of the right one.
fn merge_leaf<K: Clone, V: Clone + PartialEq>(s: &Tree<K, V>, t: &Tree<K, V>, leaf_left: bool, g: &mut Merge<K, V>) -> Option<Tree<K, V>> {
    let Node::Leaf { h, k, v } = &**s else { unreachable!() };
    match &**t {
        Node::Leaf { h: j, k: _, v: w } if j == h => {
            if leaf_left {
                if g.equal_identity && v == w {
                    return Some(s.clone());
                }
                both_leaf(*h, k, v, w, g)
            } else {
                if g.equal_identity && v == w {
                    return Some(t.clone());
                }
                both_leaf(*h, k, w, v, g)
            }
        }
        Node::Branch { p, m, l, r } if match_prefix(*h, *p, *m) => {
            if zero_bit(*h, *m) {
                let nl = merge_leaf(s, l, leaf_left, g);
                let nr = map_side(r, !leaf_left, g);
                branch(*p, *m, nl, nr, Some(t))
            } else {
                let nl = map_side(l, !leaf_left, g);
                let nr = merge_leaf(s, r, leaf_left, g);
                branch(*p, *m, nl, nr, Some(t))
            }
        }
        _ => {
            let a = map_side(s, leaf_left, g);
            let b = map_side(t, !leaf_left, g);
            match (a, b) {
                (None, x) | (x, None) => x,
                (Some(a), Some(b)) => {
                    let (pa, pb) = (prefix_of(&a), prefix_of(&b));
                    Some(link(pa, a, pb, b))
                }
            }
        }
    }
}

fn merge<K: Clone, V: Clone + PartialEq>(s: &Tree<K, V>, t: &Tree<K, V>, g: &mut Merge<K, V>) -> Option<Tree<K, V>> {
    if g.equal_identity && Arc::ptr_eq(s, t) {
        return Some(s.clone());
    }
    match (&**s, &**t) {
        (Node::Leaf { .. }, _) => merge_leaf(s, t, true, g),
        (_, Node::Leaf { .. }) => merge_leaf(t, s, false, g),
        (Node::Branch { p, m, l: s0, r: s1 }, Node::Branch { p: q, m: n, l: t0, r: t1 }) => {
            if m == n && p == q {
                let nl = merge(s0, t0, g);
                let nr = merge(s1, t1, g);
                branch(*p, *m, nl, nr, Some(s))
            } else if m > n && match_prefix(*q, *p, *m) {
                if zero_bit(*q, *m) {
                    let nl = merge(s0, t, g);
                    let nr = map_side(s1, true, g);
                    branch(*p, *m, nl, nr, Some(s))
                } else {
                    let nl = map_side(s0, true, g);
                    let nr = merge(s1, t, g);
                    branch(*p, *m, nl, nr, Some(s))
                }
            } else if m < n && match_prefix(*p, *q, *n) {
                if zero_bit(*p, *n) {
                    let nl = merge(s, t0, g);
                    let nr = map_side(t1, false, g);
                    branch(*q, *n, nl, nr, Some(t))
                } else {
                    let nl = map_side(t0, false, g);
                    let nr = merge(s, t1, g);
                    branch(*q, *n, nl, nr, Some(t))
                }
            } else {
                let a = map_side(s, true, g);
                let b = map_side(t, false, g);
                match (a, b) {
                    (None, x) | (x, None) => x,
                    (Some(a), Some(b)) => {
                        let (pa, pb) = (prefix_of(&a), prefix_of(&b));
                        Some(link(pa, a, pb, b))
                    }
                }
            }
        }
    }
}

/// Calls `f` with every key whose value differs between the trees (present in one only, or
/// unequal); `f` returns false to stop. Returns false when stopped.
fn diff<K: Clone, V: PartialEq>(s: Option<&Tree<K, V>>, t: Option<&Tree<K, V>>, f: &mut dyn FnMut(&K) -> bool) -> bool {
    let (s, t) = match (s, t) {
        (None, None) => return true,
        (Some(x), None) | (None, Some(x)) => return all_keys(x, f),
        (Some(s), Some(t)) => (s, t),
    };
    if Arc::ptr_eq(s, t) {
        return true;
    }
    match (&**s, &**t) {
        (Node::Leaf { h, k, v }, _) => {
            // every key of t but h differs; h differs unless equal
            let mut seen = false;
            let ok = all_keys_where(t, &mut |kk, hh, w| {
                if hh == *h {
                    seen = true;
                    if w == v { true } else { f(kk) }
                } else {
                    f(kk)
                }
            });
            if !ok {
                return false;
            }
            if !seen {
                return f(k);
            }
            true
        }
        (_, Node::Leaf { .. }) => diff(Some(t), Some(s), f),
        (Node::Branch { p, m, l: s0, r: s1 }, Node::Branch { p: q, m: n, l: t0, r: t1 }) => {
            if m == n && p == q {
                diff(Some(s0), Some(t0), f) && diff(Some(s1), Some(t1), f)
            } else if m > n && match_prefix(*q, *p, *m) {
                if zero_bit(*q, *m) {
                    diff(Some(s0), Some(t), f) && all_keys(s1, f)
                } else {
                    all_keys(s0, f) && diff(Some(s1), Some(t), f)
                }
            } else if m < n && match_prefix(*p, *q, *n) {
                if zero_bit(*p, *n) {
                    diff(Some(s), Some(t0), f) && all_keys(t1, f)
                } else {
                    all_keys(t0, f) && diff(Some(s), Some(t1), f)
                }
            } else {
                all_keys(s, f) && all_keys(t, f)
            }
        }
    }
}

fn all_keys<K, V>(t: &Tree<K, V>, f: &mut dyn FnMut(&K) -> bool) -> bool {
    match &**t {
        Node::Leaf { k, .. } => f(k),
        Node::Branch { l, r, .. } => all_keys(l, f) && all_keys(r, f),
    }
}

fn all_keys_where<K, V>(t: &Tree<K, V>, f: &mut dyn FnMut(&K, u64, &V) -> bool) -> bool {
    match &**t {
        Node::Leaf { h, k, v } => f(k, *h, v),
        Node::Branch { l, r, .. } => all_keys_where(l, f) && all_keys_where(r, f),
    }
}

/// A persistent map (the historical name: it was a chain of layers).
pub struct LayerMap<K, V> {
    root: Option<Tree<K, V>>,
}

impl<K, V> Clone for LayerMap<K, V> {
    fn clone(&self) -> Self {
        LayerMap { root: self.root.clone() }
    }
}

impl<K: PKey, V: Clone + PartialEq> Default for LayerMap<K, V> {
    fn default() -> Self {
        LayerMap { root: None }
    }
}

impl<K: PKey, V: Clone + PartialEq> LayerMap<K, V> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, k: &K) -> Option<V> {
        self.root.as_ref().and_then(|t| get(t, k.key64())).cloned()
    }

    /// The value without a clone.
    pub fn get_ref(&self, k: &K) -> Option<&V> {
        self.root.as_ref().and_then(|t| get(t, k.key64()))
    }

    pub fn contains_key(&self, k: &K) -> bool {
        self.get_ref(k).is_some()
    }

    /// A plain map with the full contents (O(n)).
    pub fn materialise(&self) -> FxHashMap<K, V> {
        let mut out = FxHashMap::default();
        if let Some(t) = &self.root {
            for_each(t, &mut |k, v| {
                out.insert(k.clone(), v.clone());
            });
        }
        out
    }

    /// The entries in key order.
    pub fn items(&self) -> Vec<(K, V)> {
        let mut out = Vec::new();
        if let Some(t) = &self.root {
            for_each(t, &mut |k, v| out.push((k.clone(), v.clone())));
        }
        out
    }

    pub fn keys(&self) -> Vec<K> {
        let mut out = Vec::new();
        if let Some(t) = &self.root {
            for_each(t, &mut |k, _| out.push(k.clone()));
        }
        out
    }

    pub fn len(&self) -> usize {
        let mut n = 0;
        if let Some(t) = &self.root {
            for_each(t, &mut |_, _| n += 1);
        }
        n
    }

    pub fn insert(&mut self, k: K, v: V) {
        let h = k.key64();
        match &mut self.root {
            None => self.root = Some(Arc::new(Node::Leaf { h, k, v })),
            Some(t) => insert(t, h, k, v),
        }
    }

    pub fn remove(&mut self, k: &K) -> Option<V> {
        let t = self.root.as_ref()?;
        let (rest, v) = remove(t, k.key64())?;
        self.root = rest;
        Some(v)
    }

    /// The value of `k` for writing, `default()` inserted first when absent.
    pub fn entry_mut(&mut self, k: &K, default: impl FnOnce() -> V) -> &mut V {
        if !self.contains_key(k) {
            self.insert(k.clone(), default());
        }
        get_mut(self.root.as_mut().unwrap(), k.key64()).unwrap()
    }

    pub fn copy(&mut self) -> LayerMap<K, V> {
        self.clone()
    }

    pub fn copy_ref(&self) -> LayerMap<K, V> {
        self.clone()
    }

    /// (kept for the callers of the layered implementation: nothing to freeze)
    pub fn share(&mut self) {}

    /// Keys on one side only are kept; keys on both sides are combined with f (unless equal).
    pub fn join(&self, o: &LayerMap<K, V>, f: &mut dyn FnMut(&V, &V) -> V) -> LayerMap<K, V> {
        let mut both = |_: &K, a: &V, b: &V| Some(f(a, b));
        let mut keep = |_: &K, v: &V| Some(v.clone());
        let mut keep2 = |_: &K, v: &V| Some(v.clone());
        let mut g = Merge { both: &mut both, left: &mut keep, right: &mut keep2, one_side_identity: true, equal_identity: true };
        self.merge_with(o, &mut g)
    }

    /// The general merge (see `Merge`).
    pub fn merge_with(&self, o: &LayerMap<K, V>, g: &mut Merge<K, V>) -> LayerMap<K, V> {
        let root = match (&self.root, &o.root) {
            (None, None) => None,
            (Some(a), None) => map_side(a, true, g),
            (None, Some(b)) => map_side(b, false, g),
            (Some(a), Some(b)) => merge(a, b, g),
        };
        LayerMap { root }
    }

    pub fn same(&self, o: &LayerMap<K, V>) -> bool {
        diff(self.root.as_ref(), o.root.as_ref(), &mut |_| false)
    }

    /// Keys whose value differs between self and base.
    pub fn diff_keys(&self, base: &LayerMap<K, V>) -> Vec<K> {
        let mut out = Vec::new();
        diff(self.root.as_ref(), base.root.as_ref(), &mut |k| {
            out.push(k.clone());
            true
        });
        out
    }
}

// --------------------------------------------------------------------------------------

pub struct TwoLevel<K, IK, V> {
    pub m: LayerMap<K, Arc<FxHashMap<IK, V>>>,
}

impl<K: PKey, IK: Clone + Eq + Hash, V: Clone + PartialEq> Default for TwoLevel<K, IK, V> {
    fn default() -> Self {
        TwoLevel { m: LayerMap::new() }
    }
}

impl<K: PKey, IK: Clone + Eq + Hash, V: Clone + PartialEq> TwoLevel<K, IK, V> {
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
        let slot = self.m.entry_mut(key, || Arc::new(FxHashMap::default()));
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
        self.m.get_ref(key).and_then(|d| d.get(ik).cloned())
    }

    pub fn contains(&self, key: &K, ik: &IK) -> bool {
        self.m.get_ref(key).map(|d| d.contains_key(ik)).unwrap_or(false)
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
        match self.m.get_ref(key) {
            Some(d) => d.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            None => Vec::new(),
        }
    }

    pub fn drop_where(&mut self, key: &K, mut pred: impl FnMut(&IK) -> bool) {
        let Some(d) = self.m.get_ref(key) else { return };
        if !d.keys().any(|ik| pred(ik)) {
            return;
        }
        let w = self.writable(key);
        w.retain(|ik, _| !pred(ik));
        if w.is_empty() {
            self.drop_key(key);
        }
    }

    pub fn join(&mut self, o: &mut Self, f: &mut dyn FnMut(&V, &V) -> V) -> Self {
        let mut merge_inner = |d1: &Arc<FxHashMap<IK, V>>, d2: &Arc<FxHashMap<IK, V>>| -> Arc<FxHashMap<IK, V>> {
            if Arc::ptr_eq(d1, d2) {
                return d1.clone();
            }
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
        TwoLevel { m: self.m.join(&o.m, &mut merge_inner) }
    }

    /// Join where inner entries present on one side only are transformed by `one_side`.
    pub fn join_with(&mut self, o: &mut Self, both: &mut dyn FnMut(&V, &V) -> V, one_side: &mut dyn FnMut(&V) -> V) -> Self {
        self.join_with_keep(o, both, one_side, &|_| false)
    }

    /// `join_with`, but an outer key whose whole inner map one side lacks keeps the other
    /// side's values unchanged when `keep_whole` says so.
    pub fn join_with_keep(&mut self, o: &mut Self, both: &mut dyn FnMut(&V, &V) -> V, one_side: &mut dyn FnMut(&V) -> V, keep_whole: &dyn Fn(&K) -> bool) -> Self {
        let one_side = std::cell::RefCell::new(one_side);
        let mut inner_both = |_: &K, da: &Arc<FxHashMap<IK, V>>, db: &Arc<FxHashMap<IK, V>>| -> Option<Arc<FxHashMap<IK, V>>> {
            if Arc::ptr_eq(da, db) {
                return Some(da.clone());
            }
            let mut os = one_side.borrow_mut();
            let mut merged: FxHashMap<IK, V> = FxHashMap::default();
            for nk in da.keys().chain(db.keys().filter(|nk| !da.contains_key(*nk))) {
                let v = match (da.get(nk), db.get(nk)) {
                    (Some(va), None) => os(va),
                    (None, Some(vb)) => os(vb),
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
            Some(Arc::new(merged))
        };
        let mut side = |k: &K, d: &Arc<FxHashMap<IK, V>>| -> Option<Arc<FxHashMap<IK, V>>> {
            if keep_whole(k) {
                return Some(d.clone());
            }
            let mut os = one_side.borrow_mut();
            Some(Arc::new(d.iter().map(|(nk, v)| (nk.clone(), os(v))).collect()))
        };
        let side_cell = std::cell::RefCell::new(&mut side);
        let mut left = |k: &K, d: &Arc<FxHashMap<IK, V>>| (side_cell.borrow_mut())(k, d);
        let mut right = |k: &K, d: &Arc<FxHashMap<IK, V>>| (side_cell.borrow_mut())(k, d);
        // (equal inner maps: a ptr-equal pair keeps its map; an equal pair is merged by
        // `inner_both`, which keeps equal entries)
        let mut g = Merge { both: &mut inner_both, left: &mut left, right: &mut right, one_side_identity: false, equal_identity: true };
        TwoLevel { m: self.m.merge_with(&o.m, &mut g) }
    }

    pub fn same(&self, o: &Self) -> bool {
        self.m.same(&o.m)
    }

    pub fn diff_keys(&self, base: &Self) -> Vec<(K, IK)> {
        let mut out = Vec::new();
        let empty = Arc::new(FxHashMap::default());
        for key in self.m.diff_keys(&base.m) {
            let d = self.m.get_ref(&key).unwrap_or(&empty);
            let b = base.m.get_ref(&key).unwrap_or(&empty);
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

#[cfg(test)]
mod wide_key_tests {
    //! The model tests on wide, sparse keys (deep tries, every merge case).
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

    fn key(rng: &mut Lcg, pool: &[u32]) -> u32 {
        // mostly keys seen before (so the maps overlap), sometimes fresh ones
        if !pool.is_empty() && rng.below(4) != 0 { pool[rng.below(pool.len() as u64) as usize] } else { (rng.next() & 0xFFFFF) as u32 }
    }

    #[test]
    fn layer_map_wide_keys() {
        let mut rng = Lcg(99);
        for round in 0..400 {
            let mut pool: Vec<u32> = Vec::new();
            let mut maps: Vec<LayerMap<u32, u32>> = vec![LayerMap::new()];
            let mut models: Vec<FxHashMap<u32, u32>> = vec![FxHashMap::default()];
            for _ in 0..(10 + rng.below(120)) {
                let i = rng.below(maps.len() as u64) as usize;
                match rng.below(8) {
                    0 | 1 | 2 => {
                        let k = key(&mut rng, &pool);
                        pool.push(k);
                        let v = rng.below(30) as u32;
                        maps[i].insert(k, v);
                        models[i].insert(k, v);
                    }
                    3 => {
                        let c = maps[i].copy();
                        maps.push(c);
                        models.push(models[i].clone());
                    }
                    4 => {
                        let k = key(&mut rng, &pool);
                        assert_eq!(maps[i].remove(&k), models[i].remove(&k), "round {} remove", round);
                    }
                    5 => {
                        let j = rng.below(maps.len() as u64) as usize;
                        let joined = maps[i].join(&maps[j], &mut |p, q| p.max(q) + 1);
                        let mut exp = models[i].clone();
                        for (k, v) in &models[j] {
                            match exp.get(k).copied() {
                                None => {
                                    exp.insert(*k, *v);
                                }
                                Some(cur) if cur != *v => {
                                    exp.insert(*k, cur.max(*v) + 1);
                                }
                                _ => {}
                            }
                        }
                        assert_eq!(joined.materialise(), exp, "round {} join", round);
                        maps.push(joined);
                        models.push(exp);
                    }
                    6 => {
                        let j = rng.below(maps.len() as u64) as usize;
                        assert_eq!(maps[i].same(&maps[j]), models[i] == models[j], "round {} same", round);
                        let mut d = maps[i].diff_keys(&maps[j]);
                        d.sort();
                        let mut exp: Vec<u32> = models[i].keys().chain(models[j].keys()).copied().filter(|k| models[i].get(k) != models[j].get(k)).collect();
                        exp.sort();
                        exp.dedup();
                        assert_eq!(d, exp, "round {} diff", round);
                    }
                    _ => {
                        let k = key(&mut rng, &pool);
                        assert_eq!(maps[i].get(&k), models[i].get(&k).copied(), "round {} get", round);
                    }
                }
            }
            for (m, model) in maps.iter().zip(models.iter()) {
                assert_eq!(&m.materialise(), model, "round {} final", round);
                assert_eq!(m.len(), model.len());
                let ks = m.keys();
                assert!(ks.windows(2).all(|w| w[0] < w[1]), "keys in order");
            }
        }
    }

    #[test]
    fn two_level_wide_keys() {
        let mut rng = Lcg(5);
        type Model = FxHashMap<u32, FxHashMap<u32, u32>>;
        for round in 0..200 {
            let mut pool: Vec<u32> = Vec::new();
            let mut maps: Vec<TwoLevel<u32, u32, u32>> = vec![TwoLevel::new()];
            let mut models: Vec<Model> = vec![FxHashMap::default()];
            for _ in 0..(10 + rng.below(80)) {
                let i = rng.below(maps.len() as u64) as usize;
                match rng.below(6) {
                    0 | 1 => {
                        let k = key(&mut rng, &pool);
                        pool.push(k);
                        let (ik, v) = (rng.below(5) as u32, rng.below(30) as u32);
                        maps[i].set(&k, ik, v);
                        models[i].entry(k).or_default().insert(ik, v);
                    }
                    2 => {
                        let c = maps[i].copy();
                        maps.push(c);
                        models.push(models[i].clone());
                    }
                    3 => {
                        let k = key(&mut rng, &pool);
                        let ik = rng.below(5) as u32;
                        maps[i].remove(&k, &ik);
                        if let Some(d) = models[i].get_mut(&k) {
                            d.remove(&ik);
                            if d.is_empty() {
                                models[i].remove(&k);
                            }
                        }
                    }
                    4 => {
                        let j = rng.below(maps.len() as u64) as usize;
                        if j != i {
                            let mut a = maps[i].copy();
                            let mut b = maps[j].copy();
                            let joined = a.join_with(&mut b, &mut |p, q| p.max(q) + 1, &mut |p| p + 1000);
                            let mut exp: Model = FxHashMap::default();
                            for k in models[i].keys().chain(models[j].keys()) {
                                let mut m: FxHashMap<u32, u32> = FxHashMap::default();
                                match (models[i].get(k), models[j].get(k)) {
                                    (Some(x), None) | (None, Some(x)) => {
                                        for (nk, v) in x {
                                            m.insert(*nk, v + 1000);
                                        }
                                    }
                                    (Some(x), Some(y)) => {
                                        for nk in x.keys().chain(y.keys()) {
                                            let v = match (x.get(nk), y.get(nk)) {
                                                (Some(p), None) | (None, Some(p)) => p + 1000,
                                                (Some(p), Some(q)) => if p == q { *p } else { p.max(q) + 1 },
                                                _ => unreachable!(),
                                            };
                                            m.insert(*nk, v);
                                        }
                                    }
                                    _ => {}
                                }
                                if !m.is_empty() {
                                    exp.insert(*k, m);
                                }
                            }
                            let got: Model = joined.m.materialise().into_iter().map(|(k, d)| (k, (*d).clone())).filter(|(_, d)| !d.is_empty()).collect();
                            assert_eq!(got, exp, "round {} join_with", round);
                            maps.push(joined);
                            models.push(exp);
                        }
                    }
                    _ => {
                        let j = rng.below(maps.len() as u64) as usize;
                        let mut d = maps[i].diff_keys(&maps[j]);
                        d.sort();
                        let mut exp: Vec<(u32, u32)> = Vec::new();
                        let e = FxHashMap::default();
                        for k in models[i].keys().chain(models[j].keys()) {
                            let (x, y) = (models[i].get(k).unwrap_or(&e), models[j].get(k).unwrap_or(&e));
                            for nk in x.keys().chain(y.keys()) {
                                if x.get(nk) != y.get(nk) {
                                    exp.push((*k, *nk));
                                }
                            }
                        }
                        exp.sort();
                        exp.dedup();
                        assert_eq!(d, exp, "round {} diff", round);
                    }
                }
            }
        }
    }
}
