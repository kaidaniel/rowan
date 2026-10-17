//! Persistent maps for the fork/join pattern of abstract states: path-compressed radix-16
//! tries on integer keys (Okasaki & Gill's mergeable Patricia tries with 16-way nodes, as in
//! Bagwell's HAMT and Steindorfer & Vinju's CHAMP, but keyed by the integer itself, so a key
//! set has one shape). A copy is O(1) (an `Arc`), a write copies the path to its leaf
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

/// A trie node. Keys are read four bits (a digit) at a time from the top: a branch at `s`
/// holds keys that agree above bit `s + W` (`p`) and differ in the digit at `s`; it has a
/// child per digit present (`bm`, in digit order), at least two. A node with one child is
/// never made (path compression), so a key set has exactly one shape.
enum Node<K, V> {
    Leaf { h: u64, k: K, v: V },
    Branch { p: u64, s: u8, bm: u16, kids: Box<[Tree<K, V>]> },
}

impl<K: Clone, V: Clone> Clone for Node<K, V> {
    fn clone(&self) -> Self {
        match self {
            Node::Leaf { h, k, v } => Node::Leaf { h: *h, k: k.clone(), v: v.clone() },
            Node::Branch { p, s, bm, kids } => Node::Branch { p: *p, s: *s, bm: *bm, kids: kids.clone() },
        }
    }
}

type Tree<K, V> = Arc<Node<K, V>>;

/// A branch's children by digit while it is built (on the stack).
type Kids<K, V> = smallvec::SmallVec<[(u32, Tree<K, V>); 1 << W]>;

/// The bits above digit `s`.
/// Bits per digit (a branch has up to 2^W children).
const W: u8 = 4;

#[inline]
fn hi_mask(s: u8) -> u64 {
    if s + W >= 64 {
        0
    } else {
        !0u64 << (s + W)
    }
}

#[inline]
fn digit(h: u64, s: u8) -> u32 {
    ((h >> s) & ((1 << W) - 1)) as u32
}

/// The position of digit `d`'s child among a branch's children.
#[inline]
fn slot(bm: u16, d: u32) -> usize {
    (bm & ((1u16 << d) - 1)).count_ones() as usize
}

#[inline]
fn covers(h: u64, p: u64, s: u8) -> bool {
    h & hi_mask(s) == p
}

fn prefix_of<K, V>(t: &Node<K, V>) -> u64 {
    match t {
        Node::Leaf { h, .. } => *h,
        Node::Branch { p, .. } => *p,
    }
}

/// Two trees with disjoint prefixes under one branch.
#[inline]
fn link<K, V>(p0: u64, t0: Tree<K, V>, p1: u64, t1: Tree<K, V>) -> Tree<K, V> {
    let b = 63 - (p0 ^ p1).leading_zeros() as u8;
    let s = b - b % W;
    let (d0, d1) = (digit(p0, s), digit(p1, s));
    let kids: Box<[Tree<K, V>]> = if d0 < d1 { Box::new([t0, t1]) } else { Box::new([t1, t0]) };
    Arc::new(Node::Branch { p: p0 & hi_mask(s), s, bm: (1 << d0) | (1 << d1), kids })
}

/// A branch from its children by digit (possibly none, or one: then that child itself);
/// `orig` is returned when the children are exactly its own.
#[inline]
fn branch<K, V>(p: u64, s: u8, kids: Kids<K, V>, orig: Option<&Tree<K, V>>) -> Option<Tree<K, V>> {
    match kids.len() {
        0 => None,
        1 => kids.into_iter().next().map(|(_, t)| t),
        _ => {
            let bm = kids.iter().fold(0u16, |b, (d, _)| b | (1 << d));
            if let Some(o) = orig {
                if let Node::Branch { bm: obm, kids: ok, .. } = &**o {
                    if *obm == bm && ok.iter().zip(kids.iter()).all(|(a, (_, b))| Arc::ptr_eq(a, b)) {
                        return Some(o.clone());
                    }
                }
            }
            Some(Arc::new(Node::Branch { p, s, bm, kids: kids.into_iter().map(|(_, t)| t).collect() }))
        }
    }
}

/// A branch's children with their digits.
#[inline]
fn kids_of<K, V>(bm: u16, kids: &[Tree<K, V>]) -> impl Iterator<Item = (u32, &Tree<K, V>)> {
    let mut b = bm;
    kids.iter().map(move |t| {
        let d = b.trailing_zeros();
        b &= b - 1;
        (d, t)
    })
}

