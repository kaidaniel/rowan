//! Bounded proofs (Kani) of the sorted id sets: every operation agrees with a bit mask of the
//! same elements, for all sets of up to three ids below 8 (the merge loops' every interleaving)
//! and, for `union_bits`, two sets of two ids anywhere in two words.

use super::*;

/// A set of up to three symbolic ids below `n` and its mask.
fn any_set(n: u32) -> (IdSet<u32>, u64) {
    let len: usize = kani::any();
    kani::assume(len <= 3);
    let xs: [u32; 3] = [kani::any(), kani::any(), kani::any()];
    let mut v = Vec::new();
    let mut mask = 0u64;
    for x in xs.iter().take(len) {
        kani::assume(*x < n);
        v.push(*x);
        mask |= 1 << x;
    }
    (IdSet::from_vec(v), mask)
}

fn mask_of(s: &IdSet<u32>) -> u64 {
    // (sorted, without repeats)
    assert!(s.as_slice().windows(2).all(|w| w[0] < w[1]));
    s.iter().fold(0, |m, x| m | 1 << x)
}

#[kani::proof]
#[kani::unwind(8)]
fn set_operations_agree_with_masks() {
    let (a, ma) = any_set(8);
    let (b, mb) = any_set(8);
    assert_eq!(mask_of(&a.union(&b)), ma | mb);
    assert_eq!(mask_of(&a.intersection(&b)), ma & mb);
    assert_eq!(mask_of(&a.difference(&b)), ma & !mb);
    assert_eq!(a.is_subset(&b), ma & !mb == 0);
    assert_eq!(a.intersects(&b), ma & mb != 0);
    let x: u32 = kani::any();
    kani::assume(x < 8);
    assert_eq!(a.contains(x), ma >> x & 1 == 1);
    assert_eq!(mask_of(&a.insert(x)), ma | 1 << x);
    assert_eq!(mask_of(&a.remove(x)), ma & !(1 << x));
}

/// `union_all` past its threshold: the bit per id lands on the id it stands for.
#[kani::proof]
#[kani::unwind(5)]
fn union_bits_is_the_union() {
    let two = || {
        let (x, y): (u32, u32) = (kani::any(), kani::any());
        kani::assume(x < 128 && y < 128);
        IdSet::from_vec(vec![x, y])
    };
    let (a, b) = (two(), two());
    let u = IdSet::union_bits(&[&a, &b], |x| x as usize, |i| i as u32);
    assert!(u == a.union(&b));
}
