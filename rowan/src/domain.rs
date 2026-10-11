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
pub type Rho = u32;

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

pub fn rho(oid: ObjId, ns: Ns) -> Rho {
    oid * 2 + if ns == Ns::Attr { 1 } else { 0 }
}

pub fn unrho(r: Rho) -> (ObjId, Ns) {
    (r / 2, if r % 2 == 1 { Ns::Attr } else { Ns::Item })
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

pub fn lens_image(l: LensId, keys: &KeySet) -> KeySet {
    if l == EMPTY_LENS {
        return keys.clone();
    }
    KeySet::from_iter(keys.iter().filter_map(|k| lens_apply(l, k)))
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

fn lens_segments(l: &Lens) -> Vec<(KeySet, Option<Step>)> {
    let mut segs = Vec::new();
    let mut kills = KeySet::new();
    for step in &l.steps {
        match step {
            Step::Kill(ks) => kills = kills.union(ks),
            _ => {
                segs.push((kills, Some(step.clone())));
                kills = KeySet::new();
            }
        }
    }
    segs.push((kills, None));
    segs
}

/// Lens whose definite image is contained in both: same renames, kills united; None when
/// the renames differ.
pub fn lens_join_definite(a: LensId, b: LensId) -> Option<LensId> {
    if a == b {
        return Some(a);
    }
    let la = lens_get(a);
    let lb = lens_get(b);
    let sa = lens_segments(&la);
    let sb = lens_segments(&lb);
    if sa.len() != sb.len() || sa.iter().zip(sb.iter()).any(|(x, y)| x.1 != y.1) {
        return None;
    }
    let mut out = Vec::new();
    for ((ka, r), (kb, _)) in sa.into_iter().zip(sb.into_iter()) {
        let ks = ka.union(&kb);
        if !ks.is_empty() {
            out.push(Step::Kill(ks));
        }
        if let Some(r) = r {
            out.push(r);
        }
    }
    Some(intern::lens(Lens { steps: out }))
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
fn join_order(a: &Arc<[KeyId]>, b: &Arc<[KeyId]>) -> Arc<[KeyId]> {
    if a == b || b.len() <= a.len() && a[..b.len()] == b[..] {
        a.clone()
    } else if b[..a.len().min(b.len())] == a[..] {
        b.clone()
    } else {
        Arc::from(Vec::new())
    }
}

pub type SrcSet = IdSet<(SourceId, LensId)>;
pub type TermSet = IdSet<(Rho, LensId)>;
pub type PStoreSet = IdSet<(Rho, bool, LensId)>;
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
    pub terms: TermSet,
    pub dterms: TermSet,
    pub lterms: TermSet,
    pub pstores: PStoreSet,
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

    pub fn with_mutators(m: StrSet) -> Row {
        Row::from_data(RowData { mutators: m, ..Default::default() })
    }

    pub fn top() -> Row {
        Row::from_data(RowData { top: true, ..Default::default() })
    }

    pub fn closed_with(keys: KeySet) -> Row {
        Row::from_data(RowData { present: keys.clone(), definite: keys.clone(), likely: keys, ..Default::default() })
    }

    /// The entry rows ρ this row's symbolic part refers to.
    pub fn refs(&self) -> IdSet<Rho> {
        IdSet::from_iter(
            self.terms
                .iter()
                .map(|(r, _)| r)
                .chain(self.dterms.iter().map(|(r, _)| r))
                .chain(self.lterms.iter().map(|(r, _)| r))
                .chain(self.pstores.iter().map(|(r, _, _)| r)),
        )
    }

    pub fn mentions(&self, ph: ObjId) -> bool {
        let (a, b) = (ph * 2, ph * 2 + 1);
        let hit = |t: &TermSet| t.iter().any(|(r, _)| r == a || r == b);
        hit(&self.terms) || hit(&self.dterms) || hit(&self.lterms) || self.pstores.iter().any(|(r, _, _)| r == a || r == b)
    }

    pub fn symbolic(&self) -> bool {
        !self.terms.is_empty() || !self.dterms.is_empty() || !self.lterms.is_empty() || !self.pstores.is_empty()
    }

    pub fn open(&self) -> bool {
        !self.sources.is_empty() || !self.mutators.is_empty() || self.top || self.symbolic()
    }

    /// Content from an input or the function's entry (not only from unknown code run on it).
    pub fn has_input(&self) -> bool {
        !self.sources.is_empty() || self.top || self.symbolic()
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

    pub fn external_sources(&self) -> SrcSet {
        self.sources.filter(|(s, _)| !source_is_data(s))
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
            mutators: self.mutators.union(&o.mutators),
            top: self.top || o.top,
            terms: if self.terms != o.terms { cap_terms(self.terms.union(&o.terms)) } else { self.terms.clone() },
            dterms: join_dterms(&self.dterms, &o.dterms),
            lterms: join_dterms(&self.lterms, &o.lterms),
            pstores: join_pstores(&self.pstores, &o.pstores),
            order: join_order(&self.order, &o.order),
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

    /// Store under the (unknown) key held by placeholder ph.
    pub fn param_store(&self, ph: ObjId, strong: bool) -> Row {
        let mut d = self.data();
        d.pstores = d.pstores.insert((ph, strong, EMPTY_LENS));
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
            d.terms = d.terms.map(|(r, l)| (r, lens_add_kill(l, k)));
            d.dterms = d.dterms.map(|(r, l)| (r, lens_add_kill(l, k)));
            d.lterms = d.lterms.map(|(r, l)| (r, lens_add_kill(l, k)));
            d.pstores = d.pstores.map(|(r, st, l)| (r, st, lens_add_kill(l, k)));
        } else {
            d.definite = d.definite.remove(k);
            d.likely = d.likely.remove(k);
            d.dterms = d.dterms.map(|(r, l)| (r, lens_add_kill(l, k)));
            d.lterms = d.lterms.map(|(r, l)| (r, lens_add_kill(l, k)));
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
            terms: self.terms.map(|(r, l)| (r, push(l))),
            dterms: self.dterms.map(|(r, l)| (r, push(l))),
            lterms: self.lterms.map(|(r, l)| (r, push(l))),
            pstores: self.pstores.map(|(r, st, l)| (r, st, push(l))),
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
            mutators: self.mutators.union(&o.mutators),
            top: self.top || o.top,
            terms: cap_terms(self.terms.union(&o.terms.map(|(r, l)| (r, lens_concat(l, pre))))),
            dterms: if strong { self.dterms.union(&o.dterms.map(|(r, l)| (r, lens_concat(l, pre)))) } else { self.dterms.clone() },
            lterms: if strong { self.lterms.union(&o.lterms.map(|(r, l)| (r, lens_concat(l, pre)))) } else { self.lterms.clone() },
            pstores: self.pstores.union(&o.pstores.map(|(r, st, l)| (r, st && strong, lens_concat(l, pre)))),
            order: {
                // a strong copy of a row whose order is known appends its new keys in order
                let mut ord = self.order.to_vec();
                match (strong, o.ordered_keys()) {
                    (true, Some(ks)) => {
                        for k in ks.into_iter().filter(|k| !kill.contains(*k)).filter_map(|k| lens_apply(chain, k)) {
                            if !ord.contains(&k) {
                                ord.push(k);
                            }
                        }
                    }
                    _ => {}
                }
                ord.into()
            },
            stored: self.stored.union(&img(&o.stored)),
        })
    }

    pub fn mutate(&self, site: StrId) -> Row {
        let mut d = self.data();
        d.likely = d.likely.union(&d.definite);
        d.definite = KeySet::new();
        d.absent_likely = d.absent_likely.union(&d.absent);
        d.absent = KeySet::new();
        d.mutators = d.mutators.insert(site);
        d.lterms = d.lterms.union(&d.dterms);
        d.dterms = TermSet::new();
        d.pstores = d.pstores.map(|(r, _, l)| (r, false, l));
        Row::from_data(d)
    }

    pub fn narrow_in(&self, k: KeyId, strong: bool) -> Row {
        // (the key was there: the program did not store it)
        let was = self.stored.contains(k);
        let r = self.store(k, strong);
        if was {
            return r;
        }
        let mut d = r.data();
        d.stored = d.stored.remove(k);
        Row::from_data(d)
    }

    pub fn narrow_out(&self, k: KeyId) -> Row {
        self.delete(k, true)
    }

    pub fn make_top(&self) -> Row {
        let mut d = self.data();
        d.definite = KeySet::new();
        d.likely = KeySet::new();
        d.absent = KeySet::new();
        d.absent_likely = KeySet::new();
        d.top = true;
        Row::from_data(d)
    }

    /// The symbolic row of an object as it is at function entry.
    pub fn entry(ph: Rho) -> Row {
        let t = TermSet::one((ph, EMPTY_LENS));
        Row::from_data(RowData { terms: t.clone(), dterms: t.clone(), lterms: t, ..Default::default() })
    }

    /// Exactly `Row.entry(ρ)` for some ρ.
    pub fn is_entry(&self) -> bool {
        let t = &self.terms;
        t.len() == 1
            && *t == self.dterms
            && *t == self.lterms
            && self.pstores.is_empty()
            && self.present.is_empty()
            && self.absent.is_empty()
            && self.absent_likely.is_empty()
            && self.sources.is_empty()
            && self.mutators.is_empty()
            && !self.top
            && t.first().unwrap().1 == EMPTY_LENS
    }

    pub fn entry_rho(&self) -> Option<Rho> {
        self.terms.first().map(|(r, _)| r)
    }

    /// Replace the symbolic part by actual entry rows and actual key values of placeholders.
    pub fn concretise(&self, entry_row_of: &mut dyn FnMut(Rho) -> Row, kv_of: &mut dyn FnMut(ObjId) -> KeyVal) -> Row {
        self.concretise_opt(entry_row_of, kv_of, false)
    }

    /// `bot_is_data`: a parameter-keyed store whose key has no known value opens the row as
    /// a data-keyed map instead of being dropped.
    pub fn concretise_opt(&self, entry_row_of: &mut dyn FnMut(Rho) -> Row, kv_of: &mut dyn FnMut(ObjId) -> KeyVal, bot_is_data: bool) -> Row {
        if !self.symbolic() {
            return self.clone();
        }
        let mut present = self.present.clone();
        let mut definite = self.definite.clone();
        let mut likely = self.likely.clone();
        let mut sources = self.sources.clone();
        let mut mutators = self.mutators.clone();
        let mut top = self.top;
        let mut terms: Vec<(Rho, LensId)> = Vec::new();
        let mut dterms: Vec<(Rho, LensId)> = Vec::new();
        let mut lterms: Vec<(Rho, LensId)> = Vec::new();
        let mut pstores: Vec<(Rho, bool, LensId)> = Vec::new();
        // insertion order: the entry row's (one plain term), then the keys stored here
        let mut order: Vec<KeyId> = Vec::new();
        if self.terms.len() == 1 && self.terms.first().map(|(_, l)| l == EMPTY_LENS).unwrap_or(false) {
            let e = entry_row_of(self.terms.first().unwrap().0);
            if let Some(ks) = e.ordered_keys() {
                order = ks;
            }
        }
        for k in self.order.iter() {
            if !order.contains(k) {
                order.push(*k);
            }
        }
        for (r, l) in self.terms.iter() {
            let e = entry_row_of(r);
            present = present.union(&lens_image(l, &e.present));
            sources = sources.union(&e.sources.map(|(s, l2)| (s, lens_concat(l2, l))));
            mutators = mutators.union(&e.mutators);
            top = top || e.top;
            terms.extend(e.terms.iter().map(|(r2, l2)| (r2, lens_concat(l2, l))));
            pstores.extend(e.pstores.iter().map(|(r2, _, l2)| (r2, false, lens_concat(l2, l))));
        }
        for (r, l) in self.dterms.iter() {
            let e = entry_row_of(r);
            definite = definite.union(&lens_image(l, &e.definite));
            dterms.extend(e.dterms.iter().map(|(r2, l2)| (r2, lens_concat(l2, l))));
            pstores.extend(e.pstores.iter().map(|(r2, st, l2)| (r2, st, lens_concat(l2, l))));
        }
        for (r, l) in self.lterms.iter() {
            let e = entry_row_of(r);
            likely = likely.union(&lens_image(l, &e.likely.union(&e.definite)));
            lterms.extend(e.lterms.union(&e.dterms).iter().map(|(r2, l2)| (r2, lens_concat(l2, l))));
        }
        present = present.union(&definite);
        likely = likely.union(&definite);
        let absent = self.absent.difference(&present);
        let absent_likely = self.absent_likely.difference(&present);
        let mut out = Row::from_data(RowData {
            present,
            definite,
            likely,
            absent,
            absent_likely,
            sources: cap_sources(sources),
            mutators,
            top,
            terms: cap_terms(TermSet::from_vec(terms)),
            dterms: TermSet::from_vec(dterms),
            lterms: TermSet::from_vec(lterms),
            pstores: PStoreSet::new(),
            order: order.into(),
            stored: self.stored.clone(),
        });
        let mut remaining: Vec<(Rho, bool, LensId)> = Vec::new();
        // (several parameter-keyed stores, or one besides literal stores: their relative order
        // is not recorded, so a new key among them leaves the order unknown)
        let all_ps = self.pstores.union(&PStoreSet::from_vec(pstores));
        let order_unsure = all_ps.len() > 1 || (!all_ps.is_empty() && !self.order.is_empty());
        let known_before: KeySet = out.present.clone();
        for (r, strong, l) in all_ps.iter() {
            let kv = kv_of(r);
            match kv {
                KeyVal::Param(ph) => {
                    remaining.push((ph, strong, l));
                    continue;
                }
                _ => {}
            }
            let lits = kv_literals(&kv);
            if !lits.is_empty() {
                let one = lits.len() == 1;
                for k in lits.iter() {
                    if let Some(k2) = lens_apply(l, k) {
                        out = out.store(k2, strong && one);
                    }
                }
            } else if let KeyVal::KeysOf(_) = kv {
                out = out.data_store(Source::new(&format!("keys-of-store@{}", r), SourceKind::Data));
            } else if let KeyVal::Bot = kv {
                if bot_is_data {
                    out = out.data_store(Source::new(&format!("store@param{}", r), SourceKind::Data));
                }
                continue;
            } else {
                out = out.data_store(Source::new(&format!("store@param{}", r), SourceKind::Data));
            }
        }
        if order_unsure && !out.present.difference(&known_before).is_empty() {
            let mut d = out.data();
            d.order = Vec::new().into();
            out = Row::from_data(d);
        }
        if !remaining.is_empty() {
            let mut d = out.data();
            d.pstores = PStoreSet::from_vec(remaining);
            out = Row::from_data(d);
        }
        out
    }
}

/// A key as it appears in the compact row form: strings in double quotes, others as in Python.
pub fn key_display(k: KeyId) -> String {
    match intern::key_as_str(k) {
        Some(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
        None => intern::key_repr(k),
    }
}

impl Row {
    /// `{"a": ?, "b"?: ?, "c"~: ?, **?}`: definite keys plain, keys set on some paths only
    /// with `?`, keys that unknown code may have removed with `~`, `**?` when other keys may
    /// exist (open; values are not part of the key set: `?`).  Attributes render as
    /// `obj(a=?, b?=?, **?)`.
    pub fn compact(&self, attrs: bool) -> String {
        let mut parts: Vec<String> = Vec::new();
        for k in intern::sorted_by_str(self.present.iter()) {
            let mark = if self.definite.contains(k) {
                ""
            } else if self.likely.contains(k) {
                "~"
            } else {
                "?"
            };
            if attrs {
                parts.push(format!("{}{}=?", intern::key_py_str(k), mark));
            } else {
                parts.push(format!("{}{}: ?", key_display(k), mark));
            }
        }
        if !self.closed() {
            parts.push("**?".into());
        }
        if attrs {
            format!("obj({})", parts.join(", "))
        } else {
            format!("{{{}}}", parts.join(", "))
        }
    }

    /// The full internal form (sets, sources, mutators, symbolic terms).
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
        if !self.terms.is_empty() {
            let mut v: Vec<String> = self.terms.iter().map(|(r, l)| format!("ρ{}{}", r, lens_str(l))).collect();
            v.sort();
            parts.push(format!("terms={{{}}}", v.join(", ")));
        }
        if !self.dterms.is_empty() && self.dterms != self.terms {
            let mut v: Vec<String> = self.dterms.iter().map(|(r, l)| format!("ρ{}{}", r, lens_str(l))).collect();
            v.sort();
            parts.push(format!("dterms={{{}}}", v.join(", ")));
        }
        if !self.pstores.is_empty() {
            let mut v: Vec<String> =
                self.pstores.iter().map(|(r, st, l)| format!("ρ{}{}{}", r, if st { "!" } else { "?" }, lens_str(l))).collect();
            v.sort();
            parts.push(format!("pstores={{{}}}", v.join(", ")));
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
        write!(f, "{}", self.compact(false))
    }
}

fn cap_terms(ts: TermSet) -> TermSet {
    // the set is sorted by (rho, lens), so the terms of one rho are contiguous: one pass
    let items = ts.as_slice();
    let runs = |f: &mut dyn FnMut(usize, usize)| {
        let mut i = 0;
        while i < items.len() {
            let mut j = i + 1;
            while j < items.len() && items[j].0 == items[i].0 {
                j += 1;
            }
            f(i, j);
            i = j;
        }
    };
    let mut over = false;
    runs(&mut |i, j| {
        if j - i > MAX_LENSES {
            over = true;
        }
    });
    if !over {
        return ts;
    }
    let mut out: Vec<(Rho, LensId)> = Vec::with_capacity(items.len());
    runs(&mut |i, j| {
        if j - i > MAX_LENSES {
            out.push((items[i].0, EMPTY_LENS));
        } else {
            out.extend_from_slice(&items[i..j]);
        }
    });
    TermSet::from_vec(out)
}

/// Definite terms common to both sides (same renames; kills united).
fn join_dterms(a: &TermSet, b: &TermSet) -> TermSet {
    if a == b {
        return a.clone();
    }
    let mut out = Vec::new();
    for (r, la) in a.iter() {
        for (rb, lb) in b.iter() {
            if rb != r {
                continue;
            }
            if let Some(m) = lens_join_definite(la, lb) {
                out.push((r, m));
            }
        }
    }
    TermSet::from_vec(out)
}

fn join_pstores(a: &PStoreSet, b: &PStoreSet) -> PStoreSet {
    if a == b {
        return a.clone();
    }
    let mut out: Vec<(Rho, bool, LensId)> = Vec::new();
    let keys: Vec<(Rho, LensId)> = a.iter().map(|(r, _, l)| (r, l)).chain(b.iter().map(|(r, _, l)| (r, l))).collect();
    let keys = IdSet::from_vec(keys);
    for (r, l) in keys.iter() {
        let sa = a.iter().find(|(x, _, y)| *x == r && *y == l).map(|(_, s, _)| s);
        let sb = b.iter().find(|(x, _, y)| *x == r && *y == l).map(|(_, s, _)| s);
        let strong = sa.unwrap_or(false) && sb.unwrap_or(false);
        out.push((r, strong, l));
    }
    PStoreSet::from_vec(out)
}

fn cap_sources(ss: SrcSet) -> SrcSet {
    let mut by: Vec<(SourceId, Vec<LensId>)> = Vec::new();
    for (s, l) in ss.iter() {
        if let Some(e) = by.iter_mut().find(|(x, _)| *x == s) {
            e.1.push(l);
        } else {
            by.push((s, vec![l]));
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
    let mut it = rows.into_iter();
    let Some(mut acc) = it.next() else { return Row::new() };
    for r in it {
        acc = acc.join(&r);
    }
    acc
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
    Sym,
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
            Tier::Sym => 9,
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
            Tier::Sym => "symbolic",
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
            Tier::Sym => "ρ",
        }
    }
    pub fn text(self) -> &'static str {
        match self {
            Tier::Proven => "ok",
            Tier::Likely => "ok (likely — unknown code ran in between)",
            Tier::Possibly => "possibly missing",
            Tier::Missing => "definitely missing",
            Tier::LikelyMissing => "likely missing",
            Tier::Input => "depends on input",
            Tier::DataKey => "data-derived key into a closed record",
            Tier::Map => "map key",
            Tier::Unknown => "unknown",
            Tier::Sym => "symbolic",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Classification {
    pub tier: Tier,
    /// external sources that may provide k (sorted by source label)
    pub providers: Vec<(SourceId, KeySet)>,
    pub note: String,
}

impl Classification {
    pub fn new(tier: Tier) -> Self {
        Classification { tier, providers: Vec::new(), note: String::new() }
    }
    pub fn with_note(tier: Tier, note: impl Into<String>) -> Self {
        Classification { tier, providers: Vec::new(), note: note.into() }
    }
}

fn joined_mutators(row: &Row) -> String {
    let mut v: Vec<String> = row.mutators.iter().map(|m| m.to_string()).collect();
    v.sort();
    v.join(", ")
}

pub fn classify(row: &Row, k: KeyId) -> Classification {
    if row.definite.contains(k) {
        return Classification::new(Tier::Proven);
    }
    if row.absent.contains(k) {
        return Classification::with_note(Tier::Missing, "removed/excluded on every path");
    }
    if row.likely.contains(k) {
        return Classification::with_note(Tier::Likely, format!("unknown code ran: {}", joined_mutators(row)));
    }
    if row.symbolic() {
        return Classification::new(Tier::Sym);
    }
    if row.top {
        return Classification::new(Tier::Unknown);
    }
    if row.closed() {
        if row.present.contains(k) {
            return Classification::with_note(Tier::Possibly, "set on some paths only");
        }
        return Classification::with_note(Tier::Missing, "all keys known");
    }
    let prov = row.provides(k);
    let mut ext: Vec<(SourceId, KeySet)> = prov.iter().filter(|(s, _)| !source_is_data(*s)).cloned().collect();
    ext.sort_by_key(|(s, _)| source_label(*s));
    let data = prov.iter().any(|(s, _)| source_is_data(*s));
    if !ext.is_empty() {
        let mut note = String::new();
        if !row.mutators.is_empty() {
            note = format!("unless {} adds it", joined_mutators(row));
        }
        if row.present.contains(k) {
            note = if note.is_empty() { "also set on some paths".into() } else { format!("{}; also set on some paths", note) };
        }
        return Classification { tier: Tier::Input, providers: ext, note };
    }
    if data {
        return Classification::new(Tier::Map);
    }
    if row.present.contains(k) {
        return Classification::with_note(Tier::Possibly, "set on some paths only");
    }
    if !row.mutators.is_empty() {
        return Classification::with_note(Tier::LikelyMissing, format!("only {} could have added it", joined_mutators(row)));
    }
    Classification::with_note(Tier::Missing, "no source can provide it")
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
    /// the (unknown) value of placeholder object `ph`
    Param(ObjId),
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
    pub fn as_lit(&self) -> Option<KeyId> {
        match self {
            KeyVal::Lit(k) => Some(*k),
            _ => None,
        }
    }
    pub fn as_param(&self) -> Option<ObjId> {
        match self {
            KeyVal::Param(p) => Some(*p),
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
            KeyVal::Param(p) => write!(f, "param(ρ{})", p),
        }
    }
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
    if matches!(a, KeyVal::Param(_)) || matches!(b, KeyVal::Param(_)) {
        return DATA;
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
        if let Some(j) = crate::shape::join_labels(&crate::intern::str_get(*x), &crate::intern::str_get(*y)) {
            return KeyVal::data(&j);
        }
        return DATA;
    }
    // a symbolic dim joined with a literal: an unknown dim (not "anything": the literal stays
    // possible); a tensor joined with a number: rank 0 possible
    for (d, l) in [(a, b), (b, a)] {
        if let (KeyVal::Data(x), true) = (d, l.is_literal()) {
            let t = crate::intern::str_get(*x);
            if crate::shape::parse_dim_label(&t).is_some() {
                let lits = kv_literals(l);
                let same = crate::shape::parse_dim_label(&t).and_then(crate::shape::as_lit).map(|n| lits.iter().all(|k| crate::intern::key_as_int(k) == Some(n))).unwrap_or(false);
                return if same { d.clone() } else { KeyVal::data(&crate::shape::dim_label(crate::shape::unknown())) };
            }
            if let Some(sid) = crate::shape::parse_tensor_label(&t) {
                if kv_literals(l).iter().any(crate::intern::key_is_number) {
                    let s = crate::shape::join_shapes(&crate::shape::shape_get(sid), &crate::shape::Shape::known(vec![]));
                    return KeyVal::data(&crate::shape::tensor_label(crate::shape::shape_id(s)));
                }
            }
        }
    }
    if let KeyVal::Data(_) = a {
        return a.clone();
    }
    if let KeyVal::Data(_) = b {
        return b.clone();
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

pub fn kv_map(kv: &KeyVal, step: &RenameFn) -> KeyVal {
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
        KeyVal::Param(_) => Some(kv.clone()),
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
    pub sites: StrSet,
    /// other sources that could satisfy the same access
    pub alternatives: IdSet<SourceId>,
    /// mutators that might have added the key instead
    pub caveats: StrSet,
    /// other key names of the same source that would also satisfy it
    pub alt_keys: KeySet,
}

impl ReqInfo {
    pub fn join(&self, o: &ReqInfo) -> ReqInfo {
        ReqInfo {
            always: self.always && o.always,
            sites: self.sites.union(&o.sites),
            alternatives: self.alternatives.union(&o.alternatives),
            caveats: self.caveats.union(&o.caveats),
            alt_keys: self.alt_keys.union(&o.alt_keys),
        }
    }
    /// Same path, second requirement on the same key: always if either is.
    pub fn seq(&self, o: &ReqInfo) -> ReqInfo {
        ReqInfo {
            always: self.always || o.always,
            sites: self.sites.union(&o.sites),
            alternatives: if !self.alternatives.is_empty() && !o.alternatives.is_empty() {
                self.alternatives.intersection(&o.alternatives)
            } else {
                IdSet::new()
            },
            caveats: self.caveats.union(&o.caveats),
            alt_keys: if !self.alt_keys.is_empty() && !o.alt_keys.is_empty() { self.alt_keys.intersection(&o.alt_keys) } else { KeySet::new() },
        }
    }
    pub fn demote(&self) -> ReqInfo {
        if !self.always {
            self.clone()
        } else {
            ReqInfo { always: false, ..self.clone() }
        }
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
        if self.pts.len() == 1 {
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