fn get<'a, K, V>(mut t: &'a Tree<K, V>, h: u64) -> Option<&'a V> {
    loop {
        match &**t {
            Node::Leaf { h: j, v, .. } => return if *j == h { Some(v) } else { None },
            Node::Branch { p, s, bm, kids } => {
                if !covers(h, *p, *s) {
                    return None;
                }
                let d = digit(h, *s);
                if bm & (1 << d) == 0 {
                    return None;
                }
                t = &kids[slot(*bm, d)];
            }
        }
    }
}

fn insert<K: Clone, V: Clone>(t: &mut Tree<K, V>, h: u64, k: K, v: V) {
    let fits = match &**t {
        Node::Leaf { h: j, .. } => *j == h,
        Node::Branch { p, s, .. } => covers(h, *p, *s),
    };
    if !fits {
        let old = t.clone();
        let po = prefix_of(&old);
        *t = link(h, Arc::new(Node::Leaf { h, k, v }), po, old);
        return;
    }
    match Arc::make_mut(t) {
        Node::Leaf { v: val, .. } => *val = v,
        Node::Branch { s, bm, kids, .. } => {
            let d = digit(h, *s);
            let i = slot(*bm, d);
            if *bm & (1 << d) != 0 {
                insert(&mut kids[i], h, k, v)
            } else {
                let mut nk: Vec<Tree<K, V>> = Vec::with_capacity(kids.len() + 1);
                nk.extend_from_slice(&kids[..i]);
                nk.push(Arc::new(Node::Leaf { h, k, v }));
                nk.extend_from_slice(&kids[i..]);
                *kids = nk.into_boxed_slice();
                *bm |= 1 << d;
            }
        }
    }
}

/// `t` without `h`: None when nothing changes, Some(rest) otherwise (rest None: empty).
fn remove<K: Clone, V: Clone>(t: &Tree<K, V>, h: u64) -> Option<(Option<Tree<K, V>>, V)> {
    match &**t {
        Node::Leaf { h: j, v, .. } => (*j == h).then(|| (None, v.clone())),
        Node::Branch { p, s, bm, kids } => {
            if !covers(h, *p, *s) {
                return None;
            }
            let d = digit(h, *s);
            if bm & (1 << d) == 0 {
                return None;
            }
            let (nt, v) = remove(&kids[slot(*bm, d)], h)?;
            Some((replace_kid(*p, *s, *bm, kids, d, nt), v))
        }
    }
}

/// A branch with digit `d`'s child replaced (or dropped).
#[inline]
fn replace_kid<K, V>(p: u64, s: u8, bm: u16, kids: &[Tree<K, V>], d: u32, nt: Option<Tree<K, V>>) -> Option<Tree<K, V>> {
    let mut out: Kids<K, V> = Kids::new();
    for (e, t) in kids_of(bm, kids) {
        if e == d {
            if let Some(n) = &nt {
                out.push((e, n.clone()));
            }
        } else {
            out.push((e, t.clone()));
        }
    }
    branch(p, s, out, None)
}

