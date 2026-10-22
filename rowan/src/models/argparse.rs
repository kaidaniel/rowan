//! argparse: parsers keep their arguments (dest, option strings, action, default, type, nargs,
//! choices, required), groups, subparsers and `set_defaults`; `parse_args` on a known argv
//! runs the way argparse parses it (option abbreviations, `--opt=value`, short options with
//! attached values, nargs, append / count / store_const, SystemExit on an error), giving a
//! Namespace with exactly those attributes and values. On an unknown argv every attribute a
//! parse can set, with its default and the values its type allows; with subparsers one
//! Namespace per subcommand.

use indexmap::IndexMap;

use crate::domain::{join_values, KeyVal, Ns, ObjId, Row, Value};
use crate::frontend::Node;
use crate::intern::{self, key_str_lit};
use crate::interp::{Analyzer, Call, Payload};
use crate::state::{Kind, State};

/// An argument's `action=`.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Store,
    StoreConst,
    StoreTrue,
    StoreFalse,
    Append,
    AppendConst,
    Extend,
    Count,
    Help,
    Version,
    Other,
}

/// An argument's `type=` conversion.
#[derive(Clone, Debug, PartialEq)]
pub enum Conv {
    Str,
    Int,
    Float,
    /// a callable we do not run on the text (its result is unknown)
    Other,
}

/// An argument's `nargs=`: one value, `?`, `*`, `+`, N, or `argparse.REMAINDER`.
#[derive(Clone, Debug, PartialEq)]
pub enum Nargs {
    One,
    Opt,
    Star,
    Plus,
    N(usize),
    Rest,
}

/// One `add_argument` call: its option strings or name and how it parses.
#[derive(Clone, Debug)]
pub struct ArgSpec {
    pub flags: Vec<String>,
    pub dest: String,
    pub action: Action,
    pub default: Value,
    pub has_default: bool,
    pub konst: Value,
    pub conv: Conv,
    pub nargs: Nargs,
    pub required: bool,
    pub choices: Option<Vec<Value>>,
    /// `default=argparse.SUPPRESS`: no attribute unless given
    pub suppress: bool,
}

/// A parser's `add_subparsers()`: its `dest`, whether a command is required, the commands.
#[derive(Clone, Debug, Default)]
pub struct SubSpec {
    pub dest: Option<String>,
    pub required: bool,
    pub parsers: Vec<(Vec<String>, ObjId)>,
    /// the number of positionals added before the subparsers (their order)
    pub after_positionals: usize,
}

/// What a parser knows: its arguments, `set_defaults`, subparsers.
#[derive(Clone, Debug, Default)]
pub struct ParserSpec {
    pub args: Vec<ArgSpec>,
    pub defaults: Vec<(String, Value)>,
    pub sub: Option<SubSpec>,
    pub allow_abbrev: bool,
    /// some argument we could not read (an option string or keyword not known): no exact parse
    pub opaque: bool,
    pub exit_on_error: bool,
}

/// An argparse object the model keeps (`Tables::argp`): a parser, its subparsers, a group.
#[derive(Clone, Debug)]
pub enum ArgObj {
    Parser(ParserSpec),
    /// `add_subparsers()` of a parser
    Subs(ObjId),
    /// `add_argument_group()` / `add_mutually_exclusive_group()` of a parser
    Group(ObjId),
}

/// The objects a parser's table entry holds (defaults, constants, choices, subparsers, groups'
/// parsers): GC edges of the parser.
pub fn push_objs(a: &ArgObj, work: &mut Vec<ObjId>) {
    match a {
        ArgObj::Parser(p) => {
            for s in &p.args {
                s.default.push_objs(work);
                s.konst.push_objs(work);
                s.choices.iter().flatten().for_each(|v| v.push_objs(work));
            }
            p.defaults.iter().for_each(|(_, v)| v.push_objs(work));
            work.extend(p.sub.iter().flat_map(|s| s.parsers.iter().map(|(_, c)| *c)));
        }
        ArgObj::Subs(o) | ArgObj::Group(o) => work.push(*o),
    }
}

fn lit_str(v: &Value) -> Option<String> {
    v.kv.as_lit().filter(|_| v.pts.is_empty()).and_then(intern::key_as_str).map(|s| s.to_string())
}

fn lit_bool(v: &Value) -> Option<bool> {
    let k = v.kv.as_lit().filter(|_| v.pts.is_empty())?;
    intern::with_key(k, |kk| if let intern::Key::Bool(b) = kk { Some(*b) } else { None })
}

fn new_parser_obj(an: &mut Analyzer, st: &mut State, node: &Node, label: &str, ctx: &str, spec: ArgObj) -> ObjId {
    let oid = an.alloc_at(Kind::Parser, node, st, label, Payload::None, None, Ns::Attr, None, Some(ctx));
    st.set_row(oid, Ns::Attr, Row::new());
    an.m.argp.insert(oid, spec);
    oid
}

