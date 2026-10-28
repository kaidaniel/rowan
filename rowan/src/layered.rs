//! Persistent maps for the fork/join pattern of abstract states: big-endian Patricia tries
//! on integer keys (Okasaki & Gill, "Fast Mergeable Integer Maps"; the design of SPARTA's
//! `PatriciaTreeMap`). A copy is O(1) (an `Arc`), a write copies the path to its leaf
//! (in place when no copy shares it), and `join` / `same` / `diff_keys` skip subtrees the
//! two maps share, so they cost what the maps differ in, whatever their history. `TwoLevel`
//! adds a copy-on-write inner map per key (object slots, source requirements).

use crate::domain::Ns;
use crate::intern::{KeyId, SourceId};
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

impl PKey for (u32, Ns) {
    fn key64(&self) -> u64 {
        ((self.0 as u64) << 3) | self.1 as u64
    }
}

impl PKey for SourceId {
    fn key64(&self) -> u64 {
        self.0 as u64
    }
}

/// A map's value. Writing a value the key already has (`same_write`) copies nothing, so the
/// map stays shared with its copies (a row only when it is the same row, not an equal one).
pub trait MapValue: Clone + PartialEq {
    fn same_write(&self, o: &Self) -> bool {
        self == o
    }
}

impl MapValue for u32 {}

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

/// A branch from possibly empty children; `orig` (or `alt`, a branch of the other map at the
/// same place) is returned when both children are its own, so joins keep subtrees shared.
fn branch<K, V>(p: u64, m: u64, l: Option<Tree<K, V>>, r: Option<Tree<K, V>>, orig: Option<&Tree<K, V>>) -> Option<Tree<K, V>> {
    branch2(p, m, l, r, orig, None)
}

