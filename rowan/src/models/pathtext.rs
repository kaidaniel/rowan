//! Path and text utilities on known strings, as CPython computes them on POSIX: `os.path`
//! (posixpath), `pathlib` pure paths (`PurePosixPath`, `PurePath`, and the pure part of
//! `Path`: a path object holds its normalised text; file-system calls stay unknown), and
//! `textwrap` (`wrap` / `fill` / `shorten` / `dedent` / `indent`).

use crate::domain::{join_values, KeyVal, Ns, Row, Value, DATA};
use crate::frontend::Node;
use crate::interp::{Analyzer, Call, Payload};
use crate::intern::{self, key_str_lit, KeySet};
use crate::state::{Kind, State};

fn lit_str(v: &Value) -> Option<String> {
    v.kv.as_lit().filter(|_| v.pts.is_empty()).and_then(intern::key_as_str).map(|s| s.to_string())
}

// ------------------------------------------------------------------- posixpath

pub fn pjoin(parts: &[String]) -> String {
    let mut out = String::new();
    for (i, p) in parts.iter().enumerate() {
        if i == 0 {
            out = p.clone();
        } else if p.starts_with('/') {
            out = p.clone();
        } else if out.is_empty() || out.ends_with('/') {
            out.push_str(p);
        } else {
            out.push('/');
            out.push_str(p);
        }
    }
    out
}

pub fn psplit(p: &str) -> (String, String) {
    let i = p.rfind('/').map(|i| i + 1).unwrap_or(0);
    let (head, tail) = (&p[..i], &p[i..]);
    let mut head = head.to_string();
    if !head.is_empty() && head != "/".repeat(head.len()) {
        head = head.trim_end_matches('/').to_string();
    }
    (head, tail.to_string())
}

pub fn basename(p: &str) -> String {
    psplit(p).1
}

pub fn dirname(p: &str) -> String {
    psplit(p).0
}

pub fn splitext(p: &str) -> (String, String) {
    // genericpath._splitext: the last dot after the last slash, past the name's leading dots
    let sep = p.rfind('/').map(|i| i as i64).unwrap_or(-1);
    if let Some(dot) = p.rfind('.') {
        let dot = dot as i64;
        if dot > sep {
            let b = p.as_bytes();
            let mut i = sep + 1;
            while i < dot {
                if b[i as usize] != b'.' {
                    return (p[..dot as usize].to_string(), p[dot as usize..].to_string());
                }
                i += 1;
            }
        }
    }
    (p.to_string(), String::new())
}

pub fn normpath(p: &str) -> String {
    if p.is_empty() {
        return ".".into();
    }
    let initial = if p.starts_with('/') {
        if p.starts_with("//") && !p.starts_with("///") {
            2
        } else {
            1
        }
    } else {
        0
    };
    let mut comps: Vec<&str> = Vec::new();
    for c in p.split('/') {
        if c.is_empty() || c == "." {
            continue;
        }
        if c != ".." || (initial == 0 && comps.is_empty()) || comps.last() == Some(&"..") {
            comps.push(c);
        } else if !comps.is_empty() {
            comps.pop();
        }
    }
    let s = format!("{}{}", "/".repeat(initial), comps.join("/"));
    if s.is_empty() {
        ".".into()
    } else {
        s
    }
}

/// `relpath(path, start)` of two absolute paths (a relative one depends on the cwd).
pub fn relpath(path: &str, start: &str) -> Option<String> {
    if path.is_empty() || !path.starts_with('/') || !start.starts_with('/') {
        return None;
    }
    let a: Vec<String> = normpath(start).split('/').filter(|x| !x.is_empty()).map(|s| s.to_string()).collect();
    let b: Vec<String> = normpath(path).split('/').filter(|x| !x.is_empty()).map(|s| s.to_string()).collect();
    let n = a.iter().zip(b.iter()).take_while(|(x, y)| x == y).count();
    let mut rel: Vec<String> = vec!["..".into(); a.len() - n];
    rel.extend(b[n..].iter().cloned());
    Some(if rel.is_empty() { ".".into() } else { rel.join("/") })
}

