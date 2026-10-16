//! Abstract domains: `KeyVal`, `Lens`, `Source`, `Row`,
//! `ReqInfo`, `Value`, `classify`.  Port of `rowan/domain.py` over interned ids.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use rustc_hash::FxHasher;

pub use crate::intern::SourceId;
use crate::intern::{
    self, key_as_str, key_is_inv, key_is_str, key_py_str, key_repr, keys_str, lens_get, source_get, with_lens, with_source,
    IdSet, KeyId, KeySet, LensId, StrId, EMPTY_LENS,
};

pub const MAX_LITS: usize = 64;
pub const MAX_LENSES: usize = 6;
pub const MAX_CHAIN: usize = 6;

pub type ObjId = u32;
pub type ObjSet = IdSet<ObjId>;
/// entry-row id: oid * 2 + (1 if attr)

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum Ns {
    Item,
    Attr,
    Cell,
    Pos,
    Elem,
    Meta,
    /// a variable (verdicts only: a read of a name bound on some paths only)
    Name,
}

impl Ns {
    pub fn as_str(self) -> &'static str {
        match self {
            Ns::Item => "item",
            Ns::Attr => "attr",
            Ns::Cell => "cell",
            Ns::Pos => "pos",
            Ns::Elem => "elem",
            Ns::Meta => "meta",
            Ns::Name => "name",
        }
    }
}

impl fmt::Display for Ns {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// lens steps
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum FnKind {
    Upper,
    Lower,
    Strip,
    Title,
    Prefix,
    Suffix,
    Str,
    Replace,
    /// `sep.join(key)`: a tuple key's parts (a string's characters) joined by the arg
    Join,
}

impl FnKind {
    pub fn name(self) -> &'static str {
        match self {
            FnKind::Upper => "upper",
            FnKind::Lower => "lower",
            FnKind::Strip => "strip",
            FnKind::Title => "title",
            FnKind::Prefix => "prefix",
            FnKind::Suffix => "suffix",
            FnKind::Str => "str",
            FnKind::Replace => "replace",
            FnKind::Join => "join",
        }
    }
    pub fn from_name(n: &str) -> Option<FnKind> {
        Some(match n {
            "upper" => FnKind::Upper,
            "lower" => FnKind::Lower,
            "strip" => FnKind::Strip,
            "title" => FnKind::Title,
            "prefix" => FnKind::Prefix,
            "suffix" => FnKind::Suffix,
            "str" => FnKind::Str,
            "replace" => FnKind::Replace,
            _ => return None,
        })
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct RenameFn {
    pub fn_: FnKind,
    /// prefix/suffix: the affix key; replace: a 2-tuple key (a, b)
    pub arg: Option<KeyId>,
}

impl RenameFn {
    pub fn new(fn_: FnKind) -> Self {
        RenameFn { fn_, arg: None }
    }
    pub fn with_arg(fn_: FnKind, arg: KeyId) -> Self {
        RenameFn { fn_, arg: Some(arg) }
    }

    fn arg_str(&self) -> String {
        self.arg.map(key_py_str).unwrap_or_default()
    }

    fn replace_pair(&self) -> Option<(String, String)> {
        let a = self.arg?;
        intern::with_key(a, |k| match k {
            intern::Key::Tuple(items) if items.len() == 2 => Some((key_py_str(items[0]), key_py_str(items[1]))),
            _ => None,
        })
    }

    /// About how long `apply`'s text is (in bytes) for `k`, without computing it.
    pub fn out_len(&self, k: KeyId) -> usize {
        let Some(s) = key_as_str(k) else { return 0 };
        match self.fn_ {
            FnKind::Replace => match self.replace_pair() {
                Some((a, b)) if a.is_empty() => s.len() + (s.chars().count() + 1) * b.len(),
                Some((a, b)) => s.len() + s.matches(&*a).count() * b.len(),
                None => s.len(),
            },
            FnKind::Prefix | FnKind::Suffix => s.len() + self.arg_str().len(),
            FnKind::Join => s.len() * (1 + self.arg_str().len()),
            _ => s.len() * 4,
        }
    }

    pub fn apply(&self, k: KeyId) -> KeyId {
        if self.fn_ == FnKind::Join {
            let sep = self.arg_str();
            let parts: Option<Vec<String>> = intern::with_key(k, |kk| match kk {
                intern::Key::Tuple(items) => Some(items.clone()),
                _ => None,
            })
            .and_then(|items| items.into_iter().map(|x| key_as_str(x).map(|s| s.to_string())).collect())
            .or_else(|| key_as_str(k).map(|s| s.chars().map(|c| c.to_string()).collect()));
            return match parts {
                Some(p) => intern::key_str_lit(&p.join(&sep)),
                None => k,
            };
        }
        if self.fn_ == FnKind::Str {
            return if key_is_str(k) { k } else { intern::key_str_lit(&key_py_str(k)) };
        }
        let Some(s) = key_as_str(k) else { return k };
        let out: String = match self.fn_ {
            FnKind::Upper => s.to_uppercase(),
            FnKind::Lower => s.to_lowercase(),
            FnKind::Strip => s.trim().to_string(),
            FnKind::Title => py_title(&s),
            FnKind::Prefix => format!("{}{}", self.arg_str(), s),
            FnKind::Suffix => format!("{}{}", s, self.arg_str()),
            FnKind::Replace => match self.replace_pair() {
                Some((a, b)) => s.replace(&a, &b),
                None => return k,
            },
            FnKind::Str | FnKind::Join => unreachable!(),
        };
        intern::key_str_lit(&out)
    }

    pub fn inverse(&self, k: KeyId) -> KeySet {
        let sk = key_as_str(k);
        let name = self.fn_.name();
        match self.fn_ {
            FnKind::Prefix => {
                let p = self.arg_str();
                match sk {
                    Some(s) if s.starts_with(p.as_str()) => KeySet::one(intern::key_str_lit(&s[p.len()..])),
                    _ => KeySet::new(),
                }
            }
            FnKind::Suffix => {
                let suf = self.arg_str();
                match sk {
                    Some(s) if !suf.is_empty() && s.ends_with(suf.as_str()) => {
                        KeySet::one(intern::key_str_lit(&s[..s.len() - suf.len()]))
                    }
                    _ => KeySet::new(),
                }
            }
            FnKind::Upper | FnKind::Lower | FnKind::Title | FnKind::Strip => match sk {
                Some(s) => {
                    let same = match self.fn_ {
                        FnKind::Upper => *s == *s.to_uppercase(),
                        FnKind::Lower => *s == *s.to_lowercase(),
                        FnKind::Title => *s == *py_title(&s),
                        FnKind::Strip => *s == *s.trim(),
                        _ => false,
                    };
                    if same {
                        KeySet::one(intern::key_inv(name, k))
                    } else {
                        KeySet::new()
                    }
                }
                None => KeySet::new(),
            },
            FnKind::Str => {
                let mut out = vec![intern::key_inv("str", k)];
                if let Some(s) = sk {
                    let digits = s.trim_start_matches('-');
                    if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) {
                        if let Ok(i) = s.parse::<i64>() {
                            out.push(intern::key_int(i));
                        }
                    }
                }
                KeySet::from_vec(out)
            }
            FnKind::Join => KeySet::one(intern::key_inv(name, k)),
            FnKind::Replace => {
                let Some((a, b)) = self.replace_pair() else { return KeySet::one(intern::key_inv(name, k)) };
                let Some(s) = sk else { return KeySet::one(k) };
                if s.contains(a.as_str()) && !b.contains(a.as_str()) {
                    return KeySet::new();
                }
                if s.contains(b.as_str()) {
                    let tag = format!("replace({},{})", intern::py_str_repr(&a), intern::py_str_repr(&b));
                    return KeySet::from_vec(vec![k, intern::key_inv(&tag, k)]);
                }
                KeySet::one(k)
            }
        }
    }
}

impl fmt::Display for RenameFn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.arg {
            None => f.write_str(self.fn_.name()),
            Some(a) => write!(f, "{}({})", self.fn_.name(), key_repr(a)),
        }
    }
}

