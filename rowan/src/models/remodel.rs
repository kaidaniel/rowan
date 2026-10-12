//! `re` on known patterns (models::pyre): `compile` gives a Pattern object, and `match` /
//! `search` / `fullmatch` / `findall` / `finditer` / `sub` / `subn` / `split` (module
//! functions and Pattern methods) are computed when the string is known too. A Match object
//! knows its groups: `group` / `groups` / `groupdict` / `start` / `end` / `span` / `expand`,
//! `m[g]`, `lastgroup` / `lastindex` / `string` / `re` / `pos` / `endpos`. A known pattern on
//! an unknown string keeps what the pattern says (groupdict keys, group counts); an unknown
//! pattern is an unknown call.

use indexmap::IndexMap;

use crate::domain::{join_values, KeyVal, Ns, Row, Value, DATA};
use crate::frontend::Node;
use crate::interp::{Analyzer, Call, Payload};
use crate::intern::{self, key_str_lit, KeySet};
use crate::models::pyre::{self, Regex, Spans};
use crate::state::{Kind, State};

#[derive(Clone, Debug)]
pub struct ReMatch {
    pub pat: String,
    pub flags: u32,
    pub text: Vec<char>,
    pub spans: Spans,
    pub pos: usize,
    pub endpos: usize,
}

fn lit_str(v: &Value) -> Option<String> {
    v.kv.as_lit().filter(|_| v.pts.is_empty()).and_then(intern::key_as_str).map(|s| s.to_string())
}

fn lit_int(v: &Value) -> Option<i64> {
    v.kv.as_lit().filter(|_| v.pts.is_empty()).and_then(intern::key_as_int)
}

/// The pattern text and flags of a pattern argument (a literal string or a Pattern object).
fn pattern_of(an: &Analyzer, v: &Value, flags: Option<&Value>) -> Option<(String, u32)> {
    let extra = match flags {
        None => 0,
        Some(f) => lit_int(f)? as u32,
    };
    if let Some(s) = lit_str(v) {
        return Some((s, extra));
    }
    let o = v.single()?;
    let (p, f) = an.m.re_patterns.get(&o)?.clone();
    Some((p, f | extra))
}

fn new_stdobj(an: &mut Analyzer, st: &mut State, node: &Node, kind: &str, label: &str, ctx: Option<&str>) -> u32 {
    let p = Payload::Method(format!("stdobj.{}", kind), Value::new());
    let oid = an.alloc_at(Kind::Namespace, node, st, label, p.clone(), Some(Row::new()), Ns::Attr, None, ctx);
    an.obj_mut(oid).payload = p;
    st.set_row(oid, Ns::Attr, Row::new());
    oid
}

fn set_attr(st: &mut State, oid: u32, name: &str, v: Value) {
    let k = key_str_lit(name);
    let r = st.row(oid, Ns::Attr).store(k, true);
    st.set_row(oid, Ns::Attr, r);
    st.set_slot(oid, Ns::Attr, k, v, true);
}

fn group_index_dict(an: &mut Analyzer, st: &mut State, node: &Node, re: &Regex, ctx: &str) -> Value {
    let names: Vec<String> = re.names.iter().map(|(n, _)| n.clone()).collect();
    let vals: Vec<Value> = re.names.iter().map(|(_, g)| Value::lit(intern::key_int(*g as i64))).collect();
    let d = crate::models::names_dict(an, st, node, &names, Some(vals), "groupindex");
    let _ = ctx;
    d
}

/// A Pattern object for a known pattern.
fn pattern_obj(an: &mut Analyzer, st: &mut State, node: &Node, pat: &str, flags: u32, re: &Regex) -> Value {
    let oid = new_stdobj(an, st, node, "repattern", &format!("re.compile({})", intern::py_str_repr(pat)), None);
    an.m.re_patterns.insert(oid, (pat.to_string(), flags));
    set_attr(st, oid, "pattern", Value::lit_str(pat));
    set_attr(st, oid, "flags", Value::lit(intern::key_int((re.flags | 32) as i64)));
    set_attr(st, oid, "groups", Value::lit(intern::key_int(re.ngroups as i64)));
    let gi = group_index_dict(an, st, node, re, "gi");
    set_attr(st, oid, "groupindex", gi);
    Value::obj(oid)
}

fn group_value(m: &ReMatch, g: usize, default: &Value) -> Value {
    match m.spans.get(g).copied().flatten() {
        Some((a, b)) => Value::lit_str(&m.text[a..b].iter().collect::<String>()),
        None => default.clone(),
    }
}