fn branch2<K, V>(p: u64, m: u64, l: Option<Tree<K, V>>, r: Option<Tree<K, V>>, orig: Option<&Tree<K, V>>, alt: Option<&Tree<K, V>>) -> Option<Tree<K, V>> {
    match (l, r) {
        (None, None) => None,
        (Some(x), None) | (None, Some(x)) => Some(x),
        (Some(l), Some(r)) => {
            for o in orig.into_iter().chain(alt) {
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

fn map_side<K: Clone, V: Clone + PartialEq>(t: &Tree<K, V>, left: bool, g: &mut Merge<K, V>) -> Option<Tree<K, V>> {
    if g.one_side_identity {
        return Some(t.clone());
    }
    match &**t {
        Node::Leaf { h, k, v } => {
            let nv = if left { (g.left)(k, v) } else { (g.right)(k, v) }?;
            // (an unchanged value keeps its leaf: the result shares it)
            if nv == *v {
                return Some(t.clone());
            }
            Some(Arc::new(Node::Leaf { h: *h, k: k.clone(), v: nv }))
        }
        Node::Branch { p, m, l, r } => {
            let nl = map_side(l, left, g);
            let nr = map_side(r, left, g);
            branch(*p, *m, nl, nr, Some(t))
        }
    }
}

/// Leaves `ta` (left) and `tb` (right) with one key merged: a result equal to one side's value
/// is that side's leaf (the joined map keeps sharing it: later joins and diffs skip it).
fn both_leaf<K: Clone, V: Clone + PartialEq>(ta: &Tree<K, V>, tb: &Tree<K, V>, g: &mut Merge<K, V>) -> Option<Tree<K, V>> {
    let (Node::Leaf { h, k, v: a }, Node::Leaf { v: b, .. }) = (&**ta, &**tb) else { unreachable!() };
    let v = (g.both)(k, a, b)?;
    if v == *a {
        return Some(ta.clone());
    }
    if v == *b {
        return Some(tb.clone());
    }
    Some(Arc::new(Node::Leaf { h: *h, k: k.clone(), v }))
}

/// A leaf of the left map merged with a tree of the right one.
fn merge_leaf<K: Clone, V: Clone + PartialEq>(s: &Tree<K, V>, t: &Tree<K, V>, leaf_left: bool, g: &mut Merge<K, V>) -> Option<Tree<K, V>> {
    let Node::Leaf { h, v, .. } = &**s else { unreachable!() };
    match &**t {
        Node::Leaf { h: j, k: _, v: w } if j == h => {
            if leaf_left {
                if g.equal_identity && v == w {
                    return Some(s.clone());
                }
                both_leaf(s, t, g)
            } else {
                if g.equal_identity && v == w {
                    return Some(t.clone());
                }
                both_leaf(t, s, g)
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
                branch2(*p, *m, nl, nr, Some(s), Some(t))
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
    diff_by(s, t, &|a: &V, b: &V| a == b, f)
}

/// `diff` with values equal when `eq` says so.
fn diff_by<K: Clone, V>(s: Option<&Tree<K, V>>, t: Option<&Tree<K, V>>, eq: &dyn Fn(&V, &V) -> bool, f: &mut dyn FnMut(&K) -> bool) -> bool {
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
                    if eq(w, v) { true } else { f(kk) }
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
        (_, Node::Leaf { .. }) => diff_by(Some(t), Some(s), eq, f),
        (Node::Branch { p, m, l: s0, r: s1 }, Node::Branch { p: q, m: n, l: t0, r: t1 }) => {
            if m == n && p == q {
                diff_by(Some(s0), Some(t0), eq, f) && diff_by(Some(s1), Some(t1), eq, f)
            } else if m > n && match_prefix(*q, *p, *m) {
                if zero_bit(*q, *m) {
                    diff_by(Some(s0), Some(t), eq, f) && all_keys(s1, f)
                } else {
                    all_keys(s0, f) && diff_by(Some(s1), Some(t), eq, f)
                }
            } else if m < n && match_prefix(*p, *q, *n) {
                if zero_bit(*p, *n) {
                    diff_by(Some(s), Some(t0), eq, f) && all_keys(t1, f)
                } else {
                    all_keys(t0, f) && diff_by(Some(s), Some(t1), eq, f)
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

impl<K: PKey, V: MapValue> Default for LayerMap<K, V> {
    fn default() -> Self {
        LayerMap { root: None }
    }
}

impl<K: PKey, V: MapValue> LayerMap<K, V> {
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
            // (writing the value already there copies nothing: the map stays shared)
            Some(t) if get(t, h).is_some_and(|old| old.same_write(&v)) => {}
            Some(t) => insert(t, h, k, v),
        }
    }

    pub fn remove(&mut self, k: &K) -> Option<V> {
        let t = self.root.as_ref()?;
        let (rest, v) = remove(t, k.key64())?;
        self.root = rest;
        Some(v)
    }

    /// Removes every key whose image agrees with `k`'s from bit `bits` up (an object's rows in
    /// all namespaces), in one path copy.
    pub fn remove_block(&mut self, k: &K, bits: u32) {
        if let Some(rest) = self.root.as_ref().and_then(|t| remove_hi(t, k.key64() >> bits, bits)) {
            self.root = rest;
        }
    }

    /// Keys on one side only are kept; keys on both sides are combined with f (unless equal).
    pub fn join(&self, o: &LayerMap<K, V>, f: &mut dyn FnMut(&V, &V) -> V) -> LayerMap<K, V> {
        let mut both = |_: &K, a: &V, b: &V| Some(f(a, b));
        let mut keep = |_: &K, v: &V| Some(v.clone());
        let mut keep2 = |_: &K, v: &V| Some(v.clone());
        let mut g = Merge { both: &mut both, left: &mut keep, right: &mut keep2, one_side_identity: true, equal_identity: true };
        self.merge_with(o, &mut g)
    }

    /// `join`, but a key on one side only takes `side`'s value (None: dropped).
    pub fn join_sides(&self, o: &LayerMap<K, V>, f: &mut dyn FnMut(&V, &V) -> V, side: &mut dyn FnMut(&K, &V) -> Option<V>) -> LayerMap<K, V> {
        let side = std::cell::RefCell::new(side);
        let mut both = |_: &K, a: &V, b: &V| Some(f(a, b));
        let mut left = |k: &K, v: &V| (side.borrow_mut())(k, v);
        let mut right = |k: &K, v: &V| (side.borrow_mut())(k, v);
        let mut g = Merge { both: &mut both, left: &mut left, right: &mut right, one_side_identity: false, equal_identity: true };
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

    /// Do the two maps hold the same entries? (Shared subtrees are not visited.)
    pub fn same(&self, o: &LayerMap<K, V>) -> bool {
        diff(self.root.as_ref(), o.root.as_ref(), &mut |_| false)
    }

    /// Keys whose value differs between self and base.
    pub fn diff_keys(&self, base: &LayerMap<K, V>) -> Vec<K> {
        self.diff_upto(base, usize::MAX).unwrap_or_default()
    }

    /// The keys whose value one of the two maps wrote since they shared it (`MapValue::same_write`:
    /// a row written equal to the other's counts), or None once more than `limit` do.
    pub fn written_upto(&self, base: &LayerMap<K, V>, limit: usize) -> Option<Vec<K>> {
        let mut out = Vec::new();
        let done = diff_by(self.root.as_ref(), base.root.as_ref(), &|a: &V, b: &V| a.same_write(b), &mut |k| {
            out.push(k.clone());
            out.len() <= limit
        });
        done.then_some(out)
    }

    /// `diff_keys`, or None once more than `limit` keys differ.
    pub fn diff_upto(&self, base: &LayerMap<K, V>, limit: usize) -> Option<Vec<K>> {
        let mut out = Vec::new();
        let done = diff(self.root.as_ref(), base.root.as_ref(), &mut |k| {
            out.push(k.clone());
            out.len() <= limit
        });
        done.then_some(out)
    }
}

// --------------------------------------------------------------------------------------

/// The 32-bit image of an inner key (`TwoLevel`).
pub trait IKey: Clone + Eq + Hash {
    fn key32(&self) -> u32;
    /// The inner key whose image is `x` (for `SENTINEL`).
    fn from32(x: u32) -> Self;
}

/// The inner key of the marker entry `TwoLevel::mark` writes: never a real key.
pub const SENTINEL: u32 = (1 << 29) - 1;

impl IKey for u32 {
    fn key32(&self) -> u32 {
        *self
    }
    fn from32(x: u32) -> Self {
        x
    }
}

impl IKey for KeyId {
    fn key32(&self) -> u32 {
        debug_assert!(self.0 < SENTINEL, "key id past 2^29");
        self.0
    }
    fn from32(x: u32) -> Self {
        KeyId(x)
    }
}

impl IKey for (Ns, KeyId) {
    fn key32(&self) -> u32 {
        debug_assert!(self.1 .0 < SENTINEL, "key id past 2^29");
        ((self.0 as u32) << 29) | self.1 .0
    }
    fn from32(x: u32) -> Self {
        (crate::domain::Ns::Item, KeyId(x))
    }
}

/// An (outer, inner) key of a `TwoLevel` map: the outer key's image in the high 32 bits, so
/// the entries of one outer key are one subtree of the trie.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Pair<K, IK>(pub K, pub IK);

impl<K: PKey, IK: IKey> PKey for Pair<K, IK> {
    fn key64(&self) -> u64 {
        debug_assert!(self.0.key64() < 1 << 32, "outer key past 2^32");
        (self.0.key64() << 32) | self.1.key32() as u64
    }
}

/// The subtree holding exactly the keys whose high 32 bits are `hi` (big-endian Patricia
/// tries keep keys with a common prefix together), if any.
fn find_hi<K, V>(t: &Tree<K, V>, hi: u64) -> Option<&Tree<K, V>> {
    let lo = hi << 32;
    let mut t = t;
    loop {
        match &**t {
            Node::Leaf { h, .. } => return if h >> 32 == hi { Some(t) } else { None },
            Node::Branch { p, m, l, r } => {
                if *m < 1 << 32 {
                    // (every key below shares the high bits)
                    return if p >> 32 == hi { Some(t) } else { None };
                }
                if !match_prefix(lo, *p, *m) {
                    return None;
                }
                t = if zero_bit(lo, *m) { l } else { r };
            }
        }
    }
}

/// `t` without the keys whose bits from `bits` up are `hi`, in one path copy (None: nothing
/// changes; Some(rest), rest None: empty).
fn remove_hi<K: Clone, V: Clone>(t: &Tree<K, V>, hi: u64, bits: u32) -> Option<Option<Tree<K, V>>> {
    match &**t {
        Node::Leaf { h, .. } => (h >> bits == hi).then_some(None),
        // (every key below shares the high bits)
        Node::Branch { p, m, .. } if *m < 1 << bits => (p >> bits == hi).then_some(None),
        Node::Branch { p, m, l, r } => {
            let lo = hi << bits;
            if !match_prefix(lo, *p, *m) {
                return None;
            }
            Some(if zero_bit(lo, *m) { branch(*p, *m, remove_hi(l, hi, bits)?, Some(r.clone()), None) } else { branch(*p, *m, Some(l.clone()), remove_hi(r, hi, bits)?, None) })
        }
    }
}

/// A map of maps (object -> slot -> value, source -> key -> requirement), stored as one
/// persistent trie on (outer, inner) pairs: a write copies a path, not an object's whole
/// inner map, and joins / diffs cost what two maps differ in.
pub struct TwoLevel<K, IK, V> {
    m: LayerMap<Pair<K, IK>, V>,
}

impl<K, IK, V> Clone for TwoLevel<K, IK, V> {
    fn clone(&self) -> Self {
        TwoLevel { m: self.m.clone() }
    }
}

impl<K: PKey, IK: IKey, V: MapValue> Default for TwoLevel<K, IK, V> {
    fn default() -> Self {
        TwoLevel { m: LayerMap::new() }
    }
}

impl<K: PKey, IK: IKey, V: MapValue> TwoLevel<K, IK, V> {
    pub fn new() -> Self {
        Self::default()
    }

    /// The subtree of `key`'s entries.
    fn sub(&self, key: &K) -> Option<&Tree<Pair<K, IK>, V>> {
        self.m.root.as_ref().and_then(|t| find_hi(t, key.key64()))
    }

    /// Does `key` have any inner entry (or its marker)?
    pub fn has_key(&self, key: &K) -> bool {
        self.sub(key).is_some()
    }

    /// Mark `key` as present with no entries of its own (the marker entry is hidden from
    /// every listing; it tells a join that this side has the key).
    pub fn mark(&mut self, key: &K, v: V) {
        if !self.has_key(key) {
            self.m.insert(Pair(key.clone(), IK::from32(SENTINEL)), v);
        }
    }

    /// Call `f` with every inner entry of `key`.
    pub fn each(&self, key: &K, f: &mut dyn FnMut(&IK, &V)) {
        if let Some(t) = self.sub(key) {
            for_each(t, &mut |k, v| {
                if k.1.key32() != SENTINEL {
                    f(&k.1, v)
                }
            });
        }
    }

    /// The inner entries of one key, as a vector (in inner-key order).
    pub fn for_key(&self, key: &K) -> Vec<(IK, V)> {
        let mut out = Vec::new();
        self.each(key, &mut |ik, v| out.push((ik.clone(), v.clone())));
        out
    }

    /// The inner entries of one key as a map (None: it has none).
    pub fn inner(&self, key: &K) -> Option<FxHashMap<IK, V>> {
        self.sub(key)?;
        let mut out = FxHashMap::default();
        self.each(key, &mut |ik, v| {
            out.insert(ik.clone(), v.clone());
        });
        Some(out)
    }

    /// Every outer key with its inner entries, in key order.
    pub fn items(&self) -> Vec<(K, Vec<(IK, V)>)> {
        let mut out: Vec<(K, Vec<(IK, V)>)> = Vec::new();
        if let Some(t) = &self.m.root {
            for_each(t, &mut |k, v| match out.last_mut() {
                _ if k.1.key32() == SENTINEL => {}
                Some((o, d)) if *o == k.0 => d.push((k.1.clone(), v.clone())),
                _ => out.push((k.0.clone(), vec![(k.1.clone(), v.clone())])),
            });
        }
        out
    }

    pub fn get(&self, key: &K, ik: &IK) -> Option<V> {
        self.m.get(&Pair(key.clone(), ik.clone()))
    }

    pub fn get_ref(&self, key: &K, ik: &IK) -> Option<&V> {
        self.m.get_ref(&Pair(key.clone(), ik.clone()))
    }

    pub fn contains(&self, key: &K, ik: &IK) -> bool {
        self.get_ref(key, ik).is_some()
    }

    pub fn set(&mut self, key: &K, ik: IK, v: V) {
        self.m.insert(Pair(key.clone(), ik), v);
    }

    pub fn remove(&mut self, key: &K, ik: &IK) -> Option<V> {
        self.m.remove(&Pair(key.clone(), ik.clone()))
    }

    /// Removes every inner entry of `key`.
    pub fn drop_key(&mut self, key: &K) {
        if let Some(rest) = self.m.root.as_ref().and_then(|t| remove_hi(t, key.key64(), 32)) {
            self.m.root = rest;
        }
    }

    /// Removes the inner entries of `key` that `pred` picks (and its marker).
    pub fn drop_where(&mut self, key: &K, mut pred: impl FnMut(&IK) -> bool) {
        let mut gone: Vec<IK> = Vec::new();
        if let Some(t) = self.sub(key) {
            for_each(t, &mut |k, _| {
                if k.1.key32() == SENTINEL || pred(&k.1) {
                    gone.push(k.1.clone());
                }
            });
        }
        for ik in gone {
            self.remove(key, &ik);
        }
    }

    /// Entries on one side only are kept; those on both combined with `f` (unless equal).
    pub fn join(&self, o: &Self, f: &mut dyn FnMut(&V, &V) -> V) -> Self {
        TwoLevel { m: self.m.join(&o.m, f) }
    }

    /// `join`, but an entry on one side only is what `side` makes of it (None: dropped).
    pub fn join_sides(&self, o: &Self, f: &mut dyn FnMut(&V, &V) -> V, side: &mut dyn FnMut(&K, &IK, &V) -> Option<V>) -> Self {
        TwoLevel { m: self.m.join_sides(&o.m, f, &mut |k, v| side(&k.0, &k.1, v)) }
    }

    /// Join where inner entries present on one side only are transformed by `one_side`.
    pub fn join_with(&self, o: &Self, both: &mut dyn FnMut(&V, &V) -> V, one_side: &mut dyn FnMut(&V) -> V) -> Self {
        self.join_with_keep(o, both, one_side, &|_| false)
    }

    /// `join_with`, but an outer key whose inner entries one side lacks entirely keeps the
    /// other side's values unchanged when `keep_whole` says so.
    pub fn join_with_keep(&self, o: &Self, both: &mut dyn FnMut(&V, &V) -> V, one_side: &mut dyn FnMut(&V) -> V, keep_whole: &dyn Fn(&K) -> bool) -> Self {
        let one_side = std::cell::RefCell::new(one_side);
        let (a, b) = (&self.m, &o.m);
        let mut inner_both = |_: &Pair<K, IK>, x: &V, y: &V| Some(if x == y { x.clone() } else { both(x, y) });
        let side = |k: &Pair<K, IK>, v: &V, other: &LayerMap<Pair<K, IK>, V>| -> Option<V> {
            if keep_whole(&k.0) && other.root.as_ref().and_then(|t| find_hi(t, k.0.key64())).is_none() {
                return Some(v.clone());
            }
            Some((one_side.borrow_mut())(v))
        };
        let mut left = |k: &Pair<K, IK>, v: &V| side(k, v, b);
        let mut right = |k: &Pair<K, IK>, v: &V| side(k, v, a);
        let mut g = Merge { both: &mut inner_both, left: &mut left, right: &mut right, one_side_identity: false, equal_identity: true };
        TwoLevel { m: a.merge_with(b, &mut g) }
    }

    /// Do the two maps hold the same entries?
    pub fn same(&self, o: &Self) -> bool {
        self.m.same(&o.m)
    }

    /// The (outer, inner) keys whose value differs between `self` and `base`.
    pub fn diff_keys(&self, base: &Self) -> Vec<(K, IK)> {
        self.diff_upto(base, usize::MAX).unwrap_or_default()
    }

    /// What `other` changed from `base` (markers too), made in this map.
    pub fn copy_changes(&mut self, other: &Self, base: &Self) {
        for p in other.m.diff_keys(&base.m) {
            match other.m.get(&p) {
                Some(v) => self.m.insert(p, v),
                None => {
                    self.m.remove(&p);
                }
            }
        }
    }

    /// `diff_keys`, or None once more than `limit` keys differ.
    pub fn diff_upto(&self, base: &Self, limit: usize) -> Option<Vec<(K, IK)>> {
        Some(self.m.diff_upto(&base.m, limit)?.into_iter().filter(|p| p.1.key32() != SENTINEL).map(|Pair(k, ik)| (k, ik)).collect())
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
        let mut a = m.clone();
        let mut b = m.clone();
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
        let ac = a.clone(); assert!(a.same(&ac));
    }

    #[test]
    fn written_counts_equal_writes() {
        #[derive(Clone, PartialEq)]
        struct W(Arc<u32>);
        impl MapValue for W {
            fn same_write(&self, o: &Self) -> bool {
                Arc::ptr_eq(&self.0, &o.0)
            }
        }
        let mut m: LayerMap<u32, W> = LayerMap::new();
        for i in 0..20 {
            m.insert(i, W(Arc::new(i)));
        }
        let mut a = m.clone();
        a.insert(3, W(Arc::new(3)));
        a.insert(25, W(Arc::new(25)));
        assert_eq!(a.diff_keys(&m), vec![25]);
        let mut w = a.written_upto(&m, 10).unwrap();
        w.sort();
        assert_eq!(w, vec![3, 25]);
        assert!(a.written_upto(&m, 1).is_none());
    }

    #[test]
    fn flatten() {
        let mut m: LayerMap<u32, u32> = LayerMap::new();
        for i in 0..40 {
            m.insert(i, i);
            m = m.clone();
        }
        assert_eq!(m.get(&3), Some(3));
        assert_eq!(m.len(), 40);
    }

    #[test]
    fn two_level() {
        let mut t: TwoLevel<u32, u32, u32> = TwoLevel::new();
        t.set(&1, 1, 5);
        let mut c = t.clone();
        c.set(&1, 2, 6);
        assert_eq!(t.get(&1, &2), None);
        assert_eq!(c.get(&1, &2), Some(6));
        let j = t.join(&c, &mut |a, b| *a.max(b));
        assert_eq!(j.get(&1, &1), Some(5));
        assert_eq!(j.get(&1, &2), Some(6));
        let d = c.diff_keys(&t);
        assert_eq!(d, vec![(1, 2)]);
        // (dropping one key's entries keeps the others, also on the copy it came from)
        let mut m: TwoLevel<u32, u32, u32> = TwoLevel::new();
        for k in 0..6 {
            for ik in 0..5 {
                m.set(&k, ik, k * 10 + ik);
            }
        }
        let keep = m.clone();
        m.drop_key(&3);
        m.drop_key(&9);
        assert!(!m.has_key(&3) && keep.has_key(&3));
        assert!((0..6).filter(|k| *k != 3).all(|k| (0..5).all(|ik| m.get(&k, &ik) == Some(k * 10 + ik))));
        assert_eq!(m.diff_keys(&keep).len(), 5);
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
        let mut a = m.clone();
        let mut b = m.clone();
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
#[path = "laws/tries.rs"]
mod laws;

#[cfg(kani)]
#[path = "laws/kani_tries.rs"]
mod kani_proofs;