/// Python's `str.title()`.
pub fn py_title(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_cased = false;
    for c in s.chars() {
        if c.is_alphabetic() {
            if prev_cased {
                out.extend(c.to_lowercase());
            } else {
                out.extend(c.to_uppercase());
            }
            prev_cased = true;
        } else {
            out.push(c);
            prev_cased = false;
        }
    }
    out
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Step {
    Kill(KeySet),
    /// ((old, new), ...), identity elsewhere
    RenameMap(Arc<Vec<(KeyId, KeyId)>>),
    RenameFn(RenameFn),
}

impl Step {
    pub fn apply(&self, k: KeyId) -> KeyId {
        match self {
            Step::Kill(_) => k,
            Step::RenameMap(pairs) => pairs.iter().find(|(o, _)| *o == k).map(|(_, n)| *n).unwrap_or(k),
            Step::RenameFn(f) => f.apply(k),
        }
    }
    pub fn inverse(&self, k: KeyId) -> KeySet {
        match self {
            Step::Kill(_) => KeySet::one(k),
            Step::RenameMap(pairs) => {
                let mut out: Vec<KeyId> = pairs.iter().filter(|(_, n)| *n == k).map(|(o, _)| *o).collect();
                if !pairs.iter().any(|(o, _)| *o == k) {
                    out.push(k);
                }
                KeySet::from_vec(out)
            }
            Step::RenameFn(f) => f.inverse(k),
        }
    }
}

impl fmt::Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Step::Kill(ks) => write!(f, "∖{}", keys_str(ks.iter())),
            Step::RenameMap(pairs) => {
                let parts: Vec<String> = pairs.iter().map(|(o, n)| format!("{}→{}", key_repr(*o), key_repr(*n))).collect();
                write!(f, "rename{{{}}}", parts.join(", "))
            }
            Step::RenameFn(r) => write!(f, "{}", r),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct Lens {
    pub steps: Vec<Step>,
}

impl Lens {
    pub fn concat(&self, o: &Lens) -> Lens {
        let mut steps = self.steps.clone();
        steps.extend(o.steps.iter().cloned());
        Lens { steps }
    }
    pub fn with_step(&self, s: Step) -> Lens {
        let mut steps = self.steps.clone();
        steps.push(s);
        Lens { steps }
    }
}

/// Source key terms that would appear as `k` after the lens; empty = cannot.
pub fn lens_provides(l: LensId, k: KeyId) -> KeySet {
    if l == EMPTY_LENS {
        return KeySet::one(k);
    }
    // (a copy: inverting a step may intern keys)
    let lens = intern::lens_get(l);
    {
        let mut cands = vec![k];
        for step in lens.steps.iter().rev() {
            match step {
                Step::Kill(keys) => cands.retain(|c| key_is_inv(*c) || !keys.contains(*c)),
                _ => {
                    let mut new = Vec::new();
                    for c in cands {
                        if key_is_inv(c) {
                            new.push(c);
                        } else {
                            new.extend(step.inverse(c).iter());
                        }
                    }
                    cands = new;
                }
            }
            if cands.is_empty() {
                break;
            }
        }
        KeySet::from_vec(cands)
    }
}

/// Forward image of a single source key; None if killed.
pub fn lens_apply(l: LensId, k: KeyId) -> Option<KeyId> {
    if l == EMPTY_LENS {
        return Some(k);
    }
    lens_apply_steps(&intern::lens_get(l).steps, k)
}

pub fn lens_apply_steps(steps: &[Step], mut k: KeyId) -> Option<KeyId> {
    for step in steps {
        match step {
            Step::Kill(keys) => {
                if keys.contains(k) {
                    return None;
                }
            }
            _ => k = step.apply(k),
        }
    }
    Some(k)
}

pub fn lens_str(l: LensId) -> String {
    with_lens(l, |lens| lens.steps.iter().map(|s| format!("·{}", s)).collect::<String>())
}

pub fn lens_concat(a: LensId, b: LensId) -> LensId {
    if a == EMPTY_LENS {
        return b;
    }
    if b == EMPTY_LENS {
        return a;
    }
    let la = lens_get(a);
    let lb = lens_get(b);
    intern::lens(la.concat(&lb))
}

pub fn lens_push(a: LensId, s: Step) -> LensId {
    let la = lens_get(a);
    intern::lens(la.with_step(s))
}

pub fn lens_add_kill(l: LensId, k: KeyId) -> LensId {
    let mut lens = lens_get(l);
    if let Some(Step::Kill(ks)) = lens.steps.last() {
        let ks2 = ks.insert(k);
        let n = lens.steps.len();
        lens.steps[n - 1] = Step::Kill(ks2);
    } else {
        lens.steps.push(Step::Kill(KeySet::one(k)));
    }
    intern::lens(lens)
}

// ---------------------------------------------------------------------------
// sources
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum SourceKind {
    External,
    Call,
    Param,
    Name,
    Data,
    Class,
}

/// A reason an object may have keys we do not see.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Source {
    pub site: Arc<str>,
    pub kind: SourceKind,
    pub path: Vec<KeyId>,
}

impl Source {
    pub fn new(site: &str, kind: SourceKind) -> SourceId {
        intern::source(Source { site: Arc::from(site), kind, path: Vec::new() })
    }
    pub fn label(&self) -> String {
        let p: String = self.path.iter().map(|k| format!("[{}]", key_repr(*k))).collect();
        format!("{}{}", self.site, p)
    }
}

pub fn source_extend(s: SourceId, k: KeyId) -> SourceId {
    let mut src = source_get(s);
    src.path.push(k);
    intern::source(src)
}

/// A source below another's key (`json.loads@L3['db']`): its requirements hold where the
/// parent's value was read.
pub fn source_is_nested(s: SourceId) -> bool {
    with_source(s, |src| !src.path.is_empty())
}

/// How many keys below its input a source is (`json.loads@L3['db']['host']`: 2).
pub fn source_depth(s: SourceId) -> usize {
    with_source(s, |src| src.path.len())
}