/// A Match object with known spans.
fn match_obj(an: &mut Analyzer, st: &mut State, node: &Node, re: &Regex, m: ReMatch, ctx: &str) -> Value {
    let oid = new_stdobj(an, st, node, "rematch", &format!("Match@L{}", node.line), Some(ctx));
    // lastindex: the last group closed (the one ending last; the outermost of those ending together)
    let mut last: Option<usize> = None;
    let mut best_end: Option<usize> = None;
    for g in 1..m.spans.len() {
        if let Some((_, e)) = m.spans[g] {
            if best_end.map(|b| e > b).unwrap_or(true) {
                best_end = Some(e);
                last = Some(g);
            }
        }
    }
    let none = an.none_v.clone();
    set_attr(st, oid, "lastindex", last.map(|g| Value::lit(intern::key_int(g as i64))).unwrap_or_else(|| none.clone()));
    let lg = last.and_then(|g| re.names.iter().find(|(_, x)| *x == g).map(|(n, _)| Value::lit_str(n))).unwrap_or_else(|| none.clone());
    set_attr(st, oid, "lastgroup", lg);
    set_attr(st, oid, "string", Value::lit_str(&m.text.iter().collect::<String>()));
    set_attr(st, oid, "pos", Value::lit(intern::key_int(m.pos as i64)));
    set_attr(st, oid, "endpos", Value::lit(intern::key_int(m.endpos as i64)));
    let pv = pattern_obj(an, st, node, &m.pat, m.flags, re);
    set_attr(st, oid, "re", pv);
    an.m.re_matches.insert(oid, m);
    Value::obj(oid)
}

/// The call's (pattern, string, rest-args offset): module functions take the pattern first.
struct Target {
    re: Regex,
    pat: String,
    flags: u32,
    /// the arguments after pattern (and string)
    args: Vec<Value>,
}

/// pos / endpos of a Pattern method (`p.search(s, pos, endpos)`).
fn bounds(c: &Call, at: usize, len: usize, is_method: bool) -> Option<(usize, usize)> {
    if !is_method {
        return Some((0, len));
    }
    let clamp = |v: i64| -> usize {
        if v < 0 {
            0
        } else {
            (v as usize).min(len)
        }
    };
    let pos = match c.arg(at, Some("pos")) {
        Some(v) => clamp(lit_int(&v)?),
        None => 0,
    };
    let endpos = match c.arg(at + 1, Some("endpos")) {
        Some(v) => clamp(lit_int(&v)?),
        None => len,
    };
    Some((pos, endpos.max(pos)))
}