fn commonprefix(xs: &[String]) -> String {
    let Some(first) = xs.iter().min() else { return String::new() };
    let last = xs.iter().max().unwrap();
    let n = first.chars().zip(last.chars()).take_while(|(a, b)| a == b).count();
    first.chars().take(n).collect()
}

fn str_tuple(an: &mut Analyzer, st: &mut State, node: &Node, xs: &[String]) -> Value {
    let vals: Vec<Value> = xs.iter().map(|x| Value::lit_str(x)).collect();
    let kv = KeyVal::lits(KeySet::from_iter(vals.iter().filter_map(|v| v.kv.as_lit())));
    an.alloc_positional(Kind::Tuple, node, Some(vals), Value::new(), kv, st)
}

fn str_list(an: &mut Analyzer, st: &mut State, node: &Node, xs: &[String]) -> Value {
    let vals: Vec<Value> = xs.iter().map(|x| Value::lit_str(x)).collect();
    let kv = if xs.is_empty() { KeyVal::Lits(KeySet::new()) } else { KeyVal::lits(KeySet::from_iter(vals.iter().filter_map(|v| v.kv.as_lit()))) };
    an.alloc_positional(Kind::List, node, Some(vals), Value::new(), kv, st)
}

/// A path argument's text: a string or a path object.
fn text_of(an: &Analyzer, v: &Value) -> Option<String> {
    if let Some(s) = lit_str(v) {
        return Some(s);
    }
    let o = v.single()?;
    path_text(an, o)
}

/// `os.path.NAME(…)`.
pub fn m_ospath(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = c.qual.rsplit('.').next().unwrap_or("").to_string();
    let args: Option<Vec<String>> = c.args.iter().map(|a| text_of(an, a)).collect();
    let Some(args) = args.filter(|_| c.kwargs.is_empty() && c.star_args.is_empty()) else {
        return unknown_ospath(an, st, c, &name);
    };
    let one = args.first().cloned();
    let s = |x: String| Value::lit_str(&x);
    let b = |x: bool| Value::lit(intern::key_bool(x));
    match (name.as_str(), one) {
        ("join", Some(_)) => s(pjoin(&args)),
        ("basename", Some(p)) => s(basename(&p)),
        ("dirname", Some(p)) => s(dirname(&p)),
        ("split", Some(p)) => {
            let (h, t) = psplit(&p);
            str_tuple(an, st, &c.node, &[h, t])
        }
        ("splitext", Some(p)) => {
            let (r, e) = splitext(&p);
            str_tuple(an, st, &c.node, &[r, e])
        }
        ("splitdrive", Some(p)) => str_tuple(an, st, &c.node, &[String::new(), p]),
        ("normpath", Some(p)) => s(normpath(&p)),
        ("isabs", Some(p)) => b(p.starts_with('/')),
        ("abspath", Some(p)) if p.starts_with('/') => s(normpath(&p)),
        ("relpath", Some(p)) => {
            let start = args.get(1).cloned();
            match start.and_then(|st0| relpath(&p, &st0)) {
                Some(r) => s(r),
                None => Value::data("path"),
            }
        }
        ("commonprefix", _) => {
            // one list argument
            let Some(l) = c.args.first() else { return Value::data("path") };
            let it = an.iterate(l, &c.node, st);
            match it.items.and_then(|xs| xs.iter().map(lit_str).collect::<Option<Vec<String>>>()) {
                Some(xs) => s(commonprefix(&xs)),
                None => Value::data("path"),
            }
        }
        ("expanduser", Some(p)) if !p.starts_with('~') => s(p),
        ("expandvars", Some(p)) if !p.contains('$') => s(p),
        _ => unknown_ospath(an, st, c, &name),
    }
}

