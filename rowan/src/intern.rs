//! Global interners for keys, sources, lenses and small strings (every set the
//! analysis manipulates is a sorted vector of `u32` ids, so joins are merges and comparisons are memcmp).
//!
//! Interners are thread-local: the analysis of one program runs on one thread.

use std::cell::RefCell;
use std::fmt;
use std::sync::Arc;

use rustc_hash::FxHashMap;

use crate::domain::{Lens, Source};

// ---------------------------------------------------------------------------
// keys
// ---------------------------------------------------------------------------

/// A Python value usable as a dict key, plus the symbolic inverse-image term `Inv`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Str(Arc<str>),
    Int(i64),
    /// bit pattern of the f64
    Float(u64),
    Bool(bool),
    None,
    Bytes(Vec<u8>),
    Ellipsis,
    Tuple(Vec<KeyId>),
    /// `fn⁻¹(key)`: the source key x with fn(x) == key
    Inv(Arc<str>, KeyId),
    /// an Enum member as a key: `Cls.NAME`, with its value when the enum mixes in str / int
    /// (`StrEnum`, `IntEnum`, `class M(str, Enum)`: equal to and hashed as the value)
    Member(Arc<str>, Option<KeyId>),
    /// a frozenset of keys (sorted by id): a value key
    Frozen(Vec<KeyId>),
    /// `decimal.Decimal`: sign, coefficient, exponent (models::pynum)
    Dec(bool, u128, i32),
    /// `fractions.Fraction`: numerator, denominator (normalised)
    Frac(i128, i128),
    /// `datetime` values: kind (date / datetime / time / timedelta / timezone), value, UTC
    /// offset (models::chrono)
    Chrono(u8, i64, i32),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct KeyId(pub u32);

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct StrId(pub u32);

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct SourceId(pub u32);

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct LensId(pub u32);

struct Table<T> {
    items: Vec<T>,
    index: FxHashMap<T, u32>,
}

impl<T: Clone + Eq + std::hash::Hash> Table<T> {
    fn new() -> Self {
        Table { items: Vec::new(), index: FxHashMap::default() }
    }
    fn intern(&mut self, t: T) -> u32 {
        if let Some(&i) = self.index.get(&t) {
            return i;
        }
        let i = self.items.len() as u32;
        self.items.push(t.clone());
        self.index.insert(t, i);
        i
    }
    fn get(&self, i: u32) -> &T {
        &self.items[i as usize]
    }
}

struct Interners {
    keys: Table<Key>,
    strs: Table<Arc<str>>,
    sources: Table<Source>,
    lenses: Table<Lens>,
}

thread_local! {
    static INTERN: RefCell<Interners> = RefCell::new(Interners::fresh());
}

impl Interners {
    fn fresh() -> Self {
        let mut it = Interners {
            keys: Table::new(),
            strs: Table::new(),
            sources: Table::new(),
            lenses: Table::new(),
        };
        // fixed ids used all over the place
        it.strs.intern(Arc::from("")); // StrId(0) == ""
        it.strs.intern(Arc::from("…")); // StrId(1) == "…"
        it.keys.intern(Key::Str(Arc::from("*"))); // KeyId(0) == "*"
        it.lenses.intern(Lens::default()); // LensId(0) == empty lens
        it
    }
}

pub const STAR: KeyId = KeyId(0);
pub const EMPTY_STR: StrId = StrId(0);
/// The mutator standing for the ones a row no longer names (`domain::cap_mutators`).
pub const MORE: StrId = StrId(1);
pub const EMPTY_LENS: LensId = LensId(0);

pub fn key(k: Key) -> KeyId {
    INTERN.with(|i| KeyId(i.borrow_mut().keys.intern(k)))
}

pub fn key_str_lit(s: &str) -> KeyId {
    key(Key::Str(Arc::from(s)))
}

pub fn key_int(i: i64) -> KeyId {
    key(Key::Int(i))
}

pub fn key_get(id: KeyId) -> Key {
    INTERN.with(|i| i.borrow().keys.get(id.0).clone())
}

pub fn with_key<R>(id: KeyId, f: impl FnOnce(&Key) -> R) -> R {
    INTERN.with(|i| f(i.borrow().keys.get(id.0)))
}

pub fn key_is_str(id: KeyId) -> bool {
    with_key(id, |k| matches!(k, Key::Str(_)))
}

/// The string of a str key (None for other key types).
pub fn key_as_str(id: KeyId) -> Option<Arc<str>> {
    with_key(id, |k| match k {
        Key::Str(s) => Some(s.clone()),
        _ => None,
    })
}