pub fn source_is_data(s: SourceId) -> bool {
    with_source(s, |src| src.kind == SourceKind::Data)
}

pub fn source_label(s: SourceId) -> String {
    with_source(s, |src| src.label())
}

// ---------------------------------------------------------------------------
// Row
// ---------------------------------------------------------------------------

/// Insertion orders of two joined rows: equal, or one a prefix of the other (the longer
/// one; the extra keys are not definite then), else unknown.
/// The insertion order of two paths' rows: the longer when the other's is a prefix of it and
/// the other has none of the keys it adds (else it holds them at another place: `b, a` on one
/// path, `a` before `b` but not in its order on the other).
fn join_order(a: &RowData, b: &RowData) -> Arc<[KeyId]> {
    let extends = |x: &RowData, y: &RowData| y.order.len() <= x.order.len() && x.order[..y.order.len()] == y.order[..] && x.order[y.order.len()..].iter().all(|k| !y.present.contains(*k));
    if a.order == b.order || extends(a, b) {
        a.order.clone()
    } else if extends(b, a) {
        b.order.clone()
    } else {
        Arc::from(Vec::new())
    }
}

pub type SrcSet = IdSet<(SourceId, LensId)>;
pub type StrSet = IdSet<StrId>;

#[derive(Clone, Default, PartialEq, Eq, Hash)]
pub struct RowData {
    pub present: KeySet,
    pub definite: KeySet,
    pub likely: KeySet,
    pub absent: KeySet,
    pub absent_likely: KeySet,
    pub sources: SrcSet,
    pub mutators: StrSet,
    pub top: bool,
    /// insertion order of the keys stored strongly (a dict iterates in this order); it
    /// describes the row only when it holds exactly the present keys (`ordered_keys`)
    pub order: Arc<[KeyId]>,
    /// keys the program stored (on some path): an input need not provide them where it did
    pub stored: KeySet,
}

/// Provision facts for one (object, namespace) at one program point; immutable, shared,
/// hash cached.  See `domain.py` for the meaning of the fields.
#[derive(Clone)]
pub struct Row {
    inner: Arc<RowInner>,
}

struct RowInner {
    data: RowData,
    hash: u64,
}

impl PartialEq for Row {
    fn eq(&self, o: &Row) -> bool {
        Arc::ptr_eq(&self.inner, &o.inner) || (self.inner.hash == o.inner.hash && self.inner.data == o.inner.data)
    }
}
impl Eq for Row {}

impl Hash for Row {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.inner.hash);
    }
}

impl std::ops::Deref for Row {
    type Target = RowData;
    fn deref(&self) -> &RowData {
        &self.inner.data
    }
}

