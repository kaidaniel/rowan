//! Bounded proofs (Kani) of the tries' bit arithmetic and key images, for all inputs where the
//! property tests only sample: `cargo kani --lib` in `rowan/` (`AGENTS.md`, Tests; ~1 minute,
//! mostly the build). Harnesses that allocate (a trie's `Arc` nodes beyond one link, an
//! `IdSet`'s sorted `Arc<[T]>`) ran cbmc past 7-10 minutes even at two elements: the property
//! tests cover those.

use super::*;

/// The branching bit of two different keys is the highest bit they differ in: both keys agree
/// above it (same `mask`), go to different sides (`zero_bit`), and differ in no higher bit.
#[kani::proof]
fn branching_bit_splits_two_keys() {
    let (a, b): (u64, u64) = (kani::any(), kani::any());
    kani::assume(a != b);
    let m = branching_bit(a, b);
    assert!(m.is_power_of_two());
    assert_eq!(mask(a, m), mask(b, m));
    assert!(match_prefix(b, mask(a, m), m));
    assert_ne!(zero_bit(a, m), zero_bit(b, m));
    assert_eq!((a ^ b) & !(m | (m - 1)), 0);
}

/// `link` puts the subtree whose prefix has the branching bit clear on the left, under a prefix
/// both agree with.
#[kani::proof]
#[kani::unwind(3)]
fn link_orders_its_subtrees() {
    let (p0, p1): (u64, u64) = (kani::any(), kani::any());
    kani::assume(p0 != p1);
    let t0: Tree<u32, u32> = Arc::new(Node::Leaf { h: p0, k: 0, v: 0 });
    let t1: Tree<u32, u32> = Arc::new(Node::Leaf { h: p1, k: 1, v: 1 });
    let t = link(p0, t0, p1, t1);
    let Node::Branch { p, m, l, r } = &*t else { panic!("not a branch") };
    assert!(match_prefix(p0, *p, *m) && match_prefix(p1, *p, *m));
    assert!(zero_bit(prefix_of(l), *m) && !zero_bit(prefix_of(r), *m));
}

fn ns(i: u8) -> Ns {
    [Ns::Item, Ns::Attr, Ns::Cell, Ns::Pos, Ns::Elem, Ns::Meta, Ns::Name][i as usize]
}

/// A row's key `(object, namespace)` has an image of its own (the trie never mixes two rows),
/// and an object's rows share the bits above the namespace (`remove_block(.., 3)`).
#[kani::proof]
fn row_keys_are_injective() {
    let (o1, o2): (u32, u32) = (kani::any(), kani::any());
    let (n1, n2): (u8, u8) = (kani::any(), kani::any());
    kani::assume(n1 < 7 && n2 < 7);
    let (a, b) = ((o1, ns(n1)), (o2, ns(n2)));
    assert_eq!(a.key64() == b.key64(), a == b);
    assert_eq!(a.key64() >> 3, o1 as u64);
}

/// A slot's key (object, (namespace, key)) has an image of its own, never the marker's, and its
/// object's entries share the high 32 bits (`find_hi`, `drop_key`).
#[kani::proof]
fn slot_keys_are_injective() {
    let (o1, o2): (u32, u32) = (kani::any(), kani::any());
    let (n1, n2): (u8, u8) = (kani::any(), kani::any());
    let (k1, k2): (u32, u32) = (kani::any(), kani::any());
    kani::assume(n1 < 7 && n2 < 7 && k1 < SENTINEL && k2 < SENTINEL);
    let a = Pair(o1, (ns(n1), KeyId(k1)));
    let b = Pair(o2, (ns(n2), KeyId(k2)));
    assert_eq!(a.key64() == b.key64(), a == b);
    assert_ne!(a.1.key32(), SENTINEL);
    assert_eq!(a.key64() >> 32, o1 as u64);
}