fn unknown_ospath(an: &mut Analyzer, st: &mut State, c: &Call, name: &str) -> Value {
    match name {
        "split" | "splitext" | "splitdrive" => {
            let vals = vec![Value::data("path"), Value::data("path")];
            an.alloc_positional(Kind::Tuple, &c.node, Some(vals), Value::new(), DATA, st)
        }
        "isabs" | "exists" | "isfile" | "isdir" | "islink" | "lexists" | "ismount" | "samefile" => Value::data("bool"),
        _ => {
            let _ = (an, st);
            Value::data("path")
        }
    }
}

// --------------------------------------------------------------------- pathlib

const PATH_KIND: &str = "stdobj.path";

/// A path object's normalised text.
pub fn path_text(an: &Analyzer, o: u32) -> Option<String> {
    if an.heap.kind(o) != Kind::Namespace {
        return None;
    }
    match &an.heap.obj(o).payload {
        Payload::Method(q, t) if q == PATH_KIND => lit_str(t),
        _ => None,
    }
}

pub fn is_path(an: &Analyzer, o: u32) -> bool {
    matches!(&an.heap.obj(o).payload, Payload::Method(q, _) if q == PATH_KIND) && an.heap.kind(o) == Kind::Namespace
}

/// The class a path object was made with (`PurePosixPath`, `PosixPath`, …).
fn path_class(an: &Analyzer, o: u32) -> String {
    an.m.path_classes.get(&o).cloned().unwrap_or_else(|| "PurePosixPath".into())
}

/// pathlib's normalisation: `//` and `.` parts collapse, a trailing `/` goes, `..` stays.
pub fn pathlib_norm(parts: &[String]) -> String {
    let mut root = String::new();
    let mut comps: Vec<String> = Vec::new();
    for p in parts {
        if p.starts_with('/') {
            root = if p.starts_with("//") && !p.starts_with("///") { "//".into() } else { "/".into() };
            comps.clear();
        }
        for c in p.split('/') {
            if c.is_empty() || c == "." {
                continue;
            }
            comps.push(c.to_string());
        }
    }
    let s = format!("{}{}", root, comps.join("/"));
    if s.is_empty() {
        ".".into()
    } else {
        s
    }
}

fn new_path(an: &mut Analyzer, st: &mut State, node: &Node, text: &str, cls: &str, ctx: &str) -> Value {
    let p = Payload::Method(PATH_KIND.into(), Value::lit_str(text));
    let oid = an.alloc_at(Kind::Namespace, node, st, &format!("{}('{}')", cls, text), p.clone(), Some(Row::new()), Ns::Attr, None, Some(&format!("path:{}:{}", ctx, text)));
    an.heap.obj_mut(oid).payload = p;
    st.set_row(oid, Ns::Attr, Row::new());
    an.m.path_classes.insert(oid, cls.to_string());
    Value::obj(oid)
}

fn name_of(t: &str) -> String {
    if t == "." || t == "/" || t == "//" {
        return String::new();
    }
    t.rsplit('/').next().unwrap_or("").to_string()
}

// (3.14: the name's leading dots never start a suffix)
fn suffix_of(name: &str) -> String {
    let n = name.trim_start_matches('.');
    match n.rfind('.') {
        Some(i) => n[i..].to_string(),
        None => String::new(),
    }
}

fn suffixes_of(name: &str) -> Vec<String> {
    name.trim_start_matches('.').split('.').skip(1).map(|x| format!(".{}", x)).collect()
}

fn stem_of(name: &str) -> String {
    let s = suffix_of(name);
    name[..name.len() - s.len()].to_string()
}

fn parent_of(t: &str) -> String {
    let anchor = if t.starts_with("//") && !t.starts_with("///") {
        "//"
    } else if t.starts_with('/') {
        "/"
    } else {
        ""
    };
    let rest = &t[anchor.len()..];
    if rest.is_empty() || t == "." {
        return if anchor.is_empty() { ".".into() } else { anchor.into() };
    }
    match rest.rfind('/') {
        Some(i) => format!("{}{}", anchor, &rest[..i]),
        None => {
            if anchor.is_empty() {
                ".".into()
            } else {
                anchor.into()
            }
        }
    }
}

