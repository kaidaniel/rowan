//! Sources and lenses: where a record's unseen keys come from. A `Source` is an input (a
//! parameter of a run for any call, a library call's result, `os.environ`, …) a row is open
//! through; a `Lens` is how the program renamed or removed the input's keys on the way (a
//! `{k.upper(): v …}`, a `df.rename(…)`, a `del`), so a read of `k` maps back to the input's
//! key (`lens_provides`) and the requirement lands on it.

use std::fmt;
use std::sync::Arc;

pub use crate::intern::SourceId;
use crate::intern::{self, key_as_str, key_is_inv, key_is_str, key_py_str, key_repr, keys_str, lens_get, source_get, with_lens, with_source, KeyId, KeySet, LensId, EMPTY_LENS};

// ---------------------------------------------------------------------------
// lens steps
// ---------------------------------------------------------------------------

/// A str function applied to every key of a record (`{k.upper(): v for k, v in d.items()}`,
/// `df.add_prefix("x_")`): a lens step that renames keys.
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

/// A key-renaming function with its argument (the affix of `Prefix` / `Suffix`, the pair of
/// `Replace`, the separator of `Join`).
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

    /// The key `k` becomes (`k` itself where the function does not apply to it).
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

    /// The keys that become `k` (empty: none can); a key not computable back is an *inverse
    /// term* (`intern::key_inv`: "the key whose upper() is k").
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

/// One step of a lens: keys removed, keys renamed one by one, or every key renamed by a
/// function.
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

/// How the keys of an input reached a record (its steps in order): a row open through a source
/// with a lens may hold `k` when the input holds a key the lens maps to `k`.
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

/// `lens_apply` on the steps of a lens.
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

/// A lens for people: `·upper·∖{"a"}`.
pub fn lens_str(l: LensId) -> String {
    with_lens(l, |lens| lens.steps.iter().map(|s| format!("·{}", s)).collect::<String>())
}

/// `a` then `b`.
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

/// `a` then the step `s`.
pub fn lens_push(a: LensId, s: Step) -> LensId {
    let la = lens_get(a);
    intern::lens(la.with_step(s))
}

/// `l` then the removal of `k` (joined into a last removal step).
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

/// What kind of input a source is.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum SourceKind {
    /// an input the program reads from outside (`os.environ`, `sys.argv`, a request)
    External,
    /// the result of a library call (`json.loads(…)`, `pd.read_csv(…)`)
    Call,
    /// a parameter of a function run with open parameters (`param F.x`)
    Param,
    /// a global name with no binding rowan sees, not a builtin (one a star import of a library
    /// or `exec` may define)
    Name,
    /// keys computed from data (a mapping built from unknown keys): reads are `map` verdicts,
    /// not requirements
    Data,
    /// an instance with attributes rowan may not see: of a class with a base it does not see,
    /// or a receiver of a run for any call
    Class,
}

/// A reason an object may have keys rowan does not see: an input, named by where it comes
/// from (`site`), and the keys below it the object was read through (`path`:
/// `json.loads@L3['db']`). Interned: rows hold `SourceId`s.
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
    /// `site['k1']['k2']`.
    pub fn label(&self) -> String {
        let p: String = self.path.iter().map(|k| format!("[{}]", key_repr(*k))).collect();
        format!("{}{}", self.site, p)
    }
}

/// The source of what `s` holds under key `k`.
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

/// Is `s` keys computed from data (`SourceKind::Data`), not an input?
pub fn source_is_data(s: SourceId) -> bool {
    with_source(s, |src| src.kind == SourceKind::Data)
}

pub fn source_label(s: SourceId) -> String {
    with_source(s, |src| src.label())
}