/// `argparse.ArgumentParser(prog=…, add_help=True, allow_abbrev=True, parents=[…], …)`.
pub fn parser_new(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let mut spec = ParserSpec { allow_abbrev: true, exit_on_error: true, ..Default::default() };
    if let Some(b) = c.kw("allow_abbrev").as_ref().and_then(lit_bool) {
        spec.allow_abbrev = b;
    }
    if let Some(b) = c.kw("exit_on_error").as_ref().and_then(lit_bool) {
        spec.exit_on_error = b;
    }
    let add_help = c.kw("add_help").as_ref().and_then(lit_bool).unwrap_or(true);
    if c.kw("add_help").is_some() && c.kw("add_help").as_ref().and_then(lit_bool).is_none() {
        spec.opaque = true;
    }
    if c.kw("prefix_chars").is_some() || c.kw("fromfile_prefix_chars").is_some() || c.kw("argument_default").is_some() {
        spec.opaque = true;
    }
    // parents: their arguments first
    if let Some(ps) = c.kw("parents") {
        let it = an.iterate(&ps, &c.node, st);
        match it.items {
            Some(items) => {
                for p in items {
                    match p.single().and_then(|o| an.m.argp.get(&o).cloned()) {
                        Some(ArgObj::Parser(ps)) => {
                            spec.args.extend(ps.args.iter().filter(|a| a.action != Action::Help).cloned());
                            spec.defaults.extend(ps.defaults.iter().cloned());
                            spec.opaque |= ps.opaque;
                        }
                        _ => spec.opaque = true,
                    }
                }
            }
            None => spec.opaque = true,
        }
    }
    if add_help {
        spec.args.insert(0, ArgSpec { flags: vec!["-h".into(), "--help".into()], dest: "help".into(), action: Action::Help, default: Value::new(), has_default: false, konst: Value::new(), conv: Conv::Str, nargs: Nargs::N(0), required: false, choices: None, suppress: true });
    }
    let oid = new_parser_obj(an, st, &c.node, &format!("ArgumentParser@L{}", c.line), "", ArgObj::Parser(spec));
    Value::obj(oid)
}

/// The parser a parser / group object adds arguments to.
fn target_parser(an: &Analyzer, o: ObjId) -> Option<ObjId> {
    match an.m.argp.get(&o)? {
        ArgObj::Parser(_) => Some(o),
        ArgObj::Group(p) => target_parser(an, *p),
        ArgObj::Subs(_) => None,
    }
}

fn spec_mut(an: &mut Analyzer, p: ObjId) -> Option<&mut ParserSpec> {
    match an.m.argp.get_mut(&p)? {
        ArgObj::Parser(s) => Some(s),
        _ => None,
    }
}

fn read_arg(an: &mut Analyzer, st: &mut State, c: &Call) -> Option<ArgSpec> {
    let names: Option<Vec<String>> = c.args.iter().map(lit_str).collect();
    let names = names?;
    if names.is_empty() {
        return None;
    }
    let optional = names[0].starts_with('-');
    if !names.iter().all(|n| n.starts_with('-') == optional) {
        return None;
    }
    let action = match c.kw("action") {
        None => Action::Store,
        Some(v) => match lit_str(&v).as_deref() {
            Some("store") => Action::Store,
            Some("store_const") => Action::StoreConst,
            Some("store_true") => Action::StoreTrue,
            Some("store_false") => Action::StoreFalse,
            Some("append") => Action::Append,
            Some("append_const") => Action::AppendConst,
            Some("extend") => Action::Extend,
            Some("count") => Action::Count,
            Some("help") => Action::Help,
            Some("version") => Action::Version,
            _ => Action::Other,
        },
    };
    let dest = match c.kw("dest") {
        Some(d) => lit_str(&d)?,
        None if optional => {
            let long = names.iter().find(|n| n.starts_with("--")).unwrap_or(&names[0]);
            long.trim_start_matches('-').replace('-', "_")
        }
        None => names[0].clone(),
    };
    let nargs = match c.kw("nargs") {
        None => {
            if matches!(action, Action::StoreConst | Action::StoreTrue | Action::StoreFalse | Action::AppendConst | Action::Count | Action::Help | Action::Version) {
                Nargs::N(0)
            } else {
                Nargs::One
            }
        }
        Some(v) => {
            let k = v.kv.as_lit().filter(|_| v.pts.is_empty())?;
            match intern::key_get(k) {
                intern::Key::Int(n) if n >= 0 => Nargs::N(n as usize),
                intern::Key::Str(s) => match &*s {
                    "?" => Nargs::Opt,
                    "*" => Nargs::Star,
                    "+" => Nargs::Plus,
                    "..." => Nargs::Rest,
                    _ => return None,
                },
                _ => return None,
            }
        }
    };
    let conv = match c.kw("type") {
        None => Conv::Str,
        Some(t) => match t.single().map(|o| (an.heap.kind(o), o)) {
            Some((Kind::External, o)) => match an.heap.external_qual(o).as_str() {
                "builtins.int" => Conv::Int,
                "builtins.float" => Conv::Float,
                "builtins.str" => Conv::Str,
                _ => Conv::Other,
            },
            _ => Conv::Other,
        },
    };
    let none = Value::none();
    let suppress_v = |v: &Value, an: &Analyzer| v.pts.iter().any(|o| an.heap.kind(o) == Kind::External && an.heap.external_qual(o).ends_with("SUPPRESS")) || matches!(&v.kv, KeyVal::Data(d, _) if intern::str_get(*d).contains("SUPPRESS")) || lit_str(v).as_deref() == Some("==SUPPRESS==");
    let (default, has_default, suppress) = match c.kw("default") {
        Some(d) => {
            let sup = suppress_v(&d, an);
            (d, true, sup)
        }
        None => match action {
            Action::StoreTrue => (Value::lit(intern::key_bool(false)), true, false),
            Action::StoreFalse => (Value::lit(intern::key_bool(true)), true, false),
            Action::Help | Action::Version => (Value::new(), false, true),
            _ => (none.clone(), false, false),
        },
    };
    let konst = match (action.clone(), c.kw("const")) {
        (Action::StoreTrue, _) => Value::lit(intern::key_bool(true)),
        (Action::StoreFalse, _) => Value::lit(intern::key_bool(false)),
        (_, Some(k)) => k,
        _ => none.clone(),
    };
    let required = match c.kw("required") {
        Some(v) => lit_bool(&v)?,
        None => !optional && !matches!(nargs, Nargs::Opt | Nargs::Star),
    };
    let choices = match c.kw("choices") {
        Some(ch) => {
            // (choices not known item by item: values are not checked against them)
            let it = an.iterate(&ch, &c.node, st);
            if it.items.is_none() {
                // (choices from a parameter or a global: decided per call site)
            }
            Some(it.any_order_items().cloned().unwrap_or_else(|| vec![Value::data("choice")]))
        }
        None => None,
    };
    if action == Action::Other || c.kwargs.keys().any(|k| !matches!(&*intern::str_get(*k), "action" | "dest" | "nargs" | "type" | "default" | "const" | "required" | "choices" | "help" | "metavar" | "version")) {
        return None;
    }
    Some(ArgSpec { flags: if optional { names } else { vec![] }, dest, action, default, has_default, konst, conv, nargs, required, choices, suppress })
}