fn parts_of(t: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = t;
    if t.starts_with("//") && !t.starts_with("///") {
        out.push("//".to_string());
        rest = &t[2..];
    } else if t.starts_with('/') {
        out.push("/".to_string());
        rest = &t[1..];
    }
    if t == "." {
        return Vec::new();
    }
    out.extend(rest.split('/').filter(|x| !x.is_empty()).map(|s| s.to_string()));
    out
}

/// `PurePosixPath(*parts)` / `Path(*parts)` / …
pub fn m_path_new(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let cls = c.qual.rsplit('.').next().unwrap_or("PurePosixPath").to_string();
    let cls = match cls.as_str() {
        "Path" => "PosixPath".to_string(),
        "PurePath" => "PurePosixPath".to_string(),
        other => other.to_string(),
    };
    if cls.contains("Windows") {
        return an.unknown_call(&c.qual, None, c.arguments(), st, &c.node);
    }
    let parts: Option<Vec<String>> = c.args.iter().map(|a| text_of(an, a)).collect();
    match parts.filter(|_| c.star_args.is_empty()) {
        Some(ps) => {
            let t = pathlib_norm(&ps);
            new_path(an, st, &c.node, &t, &cls, "new")
        }
        None => {
            Value::data("path")
        }
    }
}

pub const PATH_PROPS: &[&str] = &["name", "stem", "suffix", "suffixes", "parent", "parents", "parts", "anchor", "root", "drive"];

/// A path property (`p.name`, `p.parent`, …).
pub fn path_attr(an: &mut Analyzer, st: &mut State, o: u32, name: &str, node: &Node) -> Option<Value> {
    let t = path_text(an, o)?;
    let cls = path_class(an, o);
    let nm = name_of(&t);
    Some(match name {
        "name" => Value::lit_str(&nm),
        "stem" => Value::lit_str(&stem_of(&nm)),
        "suffix" => Value::lit_str(&suffix_of(&nm)),
        "suffixes" => str_list(an, st, node, &suffixes_of(&nm)),
        "parent" => new_path(an, st, node, &parent_of(&t), &cls, "parent"),
        "parents" => {
            let mut ps = Vec::new();
            let mut cur = t.clone();
            loop {
                let p = parent_of(&cur);
                if p == cur {
                    break;
                }
                ps.push(new_path(an, st, node, &p, &cls, &format!("parents{}", ps.len())));
                cur = p;
            }
            an.alloc_positional(Kind::Tuple, node, Some(ps), Value::new(), DATA, st)
        }
        "parts" => str_tuple(an, st, node, &parts_of(&t)),
        "anchor" | "root" => Value::lit_str(if t.starts_with("//") && !t.starts_with("///") { "//" } else if t.starts_with('/') { "/" } else { "" }),
        "drive" => Value::lit_str(""),
        _ => return None,
    })
}

/// `p / q` and `q / p` with a path object.
pub fn path_div(an: &mut Analyzer, st: &mut State, a: &Value, b: &Value, node: &Node) -> Option<Value> {
    let ap = a.single().filter(|o| is_path(an, *o));
    let bp = b.single().filter(|o| is_path(an, *o));
    if ap.is_none() && bp.is_none() {
        return None;
    }
    let cls = path_class(an, ap.or(bp).unwrap());
    match (text_of(an, a), text_of(an, b)) {
        (Some(x), Some(y)) => {
            let t = pathlib_norm(&[x, y]);
            Some(new_path(an, st, node, &t, &cls, "div"))
        }
        _ => Some(Value::data("path")),
    }
}