fn for_each<K, V>(t: &Tree<K, V>, f: &mut dyn FnMut(&K, &V)) {
    match &**t {
        Node::Leaf { k, v, .. } => f(k, v),
        Node::Branch { kids, .. } => kids.iter().for_each(|c| for_each(c, f)),
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
        Node::Branch { p, s, bm, kids } => {
            let out = kids_of(*bm, kids).filter_map(|(d, c)| Some((d, map_side(c, left, g)?))).collect();
            branch(*p, *s, out, None)
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

/// Two results with disjoint prefixes, either possibly empty.
#[inline]
fn link_opt<K, V>(a: Option<Tree<K, V>>, b: Option<Tree<K, V>>) -> Option<Tree<K, V>> {
    match (a, b) {
        (None, x) | (x, None) => x,
        (Some(a), Some(b)) => {
            let (pa, pb) = (prefix_of(&a), prefix_of(&b));
            Some(link(pa, a, pb, b))
        }
    }
}

/// `inner` (a tree under one digit of branch `t`, or a leaf) merged into `t`: the child at
/// its digit merges with it (`f`), the other children are one-sided (`t_left`: t is the left map).
fn merge_into<K: Clone, V: Clone + PartialEq>(t: &Tree<K, V>, d_in: u32, g: &mut Merge<K, V>, t_left: bool, f: &mut dyn FnMut(Option<&Tree<K, V>>, &mut Merge<K, V>) -> Option<Tree<K, V>>) -> Option<Tree<K, V>> {
    let Node::Branch { p, s, bm, kids } = &**t else { unreachable!() };
    let mut out: Kids<K, V> = Kids::new();
    let mut done = false;
    for (d, c) in kids_of(*bm, kids) {
        if !done && d > d_in {
            out.extend(f(None, g).map(|x| (d_in, x)));
            done = true;
        }
        if d == d_in {
            out.extend(f(Some(c), g).map(|x| (d, x)));
            done = true;
        } else {
            out.extend(map_side(c, t_left, g).map(|x| (d, x)));
        }
    }
    if !done {
        out.extend(f(None, g).map(|x| (d_in, x)));
    }
    branch(*p, *s, out, Some(t))
}

/// A leaf of one map merged with a tree of the other.
fn merge_leaf<K: Clone, V: Clone + PartialEq>(s: &Tree<K, V>, t: &Tree<K, V>, leaf_left: bool, g: &mut Merge<K, V>) -> Option<Tree<K, V>> {
    let Node::Leaf { h, k, v } = &**s else { unreachable!() };
    match &**t {
        Node::Leaf { h: j, k: _, v: w } if j == h => {
            if g.equal_identity && v == w {
                return Some(if leaf_left { s.clone() } else { t.clone() });
            }
            if leaf_left {
                both_leaf(*h, k, v, w, g)
            } else {
                both_leaf(*h, k, w, v, g)
            }
        }
        Node::Branch { p, s: sh, .. } if covers(*h, *p, *sh) => {
            let d = digit(*h, *sh);
            merge_into(t, d, g, !leaf_left, &mut |c, g| match c {
                Some(c) => merge_leaf(s, c, leaf_left, g),
                None => map_side(s, leaf_left, g),
            })
        }
        _ => {
            let a = map_side(s, leaf_left, g);
            let b = map_side(t, !leaf_left, g);
            link_opt(a, b)
        }
    }
}

fn merge<K: Clone, V: Clone + PartialEq>(a: &Tree<K, V>, b: &Tree<K, V>, g: &mut Merge<K, V>) -> Option<Tree<K, V>> {
    if g.equal_identity && Arc::ptr_eq(a, b) {
        return Some(a.clone());
    }
    match (&**a, &**b) {
        (Node::Leaf { .. }, _) => merge_leaf(a, b, true, g),
        (_, Node::Leaf { .. }) => merge_leaf(b, a, false, g),
        (Node::Branch { p, s, bm, kids }, Node::Branch { p: q, s: t, bm: bn, kids: kn }) => {
            if s == t && p == q {
                let all = bm | bn;
                let mut out: Kids<K, V> = Kids::new();
                let mut x = all;
                while x != 0 {
                    let d = x.trailing_zeros();
                    x &= x - 1;
                    let r = match (bm & (1 << d) != 0, bn & (1 << d) != 0) {
                        (true, true) => merge(&kids[slot(*bm, d)], &kn[slot(*bn, d)], g),
                        (true, false) => map_side(&kids[slot(*bm, d)], true, g),
                        _ => map_side(&kn[slot(*bn, d)], false, g),
                    };
                    out.extend(r.map(|r| (d, r)));
                }
                branch(*p, *s, out, Some(a))
            } else if s > t && covers(*q, *p, *s) {
                merge_into(a, digit(*q, *s), g, true, &mut |c, g| match c {
                    Some(c) => merge(c, b, g),
                    None => map_side(b, false, g),
                })
            } else if t > s && covers(*p, *q, *t) {
                merge_into(b, digit(*p, *t), g, false, &mut |c, g| match c {
                    Some(c) => merge(a, c, g),
                    None => map_side(a, true, g),
                })
            } else {
                let x = map_side(a, true, g);
                let y = map_side(b, false, g);
                link_opt(x, y)
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
        (Node::Branch { p, s: a, bm, kids }, Node::Branch { p: q, s: b, bm: bn, kids: kn }) => {
            if a == b && p == q {
                let mut x = bm | bn;
                while x != 0 {
                    let d = x.trailing_zeros();
                    x &= x - 1;
                    let l = (bm & (1 << d) != 0).then(|| &kids[slot(*bm, d)]);
                    let r = (bn & (1 << d) != 0).then(|| &kn[slot(*bn, d)]);
                    if !diff(l, r, f) {
                        return false;
                    }
                }
                true
            } else if a > b && covers(*q, *p, *a) {
                let din = digit(*q, *a);
                diff_into(*bm, kids, din, t, f)
            } else if b > a && covers(*p, *q, *b) {
                let din = digit(*p, *b);
                diff_into(*bn, kn, din, s, f)
            } else {
                all_keys(s, f) && all_keys(t, f)
            }
        }
    }
}

/// `diff` of a branch's children and a tree `other` under digit `din` of it.
fn diff_into<K: Clone, V: PartialEq>(bm: u16, kids: &[Tree<K, V>], din: u32, other: &Tree<K, V>, f: &mut dyn FnMut(&K) -> bool) -> bool {
    if bm & (1 << din) == 0 && !all_keys(other, f) {
        return false;
    }
    kids_of(bm, kids).all(|(d, c)| if d == din { diff(Some(c), Some(other), f) } else { all_keys(c, f) })
}

fn all_keys<K, V>(t: &Tree<K, V>, f: &mut dyn FnMut(&K) -> bool) -> bool {
    match &**t {
        Node::Leaf { k, .. } => f(k),
        Node::Branch { kids, .. } => kids.iter().all(|c| all_keys(c, f)),
    }
}

fn all_keys_where<K, V>(t: &Tree<K, V>, f: &mut dyn FnMut(&K, u64, &V) -> bool) -> bool {
    match &**t {
        Node::Leaf { h, k, v } => f(k, *h, v),
        Node::Branch { kids, .. } => kids.iter().all(|c| all_keys_where(c, f)),
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

    /// Do the two maps hold the same entries? (Shared subtrees are not visited.)
    pub fn same(&self, o: &LayerMap<K, V>) -> bool {
        diff(self.root.as_ref(), o.root.as_ref(), &mut |_| false)
    }

    /// Keys whose value differs between self and base.
    pub fn diff_keys(&self, base: &LayerMap<K, V>) -> Vec<K> {
        self.diff_upto(base, usize::MAX).unwrap_or_default()
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

/// The subtree holding exactly the keys whose high 32 bits are `hi` (the trie keeps keys
/// with a common prefix together), if any.
fn find_hi<K, V>(t: &Tree<K, V>, hi: u64) -> Option<&Tree<K, V>> {
    let lo = hi << 32;
    let mut t = t;
    loop {
        match &**t {
            Node::Leaf { h, .. } => return if h >> 32 == hi { Some(t) } else { None },
            Node::Branch { p, s, bm, kids } => {
                if *s < 32 {
                    // (every key below shares the high bits)
                    return if p >> 32 == hi { Some(t) } else { None };
                }
                if !covers(lo, *p, *s) {
                    return None;
                }
                let d = digit(lo, *s);
                if bm & (1 << d) == 0 {
                    return None;
                }
                t = &kids[slot(*bm, d)];
            }
        }
    }
}

/// `t` without the keys whose high 32 bits are `hi`, in one path copy (None: nothing changes;
/// Some(rest), rest None: empty).
fn remove_hi<K: Clone, V: Clone>(t: &Tree<K, V>, hi: u64) -> Option<Option<Tree<K, V>>> {
    match &**t {
        Node::Leaf { h, .. } => (h >> 32 == hi).then_some(None),
        // (every key below shares the high bits)
        Node::Branch { p, s, .. } if *s < 32 => (p >> 32 == hi).then_some(None),
        Node::Branch { p, s, bm, kids } => {
            let lo = hi << 32;
            if !covers(lo, *p, *s) {
                return None;
            }
            let d = digit(lo, *s);
            if bm & (1 << d) == 0 {
                return None;
            }
            let nt = remove_hi(&kids[slot(*bm, d)], hi)?;
            Some(replace_kid(*p, *s, *bm, kids, d, nt))
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

impl<K: PKey, IK: IKey, V: Clone + PartialEq> Default for TwoLevel<K, IK, V> {
    fn default() -> Self {
        TwoLevel { m: LayerMap::new() }
    }
}

impl<K: PKey, IK: IKey, V: Clone + PartialEq> TwoLevel<K, IK, V> {
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
        if let Some(rest) = self.m.root.as_ref().and_then(|t| remove_hi(t, key.key64())) {
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
        t.items().into_iter().map(|(k, d)| (k, d.into_iter().collect())).collect()
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
                        let c = maps[i].clone();
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
                        let got = maps[i].inner(&k).unwrap_or_default();
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
                        let c = maps[i].clone();
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
                        let c = maps[i].clone();
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
                        let c = maps[i].clone();
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
                            let mut a = maps[i].clone();
                            let mut b = maps[j].clone();
                            let joined = a.join_with(&b, &mut |p, q| p.max(q) + 1, &mut |p| p + 1000);
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
                            let got: Model = joined.items().into_iter().map(|(k, d)| (k, d.into_iter().collect::<FxHashMap<u32, u32>>())).filter(|(_, d)| !d.is_empty()).collect();
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
