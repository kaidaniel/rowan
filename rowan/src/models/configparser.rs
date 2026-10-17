//! `configparser`: a parser over literal INI text is simulated. The parser is a closed dict of
//! section names (`DEFAULT` included); each section is a case-insensitive closed dict of its
//! options (lower-cased, the DEFAULT options inherited) holding the interpolated strings.
//! Text that is not known (`read(path)`, a parameter) leaves the parser open: its sections
//! and options are requirements on that input.

use indexmap::IndexMap;

use crate::domain::{KeyVal, Ns, Row, Value, DATA};
use crate::frontend::Node;
use crate::intern::{self, key_str_lit, KeySet};
use crate::interp::{Analyzer, Call, Payload};
use crate::source::{Source, SourceKind};
use crate::state::{Kind, State};

/// What a `ConfigParser` read: its sections and options when the text is known.
#[derive(Clone, Debug, Default)]
pub struct CfgSpec {
    /// the text read so far is known
    pub known: bool,
    pub interpolate: bool,
    pub default_section: String,
    pub defaults: IndexMap<String, String>,
    pub sections: IndexMap<String, IndexMap<String, String>>,
    /// where the parser was made (the source of an unknown parser's content)
    pub line: u32,
}

/// Parse INI text the way `ConfigParser()` with default options does; None where it would
/// raise (a missing section header, a duplicate, an option without a value) or where the
/// text uses what is not simulated.
pub fn parse_ini(text: &str, spec: &mut CfgSpec) -> Option<()> {
    let mut cur: Option<String> = None;
    let mut last_opt: Option<String> = None;
    let mut seen_sections: Vec<String> = Vec::new();
    let mut seen_opts: Vec<(String, String)> = Vec::new();
    let mut blank_since = false;
    for raw in text.lines() {
        let line = raw.trim_end();
        let stripped = line.trim_start();
        if stripped.is_empty() {
            blank_since = true;
            continue;
        }
        if stripped.starts_with('#') || stripped.starts_with(';') {
            continue;
        }
        let indented = line.len() != stripped.len();
        if indented {
            if let (Some(s), Some(o)) = (cur.as_ref(), last_opt.as_ref()) {
                // a continuation line (after a blank line it would keep the blank line)
                if blank_since {
                    return None;
                }
                let tbl = if *s == spec.default_section { &mut spec.defaults } else { spec.sections.get_mut(s)? };
                let v = tbl.get_mut(o)?;
                v.push('\n');
                v.push_str(stripped);
                continue;
            }
        }
        blank_since = false;
        if stripped.starts_with('[') {
            let end = stripped.find(']')?;
            let name = stripped[1..end].to_string();
            if seen_sections.contains(&name) {
                return None;
            }
            seen_sections.push(name.clone());
            if name != spec.default_section && !spec.sections.contains_key(&name) {
                spec.sections.insert(name.clone(), IndexMap::new());
            }
            cur = Some(name);
            last_opt = None;
            continue;
        }
        let s = cur.clone()?;
        let i = stripped.find(|c| c == '=' || c == ':')?;
        let key = stripped[..i].trim().to_lowercase();
        let val = stripped[i + 1..].trim().to_string();
        if key.is_empty() || seen_opts.contains(&(s.clone(), key.clone())) {
            return None;
        }
        seen_opts.push((s.clone(), key.clone()));
        let tbl = if s == spec.default_section { &mut spec.defaults } else { spec.sections.get_mut(&s)? };
        tbl.insert(key.clone(), val);
        last_opt = Some(key);
    }
    Some(())
}

impl CfgSpec {
    /// The options of a section (DEFAULT's included), raw.
    fn raw_options(&self, sec: &str) -> Option<IndexMap<String, String>> {
        if sec == self.default_section {
            return Some(self.defaults.clone());
        }
        let own = self.sections.get(sec)?;
        let mut out = self.defaults.clone();
        for (k, v) in own {
            out.insert(k.clone(), v.clone());
        }
        Some(out)
    }