impl fmt::Debug for Row {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

thread_local! {
    static EMPTY_ROW: Row = Row::from_data(RowData::default());
    /// joins of two different rows (a deterministic cost measure: golden `#@joins <= N`)
    static ROW_JOINS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The number of joins of two different rows so far.
pub fn row_joins() -> usize {
    ROW_JOINS.with(|c| c.get())
}

pub fn reset_row_joins() {
    ROW_JOINS.with(|c| c.set(0));
}

impl Default for Row {
    fn default() -> Self {
        EMPTY_ROW.with(|r| r.clone())
    }
}

impl Row {
    pub fn from_data(data: RowData) -> Row {
        let mut h = FxHasher::default();
        data.hash(&mut h);
        Row { inner: Arc::new(RowInner { data, hash: h.finish() }) }
    }

    pub fn new() -> Row {
        Row::default()
    }

    pub fn data(&self) -> RowData {
        self.inner.data.clone()
    }

    pub fn with_sources(sources: SrcSet) -> Row {
        Row::from_data(RowData { sources, ..Default::default() })
    }

    pub fn with_source(src: SourceId) -> Row {
        Row::with_sources(SrcSet::one((src, EMPTY_LENS)))
    }

    /// The same row value, not only an equal one: no write made it since (every write makes a
    /// new row; `CondFact`s hold only while their rows are these very ones).
    pub fn same_ptr(&self, o: &Row) -> bool {
        Arc::ptr_eq(&self.inner, &o.inner)
    }

    pub fn top() -> Row {
        Row::from_data(RowData { top: true, ..Default::default() })
    }

    pub fn closed_with(keys: KeySet) -> Row {
        Row::from_data(RowData { present: keys.clone(), definite: keys.clone(), likely: keys, ..Default::default() })
    }

    pub fn open(&self) -> bool {
        !self.sources.is_empty() || !self.mutators.is_empty() || self.top
    }

    /// Content from an input (not only from unknown code run on it).
    pub fn has_input(&self) -> bool {
        !self.sources.is_empty() || self.top
    }

    pub fn closed(&self) -> bool {
        !self.open()
    }

    /// k is surely not a key: removed on every path, or the row is closed without it.
    pub fn lacks(&self, k: KeyId) -> bool {
        self.absent.contains(k) || (self.closed() && !self.present.contains(k))
    }

    pub fn is_map(&self) -> bool {
        self.sources.iter().any(|(s, _)| source_is_data(s))
    }

    /// Source -> source key terms that may surface as k (sorted by source id).
    pub fn provides(&self, k: KeyId) -> Vec<(SourceId, KeySet)> {
        let mut out: Vec<(SourceId, KeySet)> = Vec::new();
        for (s, l) in self.sources.iter() {
            let terms = lens_provides(l, k);
            if !terms.is_empty() {
                if let Some(e) = out.iter_mut().find(|(x, _)| *x == s) {
                    e.1 = e.1.union(&terms);
                } else {
                    out.push((s, terms));
                }
            }
        }
        out
    }

    // -- lattice -----------------------------------------------------------
    pub fn join(&self, o: &Row) -> Row {
        if self == o {
            return self.clone();
        }
        ROW_JOINS.with(|c| c.set(c.get() + 1));
        let sources = if o.sources.is_subset(&self.sources) {
            self.sources.clone()
        } else if self.sources.is_subset(&o.sources) {
            o.sources.clone()
        } else {
            cap_sources(self.sources.union(&o.sources))
        };
        // (a closed row lacks every key it does not have: absent there too)
        let absent_in = |x: &Row, y: &KeySet| -> KeySet { if x.closed() { x.absent.union(&y.difference(&x.present)) } else { x.absent.clone() } };
        let absent_l_in = |x: &Row, y: &KeySet| -> KeySet { if x.closed() { x.absent_likely.union(&y.difference(&x.present)) } else { x.absent_likely.clone() } };
        Row::from_data_of(RowData {
            present: self.present.union(&o.present),
            definite: self.definite.intersection(&o.definite),
            likely: self.likely.intersection(&o.likely),
            absent: absent_in(self, &o.absent).intersection(&absent_in(o, &self.absent)),
            absent_likely: absent_l_in(self, &o.absent_likely).intersection(&absent_l_in(o, &self.absent_likely)),
            sources,
            mutators: cap_mutators(self.mutators.union(&o.mutators)),
            top: self.top || o.top,
            order: join_order(self, o),
            stored: self.stored.union(&o.stored),
        }, &[self, o])
    }

    /// `from_data`, but one of `like` itself when the data is the same as its (the shared row
    /// keeps later comparisons a pointer test)
    pub fn from_data_of(data: RowData, like: &[&Row]) -> Row {
        let mut h = FxHasher::default();
        data.hash(&mut h);
        let hash = h.finish();
        for r in like {
            if r.inner.hash == hash && r.inner.data == data {
                return (*r).clone();
            }
        }
        Row { inner: Arc::new(RowInner { data, hash }) }
    }

    /// The same row with its insertion order replaced.
    pub fn with_order(&self, order: Vec<KeyId>) -> Row {
        let mut d = self.data();
        d.order = order.into();
        Row::from_data(d)
    }

    /// The present keys in insertion order, when the order is known for all of them.
    pub fn ordered_keys(&self) -> Option<Vec<KeyId>> {
        if self.order.len() == self.present.len() && self.order.iter().all(|k| self.present.contains(*k)) {
            Some(self.order.to_vec())
        } else {
            None
        }
    }

    // -- transfers (§4.1) --------------------------------------------------
    pub fn store(&self, k: KeyId, strong: bool) -> Row {
        let mut d = self.data();
        // (a key there on some paths only, stored: where it is in the order depends on the path)
        if strong && d.order.contains(&k) && !d.definite.contains(k) {
            d.order = Arc::from(Vec::new());
        }
        if strong && !d.order.contains(&k) {
            let mut o = d.order.to_vec();
            o.push(k);
            d.order = o.into();
        }
        d.present = d.present.insert(k);
        if strong {
            d.definite = d.definite.insert(k);
            d.likely = d.likely.insert(k);
        }
        d.absent = d.absent.remove(k);
        d.absent_likely = d.absent_likely.remove(k);
        d.stored = d.stored.insert(k);
        Row::from_data(d)
    }

    /// One of several keys was stored (weak).
    pub fn store_many(&self, ks: &KeySet) -> Row {
        let mut d = self.data();
        d.present = d.present.union(ks);
        d.absent = d.absent.difference(ks);
        d.absent_likely = d.absent_likely.difference(ks);
        Row::from_data(d)
    }

    pub fn delete(&self, k: KeyId, strong: bool) -> Row {
        let mut d = self.data();
        if strong {
            if d.order.contains(&k) {
                d.order = d.order.iter().copied().filter(|x| *x != k).collect::<Vec<_>>().into();
            }
            d.present = d.present.remove(k);
            d.definite = d.definite.remove(k);
            d.likely = d.likely.remove(k);
            d.absent = d.absent.insert(k);
            d.absent_likely = d.absent_likely.insert(k);
            d.stored = d.stored.remove(k);
            d.sources = d.sources.map(|(s, l)| (s, lens_add_kill(l, k)));
        } else {
            d.definite = d.definite.remove(k);
            d.likely = d.likely.remove(k);
        }
        Row::from_data(d)
    }

    pub fn data_store(&self, src: SourceId) -> Row {
        let mut d = self.data();
        d.sources = cap_sources(d.sources.insert((src, EMPTY_LENS)));
        Row::from_data(d)
    }

    pub fn rename(&self, step: &Step) -> Row {
        let f = |ks: &KeySet| ks.map(|k| step.apply(k));
        let push = |l: LensId| lens_push(l, step.clone());
        // a key stays absent when every key renamed to it was absent
        let absent = { self.absent.filter(|k| step.inverse(k).iter().all(|c| self.absent.contains(c))) };
        Row::from_data(RowData {
            present: f(&self.present),
            definite: f(&self.definite),
            likely: f(&self.likely),
            absent,
            absent_likely: KeySet::new(),
            sources: self.sources.map(|(s, l)| (s, push(l))),
            mutators: self.mutators.clone(),
            top: self.top,
            order: self.order.iter().map(|k| step.apply(*k)).collect::<Vec<_>>().into(),
            stored: f(&self.stored),
        })
    }

    pub fn project(&self, keys: &KeySet) -> Row {
        Row::closed_with(keys.clone())
    }

    /// self ∪= chain(o ∖ kill).  strong: definite keys of o are definitely copied.
    pub fn copy_from(&self, o: &Row, strong: bool, kill: &KeySet, chain: LensId) -> Row {
        let img = |ks: &KeySet| -> KeySet {
            KeySet::from_iter(ks.iter().filter(|k| !kill.contains(*k)).filter_map(|k| lens_apply(chain, k)))
        };
        let pre: LensId = if kill.is_empty() {
            chain
        } else {
            lens_concat(intern::lens(Lens { steps: vec![Step::Kill(kill.clone())] }), chain)
        };
        let present = self.present.union(&img(&o.present));
        let definite = if strong { self.definite.union(&img(&o.definite)) } else { self.definite.clone() };
        let likely = if strong { self.likely.union(&img(&o.likely)) } else { self.likely.clone() };
        let (absent, absent_likely) = if o.open() {
            (KeySet::new(), KeySet::new())
        } else {
            (self.absent.difference(&present), self.absent_likely.difference(&present))
        };
        Row::from_data(RowData {
            present,
            definite,
            likely,
            absent,
            absent_likely,
            sources: cap_sources(self.sources.union(&o.sources.map(|(s, l)| (s, lens_concat(l, pre))))),
            mutators: cap_mutators(self.mutators.union(&o.mutators)),
            top: self.top || o.top,
            order: {
                // a strong copy of a row whose order is known appends its new keys in order (a
                // key there on some paths only: where it is depends on the path, as for `store`)
                let mut ord = self.order.to_vec();
                if let (true, Some(ks)) = (strong, o.ordered_keys()) {
                    for k in ks.into_iter().filter(|k| !kill.contains(*k)).filter_map(|k| lens_apply(chain, k)) {
                        if !ord.contains(&k) {
                            ord.push(k);
                        } else if !self.definite.contains(k) {
                            ord.clear();
                            break;
                        }
                    }
                }
                ord.into()
            },
            stored: self.stored.union(&img(&o.stored)),
        })
    }

    pub fn mutate(&self, site: StrId) -> Row {
        // (mutated already, by this site or by one of those no longer named: nothing changes)
        if self.definite.is_empty() && self.absent.is_empty() && (self.mutators.contains(site) || self.mutators.contains(intern::MORE)) {
            return self.clone();
        }
        let mut d = self.data();
        d.likely = d.likely.union(&d.definite);
        d.definite = KeySet::new();
        d.absent_likely = d.absent_likely.union(&d.absent);
        d.absent = KeySet::new();
        if !d.mutators.contains(site) {
            d.mutators = cap_mutators(d.mutators.insert(site));
        }
        Row::from_data(d)
    }

    /// `mutate` of the keys `like` names only: unknown code may have stored such keys (new
    /// ones too) or removed them; the others are as they were.
    pub fn mutate_like(&self, site: StrId, like: &dyn Fn(KeyId) -> bool) -> Row {
        let mut d = self.data();
        let (moved, kept): (Vec<KeyId>, Vec<KeyId>) = d.definite.iter().partition(|k| like(*k));
        d.likely = d.likely.union(&KeySet::from_iter(moved));
        d.definite = KeySet::from_iter(kept);
        let (moved, kept): (Vec<KeyId>, Vec<KeyId>) = d.absent.iter().partition(|k| like(*k));
        d.absent_likely = d.absent_likely.union(&KeySet::from_iter(moved));
        d.absent = KeySet::from_iter(kept);
        if !d.mutators.contains(site) {
            d.mutators = cap_mutators(d.mutators.insert(site));
        }
        Row::from_data(d)
    }

    pub fn narrow_in(&self, k: KeyId, strong: bool) -> Row {
        // (the key was there: the program did not store it, nor last: the order stays)
        let mut d = self.store(k, strong).data();
        if !self.stored.contains(k) {
            d.stored = d.stored.remove(k);
        }
        d.order = self.order.clone();
        Row::from_data(d)
    }

    pub fn narrow_out(&self, k: KeyId) -> Row {
        self.delete(k, true)
    }

}

/// A key as it appears in the compact row form: strings in double quotes, others as in Python.
pub fn key_display(k: KeyId) -> String {
    match intern::key_as_str(k) {
        Some(s) => {
            // (as Python would write it, double-quoted: escapes for quotes, backslashes and
            // control characters, so a multi-line string stays on one line)
            let mut out = String::from("\"");
            for c in s.chars() {
                match c {
                    '"' => out.push_str("\\\""),
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    '\t' => out.push_str("\\t"),
                    '\r' => out.push_str("\\r"),
                    c if c.is_control() => out.push_str(&format!("\\x{:02x}", c as u32)),
                    c => out.push(c),
                }
            }
            out.push('"');
            out
        }
        None => intern::key_repr(k),
    }
}

impl Row {
    /// How key `k` is marked where it is shown: "" (there on every path), "?" (on some paths
    /// only), "~" (there unless unknown code removed it).
    pub fn mark(&self, k: KeyId) -> &'static str {
        if self.definite.contains(k) {
            ""
        } else if self.likely.contains(k) {
            "~"
        } else {
            "?"
        }
    }

    /// The full internal form (sets, sources, mutators).
    pub fn details(&self) -> String {
        let mut parts = vec![format!("present={}", keys_str(self.present.iter())), format!("definite={}", keys_str(self.definite.iter()))];
        if self.likely != self.definite {
            parts.push(format!("likely={}", keys_str(self.likely.iter())));
        }
        if !self.absent.is_empty() {
            parts.push(format!("absent={}", keys_str(self.absent.iter())));
        }
        if !self.sources.is_empty() {
            let mut v: Vec<String> = self.sources.iter().map(|(s, l)| format!("{}{}", source_label(s), lens_str(l))).collect();
            v.sort();
            parts.push(format!("sources={{{}}}", v.join(", ")));
        }
        if !self.mutators.is_empty() {
            let mut v: Vec<String> = self.mutators.iter().map(|m| m.to_string()).collect();
            v.sort();
            parts.push(format!("mutators={{{}}}", v.join(", ")));
        }
        if self.top {
            parts.push("TOP".into());
        }
        parts.push(if self.closed() { "closed".into() } else { "open".into() });
        format!("⟨{}⟩", parts.join(", "))
    }
}

impl fmt::Display for Row {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `{"a": ?, "b"?: ?, "c"~: ?, **?}` (values are not part of the key set)
        let mut parts: Vec<String> = intern::sorted_by_str(self.present.iter()).into_iter().map(|k| format!("{}{}: ?", key_display(k), self.mark(k))).collect();
        if !self.closed() {
            parts.push("**?".into());
        }
        write!(f, "{{{}}}", parts.join(", "))
    }
}