/// A path object's methods (`stdobj.path.NAME`).
pub fn m_path_method(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let r = path_method(an, st, c);
    // (a file's text is a str, its bytes bytes, whatever the path)
    let ty = match c.qual.rsplit('.').next() {
        Some("read_text") => "str",
        Some("read_bytes") => "bytes",
        _ => return r,
    };
    for o in r.pts.iter() {
        if an.heap.kind(o) == Kind::Unknown && an.heap.obj(o).site.starts_with("ret:") {
            an.heap.obj_mut(o).ty = Some(ty);
        }
    }
    match r.kv {
        KeyVal::Data(_) => Value::with_kv(r.pts.clone(), KeyVal::data(&crate::domain::typed_label(ty, ""))),
        _ => r,
    }
}

/// A path method's result (`m_path_method`).
fn path_method(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = c.qual.rsplit('.').next().unwrap_or("").to_string();
    let Some(o) = c.recv.as_ref().and_then(|r| r.single()) else { return Value::data("path") };
    let Some(t) = path_text(an, o) else { return Value::data("path") };
    let cls = path_class(an, o);
    let args: Option<Vec<String>> = c.args.iter().map(|a| text_of(an, a)).collect();
    let Some(args) = args else {
        return Value::data("path");
    };
    let nm = name_of(&t);
    let path = |an: &mut Analyzer, st: &mut State, x: &str| new_path(an, st, &c.node, x, &cls, &name);
    match name.as_str() {
        "with_suffix" => {
            let s0 = args.first().cloned().unwrap_or_default();
            if nm.is_empty() || (!s0.is_empty() && (!s0.starts_with('.') || s0 == "." || s0.contains('/'))) {
                an.raise_here("ValueError", st);
                return Value::new();
            }
            let stem = stem_of(&nm);
            let par = parent_of(&t);
            let np = pathlib_norm(&[par, format!("{}{}", stem, s0)]);
            path(an, st, &np)
        }
        "with_name" | "with_stem" => {
            let Some(n0) = args.first().cloned() else { return Value::data("path") };
            if nm.is_empty() || n0.is_empty() || n0.contains('/') || n0 == "." {
                an.raise_here("ValueError", st);
                return Value::new();
            }
            let newname = if name == "with_stem" { format!("{}{}", n0, suffix_of(&nm)) } else { n0 };
            let np = pathlib_norm(&[parent_of(&t), newname]);
            path(an, st, &np)
        }
        "joinpath" => {
            let mut ps = vec![t.clone()];
            ps.extend(args);
            let np = pathlib_norm(&ps);
            path(an, st, &np)
        }
        "relative_to" | "is_relative_to" => {
            let Some(other) = args.first().map(|x| pathlib_norm(&[x.clone()])) else { return Value::data("path") };
            let (a, b) = (parts_of(&t), parts_of(&other));
            let ok = b.len() <= a.len() && a[..b.len()] == b[..];
            if name == "is_relative_to" {
                return Value::lit(intern::key_bool(ok));
            }
            if !ok {
                an.raise_here("ValueError", st);
                return Value::new();
            }
            let rest: Vec<String> = a[b.len()..].to_vec();
            let np = if rest.is_empty() { ".".to_string() } else { rest.join("/") };
            path(an, st, &np)
        }
        "is_absolute" => Value::lit(intern::key_bool(t.starts_with('/'))),
        "as_posix" | "__str__" | "__fspath__" => Value::lit_str(&t),
        "as_uri" if t.starts_with('/') => Value::lit_str(&format!("file://{}", t)),
        "match" | "full_match" => Value::data("bool"),
        // the file system
        _ => an.unknown_call(&c.qual, c.recv.as_ref(), c.arguments(), st, &c.node),
    }
}

/// `repr(p)`.
pub fn path_repr(an: &Analyzer, o: u32) -> Option<String> {
    let t = path_text(an, o)?;
    Some(format!("{}({})", path_class(an, o), intern::py_str_repr(&t)))
}

// -------------------------------------------------------------------- textwrap