    /// `BasicInterpolation`: `%(name)s` from the section, `%%` a percent sign; None when a
    /// reference is missing or nests too deep.
    fn interpolated(&self, opts: &IndexMap<String, String>, v: &str, depth: usize) -> Option<String> {
        if !self.interpolate || !v.contains('%') {
            return Some(v.to_string());
        }
        if depth > 10 {
            return None;
        }
        let mut out = String::new();
        let mut rest = v;
        while let Some(i) = rest.find('%') {
            out.push_str(&rest[..i]);
            let after = &rest[i + 1..];
            if let Some(r) = after.strip_prefix('%') {
                out.push('%');
                rest = r;
            } else if let Some(r) = after.strip_prefix('(') {
                let j = r.find(")s")?;
                let name = r[..j].to_lowercase();
                let refv = opts.get(&name)?;
                out.push_str(&self.interpolated(opts, refv, depth + 1)?);
                rest = &r[j + 2..];
            } else {
                return None;
            }
        }
        out.push_str(rest);
        Some(out)
    }

    /// The options of a section as they read (interpolated; None: unknown text).
    pub fn options(&self, sec: &str) -> Option<IndexMap<String, Option<String>>> {
        let raw = self.raw_options(sec)?;
        Some(raw.iter().map(|(k, v)| (k.clone(), self.interpolated(&raw, v, 0))).collect())
    }
}

fn lit_str(v: &Value) -> Option<String> {
    v.kv.as_lit().filter(|_| v.pts.is_empty()).and_then(intern::key_as_str).map(|s| s.to_string())
}

fn parser_of(an: &Analyzer, c: &Call) -> Option<u32> {
    c.recv.as_ref().and_then(|r| r.single()).filter(|o| an.m.cfg_parsers.contains_key(o))
}

/// `configparser.ConfigParser(...)` / `RawConfigParser(...)`.
pub fn m_new(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let raw = c.qual.ends_with("RawConfigParser");
    let mut spec = CfgSpec { known: true, interpolate: !raw, default_section: "DEFAULT".into(), line: c.line, ..Default::default() };
    for (k, v) in c.kwargs.iter() {
        match &*intern::str_get(*k) {
            "interpolation" if v.pts.is_empty() && v.kv.as_lit() == Some(intern::key_none()) => spec.interpolate = false,
            "default_section" if lit_str(v).is_some() => spec.default_section = lit_str(v).unwrap(),
            "defaults" => match literal_mapping(an, st, v) {
                Some(m) => {
                    for (k, v) in m {
                        spec.defaults.insert(k.to_lowercase(), v);
                    }
                }
                None => spec.known = false,
            },
            "strict" | "empty_lines_in_values" => {}
            _ => spec.known = false,
        }
    }
    if !c.args.is_empty() || !c.star_args.is_empty() || !c.star_kwargs.is_empty() {
        spec.known = false;
    }
    let oid = an.alloc_at(Kind::Dict, &c.node, st, &format!("configparser.ConfigParser@L{}", c.line), Payload::None, Some(Row::new()), Ns::Item, None, None);
    an.m.cfg_parsers.insert(oid, spec);
    rebuild(an, st, &c.node, oid);
    Value::obj(oid)
}

/// A dict of literal str keys and literal values (`str()` of them).
fn literal_mapping(an: &mut Analyzer, st: &State, v: &Value) -> Option<IndexMap<String, String>> {
    let d = v.single().filter(|o| an.heap.kind(*o) == Kind::Dict)?;
    let row = st.row(d, Ns::Item);
    if row.open() || row.present != row.definite {
        return None;
    }
    let mut out = IndexMap::new();
    for k in an.literal_dict_items(d, st)?.into_iter() {
        let (k, val) = k;
        let ks = intern::key_as_str(k)?.to_string();
        let lv = val.kv.as_lit().filter(|_| val.pts.is_empty())?;
        out.insert(ks, intern::key_py_str(lv));
    }
    Some(out)
}