/// `re.NAME(pattern, …)` and `pattern.NAME(…)`.
pub fn m_re(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = c.qual.rsplit('.').next().unwrap_or("").to_string();
    let is_method = c.qual.starts_with("stdobj.repattern.");
    // the pattern and the remaining positional arguments
    let (pv, flags_v, rest): (Value, Option<Value>, Vec<Value>) = if is_method {
        (c.recv.clone().unwrap_or_default(), None, c.args.clone())
    } else {
        let Some(p) = c.arg(0, Some("pattern")) else { return fallback(an, st, c, None) };
        let flag_pos = match name.as_str() {
            "compile" | "purge" | "escape" => 1,
            "match" | "search" | "fullmatch" | "findall" | "finditer" | "split" => 2,
            "sub" | "subn" => 4,
            _ => 99,
        };
        let f = c.args.get(flag_pos).cloned().or_else(|| c.kw("flags"));
        (p, f, c.args.iter().skip(1).cloned().collect())
    };
    if name == "escape" {
        return match lit_str(&pv) {
            Some(s) => Value::lit_str(&pyre::escape(&s)),
            None => {
                Value::data("escape")
            }
        };
    }
    let Some((pat, flags)) = pattern_of(an, &pv, flags_v.as_ref()) else {
        return fallback(an, st, c, None);
    };
    let Some(re) = Regex::new(&pat, flags) else { return fallback(an, st, c, Some(pat)) };
    if name == "compile" {
        return pattern_obj(an, st, &c.node, &pat, flags, &re);
    }
    let t = Target { re, pat, flags, args: rest };
    match name.as_str() {
        "match" | "search" | "fullmatch" | "findall" | "finditer" | "split" => {
            let sv = t.args.first().cloned().or_else(|| c.kw("string"));
            let Some(text) = sv.as_ref().and_then(lit_str) else {
                return unknown_string(an, st, c, &name, &t);
            };
            let s: Vec<char> = text.chars().collect();
            let Some((pos, endpos)) = bounds(c, 1, s.len(), is_method && name != "split") else { return unknown_string(an, st, c, &name, &t) };
            match name.as_str() {
                "match" | "fullmatch" | "search" => {
                    let r = match name.as_str() {
                        "search" => t.re.search(&s, pos, endpos, None),
                        _ => t.re.match_at(&s, pos, endpos, name == "fullmatch", None),
                    };
                    match r {
                        Ok(Some(spans)) => {
                            let m = ReMatch { pat: t.pat.clone(), flags: t.flags, text: s, spans, pos, endpos };
                            match_obj(an, st, &c.node, &t.re, m, "m")
                        }
                        Ok(None) => an.none_v.clone(),
                        Err(()) => unknown_string(an, st, c, &name, &t),
                    }
                }
                "findall" | "finditer" => {
                    let Ok(ms) = t.re.find_all(&s, pos, endpos, 0) else { return unknown_string(an, st, c, &name, &t) };
                    let mut items: Vec<Value> = Vec::new();
                    for (i, spans) in ms.into_iter().enumerate() {
                        let m = ReMatch { pat: t.pat.clone(), flags: t.flags, text: s.clone(), spans, pos, endpos };
                        if name == "finditer" {
                            items.push(match_obj(an, st, &c.node, &t.re, m, &format!("it{}", i)));
                        } else {
                            items.push(findall_item(an, st, &c.node, &t.re, &m, i));
                        }
                    }
                    list_of(an, st, &c.node, items)
                }
                _ => {
                    let maxsplit = match t.args.get(1).cloned().or_else(|| c.kw("maxsplit")) {
                        Some(v) => match lit_int(&v) {
                            Some(n) => n.max(0) as usize,
                            None => return unknown_string(an, st, c, &name, &t),
                        },
                        None => 0,
                    };
                    let Ok(ms) = t.re.find_all(&s, 0, s.len(), maxsplit) else { return unknown_string(an, st, c, &name, &t) };
                    let mut items: Vec<Value> = Vec::new();
                    let mut at = 0;
                    let none = an.none_v.clone();
                    for spans in ms {
                        let (a, b) = spans[0].unwrap();
                        items.push(Value::lit_str(&s[at..a].iter().collect::<String>()));
                        for g in 1..spans.len() {
                            items.push(match spans[g] {
                                Some((x, y)) => Value::lit_str(&s[x..y].iter().collect::<String>()),
                                None => none.clone(),
                            });
                        }
                        at = b;
                    }
                    items.push(Value::lit_str(&s[at..].iter().collect::<String>()));
                    list_of(an, st, &c.node, items)
                }
            }
        }
        "sub" | "subn" => {
            let repl = t.args.first().cloned().or_else(|| c.kw("repl"));
            let sv = t.args.get(1).cloned().or_else(|| c.kw("string"));
            let count = match t.args.get(2).cloned().or_else(|| c.kw("count")) {
                Some(v) => match lit_int(&v) {
                    Some(n) => n.max(0) as usize,
                    None => return unknown_sub(an, st, c, &name),
                },
                None => 0,
            };
            let (Some(repl), Some(text)) = (repl, sv.as_ref().and_then(lit_str)) else {
                return unknown_sub(an, st, c, &name);
            };
            let s: Vec<char> = text.chars().collect();
            let Ok(ms) = t.re.find_all(&s, 0, s.len(), count) else { return unknown_sub(an, st, c, &name) };
            let n = ms.len();
            let repl_text = lit_str(&repl);
            let mut out = Some(String::new());
            let mut at = 0;
            for (i, spans) in ms.into_iter().enumerate() {
                let (a, b) = spans[0].unwrap();
                if let Some(o) = out.as_mut() {
                    o.extend(&s[at..a]);
                }
                at = b;
                let piece = match &repl_text {
                    Some(tmpl) => match pyre::expand(&t.re, tmpl, &s, &spans) {
                        Some(x) => Some(x),
                        None => {
                            an.raise_here("re.error", st);
                            return Value::new();
                        }
                    },
                    None => {
                        // a function: called with each match
                        let m = ReMatch { pat: t.pat.clone(), flags: t.flags, text: s.clone(), spans, pos: 0, endpos: s.len() };
                        let mv = match_obj(an, st, &c.node, &t.re, m, &format!("sub{}", i));
                        let r = an.call_value(&repl, &c.node, None, st, Some(vec![mv]));
                        lit_str(&r)
                    }
                };
                match (out.as_mut(), piece) {
                    (Some(o), Some(p)) => o.push_str(&p),
                    _ => out = None,
                }
            }
            if !st.reachable {
                return Value::new();
            }
            let sv = match out {
                Some(mut o) => {
                    o.extend(&s[at..]);
                    Value::lit_str(&o)
                }
                None => Value::data("sub"),
            };
            if name == "subn" {
                let vals = vec![sv, Value::lit(intern::key_int(n as i64))];
                return an.alloc_positional(Kind::Tuple, &c.node, Some(vals), Value::new(), DATA, st);
            }
            sv
        }
        _ => fallback(an, st, c, Some(t.pat)),
    }
}