/// `textwrap`'s options.
pub struct Wrap {
    pub width: usize,
    pub initial_indent: String,
    pub subsequent_indent: String,
    pub expand_tabs: bool,
    pub replace_whitespace: bool,
    pub drop_whitespace: bool,
    pub break_long_words: bool,
    pub break_on_hyphens: bool,
    pub max_lines: Option<usize>,
    pub placeholder: String,
    pub tabsize: usize,
}

impl Default for Wrap {
    fn default() -> Self {
        Wrap {
            width: 70,
            initial_indent: String::new(),
            subsequent_indent: String::new(),
            expand_tabs: true,
            replace_whitespace: true,
            drop_whitespace: true,
            break_long_words: true,
            break_on_hyphens: true,
            max_lines: None,
            placeholder: " [...]".into(),
            tabsize: 8,
        }
    }
}

fn expand_tabs(s: &str, tab: usize) -> String {
    let mut out = String::new();
    let mut col = 0;
    for ch in s.chars() {
        match ch {
            '\t' => {
                let n = if tab == 0 { 0 } else { tab - col % tab };
                out.push_str(&" ".repeat(n));
                col += n;
            }
            '\n' | '\r' => {
                out.push(ch);
                col = 0;
            }
            c => {
                out.push(c);
                col += 1;
            }
        }
    }
    out
}

const TW_WS: &[char] = &['\t', '\n', '\x0b', '\x0c', '\r', ' '];

/// `TextWrapper.wrap(text)`; None for what is not simulated (hyphen breaking).
pub fn wrap(text: &str, o: &Wrap) -> Option<Vec<String>> {
    if o.width == 0 {
        return None;
    }
    let mut t = if o.expand_tabs { expand_tabs(text, o.tabsize) } else { text.to_string() };
    if o.replace_whitespace {
        t = t.chars().map(|c| if TW_WS.contains(&c) { ' ' } else { c }).collect();
    }
    if o.break_on_hyphens && t.contains('-') {
        return None;
    }
    // chunks: runs of whitespace and of everything else
    let mut chunks: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut cur_ws: Option<bool> = None;
    for ch in t.chars() {
        let ws = TW_WS.contains(&ch);
        if cur_ws.is_some() && cur_ws != Some(ws) {
            chunks.push(std::mem::take(&mut cur));
        }
        cur_ws = Some(ws);
        cur.push(ch);
    }
    if !cur.is_empty() {
        chunks.push(cur);
    }
    let clen = |s: &str| s.chars().count();
    if let Some(m) = o.max_lines {
        let indent = if m > 1 { &o.subsequent_indent } else { &o.initial_indent };
        if clen(indent) + clen(o.placeholder.trim_start()) > o.width {
            return None;
        }
    }
    let mut lines: Vec<String> = Vec::new();
    chunks.reverse();
    while !chunks.is_empty() {
        let mut cur_line: Vec<String> = Vec::new();
        let mut cur_len = 0usize;
        let indent = if lines.is_empty() { o.initial_indent.clone() } else { o.subsequent_indent.clone() };
        let width = o.width.checked_sub(clen(&indent))?;
        if o.drop_whitespace && chunks.last().map(|c| c.trim_matches(TW_WS).is_empty()).unwrap_or(false) && !lines.is_empty() {
            chunks.pop();
        }
        while let Some(ch) = chunks.last() {
            let l = clen(ch);
            if cur_len + l <= width {
                cur_len += l;
                cur_line.push(chunks.pop().unwrap());
            } else {
                break;
            }
        }
        if let Some(ch) = chunks.last().cloned() {
            if clen(&ch) > width {
                // a long word
                let space_left = if width < 1 { 1 } else { width - cur_len.min(width) }.max(1);
                if o.break_long_words {
                    let head: String = ch.chars().take(space_left).collect();
                    let tail: String = ch.chars().skip(space_left).collect();
                    cur_len += clen(&head);
                    cur_line.push(head);
                    *chunks.last_mut().unwrap() = tail;
                } else if cur_line.is_empty() {
                    cur_len += clen(&ch);
                    cur_line.push(chunks.pop().unwrap());
                }
            }
        }
        if o.drop_whitespace && cur_line.last().map(|c| c.trim_matches(TW_WS).is_empty()).unwrap_or(false) {
            cur_len -= clen(cur_line.last().unwrap());
            cur_line.pop();
        }
        if !cur_line.is_empty() {
            let fits_rest = chunks.is_empty() || (o.drop_whitespace && chunks.len() == 1 && chunks[0].trim_matches(TW_WS).is_empty());
            if o.max_lines.map(|m| lines.len() + 1 < m).unwrap_or(true) || (fits_rest && cur_len <= width) {
                lines.push(format!("{}{}", indent, cur_line.concat()));
            } else {
                let mut placed = false;
                while let Some(last) = cur_line.last().cloned() {
                    if !last.trim_matches(TW_WS).is_empty() && cur_len + clen(&o.placeholder) <= width {
                        cur_line.push(o.placeholder.clone());
                        lines.push(format!("{}{}", indent, cur_line.concat()));
                        placed = true;
                        break;
                    }
                    cur_len -= clen(&last);
                    cur_line.pop();
                }
                if !placed {
                    if let Some(prev) = lines.last().cloned() {
                        let prev = prev.trim_end_matches(TW_WS).to_string();
                        if clen(&prev) + clen(&o.placeholder) <= o.width {
                            *lines.last_mut().unwrap() = format!("{}{}", prev, o.placeholder);
                            break;
                        }
                    }
                    lines.push(format!("{}{}", indent, o.placeholder.trim_start()));
                }
                break;
            }
        }
    }
    Some(lines)
}