/// Make the parser's rows and section dicts match its spec.
pub fn rebuild(an: &mut Analyzer, st: &mut State, node: &Node, oid: u32) {
    let Some(spec) = an.m.cfg_parsers.get(&oid).cloned() else { return };
    if !spec.known {
        let src = Source::new(&format!("configparser.ConfigParser@L{}", spec.line), SourceKind::External);
        st.set_row(oid, Ns::Item, Row::with_source(src));
        st.drop_obj_ns(oid, Ns::Item);
        return;
    }
    let mut row = Row::new();
    let names: Vec<String> = std::iter::once(spec.default_section.clone()).chain(spec.sections.keys().cloned()).collect();
    for name in names {
        let sid = an.alloc_at(Kind::Dict, node, st, &format!("section {}", name), Payload::None, Some(Row::new()), Ns::Item, None, Some(&format!("cfgsec{}:{}", oid, name)));
        let mut srow = Row::new();
        for (k, v) in spec.options(&name).unwrap_or_default() {
            let kk = key_str_lit(&k);
            srow = srow.store(kk, true);
            let val = match v {
                Some(s) => Value::lit_str(&s),
                None => Value::data("interpolation"),
            };
            st.set_slot(sid, Ns::Item, kk, val, true);
        }
        st.set_row(sid, Ns::Item, srow);
        an.m.ci_dicts.insert(sid);
        an.m.cfg_sections.insert(sid, (spec.clone(), name.clone()));
        let k = key_str_lit(&name);
        row = row.store(k, true);
        st.set_slot(oid, Ns::Item, k, Value::obj(sid), true);
    }
    st.set_row(oid, Ns::Item, row);
}

/// A configparser exception may be raised here (`except configparser.Error` catches it too).
fn cfg_raise(an: &mut Analyzer, st: &mut State, name: &str, sure: bool) {
    // (its class: configparser's hierarchy, `except configparser.Error` catches it too)
    let q = if name == "ValueError" { name.to_string() } else { format!("configparser.{}", name) };
    an.raise_exc(&q, None, Some(crate::interp::builtin_exc_names(&q)), sure, st);
}

fn convert(kind: &str, s: &str) -> Option<intern::KeyId> {
    match kind {
        "getint" => s.trim().parse::<i64>().ok().map(intern::key_int),
        "getfloat" => s.trim().parse::<f64>().ok().filter(|f| f.is_finite()).map(intern::key_float),
        "getboolean" => match s.to_lowercase().as_str() {
            "1" | "yes" | "true" | "on" => Some(intern::key_bool(true)),
            "0" | "no" | "false" | "off" => Some(intern::key_bool(false)),
            _ => None,
        },
        _ => Some(key_str_lit(s)),
    }
}

fn unknown_of(kind: &str) -> Value {
    Value::data(match kind {
        "getint" => "int",
        "getfloat" => "float",
        "getboolean" => "bool",
        _ => "str",
    })
}

/// `get` / `getint` / … of option `opt` in section `sec`, with a fallback (None: none given).
fn get_value(an: &mut Analyzer, st: &mut State, spec: &CfgSpec, kind: &str, sec: Option<String>, opt: Option<String>, fallback: Option<Value>) -> Value {
    let (Some(sec), Some(opt), true) = (sec, opt, spec.known) else {
        if fallback.is_none() {
            cfg_raise(an, st, "NoOptionError", false);
            cfg_raise(an, st, "NoSectionError", false);
        }
        // (a value that does not convert, an interpolation that fails)
        if kind != "get" {
            cfg_raise(an, st, "ValueError", false);
        }
        cfg_raise(an, st, "InterpolationError", false);
        return unknown_of(kind).join(&fallback.unwrap_or_default());
    };
    let Some(opts) = spec.options(&sec) else {
        // no such section
        return match fallback {
            Some(f) => f,
            None => {
                cfg_raise(an, st, "NoSectionError", true);
                Value::new()
            }
        };
    };
    match opts.get(&opt.to_lowercase()) {
        Some(Some(s)) => match convert(kind, s) {
            Some(k) => Value::lit(k),
            None => {
                // `int("x")`: a ValueError
                an.raise_here("ValueError", st);
                Value::new()
            }
        },
        Some(None) => unknown_of(kind),
        None => match fallback {
            Some(f) => f,
            None => {
                cfg_raise(an, st, "NoOptionError", true);
                Value::new()
            }
        },
    }
}