fn list_of(an: &mut Analyzer, st: &mut State, node: &Node, items: Vec<Value>) -> Value {
    let kv = if items.is_empty() { KeyVal::Lits(KeySet::new()) } else { join_values(items.iter().map(|v| Value::kv_only(v.kv.clone())).collect::<Vec<_>>()).kv };
    an.alloc_positional(Kind::List, node, Some(items), Value::new(), kv, st)
}

/// One `findall` item: the match (no groups), its group (one), a tuple of groups (several);
/// an unmatched group is "".
fn findall_item(an: &mut Analyzer, st: &mut State, node: &Node, re: &Regex, m: &ReMatch, i: usize) -> Value {
    let empty = Value::lit_str("");
    if re.ngroups == 0 {
        return group_value(m, 0, &empty);
    }
    if re.ngroups == 1 {
        return group_value(m, 1, &empty);
    }
    let vals: Vec<Value> = (1..=re.ngroups).map(|g| group_value(m, g, &empty)).collect();
    let tid = an.alloc_at(Kind::Tuple, node, st, &format!("tuple@L{}", node.line), Payload::None, None, Ns::Item, None, Some(&format!("fa{}", i)));
    let acc = join_values(vals.clone());
    for (j, v) in vals.iter().enumerate() {
        st.set_slot(tid, Ns::Pos, intern::key_int(j as i64), v.clone(), true);
    }
    st.set_slot(tid, Ns::Elem, intern::STAR, Value::new(), true);
    st.elems.insert(tid, acc.kv);
    an.set_llen(tid, st, Some(vals.len()));
    Value::obj(tid)
}

/// A known pattern on an unknown string: what the pattern says.
fn unknown_string(an: &mut Analyzer, st: &mut State, c: &Call, name: &str, t: &Target) -> Value {
    match name {
        "match" | "search" | "fullmatch" => {
            let call = Call { qual: format!("re.{}", name), recv: None, args: vec![Value::lit_str(&t.pat)], kwargs: IndexMap::new(), star_args: vec![], star_kwargs: vec![], node: c.node, call_ast: c.call_ast, site: c.site.clone(), line: c.line };
            crate::models::re_match_generated(an, st, &call)
        }
        "findall" | "split" => {
            let e = if name == "findall" && t.re.ngroups > 1 { Value::data("tuple") } else { Value::data("str") };
            let e = if name == "split" && t.re.ngroups > 0 { e.join(&an.none_v.clone()) } else { e };
            an.alloc_positional(Kind::List, &c.node, None, e, DATA, st)
        }
        "finditer" => {
            let call = Call { qual: "re.match".into(), recv: None, args: vec![Value::lit_str(&t.pat)], kwargs: IndexMap::new(), star_args: vec![], star_kwargs: vec![], node: c.node, call_ast: c.call_ast, site: c.site.clone(), line: c.line };
            // (the iterator yields matches only, never None)
            let m = crate::models::re_match_generated(an, st, &call);
            let mv = Value::with_kv(m.pts, KeyVal::Bot);
            an.alloc_positional(Kind::List, &c.node, None, mv, DATA, st)
        }
        _ => Value::data(name),
    }
}

fn unknown_sub(an: &mut Analyzer, st: &mut State, c: &Call, name: &str) -> Value {
    if name == "subn" {
        let vals = vec![Value::data("str"), Value::data("int")];
        return an.alloc_positional(Kind::Tuple, &c.node, Some(vals), Value::new(), DATA, st);
    }
    Value::data("sub")
}