/// `parser.*` methods: add_argument, add_subparsers, add_parser, set_defaults, groups, parse_args.
pub fn m_parser(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = c.qual.rsplit('.').next().unwrap_or("").to_string();
    let recv = c.recv.clone().unwrap_or_default();
    let objs: Vec<ObjId> = recv.pts.iter().filter(|o| an.m.argp.contains_key(o)).collect();
    if objs.is_empty() {
        return Value::data(&format!("L{}", c.line));
    }
    match name.as_str() {
        "add_argument" => {
            let spec = read_arg(an, st, c);
            for o in &objs {
                let Some(p) = target_parser(an, *o) else { continue };
                if let Some(s) = spec_mut(an, p) {
                    match &spec {
                        Some(a) => {
                            // a positional added before the subparsers keeps its place
                            s.args.push(a.clone());
                        }
                        None => s.opaque = true,
                    }
                }
            }
            Value::data("action")
        }
        "add_argument_group" | "add_mutually_exclusive_group" => {
            let o = objs[0];
            let gid = new_parser_obj(an, st, &c.node, "argument group", &format!("g{}", c.site), ArgObj::Group(o));
            Value::obj(gid)
        }
        "add_subparsers" => {
            let o = objs[0];
            let Some(p) = target_parser(an, o) else { return Value::data("subparsers") };
            let dest = c.kw("dest").as_ref().and_then(lit_str);
            let required = c.kw("required").as_ref().and_then(lit_bool).unwrap_or(false);
            if let Some(s) = spec_mut(an, p) {
                let after = s.args.iter().filter(|a| a.flags.is_empty()).count();
                s.sub = Some(SubSpec { dest, required, parsers: vec![], after_positionals: after });
            }
            let sid = new_parser_obj(an, st, &c.node, "subparsers", &format!("s{}", c.site), ArgObj::Subs(p));
            Value::obj(sid)
        }
        "add_parser" => {
            let o = objs[0];
            let Some(ArgObj::Subs(p)) = an.m.argp.get(&o).cloned() else { return Value::data("parser") };
            let Some(cmd) = c.args.first().and_then(lit_str) else {
                if let Some(s) = spec_mut(an, p) {
                    s.opaque = true;
                }
                return Value::data("parser");
            };
            let mut names = vec![cmd.clone()];
            if let Some(al) = c.kw("aliases") {
                let it = an.iterate(&al, &c.node, st);
                for a in it.items.unwrap_or_default() {
                    if let Some(x) = lit_str(&a) {
                        names.push(x);
                    }
                }
            }
            let add_help = c.kw("add_help").as_ref().and_then(lit_bool).unwrap_or(true);
            let mut spec = ParserSpec { allow_abbrev: true, exit_on_error: true, ..Default::default() };
            if add_help {
                spec.args.push(ArgSpec { flags: vec!["-h".into(), "--help".into()], dest: "help".into(), action: Action::Help, default: Value::new(), has_default: false, konst: Value::new(), conv: Conv::Str, nargs: Nargs::N(0), required: false, choices: None, suppress: true });
            }
            let child = new_parser_obj(an, st, &c.node, &format!("parser {}", cmd), &format!("sub{}", cmd), ArgObj::Parser(spec));
            if let Some(s) = spec_mut(an, p) {
                if let Some(sub) = s.sub.as_mut() {
                    sub.parsers.push((names, child));
                }
            }
            Value::obj(child)
        }
        "set_defaults" => {
            for o in &objs {
                let Some(p) = target_parser(an, *o) else { continue };
                let kws: Vec<(String, Value)> = c.kwargs.iter().map(|(k, v)| (intern::str_get(*k).to_string(), v.clone())).collect();
                if let Some(s) = spec_mut(an, p) {
                    for (k, v) in kws {
                        s.defaults.retain(|(n, _)| *n != k);
                        s.defaults.push((k, v));
                    }
                    if !c.star_kwargs.is_empty() {
                        s.opaque = true;
                    }
                }
            }
            Value::none()
        }
        "get_default" => {
            let Some(dest) = c.args.first().and_then(lit_str) else { return Value::data("default") };
            let Some(ArgObj::Parser(s)) = an.m.argp.get(&objs[0]).cloned() else { return Value::data("default") };
            if let Some((_, v)) = s.defaults.iter().find(|(n, _)| *n == dest) {
                return v.clone();
            }
            s.args.iter().find(|a| a.dest == dest).map(|a| a.default.clone()).unwrap_or_else(|| Value::none())
        }
        "parse_args" | "parse_known_args" | "parse_intermixed_args" | "parse_known_intermixed_args" => {
            let known = name.contains("known");
            let argv = c.arg(0, Some("args"));
            let tokens: Option<Vec<String>> = match &argv {
                Some(a) if !(a.pts.is_empty() && a.kv.as_lit() == Some(intern::key_none())) => {
                    let it = an.iterate(a, &c.node, st);
                    it.items.and_then(|items| items.iter().map(lit_str).collect())
                }
                // sys.argv[1:]: not known
                _ => None,
            };
            let mut outs = Vec::new();
            for o in &objs {
                let Some(p) = target_parser(an, *o) else { continue };
                let v = match &tokens {
                    Some(t) => parse_known(an, st, c, p, t, known),
                    None => None,
                };
                let v = match v {
                    Some(v) => v,
                    None => parse_unknown(an, st, c, p, known),
                };
                outs.push(v);
            }
            join_values(outs)
        }
        "print_help" | "print_usage" | "exit" | "error" => {
            if matches!(name.as_str(), "exit" | "error") {
                an.raise_here("SystemExit", st);
            }
            Value::none()
        }
        "format_help" | "format_usage" => Value::data("usage"),
        _ => Value::data(&format!("L{}", c.line)),
    }
}

