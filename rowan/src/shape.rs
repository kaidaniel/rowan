//! Tensor shapes (TORCH_PLAN.md §1): symbolic dims as normalised polynomials over atoms,
//! shapes as `prefix, [*rest], suffix` with assumptions, constraints (requirements / facts),
//! and the relational facts a path carries (`TFacts`: ranks and dim equalities).
//!
//! Values: a tensor is `KeyVal::Data` labelled `\x01T<shape id>`, a symbolic int
//! `\x01D<dim id>`, a scalar number `\x01N`; everything that does not know about tensors
//! treats them as unknown data, which is sound.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;

use rustc_hash::FxHashMap;

// ---------------------------------------------------------------------------
// interned ids
// ---------------------------------------------------------------------------

/// A named unknown: (function scope, program text), e.g. (f, "x"), (0, "cfg.d").
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct Sym(pub u32);
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct AtomId(pub u32);
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct DimId(pub u32);
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct ShapeId(pub u32);

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Atom {
    /// `x.shape[i]` (i < 0: from the end)
    Dim(Sym, i32),
    /// a named unknown int (`n`, `self.d_model`)
    Named(Sym),
    FloorDiv(DimId, DimId),
    Mod(DimId, DimId),
    /// broadcast of two dims not known equal (args sorted)
    Max(DimId, DimId),
    /// a slice bound (args sorted)
    Min(DimId, DimId),
    /// an unknown dim: never known equal to anything
    Unknown,
}