/// The unknown calls a row names (only for people: any one of them makes the row open):
/// past 4, the first 4 and "…".
fn cap_mutators(ms: StrSet) -> StrSet {
    if ms.len() <= 5 {
        return ms;
    }
    let mut v: Vec<StrId> = ms.iter().filter(|m| *m != intern::MORE).take(4).collect();
    v.push(intern::MORE);
    StrSet::from_vec(v)
}

fn cap_sources(ss: SrcSet) -> SrcSet {
    // (sorted by source: its lenses are consecutive)
    let mut by: Vec<(SourceId, Vec<LensId>)> = Vec::new();
    for (s, l) in ss.iter() {
        match by.last_mut() {
            Some(e) if e.0 == s => e.1.push(l),
            _ => by.push((s, vec![l])),
        }
    }
    if by.iter().all(|(_, ls)| ls.len() <= MAX_LENSES) {
        return ss;
    }
    let mut out = Vec::new();
    for (s, ls) in by {
        if ls.len() > MAX_LENSES {
            out.push((s, EMPTY_LENS));
        } else {
            out.extend(ls.into_iter().map(|l| (s, l)));
        }
    }
    SrcSet::from_vec(out)
}

pub fn join_rows<'a>(rows: impl IntoIterator<Item = Row>) -> Row {
    // (equal rows once — the objects of one site often share theirs — then pairwise in a
    // balanced tree: a left fold over thousands re-unioned an ever larger accumulator)
    let mut seen: rustc_hash::FxHashSet<Row> = rustc_hash::FxHashSet::default();
    let mut level: Vec<Row> = rows.into_iter().filter(|r| seen.insert(r.clone())).collect();
    if level.is_empty() {
        return Row::new();
    }
    while level.len() > 1 {
        let mut next: Vec<Row> = Vec::with_capacity(level.len().div_ceil(2));
        let mut it = level.into_iter();
        while let Some(a) = it.next() {
            next.push(match it.next() {
                Some(b) => a.join(&b),
                None => a,
            });
        }
        level = next;
    }
    level.pop().unwrap()
}

// ---------------------------------------------------------------------------
// call chains (reporting contexts)
// ---------------------------------------------------------------------------

pub type Chain = Vec<u32>;

/// Canonical call chain: a call site repeated in direct recursion is kept once and the
/// chain keeps its innermost MAX_CHAIN sites.
pub fn chain_extend(chain: &[u32], more: &[u32]) -> Chain {
    let mut out: Vec<u32> = chain.to_vec();
    for &x in more {
        if out.last() != Some(&x) {
            out.push(x);
        }
    }
    if out.len() > MAX_CHAIN {
        out.drain(..out.len() - MAX_CHAIN);
    }
    out
}

// ---------------------------------------------------------------------------
// verdict classification (§10)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum Tier {
    Proven,
    Likely,
    Map,
    Input,
    Possibly,
    DataKey,
    LikelyMissing,
    Missing,
    Unknown,
}