pub fn key_as_int(id: KeyId) -> Option<i64> {
    with_key(id, |k| match k {
        Key::Int(i) => Some(*i),
        Key::Bool(b) => Some(*b as i64),
        _ => None,
    })
}

pub fn key_is_number(id: KeyId) -> bool {
    with_key(id, |k| matches!(k, Key::Int(_) | Key::Float(_)))
}

pub fn key_is_inv(id: KeyId) -> bool {
    with_key(id, |k| matches!(k, Key::Inv(..)))
}

pub fn key_inv(fn_name: &str, k: KeyId) -> KeyId {
    key(Key::Inv(Arc::from(fn_name), k))
}

pub fn key_member(qual: &str, value: Option<KeyId>) -> KeyId {
    key(Key::Member(Arc::from(qual), value))
}

/// The value a mixed-in Enum member key equals (`Mode.FAST` → `"fast"`).
pub fn key_member_value(id: KeyId) -> Option<KeyId> {
    with_key(id, |k| match k {
        Key::Member(_, v) => *v,
        _ => None,
    })
}

pub fn key_is_member(id: KeyId) -> bool {
    with_key(id, |k| matches!(k, Key::Member(..)))
}

pub fn key_none() -> KeyId {
    key(Key::None)
}

pub fn key_bool(b: bool) -> KeyId {
    key(Key::Bool(b))
}

pub fn key_float(f: f64) -> KeyId {
    key(Key::Float(f.to_bits()))
}

pub fn key_tuple(items: Vec<KeyId>) -> KeyId {
    key(Key::Tuple(items))
}

pub fn key_frozen(mut items: Vec<KeyId>) -> KeyId {
    items.sort();
    items.dedup();
    key(Key::Frozen(items))
}

/// Python's `str(k)` for the key (used where the prototype formats a key into a string).
pub fn key_py_str(id: KeyId) -> String {
    with_key(id, |k| match k {
        Key::Str(s) => s.to_string(),
        Key::Dec(..) | Key::Frac(..) => crate::models::pynum::str_of(id).unwrap_or_default(),
        Key::Chrono(..) => crate::models::chrono::str_of(id).unwrap_or_default(),
        _ => key_repr_inner(k),
    })
}

/// Python's `repr(k)` (`key_str` in the prototype).
pub fn key_repr(id: KeyId) -> String {
    with_key(id, key_repr_inner)
}

fn key_repr_inner(k: &Key) -> String {
    match k {
        Key::Str(s) => py_str_repr(s),
        Key::Int(i) => i.to_string(),
        Key::Float(bits) => py_float_repr(f64::from_bits(*bits)),
        Key::Bool(true) => "True".into(),
        Key::Bool(false) => "False".into(),
        Key::None => "None".into(),
        Key::Bytes(b) => {
            let mut out = String::from("b'");
            for &c in b {
                match c {
                    b'\\' => out.push_str("\\\\"),
                    b'\'' => out.push_str("\\'"),
                    b'\n' => out.push_str("\\n"),
                    b'\t' => out.push_str("\\t"),
                    b'\r' => out.push_str("\\r"),
                    0x20..=0x7e => out.push(c as char),
                    _ => out.push_str(&format!("\\x{:02x}", c)),
                }
            }
            out.push('\'');
            out
        }
        Key::Ellipsis => "Ellipsis".into(),
        Key::Tuple(items) => {
            let parts: Vec<String> = items.iter().map(|k| key_repr(*k)).collect();
            if parts.len() == 1 {
                format!("({},)", parts[0])
            } else {
                format!("({})", parts.join(", "))
            }
        }
        Key::Inv(f, k) => format!("{}⁻¹({})", f, key_repr(*k)),
        Key::Member(_, Some(v)) => key_repr(*v),
        Key::Member(q, None) => q.to_string(),
        Key::Dec(n, c, e) => format!("Decimal('{}')", crate::models::pynum::Dec { neg: *n, coef: *c, exp: *e }.to_str()),
        Key::Frac(n, d) => format!("Fraction({}, {})", n, d),
        Key::Chrono(a, b, c) => crate::models::chrono::repr_raw(*a, *b, *c),
        Key::Frozen(items) if items.is_empty() => "frozenset()".into(),
        Key::Frozen(items) => {
            let parts: Vec<String> = items.iter().map(|k| key_repr(*k)).collect();
            format!("frozenset({{{}}})", parts.join(", "))
        }
    }
}