/// `textwrap.dedent(text)`.
pub fn dedent(text: &str) -> String {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let body = |l: &str| l.trim_end_matches('\n').to_string();
    let blank = |l: &str| body(l).chars().all(|c| c == ' ' || c == '\t');
    let mut margin: Option<String> = None;
    for l in &lines {
        let b = body(l);
        if b.chars().all(|c| c == ' ' || c == '\t') {
            continue;
        }
        let ind: String = b.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
        margin = Some(match margin {
            None => ind,
            Some(m) => m.chars().zip(ind.chars()).take_while(|(a, b)| a == b).map(|(a, _)| a).collect(),
        });
    }
    let m = margin.unwrap_or_default();
    let mut out = String::new();
    for l in &lines {
        let nl = l.ends_with('\n');
        if blank(l) {
            if nl {
                out.push('\n');
            }
            continue;
        }
        let b = body(l);
        out.push_str(b.strip_prefix(m.as_str()).unwrap_or(&b));
        if nl {
            out.push('\n');
        }
    }
    out
}

/// `textwrap.indent(text, prefix)` (the default predicate: lines with non-whitespace).
pub fn indent(text: &str, prefix: &str) -> String {
    let mut out = String::new();
    for l in py_splitlines_keep(text) {
        if !l.trim().is_empty() {
            out.push_str(prefix);
        }
        out.push_str(&l);
    }
    out
}