impl Tier {
    pub fn rank(self) -> u8 {
        match self {
            Tier::Proven => 0,
            Tier::Likely => 1,
            Tier::Map => 2,
            Tier::Input => 3,
            Tier::Possibly => 4,
            Tier::DataKey => 5,
            Tier::LikelyMissing => 6,
            Tier::Missing => 7,
            Tier::Unknown => 8,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Tier::Proven => "proven",
            Tier::Likely => "likely",
            Tier::Possibly => "possibly",
            Tier::Missing => "missing",
            Tier::LikelyMissing => "likely-missing",
            Tier::Input => "input",
            Tier::DataKey => "datakey",
            Tier::Map => "map",
            Tier::Unknown => "unknown",
        }
    }
    pub fn symbol(self) -> &'static str {
        match self {
            Tier::Proven => "✓",
            Tier::Likely => "✓~",
            Tier::Possibly => "⚠",
            Tier::Missing => "✗",
            Tier::LikelyMissing => "⚠~",
            Tier::Input => "◌",
            Tier::DataKey => "⚠data",
            Tier::Map => "·map",
            Tier::Unknown => "?",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Classification {
    pub tier: Tier,
    /// external sources that may provide k (sorted by source label)
    pub providers: Vec<(SourceId, KeySet)>,
    pub note: String,
    /// the unknown code that may have added or removed the key (`likely` tiers, inputs)
    pub mutators: Vec<StrId>,
}

impl Classification {
    pub fn new(tier: Tier) -> Self {
        Classification { tier, providers: Vec::new(), note: String::new(), mutators: Vec::new() }
    }
    pub fn with_note(tier: Tier, note: impl Into<String>) -> Self {
        Classification { tier, providers: Vec::new(), note: note.into(), mutators: Vec::new() }
    }
    fn by_unknown_code(tier: Tier, note: &str, row: &Row) -> Self {
        Classification { mutators: row.mutators.iter().collect(), ..Classification::with_note(tier, note) }
    }
}

/// The note of a key proven only because the code read or tested it before.
pub const FOUND_BEFORE: &str = "found before";

pub fn classify(row: &Row, k: KeyId) -> Classification {
    if row.definite.contains(k) {
        // (a key of an open record the program never stored: a read or a test found it)
        if row.open() && !row.stored.contains(k) {
            return Classification::with_note(Tier::Proven, FOUND_BEFORE);
        }
        return Classification::new(Tier::Proven);
    }
    if row.absent.contains(k) {
        return Classification::with_note(Tier::Missing, "was removed or renamed on every path to here");
    }
    if row.likely.contains(k) {
        return Classification::by_unknown_code(Tier::Likely, "unless unknown code removed it", row);
    }
    if row.top {
        return Classification::new(Tier::Unknown);
    }
    if row.closed() {
        if row.present.contains(k) {
            return Classification::with_note(Tier::Possibly, "is only set on some paths");
        }
        return Classification::with_note(Tier::Missing, "is never set");
    }
    let prov = row.provides(k);
    let mut ext: Vec<(SourceId, KeySet)> = prov.iter().filter(|(s, _)| !source_is_data(*s)).cloned().collect();
    ext.sort_by_cached_key(|(s, _)| source_label(*s));
    let data = prov.iter().any(|(s, _)| source_is_data(*s));
    if !ext.is_empty() {
        let note = if row.present.contains(k) { "also set on some paths" } else { "" };
        return Classification { providers: ext, ..Classification::by_unknown_code(Tier::Input, note, row) };
    }
    if data {
        return Classification::new(Tier::Map);
    }
    if row.present.contains(k) {
        return Classification::with_note(Tier::Possibly, "is only set on some paths");
    }
    if !row.mutators.is_empty() {
        return Classification::by_unknown_code(Tier::LikelyMissing, "unless unknown code added it", row);
    }
    Classification::with_note(Tier::Missing, "is never set")
}

// ---------------------------------------------------------------------------
// key values (§2)
// ---------------------------------------------------------------------------

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct KeysOf {
    pub objs: ObjSet,
    pub ns: Ns,
    pub exclude: KeySet,
    pub chain: LensId,
    /// conditional depth at which an iteration over these keys is exhaustive, or -1
    pub exhaustive_depth: i32,
}

impl KeysOf {
    pub fn new(objs: ObjSet, ns: Ns) -> Self {
        KeysOf { objs, ns, exclude: KeySet::new(), chain: EMPTY_LENS, exhaustive_depth: -1 }
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum KeyVal {
    Lit(KeyId),
    Lits(KeySet),
    KeysOf(KeysOf),
    /// site label ("" = plain DATA)
    Data(StrId),
    Bot,
}

pub const DATA: KeyVal = KeyVal::Data(intern::EMPTY_STR);
pub const BOT: KeyVal = KeyVal::Bot;

impl KeyVal {
    pub fn data(site: &str) -> KeyVal {
        if site.is_empty() {
            DATA
        } else {
            KeyVal::Data(intern::s(site))
        }
    }
    pub fn lits(ks: KeySet) -> KeyVal {
        if ks.len() == 1 {
            KeyVal::Lit(ks.first().unwrap())
        } else {
            KeyVal::Lits(ks)
        }
    }
    pub fn is_bot(&self) -> bool {
        matches!(self, KeyVal::Bot)
    }
    pub fn is_literal(&self) -> bool {
        matches!(self, KeyVal::Lit(_) | KeyVal::Lits(_))
    }
    /// Whether a value of this scalar part may have items (be subscripted or iterated): an
    /// unknown, a mapping's key, a str / bytes / tuple literal; not None, a number, a bool.
    pub fn may_have_items(&self) -> bool {
        match self {
            KeyVal::Bot => false,
            KeyVal::Lit(_) | KeyVal::Lits(_) => !kv_literals(self).iter().all(|k| intern::with_key(k, |kk| matches!(kk, intern::Key::None | intern::Key::Int(_) | intern::Key::Bool(_) | intern::Key::Float(_)))),
            _ => true,
        }
    }
    pub fn as_lit(&self) -> Option<KeyId> {
        match self {
            KeyVal::Lit(k) => Some(*k),
            _ => None,
        }
    }
    pub fn data_site(&self) -> Option<StrId> {
        match self {
            KeyVal::Data(s) => Some(*s),
            _ => None,
        }
    }
}

impl fmt::Display for KeyVal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyVal::Lit(k) => f.write_str(&key_repr(*k)),
            KeyVal::Lits(ks) => f.write_str(&keys_str(ks.iter())),
            KeyVal::KeysOf(ko) => {
                let objs: Vec<String> = ko.objs.iter().map(|o| format!("o{}", o)).collect();
                write!(f, "keys({})", objs.join(","))?;
                if !ko.exclude.is_empty() {
                    write!(f, "∖{}", keys_str(ko.exclude.iter()))?;
                }
                if ko.chain != EMPTY_LENS {
                    f.write_str(&lens_str(ko.chain))?;
                }
                Ok(())
            }
            KeyVal::Data(s) => {
                if *s == intern::EMPTY_STR {
                    f.write_str("data")
                } else {
                    write!(f, "data({})", s)
                }
            }
            KeyVal::Bot => f.write_str("⊥"),
        }
    }
}

/// The builtin types an unknown may be known to have (`int(?)`): from an annotation trusted in a
/// run for any call, a builtin's result, an operator on operands of these types.
pub const TYPE_NAMES: [&str; 5] = ["bool", "bytes", "float", "int", "str"];

/// The label of an unknown of type `ty` (one of `TYPE_NAMES`), named `name` (a label as
/// `Value::data` takes it, "" for none): `\u{2}int\u{2}expr:3:x + 1`.
pub fn typed_label(ty: &str, name: &str) -> String {
    format!("\u{2}{}\u{2}{}", ty, label_name(name))
}

/// The type of an unknown by its label: a typed label's (`typed_label`), and the labels that
/// name a type (`"str"`: a text not known, a str method's result; `"int"`).
pub fn label_type(label: &str) -> Option<&'static str> {
    let ty = match label.strip_prefix('\u{2}') {
        Some(rest) => rest.split('\u{2}').next()?,
        None if matches!(label, "str" | "int") => label,
        None => return None,
    };
    TYPE_NAMES.iter().copied().find(|t| *t == ty)
}