/// A parse error: SystemExit (or ArgumentError with exit_on_error=False).
fn parse_error(an: &mut Analyzer, st: &mut State, spec: &ParserSpec) -> Value {
    an.raise_here(if spec.exit_on_error { "SystemExit" } else { "ArgumentError" }, st);
    Value::new()
}

/// A converted value (Err: the type rejects the text).
fn convert(a: &ArgSpec, text: &str) -> Result<Value, ()> {
    let v = match a.conv {
        Conv::Str => Value::lit_str(text),
        Conv::Int => match text.trim().replace('_', "").parse::<i64>() {
            Ok(i) => Value::lit(intern::key_int(i)),
            Err(_) => return Err(()),
        },
        Conv::Float => match text.trim().parse::<f64>() {
            Ok(f) => Value::lit(intern::key_float(f)),
            Err(_) => return Err(()),
        },
        Conv::Other => Value::data(&format!("arg {}", a.dest)),
    };
    if let Some(ch) = &a.choices {
        if let Some(k) = v.kv.as_lit().filter(|_| v.pts.is_empty()) {
            let known = ch.iter().all(|c| c.kv.as_lit().is_some() && c.pts.is_empty());
            if known && !ch.iter().any(|c| c.kv.as_lit() == Some(k)) {
                return Err(());
            }
        }
    }
    Ok(v)
}

fn min_count(n: &Nargs) -> usize {
    match n {
        Nargs::One | Nargs::Plus => 1,
        Nargs::N(k) => *k,
        _ => 0,
    }
}

/// The values one option / positional takes from `vals` (already split off).
fn take_values(a: &ArgSpec, vals: &[String]) -> Result<Value, ()> {
    let conv: Result<Vec<Value>, ()> = vals.iter().map(|t| convert(a, t)).collect();
    let conv = conv?;
    Ok(match a.nargs {
        Nargs::One => conv.into_iter().next().ok_or(())?,
        Nargs::Opt => match conv.into_iter().next() {
            Some(v) => v,
            None => if a.flags.is_empty() { a.default.clone() } else { a.konst.clone() },
        },
        _ => Value::data("list"),
    })
}

/// Python's `argparse` on a known argv (None: a parse this model cannot follow exactly).
fn parse_known(an: &mut Analyzer, st: &mut State, c: &Call, p: ObjId, tokens: &[String], known_args: bool) -> Option<Value> {
    let Some(ArgObj::Parser(spec)) = an.m.argp.get(&p).cloned() else { return None };
    if spec.opaque {
        return None;
    }
    let mut ns: IndexMap<String, Value> = IndexMap::new();
    let mut extras: Vec<String> = Vec::new();
    match parse_into(an, st, c, &spec, tokens, &mut ns, &mut extras, known_args, 0)? {
        Ok(()) => {}
        Err(()) => return Some(parse_error(an, st, &spec)),
    }
    if !extras.is_empty() && !known_args {
        return Some(parse_error(an, st, &spec));
    }
    let nid = namespace(an, st, c, &ns, "args");
    if known_args {
        let rest: Vec<Value> = extras.iter().map(|x| Value::lit_str(x)).collect();
        let rv = list_value(an, st, c, rest, "rest");
        let tid = an.alloc_at(Kind::Tuple, &c.node, st, "parse_known_args result", Payload::None, None, Ns::Item, None, Some("pka"));
        an.heap.set_llen(tid, st, Some(2));
        st.set_slot(tid, Ns::Pos, intern::key_int(0), nid.clone(), true);
        st.set_slot(tid, Ns::Pos, intern::key_int(1), rv, true);
        return Some(Value::obj(tid));
    }
    Some(nid)
}