fn str_list(an: &mut Analyzer, st: &mut State, node: &Node, xs: &[String]) -> Value {
    let vals: Vec<Value> = xs.iter().map(|x| Value::lit_str(x)).collect();
    let kv = KeyVal::lits(KeySet::from_iter(vals.iter().filter_map(|v| v.kv.as_lit())));
    an.alloc_positional(Kind::List, node, Some(vals), Value::new(), if xs.is_empty() { KeyVal::Lits(KeySet::new()) } else { kv }, st)
}

/// The parser's methods (`cfgparser.NAME`).
pub fn m_parser(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = c.qual.rsplit('.').next().unwrap_or("").to_string();
    let Some(oid) = parser_of(an, c) else { return Value::data("configparser") };
    let mut spec = an.m.cfg_parsers.get(&oid).cloned().unwrap_or_default();
    let arg = |i: usize, kw: &str| c.arg(i, Some(kw));
    match name.as_str() {
        "read_string" | "read_file" | "read_dict" | "read" => {
            let text = match name.as_str() {
                "read_string" => arg(0, "string").as_ref().and_then(lit_str),
                "read_file" => arg(0, "f").map(|f| crate::models::csvio::unwrap_text(an, st, &f)).as_ref().and_then(lit_str),
                _ => None,
            };
            let ok = match (name.as_str(), text) {
                ("read_dict", _) => match arg(0, "dictionary").and_then(|d| nested_mapping(an, st, &d)) {
                    Some(m) => {
                        for (s, opts) in m {
                            let tbl = if s == spec.default_section {
                                &mut spec.defaults
                            } else {
                                spec.sections.entry(s).or_default()
                            };
                            for (k, v) in opts {
                                tbl.insert(k.to_lowercase(), v);
                            }
                        }
                        true
                    }
                    None => false,
                },
                (_, Some(t)) => parse_ini(&t, &mut spec).is_some(),
                _ => false,
            };
            if !ok {
                // text from a parameter: decided per call site
                spec.known = false;
                // (a text that does not parse: MissingSectionHeaderError / ParsingError /
                // DuplicateSectionError / DuplicateOptionError)
                if name != "read_dict" {
                    cfg_raise(an, st, "ParsingError", false);
                    cfg_raise(an, st, "DuplicateSectionError", false);
                }
            }
            an.m.cfg_parsers.insert(oid, spec);
            rebuild(an, st, &c.node, oid);
            if name == "read" {
                return Value::data("files");
            }
            an.none_v.clone()
        }
        "sections" => {
            if !spec.known {
                return Value::data("sections");
            }
            let xs: Vec<String> = spec.sections.keys().cloned().collect();
            str_list(an, st, &c.node, &xs)
        }
        "options" => {
            let sec = arg(0, "section").as_ref().and_then(lit_str);
            match sec.filter(|_| spec.known).and_then(|s| spec.options(&s)) {
                Some(o) => {
                    let xs: Vec<String> = o.keys().cloned().collect();
                    str_list(an, st, &c.node, &xs)
                }
                None => Value::data("options"),
            }
        }
        "has_section" => match arg(0, "section").as_ref().and_then(lit_str).filter(|_| spec.known) {
            Some(s) => Value::lit(intern::key_bool(spec.sections.contains_key(&s))),
            None => Value::data("bool"),
        },
        "has_option" => match (arg(0, "section").as_ref().and_then(lit_str), arg(1, "option").as_ref().and_then(lit_str), spec.known) {
            (Some(s), Some(o), true) => {
                let has = (s == spec.default_section || spec.sections.contains_key(&s)) && spec.options(&s).map(|m| m.contains_key(&o.to_lowercase())).unwrap_or(false);
                Value::lit(intern::key_bool(has))
            }
            _ => Value::data("bool"),
        },
        "get" | "getint" | "getfloat" | "getboolean" => {
            let sec = arg(0, "section").as_ref().and_then(lit_str);
            let opt = arg(1, "option").as_ref().and_then(lit_str);
            let fallback = c.kw("fallback");
            if c.kw("vars").is_some() {
                return unknown_of(&name).join(&fallback.unwrap_or_default());
            }
            // `raw=True`: no interpolation
            let mut spec = spec;
            match c.kw("raw").map(|r| r.kv.as_lit().filter(|_| r.pts.is_empty())) {
                None => {}
                Some(Some(k)) if k == intern::key_bool(false) => {}
                Some(Some(k)) if k == intern::key_bool(true) => spec.interpolate = false,
                Some(_) => spec.known = false,
            }
            get_value(an, st, &spec, &name, sec, opt, fallback)
        }
        "set" => {
            let (s, o, v) = (arg(0, "section").as_ref().and_then(lit_str), arg(1, "option").as_ref().and_then(lit_str), arg(2, "value").as_ref().and_then(lit_str));
            match (s, o, v) {
                (Some(s), Some(o), Some(v)) if spec.known => {
                    if s == spec.default_section {
                        spec.defaults.insert(o.to_lowercase(), v);
                    } else if let Some(t) = spec.sections.get_mut(&s) {
                        t.insert(o.to_lowercase(), v);
                    } else {
                        cfg_raise(an, st, "NoSectionError", true);
                        return Value::new();
                    }
                }
                _ => spec.known = false,
            }
            an.m.cfg_parsers.insert(oid, spec);
            rebuild(an, st, &c.node, oid);
            an.none_v.clone()
        }
        "add_section" => {
            match arg(0, "section").as_ref().and_then(lit_str) {
                Some(s) if spec.known => {
                    if spec.sections.contains_key(&s) || s == spec.default_section {
                        cfg_raise(an, st, if s == spec.default_section { "ValueError" } else { "DuplicateSectionError" }, true);
                        return Value::new();
                    }
                    spec.sections.insert(s, IndexMap::new());
                }
                _ => spec.known = false,
            }
            an.m.cfg_parsers.insert(oid, spec);
            rebuild(an, st, &c.node, oid);
            an.none_v.clone()
        }
        "remove_section" | "remove_option" => {
            let s = arg(0, "section").as_ref().and_then(lit_str);
            let out = match (name.as_str(), s, spec.known) {
                ("remove_section", Some(s), true) => Value::lit(intern::key_bool(spec.sections.shift_remove(&s).is_some())),
                ("remove_option", Some(s), true) => match arg(1, "option").as_ref().and_then(lit_str) {
                    Some(o) => {
                        let tbl = if s == spec.default_section { Some(&mut spec.defaults) } else { spec.sections.get_mut(&s) };
                        match tbl {
                            Some(t) => Value::lit(intern::key_bool(t.shift_remove(&o.to_lowercase()).is_some())),
                            None => {
                                cfg_raise(an, st, "NoSectionError", true);
                                return Value::new();
                            }
                        }
                    }
                    None => {
                        spec.known = false;
                        Value::data("bool")
                    }
                },
                _ => {
                    spec.known = false;
                    Value::data("bool")
                }
            };
            an.m.cfg_parsers.insert(oid, spec);
            rebuild(an, st, &c.node, oid);
            out
        }
        "defaults" => {
            let k = key_str_lit(&spec.default_section);
            if spec.known {
                st.slot(oid, Ns::Item, k)
            } else {
                Value::data("defaults")
            }
        }
        "items" if c.args.is_empty() => an.method_value("dict.items", &Value::obj(oid)),
        "items" => {
            // `items(section)`: (name, value) pairs
            let sec = arg(0, "section").as_ref().and_then(lit_str);
            let k = sec.map(|s| key_str_lit(&s));
            match k.filter(|k| spec.known && st.row(oid, Ns::Item).definite.contains(*k)) {
                Some(k) => {
                    let sv = st.slot(oid, Ns::Item, k);
                    let call = Call { qual: "dict.items".into(), recv: Some(sv), args: vec![], kwargs: IndexMap::new(), star_args: vec![], star_kwargs: vec![], node: c.node, call_ast: c.call_ast, site: c.site.clone(), line: c.line };
                    let views = an.model("dict.items");
                    let v = match views {
                        Some(m) => m(an, st, &call),
                        None => return Value::data("items"),
                    };
                    let call2 = Call { qual: "builtins.list".into(), recv: None, args: vec![v], kwargs: IndexMap::new(), star_args: vec![], star_kwargs: vec![], node: c.node, call_ast: c.call_ast, site: c.site.clone(), line: c.line };
                    match an.model("builtins.list") {
                        Some(m) => m(an, st, &call2),
                        None => Value::data("items"),
                    }
                }
                None => Value::data("items"),
            }
        }
        "write" => {
            crate::models::noeffect(an, st, c);
            an.none_v.clone()
        }
        _ => Value::data("configparser"),
    }
}