/// A label without its type: what names the unknown (`expr:3:x + 1`, "" for none).
pub fn label_name(label: &str) -> &str {
    match label.strip_prefix('\u{2}').and_then(|rest| rest.split_once('\u{2}')) {
        Some((_, name)) => name,
        None => label,
    }
}

/// The builtin type of a literal key (`int` for a bool too: an int it is), None for others.
pub fn key_type(k: KeyId, ty: &str) -> bool {
    crate::intern::with_key(k, |kk| match kk {
        crate::intern::Key::Str(_) => ty == "str",
        crate::intern::Key::Int(_) => ty == "int",
        crate::intern::Key::Bool(_) => ty == "bool" || ty == "int",
        crate::intern::Key::Float(_) => ty == "float",
        crate::intern::Key::Bytes(_) => ty == "bytes",
        _ => false,
    })
}

/// A typed label's type dropped (the value may be of another type now), its name kept.
fn untyped(label: StrId) -> KeyVal {
    let l = crate::intern::str_get(label);
    if label_type(&l).is_none() {
        return KeyVal::Data(label);
    }
    let name = label_name(&l);
    if name == &*l { DATA } else { KeyVal::data(name) }
}

pub fn kv_join(a: &KeyVal, b: &KeyVal) -> KeyVal {
    if a == b {
        return a.clone();
    }
    let empty_lits = |x: &KeyVal| matches!(x, KeyVal::Lits(ks) if ks.is_empty());
    if a.is_bot() || empty_lits(a) {
        return b.clone();
    }
    if b.is_bot() || empty_lits(b) {
        return a.clone();
    }
    if a.is_literal() && b.is_literal() {
        let ks = kv_literals(a).union(&kv_literals(b));
        if ks.len() > MAX_LITS {
            return DATA;
        }
        return KeyVal::lits(ks);
    }
    if let (KeyVal::KeysOf(x), KeyVal::KeysOf(y)) = (a, b) {
        if x.exclude == y.exclude && x.chain == y.chain && x.ns == y.ns {
            return KeyVal::KeysOf(KeysOf { objs: x.objs.union(&y.objs), ns: x.ns, exclude: x.exclude.clone(), chain: x.chain, exhaustive_depth: -1 });
        }
    }
    if let (KeyVal::Data(x), KeyVal::Data(y)) = (a, b) {
        let (lx, ly) = (crate::intern::str_get(*x), crate::intern::str_get(*y));
        if let Some(j) = crate::models::shape::join_labels(&lx, &ly) {
            return KeyVal::data(&j);
        }
        // (two unknowns of one type: an unknown of that type)
        return match (label_type(&lx), label_type(&ly)) {
            (Some(t), Some(u)) if t == u => KeyVal::data(&typed_label(t, "")),
            _ => DATA,
        };
    }
    // a symbolic dim joined with a literal: an unknown dim (not "anything": the literal stays
    // possible); a tensor joined with a number: rank 0 possible
    for (d, l) in [(a, b), (b, a)] {
        if let (KeyVal::Data(x), true) = (d, l.is_literal()) {
            let t = crate::intern::str_get(*x);
            if crate::models::shape::parse_dim_label(&t).is_some() {
                let lits = kv_literals(l);
                // (a dim or None, or a str: no dim)
                if !lits.iter().all(crate::intern::key_is_number) {
                    return DATA;
                }
                let same = crate::models::shape::parse_dim_label(&t).and_then(crate::models::shape::as_lit).map(|n| lits.iter().all(|k| crate::intern::key_as_int(k) == Some(n))).unwrap_or(false);
                return if same { d.clone() } else { KeyVal::data(&crate::models::shape::dim_label(crate::models::shape::unknown())) };
            }
            if let Some(sid) = crate::models::shape::parse_tensor_label(crate::models::shape::strip_optional(&t).unwrap_or(&t)) {
                // a tensor or a number: rank 0 possible; a tensor or None: an optional tensor;
                // or anything else: an unknown (the literal stays possible)
                let lits = kv_literals(l);
                let none = crate::intern::key_none();
                let (nums, rest): (Vec<KeyId>, Vec<KeyId>) = lits.iter().filter(|k| *k != none).partition(|k| crate::intern::key_is_number(*k));
                if !rest.is_empty() {
                    return DATA;
                }
                let mut out = t.to_string();
                if !nums.is_empty() {
                    let s = crate::models::shape::join_shapes(&crate::models::shape::shape_get(sid), &crate::models::shape::Shape::known(vec![]));
                    out = crate::models::shape::tensor_label(crate::models::shape::shape_id(s));
                    if crate::models::shape::strip_optional(&t).is_some() {
                        out = crate::models::shape::optional_label(&out);
                    }
                }
                if lits.contains(none) {
                    out = crate::models::shape::optional_label(&out);
                }
                return KeyVal::data(&out);
            }
        }
    }
    // an unknown joined with literals: the unknown, of its type if they are of it too
    for (d, l) in [(a, b), (b, a)] {
        if let KeyVal::Data(x) = d {
            let t = crate::intern::str_get(*x);
            // (a label that says what the text is — a JSON or pickle text of known parts, a CSV
            // text of known header — is no longer that)
            if ["pyjsontext:", "pickletext:", "csvhdr:"].iter().any(|p| t.starts_with(p)) {
                return DATA;
            }
            let fits = label_type(&t).is_some_and(|t| kv_literals(l).iter().all(|k| key_type(k, t)));
            return if fits || !l.is_literal() { d.clone() } else { untyped(*x) };
        }
    }
    DATA
}

pub fn kv_literals(kv: &KeyVal) -> KeySet {
    match kv {
        KeyVal::Lit(k) => KeySet::one(*k),
        KeyVal::Lits(ks) => ks.clone(),
        _ => KeySet::new(),
    }
}

/// Texts longer than this are not computed: a str not known (`s.replace("", "ab")` in a loop
/// triples each round).
pub const TEXT_MAX: usize = 1 << 16;