fn list_value(an: &mut Analyzer, st: &mut State, c: &Call, vals: Vec<Value>, ctx: &str) -> Value {
    let lid = an.alloc_at(Kind::List, &c.node, st, &format!("list@L{}", c.line), Payload::None, None, Ns::Item, None, Some(ctx));
    let acc = join_values(vals.clone());
    st.set_slot(lid, Ns::Elem, intern::STAR, Value::with_kv(acc.pts.clone(), crate::models::torch::elem_kv(&acc.kv)), true);
    st.elems.insert(lid, acc.kv.clone());
    for (i, v) in vals.iter().enumerate() {
        st.set_slot(lid, Ns::Pos, intern::key_int(i as i64), v.clone(), true);
    }
    an.heap.set_llen(lid, st, Some(vals.len()));
    Value::obj(lid)
}

fn namespace(an: &mut Analyzer, st: &mut State, c: &Call, ns: &IndexMap<String, Value>, ctx: &str) -> Value {
    let nid = an.alloc_at(Kind::Namespace, &c.node, st, &format!("args@L{}", c.line), Payload::None, Some(Row::new()), Ns::Attr, None, Some(ctx));
    let mut row = Row::new();
    for (k, v) in ns {
        let kk = key_str_lit(k);
        row = row.store(kk, true);
        st.set_slot(nid, Ns::Attr, kk, v.clone(), true);
    }
    st.set_row(nid, Ns::Attr, row);
    Value::obj(nid)
}