/// What the old model did: an unknown call (a pattern that does not parse here).
fn fallback(an: &mut Analyzer, st: &mut State, c: &Call, pat: Option<String>) -> Value {
    let _ = pat;
    if matches!(c.qual.as_str(), "re.match" | "re.search" | "re.fullmatch") {
        return crate::models::re_match_generated(an, st, c);
    }
    an.unknown_call(&c.qual, c.recv.as_ref(), &c.args, &c.kwargs, &c.star_args, &c.star_kwargs, st, &c.node)
}

/// A Match object's methods.
pub fn m_match(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = c.qual.rsplit('.').next().unwrap_or("").to_string();
    let Some(oid) = c.recv.as_ref().and_then(|r| r.single()) else { return Value::data("match") };
    let Some(m) = an.m.re_matches.get(&oid).cloned() else { return Value::data("match") };
    let Some(re) = Regex::new(&m.pat, m.flags) else { return Value::data("match") };
    let none = an.none_v.clone();
    let gidx = |v: &Value| -> Option<usize> {
        if let Some(i) = lit_int(v) {
            return (i >= 0 && (i as usize) <= re.ngroups).then_some(i as usize);
        }
        lit_str(v).and_then(|n| re.group_index(&n))
    };
    match name.as_str() {
        "group" => {
            let args: Vec<Value> = if c.args.is_empty() { vec![Value::lit(intern::key_int(0))] } else { c.args.clone() };
            let mut vals = Vec::new();
            for a in &args {
                match gidx(a) {
                    Some(g) => vals.push(group_value(&m, g, &none)),
                    None => {
                        an.raise_here("IndexError", st);
                        return Value::new();
                    }
                }
            }
            if vals.len() == 1 {
                return vals.pop().unwrap();
            }
            an.alloc_positional(Kind::Tuple, &c.node, Some(vals), Value::new(), DATA, st)
        }
        "groups" => {
            let d = c.arg(0, Some("default")).unwrap_or_else(|| none.clone());
            let vals: Vec<Value> = (1..=re.ngroups).map(|g| group_value(&m, g, &d)).collect();
            an.alloc_positional(Kind::Tuple, &c.node, Some(vals), Value::new(), DATA, st)
        }
        "groupdict" => {
            let d = c.arg(0, Some("default")).unwrap_or_else(|| none.clone());
            let names: Vec<String> = re.names.iter().map(|(n, _)| n.clone()).collect();
            let vals: Vec<Value> = re.names.iter().map(|(_, g)| group_value(&m, *g, &d)).collect();
            crate::models::names_dict(an, st, &c.node, &names, Some(vals), "groupdict")
        }
        "start" | "end" | "span" => {
            let g = match c.arg(0, Some("group")) {
                Some(v) => match gidx(&v) {
                    Some(g) => g,
                    None => {
                        an.raise_here("IndexError", st);
                        return Value::new();
                    }
                },
                None => 0,
            };
            let (a, b) = match m.spans[g] {
                Some((a, b)) => (a as i64, b as i64),
                None => (-1, -1),
            };
            match name.as_str() {
                "start" => Value::lit(intern::key_int(a)),
                "end" => Value::lit(intern::key_int(b)),
                _ => {
                    let vals = vec![Value::lit(intern::key_int(a)), Value::lit(intern::key_int(b))];
                    an.alloc_positional(Kind::Tuple, &c.node, Some(vals), Value::new(), DATA, st)
                }
            }
        }
        "expand" => match c.arg(0, Some("template")).as_ref().and_then(lit_str).and_then(|t| pyre::expand(&re, &t, &m.text, &m.spans)) {
            Some(x) => Value::lit_str(&x),
            None => Value::data("expand"),
        },
        "__getitem__" => match c.args.first().and_then(gidx) {
            Some(g) => group_value(&m, g, &none),
            None => Value::data("group"),
        },
        _ => Value::data("match"),
    }
}

/// `m[g]`: the group.
pub fn match_item(an: &mut Analyzer, oid: u32, key: &Value) -> Option<Value> {
    let m = an.m.re_matches.get(&oid)?.clone();
    let re = Regex::new(&m.pat, m.flags)?;
    let g = if let Some(i) = lit_int(key) {
        (i >= 0 && (i as usize) <= re.ngroups).then_some(i as usize)?
    } else {
        re.group_index(&lit_str(key)?)?
    };
    Some(group_value(&m, g, &an.none_v.clone()))
}

pub fn register(r: &mut dyn FnMut(&[&'static str], crate::models::Model)) {
    r(
        &["re.compile", "re.match", "re.search", "re.fullmatch", "re.findall", "re.finditer", "re.sub", "re.subn", "re.split", "re.escape"],
        m_re,
    );
}