pub fn py_str_repr(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut out = String::with_capacity(s.len() + 2);
    out.push(quote);
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 || c as u32 == 0x7f => out.push_str(&format!("\\x{:02x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

pub fn py_float_repr(f: f64) -> String {
    if f.is_nan() {
        return "nan".into();
    }
    if f.is_infinite() {
        return if f > 0.0 { "inf".into() } else { "-inf".into() };
    }
    if f == f.trunc() && f.abs() < 1e16 {
        format!("{:.1}", f)
    } else {
        let s = format!("{}", f);
        s
    }
}

/// Sort key for `sorted(keys, key=str)`: the repr.
pub fn sorted_by_str(keys: impl IntoIterator<Item = KeyId>) -> Vec<KeyId> {
    let mut v: Vec<(String, KeyId)> = keys.into_iter().map(|k| (key_repr(k), k)).collect();
    v.sort();
    v.into_iter().map(|(_, k)| k).collect()
}

pub fn keys_str(keys: impl IntoIterator<Item = KeyId>) -> String {
    let mut items: Vec<String> = keys.into_iter().map(key_repr).collect();
    items.sort();
    format!("{{{}}}", items.join(", "))
}

// ---------------------------------------------------------------------------
// strings (sites, mutators, names)
// ---------------------------------------------------------------------------

pub fn s(st: &str) -> StrId {
    INTERN.with(|i| StrId(i.borrow_mut().strs.intern(Arc::from(st))))
}

pub fn str_get(id: StrId) -> Arc<str> {
    INTERN.with(|i| i.borrow().strs.get(id.0).clone())
}

impl fmt::Display for StrId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", str_get(*self))
    }
}

// ---------------------------------------------------------------------------
// sources / lenses
// ---------------------------------------------------------------------------

pub fn source(src: Source) -> SourceId {
    INTERN.with(|i| SourceId(i.borrow_mut().sources.intern(src)))
}

pub fn source_get(id: SourceId) -> Source {
    INTERN.with(|i| i.borrow().sources.get(id.0).clone())
}

pub fn with_source<R>(id: SourceId, f: impl FnOnce(&Source) -> R) -> R {
    INTERN.with(|i| f(i.borrow().sources.get(id.0)))
}

pub fn lens(l: Lens) -> LensId {
    if l.steps.is_empty() {
        return EMPTY_LENS;
    }
    INTERN.with(|i| LensId(i.borrow_mut().lenses.intern(l)))
}

pub fn lens_get(id: LensId) -> Lens {
    INTERN.with(|i| i.borrow().lenses.get(id.0).clone())
}

pub fn with_lens<R>(id: LensId, f: impl FnOnce(&Lens) -> R) -> R {
    INTERN.with(|i| f(i.borrow().lenses.get(id.0)))
}

// ---------------------------------------------------------------------------
// sorted id sets
// ---------------------------------------------------------------------------

/// A small sorted set of `Copy + Ord` ids with structural sharing (the empty set is no
/// allocation: values are made empty everywhere).
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IdSet<T: Copy + Ord>(Option<Arc<[T]>>);

impl<T: Copy + Ord + fmt::Debug> fmt::Debug for IdSet<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.s().iter()).finish()
    }
}

impl<T: Copy + Ord> Default for IdSet<T> {
    fn default() -> Self {
        IdSet(None)
    }
}