/// Parses `tokens` with `spec` into `ns` (Ok(Err(())): a parse error; None: not followed).
#[allow(clippy::too_many_arguments)]
fn parse_into(an: &mut Analyzer, st: &mut State, c: &Call, spec: &ParserSpec, tokens: &[String], ns: &mut IndexMap<String, Value>, extras: &mut Vec<String>, known_args: bool, depth: u32) -> Option<Result<(), ()>> {
    if depth > 4 || spec.opaque {
        return None;
    }
    // defaults: the arguments', then the parser's (which win)
    for a in &spec.args {
        if a.suppress {
            continue;
        }
        if !ns.contains_key(&a.dest) || depth > 0 {
            ns.insert(a.dest.clone(), a.default.clone());
        }
    }
    if let Some(sub) = &spec.sub {
        if let Some(d) = &sub.dest {
            ns.entry(d.clone()).or_insert_with(|| Value::none());
        }
    }
    for (k, v) in &spec.defaults {
        ns.insert(k.clone(), v.clone());
    }
    let opts: Vec<&ArgSpec> = spec.args.iter().filter(|a| !a.flags.is_empty()).collect();
    let positionals: Vec<&ArgSpec> = spec.args.iter().filter(|a| a.flags.is_empty()).collect();
    let negative_like = opts.iter().any(|a| a.flags.iter().any(|f| f.len() > 1 && f[1..].chars().all(|ch| ch.is_ascii_digit() || ch == '.')));
    let is_option = |t: &str| -> bool {
        if !t.starts_with('-') || t == "-" {
            return false;
        }
        // a negative number is a value unless some option looks like one
        if !negative_like && t[1..].parse::<f64>().is_ok() {
            return false;
        }
        true
    };
    let find = |name: &str| -> Option<Vec<&ArgSpec>> {
        if let Some(a) = opts.iter().find(|a| a.flags.iter().any(|f| f == name)) {
            return Some(vec![a]);
        }
        if spec.allow_abbrev && name.starts_with("--") {
            let ms: Vec<&ArgSpec> = opts.iter().filter(|a| a.flags.iter().any(|f| f.starts_with("--") && f.starts_with(name))).copied().collect();
            return Some(ms);
        }
        Some(vec![])
    };
    let mut seen: Vec<String> = Vec::new();
    let mut pos_tokens: Vec<String> = Vec::new();
    let mut counts: IndexMap<String, i64> = IndexMap::new();
    let mut appends: IndexMap<String, Vec<Value>> = IndexMap::new();
    let mut i = 0;
    let mut only_pos = false;
    let mut sub_at: Option<usize> = None;
    let n_before_sub = spec.sub.as_ref().map(|s| s.after_positionals);
    while i < tokens.len() {
        let t = tokens[i].clone();
        if only_pos || !is_option(&t) {
            if t == "--" && !only_pos {
                only_pos = true;
                i += 1;
                continue;
            }
            // the subcommand: after the positionals that come before it
            if let Some(nb) = n_before_sub {
                let need: usize = positionals.iter().take(nb).map(|a| min_count(&a.nargs)).sum();
                if pos_tokens.len() >= need && positionals.iter().take(nb).all(|a| matches!(a.nargs, Nargs::One | Nargs::N(_))) {
                    sub_at = Some(i);
                    break;
                }
                if !positionals.iter().take(nb).all(|a| matches!(a.nargs, Nargs::One | Nargs::N(_))) {
                    return None;
                }
            }
            pos_tokens.push(t);
            i += 1;
            continue;
        }
        // an option: `--name=value`, `--name value`, `-x`, `-xVALUE`, `-abc`
        let (name, attached) = match t.split_once('=') {
            Some((n, v)) if t.starts_with("--") => (n.to_string(), Some(v.to_string())),
            _ => (t.clone(), None),
        };
        let mut matches = find(&name)?;
        let mut attached = attached;
        if matches.is_empty() && !t.starts_with("--") && t.len() > 2 {
            // `-xVALUE` / `-vq`: the first short flag
            let first = t[..2].to_string();
            if let Some(a) = opts.iter().find(|a| a.flags.iter().any(|f| *f == first)) {
                if a.nargs == Nargs::N(0) {
                    // a run of flags: each must take no value
                    let mut ok = true;
                    for ch in t[1..].chars() {
                        let fl = format!("-{}", ch);
                        match opts.iter().find(|a| a.flags.iter().any(|f| *f == fl)) {
                            Some(b) if b.nargs == Nargs::N(0) => {
                                if apply_flag(an, b, ns, &mut counts, &mut appends).is_err() {
                                    return Some(Err(()));
                                }
                                seen.push(b.dest.clone());
                            }
                            _ => ok = false,
                        }
                    }
                    if !ok {
                        return None;
                    }
                    i += 1;
                    continue;
                }
                matches = vec![a];
                attached = Some(t[2..].to_string());
            }
        }
        if matches.len() != 1 {
            if matches.is_empty() && known_args {
                extras.push(t);
                i += 1;
                continue;
            }
            // unknown or ambiguous option
            return Some(Err(()));
        }
        let a = matches[0];
        i += 1;
        if a.action == Action::Help || a.action == Action::Version {
            return Some(Err(()));
        }
        if a.nargs == Nargs::N(0) {
            if attached.is_some() {
                return Some(Err(()));
            }
            if apply_flag(an, a, ns, &mut counts, &mut appends).is_err() {
                return Some(Err(()));
            }
            seen.push(a.dest.clone());
            continue;
        }
        // the values: attached, else following non-option tokens
        let mut vals: Vec<String> = Vec::new();
        if let Some(v) = attached {
            vals.push(v);
        } else {
            let max = match a.nargs {
                Nargs::One | Nargs::Opt => 1,
                Nargs::N(k) => k,
                Nargs::Star | Nargs::Plus => usize::MAX,
                Nargs::Rest => usize::MAX,
            };
            while vals.len() < max && i < tokens.len() && (a.nargs == Nargs::Rest || !is_option(&tokens[i])) {
                vals.push(tokens[i].clone());
                i += 1;
            }
        }
        if vals.len() < min_count(&a.nargs) {
            return Some(Err(()));
        }
        match a.action {
            Action::Store => {
                if matches!(a.nargs, Nargs::Star | Nargs::Plus | Nargs::N(_) | Nargs::Rest) {
                    let conv: Result<Vec<Value>, ()> = vals.iter().map(|x| convert(a, x)).collect();
                    let Ok(cv) = conv else { return Some(Err(())) };
                    let lv = list_value(an, st, c, cv, &format!("opt{}", a.dest));
                    ns.insert(a.dest.clone(), lv);
                } else {
                    match take_values(a, &vals) {
                        Ok(v) => {
                            ns.insert(a.dest.clone(), v);
                        }
                        Err(()) => return Some(Err(())),
                    }
                }
            }
            Action::Append | Action::Extend => {
                let conv: Result<Vec<Value>, ()> = vals.iter().map(|x| convert(a, x)).collect();
                let Ok(cv) = conv else { return Some(Err(())) };
                let e = appends.entry(a.dest.clone()).or_default();
                if a.action == Action::Extend || matches!(a.nargs, Nargs::One) {
                    e.extend(cv);
                } else {
                    let lv = list_value(an, st, c, cv, &format!("app{}{}", a.dest, e.len()));
                    e.push(lv);
                }
            }
            _ => return None,
        }
        seen.push(a.dest.clone());
    }
    // positionals: in order, each takes what it may while leaving the later ones their minimum
    let mut rest: &[String] = &pos_tokens;
    let limit = n_before_sub.unwrap_or(positionals.len());
    for (j, a) in positionals.iter().enumerate().take(limit) {
        let later: usize = positionals.iter().skip(j + 1).take(limit.saturating_sub(j + 1)).map(|b| min_count(&b.nargs)).sum();
        let avail = rest.len().saturating_sub(later);
        let take = match a.nargs {
            Nargs::One => 1.min(rest.len()),
            Nargs::N(k) => k.min(rest.len()),
            Nargs::Opt => 1.min(avail),
            Nargs::Star | Nargs::Plus | Nargs::Rest => avail,
        };
        let vals: Vec<String> = rest[..take].to_vec();
        rest = &rest[take..];
        if vals.len() < min_count(&a.nargs) {
            return Some(Err(()));
        }
        let v = match a.nargs {
            Nargs::One | Nargs::Opt => match take_values(a, &vals) {
                Ok(v) => v,
                Err(()) => return Some(Err(())),
            },
            _ => {
                let conv: Result<Vec<Value>, ()> = vals.iter().map(|x| convert(a, x)).collect();
                let Ok(cv) = conv else { return Some(Err(())) };
                list_value(an, st, c, cv, &format!("pos{}", a.dest))
            }
        };
        ns.insert(a.dest.clone(), v);
        seen.push(a.dest.clone());
    }
    if !rest.is_empty() {
        if known_args {
            extras.extend(rest.iter().cloned());
        } else {
            return Some(Err(()));
        }
    }
    for (d, n) in counts {
        ns.insert(d, Value::lit(intern::key_int(n)));
    }
    for (d, vs) in appends {
        // (appending to a list default copies it first: its items come first)
        let default = opts.iter().find(|a| a.dest == d && a.has_default).map(|a| a.default.clone()).filter(|v| !(v.pts.is_empty() && v.kv == Value::none().kv));
        let (mut items, mut known) = (Vec::new(), true);
        if let Some(dv) = default {
            let it = an.iterate(&dv, &c.node, st);
            match it.items {
                Some(xs) => items = xs,
                None => {
                    known = false;
                    items.push(it.elem.flat());
                }
            }
        }
        items.extend(vs);
        let lv = list_value(an, st, c, items, &format!("apl{}", d));
        if !known {
            let l = lv.single().unwrap();
            st.drop_obj_ns(l, Ns::Pos);
            an.heap.set_llen(l, st, None);
        }
        ns.insert(d, lv);
    }
    // required options
    for a in &opts {
        if a.required && !seen.contains(&a.dest) {
            return Some(Err(()));
        }
    }
    // the subcommand
    if let Some(sub) = &spec.sub {
        match sub_at {
            Some(k) => {
                let cmd = &tokens[k];
                let Some((names, child)) = sub.parsers.iter().find(|(ns_, _)| ns_.contains(cmd)) else { return Some(Err(())) };
                if let Some(d) = &sub.dest {
                    ns.insert(d.clone(), Value::lit_str(&names[0]));
                }
                let Some(ArgObj::Parser(cspec)) = an.m.argp.get(child).cloned() else { return None };
                let mut cextras = Vec::new();
                match parse_into(an, st, c, &cspec, &tokens[k + 1..], ns, &mut cextras, known_args, depth + 1)? {
                    Ok(()) => {}
                    Err(()) => return Some(Err(())),
                }
                extras.extend(cextras);
            }
            None if sub.required => return Some(Err(())),
            None => {}
        }
    }
    Some(Ok(()))
}