/// monomial: sorted (atom, power); polynomial: sorted (monomial, coefficient), no zeros
pub type Mono = Vec<(AtomId, u32)>;
pub type Poly = Vec<(Mono, i64)>;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Rest {
    /// `x.shape[from : len-back]`
    Slice(Sym, u32, u32),
    /// broadcast of several parts (sorted, deduplicated)
    Bcast(Vec<Rest>),
    /// known dims (only inside `Bcast`)
    Dims(Vec<DimId>),
    /// unknown number of unknown dims
    Any,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum RankRel {
    Eq(u32),
    In(Vec<u32>),
    Ge(u32),
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Cons {
    Eq(DimId, DimId),
    Rank(Sym, RankRel),
    /// `main if given…`
    If(Box<Cons>, Vec<Cons>),
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Shape {
    pub prefix: Vec<DimId>,
    pub rest: Option<Rest>,
    pub suffix: Vec<DimId>,
    /// the shape holds under these (rank) assumptions
    pub assume: Vec<Cons>,
}

impl Shape {
    pub fn known(dims: Vec<DimId>) -> Shape {
        Shape { prefix: dims, rest: None, suffix: vec![], assume: vec![] }
    }
    pub fn any() -> Shape {
        Shape { prefix: vec![], rest: Some(Rest::Any), suffix: vec![], assume: vec![] }
    }
    pub fn of_sym(s: Sym) -> Shape {
        Shape { prefix: vec![], rest: Some(Rest::Slice(s, 0, 0)), suffix: vec![], assume: vec![] }
    }
    pub fn rank(&self) -> Option<usize> {
        if self.rest.is_none() {
            Some(self.prefix.len() + self.suffix.len())
        } else {
            None
        }
    }
    /// at least this many dims
    pub fn min_rank(&self) -> usize {
        self.prefix.len() + self.suffix.len()
    }
    pub fn dims(&self) -> Option<Vec<DimId>> {
        if self.rest.is_none() {
            let mut d = self.prefix.clone();
            d.extend(self.suffix.iter().copied());
            Some(d)
        } else {
            None
        }
    }
    pub fn with_assume(mut self, a: &[Cons]) -> Shape {
        for c in a {
            if !self.assume.contains(c) {
                self.assume.push(c.clone());
            }
        }
        self
    }
    pub fn is_any(&self) -> bool {
        self.prefix.is_empty() && self.suffix.is_empty() && matches!(self.rest, Some(Rest::Any))
    }
}

struct Tables {
    syms: Vec<(u32, String)>,
    sym_map: FxHashMap<(u32, String), Sym>,
    atoms: Vec<Atom>,
    atom_map: FxHashMap<Atom, AtomId>,
    dims: Vec<Poly>,
    dim_map: FxHashMap<Poly, DimId>,
    shapes: Vec<Shape>,
    shape_map: FxHashMap<Shape, ShapeId>,
}

thread_local! {
    static T: RefCell<Tables> = RefCell::new(Tables {
        syms: vec![], sym_map: FxHashMap::default(), atoms: vec![], atom_map: FxHashMap::default(),
        dims: vec![], dim_map: FxHashMap::default(), shapes: vec![], shape_map: FxHashMap::default(),
    });
}

pub fn sym(scope: u32, text: &str) -> Sym {
    T.with(|t| {
        let mut t = t.borrow_mut();
        let k = (scope, text.to_string());
        if let Some(s) = t.sym_map.get(&k) {
            return *s;
        }
        let id = Sym(t.syms.len() as u32);
        t.syms.push(k.clone());
        t.sym_map.insert(k, id);
        id
    })
}
pub fn sym_text(s: Sym) -> String {
    T.with(|t| t.borrow().syms[s.0 as usize].1.clone())
}
pub fn sym_scope(s: Sym) -> u32 {
    T.with(|t| t.borrow().syms[s.0 as usize].0)
}
pub fn atom(a: Atom) -> AtomId {
    T.with(|t| {
        let mut t = t.borrow_mut();
        if let Some(i) = t.atom_map.get(&a) {
            return *i;
        }
        let id = AtomId(t.atoms.len() as u32);
        t.atoms.push(a.clone());
        t.atom_map.insert(a, id);
        id
    })
}
pub fn atom_get(a: AtomId) -> Atom {
    T.with(|t| t.borrow().atoms[a.0 as usize].clone())
}
fn poly_id(p: Poly) -> DimId {
    T.with(|t| {
        let mut t = t.borrow_mut();
        if let Some(i) = t.dim_map.get(&p) {
            return *i;
        }
        let id = DimId(t.dims.len() as u32);
        t.dims.push(p.clone());
        t.dim_map.insert(p, id);
        id
    })
}
pub fn poly(d: DimId) -> Poly {
    T.with(|t| t.borrow().dims[d.0 as usize].clone())
}
pub fn shape_id(s: Shape) -> ShapeId {
    T.with(|t| {
        let mut t = t.borrow_mut();
        if let Some(i) = t.shape_map.get(&s) {
            return *i;
        }
        let id = ShapeId(t.shapes.len() as u32);
        t.shapes.push(s.clone());
        t.shape_map.insert(s, id);
        id
    })
}
pub fn shape_get(s: ShapeId) -> Shape {
    T.with(|t| t.borrow().shapes[s.0 as usize].clone())
}

// ---------------------------------------------------------------------------
// dims: construction and arithmetic
// ---------------------------------------------------------------------------

pub fn lit(n: i64) -> DimId {
    if n == 0 {
        poly_id(vec![])
    } else {
        poly_id(vec![(vec![], n)])
    }
}
pub fn of_atom(a: Atom) -> DimId {
    poly_id(vec![(vec![(atom(a), 1)], 1)])
}
pub fn sdim(s: Sym, i: i32) -> DimId {
    of_atom(Atom::Dim(s, i))
}
pub fn named(s: Sym) -> DimId {
    of_atom(Atom::Named(s))
}
pub fn unknown() -> DimId {
    of_atom(Atom::Unknown)
}
pub fn as_lit(d: DimId) -> Option<i64> {
    let p = poly(d);
    match &p[..] {
        [] => Some(0),
        [(m, c)] if m.is_empty() => Some(*c),
        _ => None,
    }
}
pub fn has_unknown(d: DimId) -> bool {
    poly(d).iter().any(|(m, _)| m.iter().any(|(a, _)| atom_has_unknown(*a)))
}
fn atom_has_unknown(a: AtomId) -> bool {
    match atom_get(a) {
        Atom::Unknown => true,
        Atom::FloorDiv(x, y) | Atom::Mod(x, y) | Atom::Max(x, y) | Atom::Min(x, y) => has_unknown(x) || has_unknown(y),
        _ => false,
    }
}

fn norm_poly(mut terms: Vec<(Mono, i64)>) -> Poly {
    for t in terms.iter_mut() {
        t.0.sort();
        // merge equal atoms in a monomial
        let mut m: Mono = Vec::new();
        for (a, p) in t.0.drain(..) {
            if let Some(last) = m.last_mut() {
                if last.0 == a {
                    last.1 += p;
                    continue;
                }
            }
            m.push((a, p));
        }
        t.0 = m;
    }
    terms.sort_by(|a, b| a.0.cmp(&b.0));
    let mut out: Poly = Vec::new();
    for (m, c) in terms {
        if let Some(last) = out.last_mut() {
            if last.0 == m {
                last.1 += c;
                continue;
            }
        }
        out.push((m, c));
    }
    out.retain(|(_, c)| *c != 0);
    out
}

pub fn add(a: DimId, b: DimId) -> DimId {
    let mut t = poly(a);
    t.extend(poly(b));
    poly_id(norm_poly(t))
}
pub fn scale(a: DimId, k: i64) -> DimId {
    poly_id(norm_poly(poly(a).into_iter().map(|(m, c)| (m, c * k)).collect()))
}
pub fn sub(a: DimId, b: DimId) -> DimId {
    add(a, scale(b, -1))
}
pub fn mul(a: DimId, b: DimId) -> DimId {
    let pa = poly(a);
    let pb = poly(b);
    let mut t = Vec::new();
    for (m1, c1) in &pa {
        for (m2, c2) in &pb {
            let mut m = m1.clone();
            m.extend(m2.iter().copied());
            t.push((m, c1 * c2));
        }
    }
    poly_id(norm_poly(t))
}
pub fn product(ds: &[DimId]) -> DimId {
    ds.iter().fold(lit(1), |acc, d| mul(acc, *d))
}

/// Exact division of polynomials when `b` is a monomial (or a constant) dividing every term.
pub fn div_exact(a: DimId, b: DimId) -> Option<DimId> {
    let pa = poly(a);
    let pb = poly(b);
    if pb.len() != 1 {
        if a == b {
            return Some(lit(1));
        }
        // (k*p) / p
        if !pa.is_empty() && pa.len() == pb.len() {
            let k0 = pa[0].1 / pb[0].1;
            if k0 != 0 && pa.iter().zip(pb.iter()).all(|((m1, c1), (m2, c2))| m1 == m2 && *c1 == k0 * *c2) {
                return Some(lit(k0));
            }
        }
        return None;
    }
    let (bm, bc) = &pb[0];
    let mut out = Vec::new();
    for (m, c) in &pa {
        if *bc == 0 || c % bc != 0 {
            return None;
        }
        let mut mm: Mono = m.clone();
        for (ba, bp) in bm {
            let pos = mm.iter().position(|(a, _)| a == ba)?;
            if mm[pos].1 < *bp {
                return None;
            }
            mm[pos].1 -= bp;
            if mm[pos].1 == 0 {
                mm.remove(pos);
            }
        }
        out.push((mm, c / bc));
    }
    Some(poly_id(norm_poly(out)))
}

pub fn floordiv(a: DimId, b: DimId) -> DimId {
    if let (Some(x), Some(y)) = (as_lit(a), as_lit(b)) {
        if y != 0 {
            return lit(x.div_euclid(y) - if x.rem_euclid(y) != 0 && y < 0 { 1 } else { 0 });
        }
    }
    if as_lit(b) == Some(1) {
        return a;
    }
    if let Some(q) = div_exact(a, b) {
        return q;
    }
    of_atom(Atom::FloorDiv(a, b))
}
pub fn modulo(a: DimId, b: DimId) -> DimId {
    if let (Some(x), Some(y)) = (as_lit(a), as_lit(b)) {
        if y != 0 {
            return lit(x.rem_euclid(y));
        }
    }
    if div_exact(a, b).is_some() {
        return lit(0);
    }
    of_atom(Atom::Mod(a, b))
}
pub fn max(a: DimId, b: DimId) -> DimId {
    if a == b {
        return a;
    }
    if let (Some(x), Some(y)) = (as_lit(a), as_lit(b)) {
        return lit(x.max(y));
    }
    let (x, y) = if a <= b { (a, b) } else { (b, a) };
    of_atom(Atom::Max(x, y))
}
pub fn min(a: DimId, b: DimId) -> DimId {
    if a == b {
        return a;
    }
    if let (Some(x), Some(y)) = (as_lit(a), as_lit(b)) {
        return lit(x.min(y));
    }
    let (x, y) = if a <= b { (a, b) } else { (b, a) };
    of_atom(Atom::Min(x, y))
}
/// ceil(a / b) for positive b
pub fn ceildiv(a: DimId, b: DimId) -> DimId {
    if let Some(q) = div_exact(a, b) {
        return q;
    }
    floordiv(sub(add(a, b), lit(1)), b)
}

// ---------------------------------------------------------------------------
// facts
// ---------------------------------------------------------------------------

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct TFacts {
    pub rank: BTreeMap<Sym, RankRel>,
    /// atom := dim (oriented: a literal wins, then a named int, then the older atom)
    pub subst: BTreeMap<AtomId, DimId>,
}

impl TFacts {
    pub fn is_empty(&self) -> bool {
        self.rank.is_empty() && self.subst.is_empty()
    }
    pub fn join(&self, o: &TFacts) -> TFacts {
        let mut rank = BTreeMap::new();
        for (s, r) in &self.rank {
            if let Some(r2) = o.rank.get(s) {
                if let Some(j) = rank_join(r, r2) {
                    rank.insert(*s, j);
                }
            }
        }
        let subst = self.subst.iter().filter(|(a, d)| o.subst.get(a) == Some(d)).map(|(a, d)| (*a, *d)).collect();
        TFacts { rank, subst }
    }
    pub fn rank_of(&self, s: Sym) -> Option<u32> {
        match self.rank.get(&s) {
            Some(RankRel::Eq(r)) => Some(*r),
            _ => None,
        }
    }
    pub fn rank_ge(&self, s: Sym) -> u32 {
        match self.rank.get(&s) {
            Some(RankRel::Eq(r)) => *r,
            Some(RankRel::Ge(r)) => *r,
            Some(RankRel::In(v)) => v.iter().copied().min().unwrap_or(0),
            None => 0,
        }
    }
}

fn rank_join(a: &RankRel, b: &RankRel) -> Option<RankRel> {
    let set = |r: &RankRel| -> Option<Vec<u32>> {
        match r {
            RankRel::Eq(n) => Some(vec![*n]),
            RankRel::In(v) => Some(v.clone()),
            RankRel::Ge(_) => None,
        }
    };
    let lo = |r: &RankRel| -> u32 {
        match r {
            RankRel::Eq(n) => *n,
            RankRel::In(v) => v.iter().copied().min().unwrap_or(0),
            RankRel::Ge(n) => *n,
        }
    };
    match (set(a), set(b)) {
        (Some(x), Some(y)) => {
            let mut u: Vec<u32> = x.into_iter().chain(y).collect();
            u.sort();
            u.dedup();
            Some(if u.len() == 1 { RankRel::Eq(u[0]) } else { RankRel::In(u) })
        }
        _ => {
            let m = lo(a).min(lo(b));
            if m >= 2 {
                Some(RankRel::Ge(m))
            } else {
                None
            }
        }
    }
}

/// Normalise a dim under the facts: negative indices of symbols of known rank become
/// non-negative, substituted atoms are replaced, compound atoms normalised inside.
pub fn norm(d: DimId, f: &TFacts) -> DimId {
    let mut cur = d;
    for _ in 0..6 {
        let next = norm_once(cur, f);
        if next == cur {
            return cur;
        }
        cur = next;
    }
    cur
}

fn norm_once(d: DimId, f: &TFacts) -> DimId {
    let p = poly(d);
    if p.iter().all(|(m, _)| m.is_empty()) {
        return d;
    }
    let mut acc = lit(0);
    for (m, c) in p {
        let mut term = lit(c);
        for (a, pw) in m {
            let ad = norm_atom(a, f);
            for _ in 0..pw {
                term = mul(term, ad);
            }
        }
        acc = add(acc, term);
    }
    acc
}

fn norm_atom(a: AtomId, f: &TFacts) -> DimId {
    let at = atom_get(a);
    let a2 = match at {
        Atom::Dim(s, i) if i < 0 => match f.rank_of(s) {
            Some(r) if (r as i32) + i >= 0 => atom(Atom::Dim(s, r as i32 + i)),
            _ => a,
        },
        _ => a,
    };
    if let Some(d) = f.subst.get(&a2) {
        return *d;
    }
    match atom_get(a2) {
        Atom::FloorDiv(x, y) => floordiv(norm(x, f), norm(y, f)),
        Atom::Mod(x, y) => modulo(norm(x, f), norm(y, f)),
        Atom::Max(x, y) => max(norm(x, f), norm(y, f)),
        Atom::Min(x, y) => min(norm(x, f), norm(y, f)),
        _ => poly_id(vec![(vec![(a2, 1)], 1)]),
    }
}

/// Are two dims equal? Some(true/false) when decided.
pub fn dim_eq(a: DimId, b: DimId, f: &TFacts) -> Option<bool> {
    let a = norm(a, f);
    let b = norm(b, f);
    if has_unknown(a) || has_unknown(b) {
        return None;
    }
    let d = sub(a, b);
    match as_lit(d) {
        Some(0) => Some(true),
        Some(_) => Some(false),
        None => None,
    }
}

/// Is `d` exactly 1 / known not 1?
pub fn is_one(d: DimId, f: &TFacts) -> Option<bool> {
    dim_eq(d, lit(1), f)
}

/// Learn `a == b` (both normalised): orient and add a substitution.
pub fn learn_eq(f: &mut TFacts, a: DimId, b: DimId) {
    let a = norm(a, f);
    let b = norm(b, f);
    if a == b || has_unknown(a) || has_unknown(b) {
        return;
    }
    // the side that is a single atom (with coefficient 1) is rewritten into the other
    let single = |d: DimId| -> Option<AtomId> {
        let p = poly(d);
        match &p[..] {
            [(m, 1)] if m.len() == 1 && m[0].1 == 1 => Some(m[0].0),
            _ => None,
        }
    };
    let rank_of = |d: DimId| -> u8 {
        // 0: literal, 1: named int, 2: compound, 3: shape dim
        if as_lit(d).is_some() {
            return 0;
        }
        match single(d).map(atom_get) {
            Some(Atom::Named(_)) => 1,
            Some(Atom::Dim(..)) => 3,
            _ => 2,
        }
    };
    let (ra, rb) = (rank_of(a), rank_of(b));
    // rewrite the "worse" side (higher rank; ties: the newer atom) if it is a single atom
    let (from, to) = if ra > rb || (ra == rb && a > b) { (a, b) } else { (b, a) };
    if let Some(at) = single(from) {
        if !mentions(to, at) {
            f.subst.insert(at, to);
            return;
        }
    }
    if let Some(at) = single(to) {
        if !mentions(from, at) {
            f.subst.insert(at, from);
        }
    }
}

fn mentions(d: DimId, a: AtomId) -> bool {
    poly(d).iter().any(|(m, _)| {
        m.iter().any(|(x, _)| {
            *x == a
                || match atom_get(*x) {
                    Atom::FloorDiv(p, q) | Atom::Mod(p, q) | Atom::Max(p, q) | Atom::Min(p, q) => mentions(p, a) || mentions(q, a),
                    _ => false,
                }
        })
    })
}

pub fn learn_rank(f: &mut TFacts, s: Sym, r: RankRel) {
    let new = match (f.rank.get(&s), &r) {
        (Some(RankRel::Eq(_)), _) => return,
        (Some(RankRel::Ge(a)), RankRel::Ge(b)) => RankRel::Ge((*a).max(*b)),
        (Some(RankRel::In(v)), RankRel::Ge(b)) => {
            let w: Vec<u32> = v.iter().copied().filter(|x| x >= b).collect();
            match w.len() {
                0 => return,
                1 => RankRel::Eq(w[0]),
                _ => RankRel::In(w),
            }
        }
        (Some(RankRel::In(v)), RankRel::In(u)) => {
            let w: Vec<u32> = v.iter().copied().filter(|x| u.contains(x)).collect();
            match w.len() {
                0 => return,
                1 => RankRel::Eq(w[0]),
                _ => RankRel::In(w),
            }
        }
        (Some(RankRel::Ge(a)), RankRel::In(u)) => {
            let w: Vec<u32> = u.iter().copied().filter(|x| x >= a).collect();
            match w.len() {
                0 => return,
                1 => RankRel::Eq(w[0]),
                _ => RankRel::In(w),
            }
        }
        _ => r,
    };
    f.rank.insert(s, new);
}

/// Decide a constraint under the facts (None: undecided).
pub fn decide(c: &Cons, f: &TFacts) -> Option<bool> {
    match c {
        Cons::Eq(a, b) => dim_eq(*a, *b, f),
        Cons::Rank(s, rel) => {
            let known = f.rank.get(s);
            match (known, rel) {
                (Some(RankRel::Eq(n)), RankRel::Eq(m)) => Some(n == m),
                (Some(RankRel::Eq(n)), RankRel::In(v)) => Some(v.contains(n)),
                (Some(RankRel::Eq(n)), RankRel::Ge(m)) => Some(n >= m),
                (Some(RankRel::Ge(n)), RankRel::Ge(m)) if n >= m => Some(true),
                (Some(RankRel::Ge(n)), RankRel::Eq(m)) if m < n => Some(false),
                (Some(RankRel::In(v)), RankRel::Ge(m)) if v.iter().all(|x| x >= m) => Some(true),
                (Some(RankRel::In(v)), RankRel::In(u)) if v.iter().all(|x| u.contains(x)) => Some(true),
                (Some(RankRel::In(v)), RankRel::Eq(m)) if !v.contains(m) => Some(false),
                _ => None,
            }
        }
        Cons::If(main, given) => {
            let mut all_true = true;
            for g in given {
                match decide(g, f) {
                    Some(false) => return Some(true),
                    Some(true) => {}
                    None => all_true = false,
                }
            }
            if all_true {
                decide(main, f)
            } else {
                None
            }
        }
    }
}

/// A requirement that held: learn it as a fact (conditional ones teach nothing).
pub fn learn(f: &mut TFacts, c: &Cons) {
    match c {
        Cons::Eq(a, b) => learn_eq(f, *a, *b),
        Cons::Rank(s, r) => learn_rank(f, *s, r.clone()),
        Cons::If(..) => {}
    }
}

// ---------------------------------------------------------------------------
// shapes
// ---------------------------------------------------------------------------

/// Normalise a shape under the facts: rests of known rank expand, dims normalise,
/// assumptions that hold are dropped (one that fails makes the shape unknown).
pub fn norm_shape(s: &Shape, f: &TFacts) -> Shape {
    let mut out = s.clone();
    let mut keep = Vec::new();
    for a in &s.assume {
        match decide(a, f) {
            Some(true) => {}
            Some(false) => return Shape::any(),
            None => keep.push(a.clone()),
        }
    }
    out.assume = keep;
    if let Some(Rest::Slice(sym, from, back)) = &s.rest {
        if let Some(r) = f.rank_of(*sym) {
            let r = r as i64;
            let (from, back) = (*from as i64, *back as i64);
            if r - from - back >= 0 {
                let mid: Vec<DimId> = (from..r - back).map(|i| sdim(*sym, i as i32)).collect();
                out.prefix.extend(mid);
                out.prefix.extend(out.suffix.drain(..));
                out.rest = None;
            }
        }
    }
    if let Some(Rest::Bcast(parts)) = &s.rest {
        // a broadcast whose parts all have known rank becomes known dims
        let mut lists: Vec<Vec<DimId>> = Vec::new();
        for p in parts {
            match p {
                Rest::Dims(d) => lists.push(d.clone()),
                Rest::Slice(sym, from, back) => match f.rank_of(*sym) {
                    Some(r) if r as i64 - *from as i64 - *back as i64 >= 0 => lists.push((*from as i64..r as i64 - *back as i64).map(|i| sdim(*sym, i as i32)).collect()),
                    _ => {
                        lists.clear();
                        break;
                    }
                },
                _ => {
                    lists.clear();
                    break;
                }
            }
        }
        if !lists.is_empty() {
            if let Ok(d) = broadcast_lists(&lists, f) {
                out.prefix.extend(d);
                out.prefix.extend(out.suffix.drain(..));
                out.rest = None;
            }
        }
    }
    // dims of an unsliced symbol that facts pin down are spelled out (from the end
    // preferably; never both ends, which could overlap while the rank is unknown)
    if let Some(Rest::Slice(sym, from, back)) = out.rest.clone() {
        let pinned_back: Vec<i32> = f.subst.keys().filter_map(|a| match atom_get(*a) {
            Atom::Dim(s2, i) if s2 == sym && i < 0 => Some(-i),
            _ => None,
        }).collect();
        let pinned_front: Vec<i32> = f.subst.keys().filter_map(|a| match atom_get(*a) {
            Atom::Dim(s2, i) if s2 == sym && i >= 0 => Some(i + 1),
            _ => None,
        }).collect();
        if let Some(&j) = pinned_back.iter().max() {
            if out.prefix.is_empty() && from == 0 && j as u32 > back {
                let extra: Vec<DimId> = (back as i32 + 1..=j).rev().map(|k| sdim(sym, -k)).collect();
                let mut suf = extra;
                suf.extend(out.suffix.iter().copied());
                out.suffix = suf;
                out.rest = Some(Rest::Slice(sym, 0, j as u32));
            }
        } else if let Some(&j) = pinned_front.iter().max() {
            if out.suffix.is_empty() && back == 0 && j as u32 > from && out.prefix.len() as u32 == from {
                for k in from as i32..j {
                    out.prefix.push(sdim(sym, k));
                }
                out.rest = Some(Rest::Slice(sym, j as u32, 0));
            }
        }
    }
    out.prefix = out.prefix.iter().map(|d| norm(*d, f)).collect();
    out.suffix = out.suffix.iter().map(|d| norm(*d, f)).collect();
    out
}

/// Broadcast dims; Err((a, b)) on a definite mismatch.
pub fn bdim(a: DimId, b: DimId, f: &TFacts) -> Result<DimId, (DimId, DimId)> {
    if has_unknown(a) && is_one(b, f) != Some(true) || has_unknown(b) && is_one(a, f) != Some(true) {
        return Ok(unknown());
    }
    if is_one(a, f) == Some(true) {
        return Ok(b);
    }
    if is_one(b, f) == Some(true) {
        return Ok(a);
    }
    match dim_eq(a, b, f) {
        Some(true) => Ok(a),
        Some(false) if is_one(a, f) == Some(false) && is_one(b, f) == Some(false) => Err((a, b)),
        _ => Ok(max(a, b)),
    }
}

fn broadcast_lists(lists: &[Vec<DimId>], f: &TFacts) -> Result<Vec<DimId>, (DimId, DimId)> {
    let n = lists.iter().map(|l| l.len()).max().unwrap_or(0);
    let mut out = vec![lit(1); n];
    for l in lists {
        let off = n - l.len();
        for (i, d) in l.iter().enumerate() {
            out[off + i] = bdim(out[off + i], *d, f)?;
        }
    }
    Ok(out)
}

/// Broadcast two shapes (elementwise ops). Err on a definite mismatch.
pub fn broadcast(a: &Shape, b: &Shape, f: &TFacts) -> Result<Shape, (DimId, DimId)> {
    let a = norm_shape(a, f);
    let b = norm_shape(b, f);
    let mut assume = a.assume.clone();
    for c in &b.assume {
        if !assume.contains(c) {
            assume.push(c.clone());
        }
    }
    if let (Some(x), Some(y)) = (a.dims(), b.dims()) {
        let mut s = Shape::known(broadcast_lists(&[x, y], f)?);
        s.assume = assume;
        return Ok(s);
    }
    // one side of known rank no longer than the other's known tail: the other's shape with
    // its last dims broadcast (the known side cannot add dims)
    for (x, y, swap) in [(&a, &b, false), (&b, &a, true)] {
        if let (Some(_), Some(dy)) = (&x.rest, y.dims()) {
            if dy.len() <= x.suffix.len() {
                let k = x.suffix.len() - dy.len();
                let mut tail = x.suffix[..k].to_vec();
                for (p, q) in x.suffix[k..].iter().zip(dy.iter()) {
                    tail.push(if swap { bdim(*q, *p, f)? } else { bdim(*p, *q, f)? });
                }
                let mut assume = a.assume.clone();
                for c in &b.assume {
                    if !assume.contains(c) {
                        assume.push(c.clone());
                    }
                }
                return Ok(Shape { prefix: x.prefix.clone(), rest: x.rest.clone(), suffix: tail, assume });
            }
        }
    }
    // a known shape against a fully unknown one: at least its rank, its dims maybe larger
    for (x, y) in [(&a, &b), (&b, &a)] {
        if y.is_any() {
            let n = match x.dims() {
                Some(dx) => dx.len(),
                None => x.suffix.len(),
            };
            return Ok(Shape { prefix: vec![], rest: Some(Rest::Any), suffix: vec![unknown(); n], assume: vec![] });
        }
    }
    // rests over the same symbol: spell out the same number of trailing dims on both
    // sides (they come from one tensor, so it has them)
    let (a, b) = match (&a.rest, &b.rest) {
        (Some(Rest::Slice(x, _, _)), Some(Rest::Slice(y, _, _))) if x == y && a.suffix.len() != b.suffix.len() => {
            let k = a.suffix.len().max(b.suffix.len());
            (expose(&a, 0, k), expose(&b, 0, k))
        }
        _ => (a, b),
    };
    // trailing dims known on both sides: broadcast them, the rest broadcasts symbolically
    let (sa, sb) = (&a.suffix, &b.suffix);
    let (ka, kb) = (if a.rest.is_none() { a.prefix.len() } else { sa.len() }, if b.rest.is_none() { b.prefix.len() } else { sb.len() });
    let ta: Vec<DimId> = if a.rest.is_none() { a.prefix.clone() } else { sa.clone() };
    let tb: Vec<DimId> = if b.rest.is_none() { b.prefix.clone() } else { sb.clone() };
    let common = ka.min(kb);
    let mut tail = Vec::new();
    for i in 0..common {
        tail.push(bdim(ta[ta.len() - common + i], tb[tb.len() - common + i], f)?);
    }
    let head_a = rest_part(&a, ka - common);
    let head_b = rest_part(&b, kb - common);
    let rest = bcast_rests(vec![head_a, head_b]);
    Ok(Shape { prefix: vec![], rest: Some(rest), suffix: tail, assume })
}

/// The leading part of a shape (all but its last `len(tail) - extra_tail` known trailing
/// dims) as a rest.
fn rest_part(s: &Shape, extra_tail: usize) -> Rest {
    match &s.rest {
        None => Rest::Dims(s.dims().unwrap()[..extra_tail].to_vec()),
        Some(r) => merge_rest(&s.prefix, r, &s.suffix[..extra_tail]),
    }
}

/// `pre + *r + post` as one rest (when the known dims continue a slice), else `*`.
pub fn merge_rest(pre: &[DimId], r: &Rest, post: &[DimId]) -> Rest {
    if pre.is_empty() && post.is_empty() {
        return r.clone();
    }
    if let Rest::Slice(sym, from, back) = r {
        let pre_ok = pre.len() as u32 <= *from && pre.iter().enumerate().all(|(i, d)| *d == sdim(*sym, (*from as usize - pre.len() + i) as i32));
        let post_ok = post.len() as u32 <= *back && post.iter().enumerate().all(|(i, d)| *d == sdim(*sym, -(*back as i32) + i as i32));
        if pre_ok && post_ok {
            return Rest::Slice(*sym, from - pre.len() as u32, back - post.len() as u32);
        }
    }
    Rest::Any
}

fn bcast_rests(parts: Vec<Rest>) -> Rest {
    let mut flat: Vec<Rest> = Vec::new();
    for p in parts {
        match p {
            Rest::Bcast(v) => flat.extend(v),
            Rest::Dims(d) if d.is_empty() => {}
            Rest::Any => return Rest::Any,
            other => flat.push(other),
        }
    }
    flat.sort_by_key(rest_text);
    flat.dedup();
    match flat.len() {
        0 => Rest::Dims(vec![]),
        1 => flat.pop().unwrap(),
        _ => Rest::Bcast(flat),
    }
}

/// Canonical form of a value's shape: known dims that continue a slice rest are folded into
/// it (`[x0, *x.shape[1:]]` → `[*x.shape]`).
pub fn canon(s: Shape) -> Shape {
    let mut s = tidy(s);
    if let Some(Rest::Slice(sym, mut from, mut back)) = s.rest.clone() {
        while from > 0 && s.prefix.last() == Some(&sdim(sym, from as i32 - 1)) {
            s.prefix.pop();
            from -= 1;
        }
        while back > 0 && s.suffix.first() == Some(&sdim(sym, -(back as i32))) {
            s.suffix.remove(0);
            back -= 1;
        }
        s.rest = Some(Rest::Slice(sym, from, back));
    }
    s
}

/// Clean up a shape whose rest is an empty `Dims` or known `Dims` (splice it in).
pub fn tidy(mut s: Shape) -> Shape {
    if let Some(Rest::Dims(d)) = &s.rest {
        let mut p = s.prefix.clone();
        p.extend(d.iter().copied());
        p.extend(s.suffix.iter().copied());
        s.prefix = p;
        s.suffix.clear();
        s.rest = None;
    }
    s
}

/// Dim `i` (negative: from the end) of a shape, when it can be named.
pub fn dim_at(s: &Shape, i: i64) -> Option<DimId> {
    if let Some(d) = s.dims() {
        let n = d.len() as i64;
        let j = if i < 0 { n + i } else { i };
        return if j >= 0 && j < n { Some(d[j as usize]) } else { None };
    }
    if i >= 0 {
        let i = i as usize;
        if i < s.prefix.len() {
            return Some(s.prefix[i]);
        }
        if let Some(Rest::Slice(sym, from, _)) = &s.rest {
            if s.suffix.is_empty() {
                return Some(sdim(*sym, (*from as usize + i - s.prefix.len()) as i32));
            }
        }
        None
    } else {
        let k = (-i) as usize;
        if k <= s.suffix.len() {
            return Some(s.suffix[s.suffix.len() - k]);
        }
        if let Some(Rest::Slice(sym, from, back)) = &s.rest {
            let pre_ok = s.prefix.len() as u32 == *from && s.prefix.iter().enumerate().all(|(j, d)| *d == sdim(*sym, j as i32));
            if s.prefix.is_empty() || pre_ok {
                return Some(sdim(*sym, -((*back as usize + k - s.suffix.len()) as i32)));
            }
        }
        None
    }
}

/// The shape with `n_front` leading and `n_back` trailing dims spelled out (for an op touching
/// those positions; the op requires at least that many dims). Leading positions that may
/// overlap a known tail become unknown dims and the tail is dropped.
pub fn expose(s: &Shape, n_front: usize, n_back: usize) -> Shape {
    let mut s = s.clone();
    if s.rest.is_none() {
        return s;
    }
    while s.prefix.len() < n_front {
        match s.rest.clone() {
            Some(Rest::Slice(sym, from, back)) if s.suffix.is_empty() => {
                s.prefix.push(sdim(sym, from as i32));
                s.rest = Some(Rest::Slice(sym, from + 1, back));
            }
            _ => break,
        }
    }
    if s.prefix.len() < n_front {
        while s.prefix.len() < n_front {
            s.prefix.push(unknown());
        }
        s.rest = Some(Rest::Any);
        s.suffix.clear();
    }
    while s.suffix.len() < n_back {
        match s.rest.clone() {
            Some(Rest::Slice(sym, from, back)) => {
                s.suffix.insert(0, sdim(sym, -(back as i32) - 1));
                s.rest = Some(Rest::Slice(sym, from, back + 1));
            }
            _ => s.suffix.insert(0, unknown()),
        }
    }
    s
}

/// Join of two shapes (values joined at a merge point).
pub fn join_shapes(a: &Shape, b: &Shape) -> Shape {
    if a == b {
        return a.clone();
    }
    let mut assume = a.assume.clone();
    for c in &b.assume {
        if !assume.contains(c) {
            assume.push(c.clone());
        }
    }
    if let (Some(x), Some(y)) = (a.dims(), b.dims()) {
        if x.len() == y.len() {
            let d = x.iter().zip(y.iter()).map(|(p, q)| if p == q { *p } else { unknown() }).collect();
            return Shape { prefix: d, rest: None, suffix: vec![], assume };
        }
    }
    if a.rest == b.rest && a.prefix.len() == b.prefix.len() && a.suffix.len() == b.suffix.len() {
        let pj = |x: &[DimId], y: &[DimId]| -> Vec<DimId> { x.iter().zip(y.iter()).map(|(p, q)| if p == q { *p } else { unknown() }).collect() };
        return Shape { prefix: pj(&a.prefix, &b.prefix), rest: a.rest.clone(), suffix: pj(&a.suffix, &b.suffix), assume };
    }
    // the common trailing dims (up to the first that differs); the rest unknown
    let ta: Vec<DimId> = if a.rest.is_none() { a.prefix.clone() } else { a.suffix.clone() };
    let tb: Vec<DimId> = if b.rest.is_none() { b.prefix.clone() } else { b.suffix.clone() };
    let n = ta.len().min(tb.len());
    // the trailing positions both have (dims that differ: unknown); the rest unknown
    let tail: Vec<DimId> = (0..n).map(|i| {
        let p = ta[ta.len() - n + i];
        let q = tb[tb.len() - n + i];
        if p == q { p } else { unknown() }
    }).collect();
    Shape { prefix: vec![], rest: Some(Rest::Any), suffix: tail, assume }
}

// ---------------------------------------------------------------------------
// printing (canonical: program terms; `scope` filters symbols of other functions)
// ---------------------------------------------------------------------------

/// What printing may name: symbols of `scope`, of module level, of the `extra` scopes (the
/// class's `__init__`s, printed through self-attribute aliases when there is one).
#[derive(Clone, Debug, Default)]
pub struct PrintCtx {
    /// print every symbol (no scope filter)
    pub all: bool,
    pub scope: u32,
    pub extra: Vec<u32>,
    pub sym_alias: FxHashMap<Sym, String>,
    pub dim_alias: FxHashMap<DimId, String>,
}

thread_local! {
    static CTX: RefCell<PrintCtx> = RefCell::new(PrintCtx { all: true, ..Default::default() });
}

pub fn with_ctx<R>(ctx: PrintCtx, f: impl FnOnce() -> R) -> R {
    let old = CTX.with(|c| c.replace(ctx));
    let r = f();
    CTX.with(|c| *c.borrow_mut() = old);
    r
}

pub fn with_scope<R>(scope: u32, f: impl FnOnce() -> R) -> R {
    with_ctx(PrintCtx { all: false, scope, ..Default::default() }, f)
}

pub fn sym_visible(s: Sym) -> bool {
    CTX.with(|c| {
        let c = c.borrow();
        let sc = sym_scope(s);
        c.all || sc == 0 || sc == c.scope || c.extra.contains(&sc)
    })
}

/// The text a symbol prints as here: its alias through self (`self.n_embd`, also for a
/// sub-path: `config.n_embd` → `self.config.n_embd`), else its own text; None if not visible.
pub fn sym_print(s: Sym) -> Option<String> {
    if !sym_visible(s) {
        return None;
    }
    let text = sym_text(s);
    let sc = sym_scope(s);
    CTX.with(|c| {
        let c = c.borrow();
        if c.all || sc == c.scope {
            return Some(text.clone());
        }
        if let Some(a) = c.sym_alias.get(&s) {
            return Some(a.clone());
        }
        // longest aliased prefix
        let mut best: Option<(usize, String)> = None;
        for (as_, at) in c.sym_alias.iter() {
            if sym_scope(*as_) != sc {
                continue;
            }
            let pt = sym_text(*as_);
            if text.len() > pt.len() && text.starts_with(&pt) && text[pt.len()..].starts_with('.') && best.as_ref().map(|b| pt.len() > b.0).unwrap_or(true) {
                best = Some((pt.len(), format!("{}{}", at, &text[pt.len()..])));
            }
        }
        Some(best.map(|b| b.1).unwrap_or(text))
    })
}

fn dim_alias(d: DimId) -> Option<String> {
    CTX.with(|c| c.borrow().dim_alias.get(&d).cloned())
}

fn needs_parens(t: &str) -> bool {
    let mut depth = 0;
    for ch in t.chars() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ' ' | '+' | '-' | '*' | '/' | '%' if depth == 0 => return true,
            _ => {}
        }
    }
    false
}

pub fn atom_text(a: AtomId) -> String {
    // a compound atom that a self attribute holds prints as that attribute
    if !matches!(atom_get(a), Atom::Dim(..) | Atom::Named(_) | Atom::Unknown) {
        if let Some(al) = dim_alias(poly_id(vec![(vec![(a, 1)], 1)])) {
            return al;
        }
    }
    match atom_get(a) {
        Atom::Dim(s, i) => {
            let Some(t) = sym_print(s) else { return "?".into() };
            if needs_parens(&t) {
                format!("({}).shape[{}]", t, i)
            } else {
                format!("{}.shape[{}]", t, i)
            }
        }
        Atom::Named(s) => sym_print(s).unwrap_or_else(|| "?".into()),
        Atom::FloorDiv(x, y) => format!("{} // {}", paren(&dim_text(x)), paren(&dim_text(y))),
        Atom::Mod(x, y) => format!("{} % {}", paren(&dim_text(x)), paren(&dim_text(y))),
        Atom::Max(x, y) => format!("max({}, {})", dim_text(x), dim_text(y)),
        Atom::Min(x, y) => format!("min({}, {})", dim_text(x), dim_text(y)),
        Atom::Unknown => "?".into(),
    }
}

fn paren(t: &str) -> String {
    if needs_parens(t) {
        format!("({})", t)
    } else {
        t.to_string()
    }
}

pub fn dim_text(d: DimId) -> String {
    if let Some(a) = dim_alias(d) {
        return a;
    }
    let p = poly(d);
    if p.is_empty() {
        return "0".into();
    }
    // non-constant terms first (by text), the constant last
    let mut terms: Vec<(String, i64)> = Vec::new();
    let mut constant = 0;
    for (m, c) in &p {
        if m.is_empty() {
            constant = *c;
            continue;
        }
        let body: Vec<String> = m
            .iter()
            .map(|(a, pw)| {
                let t = atom_text(*a);
                let t = if m.len() > 1 || *pw > 1 { paren_mul(&t) } else { t };
                if *pw == 1 { t } else { format!("{}**{}", t, pw) }
            })
            .collect();
        terms.push((body.join("*"), *c));
    }
    terms.sort_by(|a, b| a.0.cmp(&b.0));
    let mut out = String::new();
    for (i, (body, c)) in terms.iter().enumerate() {
        let (neg, mag) = (*c < 0, c.abs());
        let t = if mag == 1 { paren_mul(body).to_string() } else { format!("{}*{}", mag, paren_mul(body)) };
        let t = if terms.len() == 1 && constant == 0 && mag == 1 { body.clone() } else { t };
        if i == 0 {
            out.push_str(&if neg { format!("-{}", t) } else { t });
        } else {
            out.push_str(&if neg { format!(" - {}", t) } else { format!(" + {}", t) });
        }
    }
    if constant != 0 {
        if out.is_empty() {
            return constant.to_string();
        }
        out.push_str(&if constant < 0 { format!(" - {}", -constant) } else { format!(" + {}", constant) });
    }
    out
}

fn paren_mul(t: &str) -> String {
    // inside a product, sums need parentheses (`//` atoms are already grouped by `paren`)
    let mut depth = 0;
    for ch in t.chars() {
        match ch {
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            '+' | '-' if depth == 0 => return format!("({})", t),
            _ => {}
        }
    }
    if t.contains(" // ") || t.contains(" % ") {
        return format!("({})", t);
    }
    t.to_string()
}

pub fn rest_text(r: &Rest) -> String {
    match r {
        Rest::Any => "*".into(),
        Rest::Slice(s, from, back) => {
            let Some(t) = sym_print(*s) else { return "*".into() };
            let base = if needs_parens(&t) { format!("({})", t) } else { t };
            let a = if *from == 0 { String::new() } else { from.to_string() };
            let b = if *back == 0 { String::new() } else { format!("-{}", back) };
            if a.is_empty() && b.is_empty() {
                format!("{}.shape", base)
            } else {
                format!("{}.shape[{}:{}]", base, a, b)
            }
        }
        Rest::Dims(d) => format!("[{}]", d.iter().map(|x| dim_text(*x)).collect::<Vec<_>>().join(", ")),
        Rest::Bcast(v) => format!("broadcast({})", v.iter().map(rest_text).collect::<Vec<_>>().join(", ")),
    }
}

pub fn cons_text(c: &Cons) -> String {
    match c {
        Cons::Eq(a, b) => format!("{} == {}", dim_text(*a), dim_text(*b)),
        Cons::Rank(s, r) => {
            let t = sym_print(*s).unwrap_or_else(|| "?".into());
            let base = if needs_parens(&t) { format!("({})", t) } else { t };
            match r {
                RankRel::Eq(n) => format!("{}.dim() == {}", base, n),
                RankRel::Ge(n) => format!("{}.dim() >= {}", base, n),
                RankRel::In(v) => format!("{}.dim() in ({})", base, v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ")),
            }
        }
        Cons::If(m, g) => format!("{} if {}", cons_text(m), g.iter().map(cons_text).collect::<Vec<_>>().join(", ")),
    }
}

/// `[a, b, *rest, c]` (canonical)
pub fn shape_text(s: &Shape) -> String {
    let mut parts: Vec<String> = s.prefix.iter().map(|d| dim_text(*d)).collect();
    if let Some(r) = &s.rest {
        let t = rest_text(r);
        parts.push(if t == "*" { "*".into() } else { format!("*{}", t) });
    }
    parts.extend(s.suffix.iter().map(|d| dim_text(*d)));
    let mut out = format!("[{}]", parts.join(", "));
    if !s.assume.is_empty() {
        out.push_str(" if ");
        out.push_str(&s.assume.iter().map(cons_text).collect::<Vec<_>>().join(", "));
    }
    out
}

/// Structured form for `--golden` / `--facts`.
pub fn shape_json(s: &Shape) -> serde_json::Value {
    let mut dims: Vec<String> = s.prefix.iter().map(|d| dim_text(*d)).collect();
    let at = dims.len();
    dims.extend(s.suffix.iter().map(|d| dim_text(*d)));
    serde_json::json!({
        "dims": dims,
        "rest": s.rest.as_ref().map(|r| serde_json::json!({"at": at, "text": rest_text(r)})),
        "assume": s.assume.iter().map(cons_text).collect::<Vec<_>>(),
        "text": shape_text(s),
    })
}

// ---------------------------------------------------------------------------
// value labels
// ---------------------------------------------------------------------------

pub const TENSOR_PREFIX: &str = "\u{1}T";
pub const DIM_PREFIX: &str = "\u{1}D";
pub const NUM_LABEL: &str = "\u{1}N";

pub fn tensor_label(s: ShapeId) -> String {
    format!("{}{}", TENSOR_PREFIX, s.0)
}
pub fn dim_label(d: DimId) -> String {
    format!("{}{}", DIM_PREFIX, d.0)
}
pub fn parse_tensor_label(l: &str) -> Option<ShapeId> {
    let r = l.strip_prefix(TENSOR_PREFIX)?;
    let r = r.strip_prefix('B').unwrap_or(r);
    let r = r.split('~').next().unwrap_or(r);
    r.parse().ok().map(ShapeId)
}
/// A value that is a tensor of this shape, or (if the operands were ints) this dim: the
/// result of arithmetic on unknown values in a torch module (`x * 2`, `n + m`).
pub fn dual_label(s: ShapeId, d: DimId) -> String {
    format!("{}{}~{}", TENSOR_PREFIX, s.0, d.0)
}
pub fn dual_dim(l: &str) -> Option<DimId> {
    let r = l.strip_prefix(TENSOR_PREFIX)?;
    r.split_once('~').and_then(|(_, d)| d.parse().ok()).map(DimId)
}

/// Join of two labelled data values (tensors, dims); None: not both ours.
pub fn join_labels(a: &str, b: &str) -> Option<String> {
    if let (Some(x), Some(y)) = (parse_tensor_label(a), parse_tensor_label(b)) {
        let s = shape_id(join_shapes(&shape_get(x), &shape_get(y)));
        let both_bool = is_bool_label(a) && is_bool_label(b);
        return Some(match (dual_dim(a), dual_dim(b)) {
            (Some(p), Some(q)) => dual_label(s, if p == q { p } else { unknown() }),
            _ if both_bool => bool_tensor_label(s),
            _ => tensor_label(s),
        });
    }
    if let (Some(x), Some(y)) = (parse_dim_label(a), parse_dim_label(b)) {
        return Some(dim_label(if x == y { x } else { unknown() }));
    }
    None
}
/// a bool tensor (comparisons, masks): indexing with it selects
pub fn bool_tensor_label(s: ShapeId) -> String {
    format!("{}B{}", TENSOR_PREFIX, s.0)
}
pub fn is_bool_label(l: &str) -> bool {
    l.starts_with("\u{1}TB")
}
pub fn parse_dim_label(l: &str) -> Option<DimId> {
    l.strip_prefix(DIM_PREFIX)?.parse().ok().map(DimId)
}

pub fn shared(f: TFacts) -> Arc<TFacts> {
    Arc::new(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polys() {
        let x = sym(1, "x");
        let c = sdim(x, 2);
        let four_c = mul(lit(4), c);
        assert_eq!(dim_text(four_c), "4*x.shape[2]");
        assert_eq!(dim_text(sub(c, lit(1))), "x.shape[2] - 1");
        let t = sdim(x, 1);
        assert_eq!(div_exact(mul(t, c), t), Some(c));
        assert_eq!(floordiv(mul(lit(3), c), c), lit(3));
        assert_eq!(dim_text(floordiv(c, named(sym(1, "n")))), "x.shape[2] // n");
        assert_eq!(floordiv(lit(31), lit(2)), lit(15));
    }

    #[test]
    fn facts() {
        let x = sym(1, "x");
        let mut f = TFacts::default();
        let last = sdim(x, -1);
        learn_eq(&mut f, last, lit(512));
        assert_eq!(norm(last, &f), lit(512));
        learn_rank(&mut f, x, RankRel::Eq(3));
        assert_eq!(norm(sdim(x, -2), &f), sdim(x, 1));
        let s = norm_shape(&Shape::of_sym(x), &f);
        assert_eq!(shape_text(&s), "[x.shape[0], x.shape[1], x.shape[2]]");
        assert_eq!(dim_eq(sdim(x, 2), lit(512), &f), None); // x.shape[-1] substitution is on the -1 atom only
        let n = named(sym(1, "d"));
        let mut g = TFacts::default();
        learn_eq(&mut g, sdim(x, -1), n);
        assert_eq!(norm(sdim(x, -1), &g), n);
        assert_eq!(dim_eq(add(n, lit(1)), n, &g), Some(false));
    }

    #[test]
    fn bcast() {
        let f = TFacts::default();
        let a = Shape::known(vec![lit(3), lit(1)]);
        let b = Shape::known(vec![lit(1), lit(4)]);
        assert_eq!(shape_text(&broadcast(&a, &b, &f).unwrap()), "[3, 4]");
        let x = Shape::of_sym(sym(1, "x"));
        let y = Shape::of_sym(sym(1, "y"));
        assert_eq!(shape_text(&broadcast(&x, &y, &f).unwrap()), "[*broadcast(x.shape, y.shape)]");
        assert!(broadcast(&Shape::known(vec![lit(3)]), &Shape::known(vec![lit(4)]), &f).is_err());
    }
}