fn py_splitlines_keep(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let cs: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        cur.push(c);
        let brk = matches!(c, '\n' | '\r' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}' | '\u{2028}' | '\u{2029}');
        if c == '\r' && cs.get(i + 1) == Some(&'\n') {
            cur.push('\n');
            i += 1;
        }
        if brk {
            out.push(std::mem::take(&mut cur));
        }
        i += 1;
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn wrap_opts(c: &Call, skip_width_pos: usize) -> Option<Wrap> {
    let mut o = Wrap::default();
    if let Some(w) = c.args.get(skip_width_pos).cloned().or_else(|| c.kw("width")) {
        o.width = usize::try_from(w.kv.as_lit().filter(|_| w.pts.is_empty()).and_then(intern::key_as_int)?).ok()?;
    }
    for (k, v) in c.kwargs.iter() {
        let k = intern::str_get(*k);
        let lit = v.kv.as_lit().filter(|_| v.pts.is_empty())?;
        let b = || intern::with_key(lit, |kk| if let intern::Key::Bool(x) = kk { Some(*x) } else { None });
        match &*k {
            "width" => {}
            "initial_indent" => o.initial_indent = intern::key_as_str(lit)?.to_string(),
            "subsequent_indent" => o.subsequent_indent = intern::key_as_str(lit)?.to_string(),
            "placeholder" => o.placeholder = intern::key_as_str(lit)?.to_string(),
            "expand_tabs" => o.expand_tabs = b()?,
            "replace_whitespace" => o.replace_whitespace = b()?,
            "drop_whitespace" => o.drop_whitespace = b()?,
            "break_long_words" => o.break_long_words = b()?,
            "break_on_hyphens" => o.break_on_hyphens = b()?,
            "tabsize" => o.tabsize = usize::try_from(intern::key_as_int(lit)?).ok()?,
            "max_lines" => {
                o.max_lines = if lit == intern::key_none() { None } else { Some(usize::try_from(intern::key_as_int(lit)?).ok()?) };
            }
            _ => return None,
        }
    }
    Some(o)
}

/// `textwrap.NAME(…)`.
pub fn m_textwrap(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = c.qual.rsplit('.').next().unwrap_or("").to_string();
    let text = c.args.first().and_then(lit_str);
    let Some(text) = text else {
        return unknown_tw(an, st, c, &name);
    };
    match name.as_str() {
        "dedent" => Value::lit_str(&dedent(&text)),
        "indent" => match c.arg(1, Some("prefix")).as_ref().and_then(lit_str) {
            Some(p) if c.kw("predicate").is_none() => Value::lit_str(&indent(&text, &p)),
            _ => unknown_tw(an, st, c, &name),
        },
        "wrap" | "fill" | "shorten" => {
            let Some(mut o) = wrap_opts(c, 1) else { return unknown_tw(an, st, c, &name) };
            let text = if name == "shorten" {
                o.max_lines = Some(1);
                text.split_whitespace().collect::<Vec<_>>().join(" ")
            } else {
                text
            };
            match wrap(&text, &o) {
                Some(lines) if name == "wrap" => {
                    let vals: Vec<Value> = lines.iter().map(|l| Value::lit_str(l)).collect();
                    let kv = if lines.is_empty() { KeyVal::Lits(KeySet::new()) } else { join_values(vals.clone()).kv };
                    an.alloc_positional(Kind::List, &c.node, Some(vals), Value::new(), kv, st)
                }
                Some(lines) => Value::lit_str(&lines.join("\n")),
                None => unknown_tw(an, st, c, &name),
            }
        }
        _ => unknown_tw(an, st, c, &name),
    }
}

fn unknown_tw(an: &mut Analyzer, st: &mut State, c: &Call, name: &str) -> Value {
    if name == "wrap" {
        return an.alloc_positional(Kind::List, &c.node, None, Value::data("line"), DATA, st);
    }
    let _ = key_str_lit;
    Value::data("text")
}

pub fn register(r: &mut dyn FnMut(&[&'static str], crate::models::Model)) {
    r(
        &[
            "os.path.join", "os.path.basename", "os.path.dirname", "os.path.split", "os.path.splitext", "os.path.splitdrive", "os.path.normpath", "os.path.isabs",
            "os.path.abspath", "os.path.relpath", "os.path.commonprefix", "os.path.expanduser", "os.path.expandvars", "posixpath.join", "posixpath.basename",
            "posixpath.dirname", "posixpath.split", "posixpath.splitext", "posixpath.normpath",
        ],
        m_ospath,
    );
    r(&["pathlib.PurePosixPath", "pathlib.PurePath", "pathlib.Path", "pathlib.PosixPath", "pathlib.PureWindowsPath", "pathlib.WindowsPath"], m_path_new);
    r(&["textwrap.wrap", "textwrap.fill", "textwrap.shorten", "textwrap.dedent", "textwrap.indent"], m_textwrap);
}