/// A flag (no values): store_true / store_false / store_const / count / append_const.
fn apply_flag(an: &Analyzer, a: &ArgSpec, ns: &mut IndexMap<String, Value>, counts: &mut IndexMap<String, i64>, appends: &mut IndexMap<String, Vec<Value>>) -> Result<(), ()> {
    let _ = an;
    match a.action {
        Action::StoreTrue | Action::StoreFalse | Action::StoreConst => {
            ns.insert(a.dest.clone(), a.konst.clone());
        }
        Action::Count => {
            let cur = counts.get(&a.dest).copied().unwrap_or_else(|| a.default.kv.as_lit().filter(|_| a.default.pts.is_empty()).and_then(intern::key_as_int).unwrap_or(0));
            counts.insert(a.dest.clone(), cur + 1);
        }
        Action::AppendConst => {
            appends.entry(a.dest.clone()).or_default().push(a.konst.clone());
        }
        _ => return Err(()),
    }
    Ok(())
}

/// The value an argument may get from an unknown command line.
fn any_value(an: &mut Analyzer, st: &mut State, c: &Call, a: &ArgSpec) -> Value {
    let given = match a.action {
        Action::StoreTrue | Action::StoreFalse | Action::StoreConst => a.konst.clone(),
        Action::Count => Value::data(&format!("count {}", a.dest)),
        Action::Append | Action::Extend | Action::AppendConst => {
            let e = match a.conv {
                Conv::Int => Value::data(&format!("int {}", a.dest)),
                _ => Value::data(&format!("arg {}", a.dest)),
            };
            let lid = an.alloc_at(Kind::List, &c.node, st, &format!("list@L{}", c.line), Payload::None, None, Ns::Item, None, Some(&format!("anyl{}", a.dest)));
            st.set_slot(lid, Ns::Elem, intern::STAR, e.clone(), true);
            st.elems.insert(lid, e.kv.clone());
            an.heap.set_llen(lid, st, None);
            Value::obj(lid)
        }
        _ => {
            let one = match (&a.choices, &a.conv) {
                (Some(ch), _) if ch.iter().all(|x| x.pts.is_empty() && x.kv.is_literal()) => join_values(ch.clone()),
                (_, Conv::Int) => Value::data(&format!("int {}", a.dest)),
                (_, Conv::Float) => Value::data(&format!("float {}", a.dest)),
                _ => Value::data(&format!("arg {}", a.dest)),
            };
            match a.nargs {
                Nargs::One | Nargs::Opt => {
                    if a.nargs == Nargs::Opt {
                        one.join(&a.konst)
                    } else {
                        one
                    }
                }
                _ => {
                    let lid = an.alloc_at(Kind::List, &c.node, st, &format!("list@L{}", c.line), Payload::None, None, Ns::Item, None, Some(&format!("anyn{}", a.dest)));
                    st.set_slot(lid, Ns::Elem, intern::STAR, one.clone(), true);
                    st.elems.insert(lid, one.kv.clone());
                    an.heap.set_llen(lid, st, None);
                    Value::obj(lid)
                }
            }
        }
    };
    if a.required && !a.flags.is_empty() || (a.flags.is_empty() && min_count(&a.nargs) > 0) {
        given
    } else {
        given.join(&a.default)
    }
}