/// `{"section": {"opt": value, …}, …}` with literal keys / values.
fn nested_mapping(an: &mut Analyzer, st: &State, v: &Value) -> Option<IndexMap<String, IndexMap<String, String>>> {
    let d = v.single().filter(|o| an.heap.kind(*o) == Kind::Dict)?;
    let row = st.row(d, Ns::Item);
    if row.open() || row.present != row.definite {
        return None;
    }
    let mut out = IndexMap::new();
    for (k, sv) in an.literal_dict_items(d, st)? {
        let ks = intern::key_as_str(k)?.to_string();
        out.insert(ks, literal_mapping(an, st, &sv)?);
    }
    Some(out)
}

/// A section's methods (`cfgsection.NAME`): `get(option, fallback=None)` and the typed getters.
pub fn m_section(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = c.qual.rsplit('.').next().unwrap_or("").to_string();
    let Some(sid) = c.recv.as_ref().and_then(|r| r.single()) else { return Value::data("section") };
    let Some((spec, sec)) = an.m.cfg_sections.get(&sid).cloned() else { return Value::data("section") };
    let opt = c.arg(0, Some("option")).as_ref().and_then(lit_str);
    // (a section's `get` falls back to None)
    let fallback = c.arg(1, Some("fallback")).or_else(|| if name == "get" { Some(an.none_v.clone()) } else { None });
    let _ = DATA;
    get_value(an, st, &spec, &name, Some(sec), opt, fallback)
}

pub const PARSER_METHODS: &[&str] = &[
    "read_string", "read_file", "read_dict", "read", "sections", "options", "has_section", "has_option", "get", "getint", "getfloat", "getboolean", "set",
    "add_section", "remove_section", "remove_option", "defaults", "items", "write",
];
pub const SECTION_METHODS: &[&str] = &["get", "getint", "getfloat", "getboolean"];

pub fn register(r: &mut dyn FnMut(&[&'static str], crate::models::Model)) {
    r(&["configparser.ConfigParser", "configparser.RawConfigParser", "configparser.SafeConfigParser"], m_new);
    r(&["cfgparser.*"], m_parser);
    r(&["cfgsection.*"], m_section);
}