pub fn kv_map(kv: &KeyVal, step: &RenameFn) -> KeyVal {
    if kv_literals(kv).iter().any(|k| step.out_len(k) > TEXT_MAX) {
        return KeyVal::data(&typed_label("str", ""));
    }
    match kv {
        KeyVal::Lit(k) => KeyVal::Lit(step.apply(*k)),
        KeyVal::Lits(ks) => KeyVal::Lits(ks.map(|k| step.apply(k))),
        KeyVal::KeysOf(ko) => {
            let mut ko2 = ko.clone();
            ko2.chain = lens_push(ko.chain, Step::RenameFn(step.clone()));
            KeyVal::KeysOf(ko2)
        }
        _ => kv.clone(),
    }
}

/// Narrow a key value by `k not in keys`; None means the path is infeasible.
pub fn kv_exclude(kv: &KeyVal, keys: &KeySet) -> Option<KeyVal> {
    match kv {
        KeyVal::Lit(k) => {
            if keys.contains(*k) {
                None
            } else {
                Some(kv.clone())
            }
        }
        KeyVal::Lits(ks) => {
            let rest = ks.difference(keys);
            if rest.is_empty() {
                None
            } else {
                Some(KeyVal::lits(rest))
            }
        }
        KeyVal::KeysOf(ko) => {
            let mut ko2 = ko.clone();
            ko2.exclude = ko.exclude.union(keys);
            Some(KeyVal::KeysOf(ko2))
        }
        // a tensor or None, not None: the tensor
        KeyVal::Data(l) if keys.contains(crate::intern::key_none()) => {
            let t = crate::intern::str_get(*l);
            Some(crate::models::shape::strip_optional(&t).map(KeyVal::data).unwrap_or_else(|| kv.clone()))
        }
        _ => Some(kv.clone()),
    }
}

/// Narrow a key value by `k in keys`.
pub fn kv_restrict(kv: &KeyVal, keys: &KeySet) -> Option<KeyVal> {
    if keys.is_empty() {
        return None;
    }
    match kv {
        KeyVal::Lit(k) => {
            if keys.contains(*k) {
                Some(kv.clone())
            } else {
                None
            }
        }
        KeyVal::Lits(ks) => {
            let rest = ks.intersection(keys);
            if rest.is_empty() {
                None
            } else {
                Some(KeyVal::lits(rest))
            }
        }
        _ => Some(KeyVal::lits(keys.clone())),
    }
}

// ---------------------------------------------------------------------------
// requirements (§9)
// ---------------------------------------------------------------------------

#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct ReqInfo {
    pub always: bool,
    /// other sources that could satisfy the same access
    pub alternatives: IdSet<SourceId>,
    /// mutators that might have added the key instead
    pub caveats: StrSet,
    /// other key names of the same source that would also satisfy it
    pub alt_keys: KeySet,
    /// read as an attribute (`x.k`), not as an item (`x["k"]`): for reports
    pub attr: bool,
    /// tests of the function's parameters (Python text, true at its call: `admin`, `"email"
    /// not in row`) every path that read it passed: needed only if they hold (`State::path`)
    pub only_if: StrSet,
}

impl ReqInfo {
    pub fn join(&self, o: &ReqInfo) -> ReqInfo {
        ReqInfo {
            always: self.always && o.always,
            alternatives: self.alternatives.union(&o.alternatives),
            caveats: self.caveats.union(&o.caveats),
            alt_keys: self.alt_keys.union(&o.alt_keys),
            attr: self.attr || o.attr,
            only_if: self.only_if.intersection(&o.only_if),
        }
    }
    /// Same path, second requirement on the same key: always if either is.
    pub fn seq(&self, o: &ReqInfo) -> ReqInfo {
        ReqInfo {
            always: self.always || o.always,
            alternatives: if !self.alternatives.is_empty() && !o.alternatives.is_empty() {
                self.alternatives.intersection(&o.alternatives)
            } else {
                IdSet::new()
            },
            caveats: self.caveats.union(&o.caveats),
            alt_keys: if !self.alt_keys.is_empty() && !o.alt_keys.is_empty() { self.alt_keys.intersection(&o.alt_keys) } else { KeySet::new() },
            attr: self.attr || o.attr,
            // (every path here reads it now, and passed the tests that hold here)
            only_if: o.only_if.clone(),
        }
    }
    /// Needed on some paths only (the other side of a join does not read it).
    pub fn demote(&self) -> ReqInfo {
        if !self.always {
            self.clone()
        } else {
            self.sometimes()
        }
    }
    pub fn sometimes(&self) -> ReqInfo {
        ReqInfo { always: false, ..self.clone() }
    }
}

// ---------------------------------------------------------------------------
// values
// ---------------------------------------------------------------------------

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Value {
    pub pts: ObjSet,
    pub kv: KeyVal,
}

impl Default for Value {
    fn default() -> Self {
        Value { pts: ObjSet::new(), kv: BOT }
    }
}

impl Value {
    /// Push the objects this value refers to (its own, and those whose keys it ranges over).
    pub fn push_objs(&self, work: &mut Vec<ObjId>) {
        work.extend(self.pts.iter());
        if let KeyVal::KeysOf(ko) = &self.kv {
            work.extend(ko.objs.iter());
        }
    }

    pub fn new() -> Value {
        Value::default()
    }
    pub fn join(&self, o: &Value) -> Value {
        if self == o {
            return self.clone();
        }
        Value { pts: self.pts.union(&o.pts), kv: kv_join(&self.kv, &o.kv) }
    }
    pub fn empty(&self) -> bool {
        self.pts.is_empty() && self.kv.is_bot()
    }
    pub fn lit(k: KeyId) -> Value {
        Value { pts: ObjSet::new(), kv: KeyVal::Lit(k) }
    }
    pub fn lit_str(s: &str) -> Value {
        Value::lit(intern::key_str_lit(s))
    }
    pub fn obj(oid: ObjId) -> Value {
        Value { pts: ObjSet::one(oid), kv: BOT }
    }
    pub fn objs(pts: ObjSet) -> Value {
        Value { pts, kv: BOT }
    }
    pub fn data(site: &str) -> Value {
        Value { pts: ObjSet::new(), kv: KeyVal::data(site) }
    }
    pub fn with_kv(pts: ObjSet, kv: KeyVal) -> Value {
        Value { pts, kv }
    }
    pub fn kv_only(kv: KeyVal) -> Value {
        Value { pts: ObjSet::new(), kv }
    }
    pub fn single(&self) -> Option<ObjId> {
        if self.pts.len() == 1 && self.kv.is_bot() {
            self.pts.first()
        } else {
            None
        }
    }
}

pub fn join_values(vals: impl IntoIterator<Item = Value>) -> Value {
    // (the objects in one sort: pairwise unions of many large sets are quadratic)
    let mut ids: Vec<ObjId> = Vec::new();
    let mut kv = BOT;
    let mut n = 0usize;
    let mut first: Option<Value> = None;
    for v in vals {
        n += 1;
        ids.extend(v.pts.iter());
        kv = kv_join(&kv, &v.kv);
        if n == 1 {
            first = Some(v);
        }
    }
    match (n, first) {
        (0, _) => Value::new(),
        (1, Some(v)) => v,
        _ => Value { pts: ObjSet::from_vec(ids), kv },
    }
}