/// `parse_args()` on a command line we do not know: every attribute a parse may set.
fn parse_unknown(an: &mut Analyzer, st: &mut State, c: &Call, p: ObjId, known_args: bool) -> Value {
    // (any parse may fail)
    an.may_raise_here("SystemExit", st);
    let Some(ArgObj::Parser(spec)) = an.m.argp.get(&p).cloned() else { return Value::data("args") };
    let mut base: IndexMap<String, Value> = IndexMap::new();
    let mut maybe: Vec<String> = Vec::new();
    unknown_into(an, st, c, &spec, &mut base, &mut maybe);
    let mut outs: Vec<Value> = Vec::new();
    let make = |an: &mut Analyzer, st: &mut State, ns: &IndexMap<String, Value>, maybe: &[String], opaque: bool, ctx: &str| -> Value {
        let v = namespace(an, st, c, ns, ctx);
        if let Some(o) = v.single() {
            let mut r = st.row(o, Ns::Attr);
            for m in maybe {
                let k = key_str_lit(m);
                let d = r.data();
                let mut d = d;
                d.definite = d.definite.remove(k);
                d.likely = d.likely.remove(k);
                r = Row::from_data(d);
            }
            if opaque {
                r = r.data_store(crate::source::Source::new(&format!("parse_args@L{}", c.line), crate::source::SourceKind::Data));
            }
            st.set_row(o, Ns::Attr, r);
        }
        v
    };
    match spec.sub.clone() {
        Some(sub) if !sub.parsers.is_empty() => {
            for (names, child) in &sub.parsers {
                let mut ns = base.clone();
                let mut mb = maybe.clone();
                if let Some(d) = &sub.dest {
                    ns.insert(d.clone(), Value::lit_str(&names[0]));
                }
                let opaque = match an.m.argp.get(child).cloned() {
                    Some(ArgObj::Parser(cs)) => {
                        unknown_into(an, st, c, &cs, &mut ns, &mut mb);
                        cs.opaque
                    }
                    _ => true,
                };
                outs.push(make(an, st, &ns, &mb, opaque || spec.opaque, &format!("ns{}", names[0])));
            }
            if !sub.required {
                let mut ns = base.clone();
                if let Some(d) = &sub.dest {
                    ns.insert(d.clone(), Value::none());
                }
                outs.push(make(an, st, &ns, &maybe, spec.opaque, "nsnone"));
            }
        }
        _ => outs.push(make(an, st, &base, &maybe, spec.opaque || spec.sub.is_some(), "ns")),
    }
    let v = join_values(outs);
    if known_args {
        let rv = Value::obj({
            let lid = an.alloc_at(Kind::List, &c.node, st, &format!("list@L{}", c.line), Payload::None, None, Ns::Item, None, Some("rest"));
            st.set_slot(lid, Ns::Elem, intern::STAR, Value::data("arg"), true);
            an.heap.set_llen(lid, st, None);
            lid
        });
        let tid = an.alloc_at(Kind::Tuple, &c.node, st, "parse_known_args result", Payload::None, None, Ns::Item, None, Some("pka"));
        an.heap.set_llen(tid, st, Some(2));
        st.set_slot(tid, Ns::Pos, intern::key_int(0), v, true);
        st.set_slot(tid, Ns::Pos, intern::key_int(1), rv, true);
        return Value::obj(tid);
    }
    v
}

fn unknown_into(an: &mut Analyzer, st: &mut State, c: &Call, spec: &ParserSpec, ns: &mut IndexMap<String, Value>, maybe: &mut Vec<String>) {
    for a in spec.args.clone() {
        if a.action == Action::Help || a.action == Action::Version {
            continue;
        }
        let v = any_value(an, st, c, &a);
        if a.suppress && !a.required {
            maybe.push(a.dest.clone());
        }
        let nv = match ns.get(&a.dest) {
            // (two arguments with one dest)
            Some(cur) => cur.join(&v),
            None => v,
        };
        ns.insert(a.dest.clone(), nv);
    }
    if let Some(sub) = &spec.sub {
        if let Some(d) = &sub.dest {
            ns.entry(d.clone()).or_insert_with(|| Value::none());
        }
    }
    for (k, v) in &spec.defaults {
        ns.insert(k.clone(), v.clone());
    }
}

pub fn register(r: &mut dyn FnMut(&[&'static str], crate::models::Model)) {
    r(&["argparse.ArgumentParser"], parser_new);
    r(&["parser.*"], m_parser);
}