impl<T: Copy + Ord> IdSet<T> {
    pub fn new() -> Self {
        Self::default()
    }
    /// The set of a sorted vector without duplicates.
    fn sorted(v: Vec<T>) -> Self {
        IdSet(if v.is_empty() { None } else { Some(Arc::from(v)) })
    }
    fn s(&self) -> &[T] {
        self.0.as_deref().unwrap_or(&[])
    }
    pub fn one(x: T) -> Self {
        IdSet(Some(Arc::from(&[x][..])))
    }
    pub fn from_vec(mut v: Vec<T>) -> Self {
        v.sort_unstable();
        v.dedup();
        Self::sorted(v)
    }
    pub fn from_iter(it: impl IntoIterator<Item = T>) -> Self {
        Self::from_vec(it.into_iter().collect())
    }
    pub fn is_empty(&self) -> bool {
        self.s().is_empty()
    }
    pub fn len(&self) -> usize {
        self.s().len()
    }
    pub fn contains(&self, x: T) -> bool {
        self.s().binary_search(&x).is_ok()
    }
    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        self.s().iter().copied()
    }
    pub fn as_slice(&self) -> &[T] {
        &self.s()
    }
    pub fn first(&self) -> Option<T> {
        self.s().first().copied()
    }
    pub fn to_vec(&self) -> Vec<T> {
        self.s().to_vec()
    }
    pub fn union(&self, o: &Self) -> Self {
        if o.is_empty() || o.is_subset(self) {
            return self.clone();
        }
        if self.is_empty() || self.is_subset(o) {
            return o.clone();
        }
        let mut out = Vec::with_capacity(self.len() + o.len());
        let (a, b) = (&self.s(), &o.s());
        let (mut i, mut j) = (0, 0);
        while i < a.len() && j < b.len() {
            match a[i].cmp(&b[j]) {
                std::cmp::Ordering::Less => {
                    out.push(a[i]);
                    i += 1;
                }
                std::cmp::Ordering::Greater => {
                    out.push(b[j]);
                    j += 1;
                }
                std::cmp::Ordering::Equal => {
                    out.push(a[i]);
                    i += 1;
                    j += 1;
                }
            }
        }
        out.extend_from_slice(&a[i..]);
        out.extend_from_slice(&b[j..]);
        Self::sorted(out)
    }
    pub fn is_subset(&self, o: &Self) -> bool {
        if self.len() > o.len() {
            return false;
        }
        let (a, b) = (&self.s(), &o.s());
        let mut j = 0;
        for &x in a.iter() {
            while j < b.len() && b[j] < x {
                j += 1;
            }
            if j >= b.len() || b[j] != x {
                return false;
            }
            j += 1;
        }
        true
    }
    pub fn intersection(&self, o: &Self) -> Self {
        if self == o {
            return self.clone();
        }
        if self.is_empty() || o.is_empty() {
            return Self::new();
        }
        let mut out = Vec::new();
        let (a, b) = (&self.s(), &o.s());
        let (mut i, mut j) = (0, 0);
        while i < a.len() && j < b.len() {
            match a[i].cmp(&b[j]) {
                std::cmp::Ordering::Less => i += 1,
                std::cmp::Ordering::Greater => j += 1,
                std::cmp::Ordering::Equal => {
                    out.push(a[i]);
                    i += 1;
                    j += 1;
                }
            }
        }
        if out.len() == a.len() {
            return self.clone();
        }
        if out.len() == b.len() {
            return o.clone();
        }
        Self::sorted(out)
    }
    pub fn difference(&self, o: &Self) -> Self {
        if self.is_empty() || o.is_empty() {
            return self.clone();
        }
        let out: Vec<T> = self.s().iter().copied().filter(|x| !o.contains(*x)).collect();
        if out.len() == self.len() {
            return self.clone();
        }
        Self::sorted(out)
    }
    pub fn intersects(&self, o: &Self) -> bool {
        let (a, b) = (&self.s(), &o.s());
        let (mut i, mut j) = (0, 0);
        while i < a.len() && j < b.len() {
            match a[i].cmp(&b[j]) {
                std::cmp::Ordering::Less => i += 1,
                std::cmp::Ordering::Greater => j += 1,
                std::cmp::Ordering::Equal => return true,
            }
        }
        false
    }
    pub fn insert(&self, x: T) -> Self {
        if self.contains(x) {
            return self.clone();
        }
        let mut v = self.s().to_vec();
        let pos = v.binary_search(&x).unwrap_err();
        v.insert(pos, x);
        Self::sorted(v)
    }
    pub fn remove(&self, x: T) -> Self {
        if !self.contains(x) {
            return self.clone();
        }
        let v: Vec<T> = self.s().iter().copied().filter(|y| *y != x).collect();
        Self::sorted(v)
    }
    pub fn map(&self, f: impl FnMut(T) -> T) -> Self {
        Self::from_iter(self.s().iter().copied().map(f))
    }
    pub fn filter(&self, mut f: impl FnMut(T) -> bool) -> Self {
        let v: Vec<T> = self.s().iter().copied().filter(|x| f(*x)).collect();
        if v.len() == self.len() {
            return self.clone();
        }
        Self::sorted(v)
    }
}

impl<T: Copy + Ord> FromIterator<T> for IdSet<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        IdSet::from_vec(iter.into_iter().collect())
    }
}

impl<T: Copy + Ord> From<Vec<T>> for IdSet<T> {
    fn from(v: Vec<T>) -> Self {
        IdSet::from_vec(v)
    }
}

pub type KeySet = IdSet<KeyId>;
