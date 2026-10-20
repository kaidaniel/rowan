//! Values as Python text, for the report and the hovers: `render_value` writes what a value
//! may be the way the program would write it (`{"id": 1, "role"?: "admin", **?}`,
//! `list[int(?)]`, `Tensor[B, T, 4*C]`, `row["id"]` for an unknown named by where it comes
//! from), cut at a depth (`VALUE_DEPTH`) and width; `value_legend` says what the names it
//! mentions stand for; `join_alternatives` merges the texts of several runs of a line.

use crate::domain::{key_display, kv_literals, label_base, label_name, DATA, KeyVal, Ns, ObjId, Value};
use crate::intern::{self, KeyId, STAR};
use crate::interp::Analyzer;
use crate::state::{Kind, State};

/// How deep rendered values go before nested containers are cut to `[…]` / `{…}` / `Name(…)`
/// (`--value-depth N`; default 3).
pub static VALUE_DEPTH: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(3);

/// Values rendered at several runs of a line as one: their alternatives (split at the top
/// level, outside brackets and strings), each once, the unknown `?` last.
pub fn join_alternatives(texts: &[String]) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for t in texts {
        let (mut depth, mut quote, mut start) = (0i32, None::<char>, 0usize);
        let mut prev = ' ';
        for (i, c) in t.char_indices() {
            match (quote, c) {
                (Some(q), _) if c == q && prev != '\\' => quote = None,
                (Some(_), _) => {}
                (None, '"' | '\'') => quote = Some(c),
                (None, '(' | '[' | '{') => depth += 1,
                (None, ')' | ']' | '}') => depth -= 1,
                (None, '|') if depth == 0 && t[..i].ends_with(' ') && t[i + 1..].starts_with(' ') => {
                    parts.push(t[start..i - 1].trim());
                    start = i + 2;
                }
                _ => {}
            }
            prev = c;
        }
        parts.push(t[start..].trim());
    }
    let mut out: Vec<&str> = Vec::new();
    for p in parts {
        if !out.contains(&p) {
            out.push(p);
        }
    }
    if let Some(i) = out.iter().position(|p| *p == "?") {
        let q = out.remove(i);
        out.push(q);
    }
    out.join(" | ")
}

/// A value as the user would write it, as far as it is known: literals (`1`,
/// `"a"`, several as `1 | 2 | ?`), dicts with their keys and values (`{"a": 1, "b"?: ?, **?}`),
/// lists / tuples by position when known (`[1, 2]`, `(1,)`), else `list[X]` /
/// `tuple[X, ...]`, instances as `Cls(a=1, **?)`, functions / classes by name, `?` for
/// anything unknown (`**?`: other keys or attributes may exist).  Depth and width are
/// limited (`…` marks what is not shown; also a cycle).
pub fn render_value(an: &Analyzer, state: &State, v: &Value, depth: u32, seen: &mut Vec<ObjId>) -> String {
    let mut parts: Vec<String> = Vec::new();
    // literals
    // (an unknown's literals it may be: `8080 | ?`)
    let lits = if v.kv.is_literal() { kv_literals(&v.kv) } else { v.kv.hint() };
    if !lits.is_empty() {
        let mut ls: Vec<String> = intern::sorted_by_str(lits.iter()).iter().map(|k| short_literal(key_display(*k))).collect();
        if ls.len() > 4 {
            ls.truncate(4);
            ls.push("…".into());
        }
        parts.extend(ls);
    }
    // a tensor parameter with known shape facts: its shape
    if let Some(s) = crate::models::torch::t_known_param_shape(an, v, state) {
        return crate::models::torch::tensor_text(&s);
    }
    for o in v.pts.iter() {
        parts.push(render_object(an, state, o, depth, seen));
    }
    // a part nothing is known about (data, keys of something): `?`; keys of a builtin type
    let keys = match &v.kv {
        KeyVal::KeysOf(ko) if ko.chain == intern::EMPTY_LENS => state.keys_type(ko.objs.iter(), ko.ns),
        _ => None,
    };
    if let Some(t) = keys {
        parts.push(format!("{}(?)", t));
    } else if let Some(t) = crate::models::torch::t_render(an, v, state) {
        parts.push(t);
    } else if let Some(expr) = data_expr(&v.kv) {
        parts.push(expr);
    } else if let Some((t, opt)) = v.kv.data_site().and_then(|l| label_base(&intern::str_get(l))) {
        // (an unknown of a type or None: `None | int(?)`)
        if opt {
            parts.push("None".into());
        }
        parts.push(format!("{}(?)", t));
    } else if !v.kv.is_literal() && !v.kv.is_bot() {
        parts.push("?".into());
    }
    if parts.is_empty() {
        return "?".into();
    }
    parts.sort();
    parts.dedup();
    // the unknown part last: `1 | 2 | ?`
    if parts.len() > 1 && parts.iter().any(|p| p == "?") {
        parts.retain(|p| p != "?");
        parts.push("?".into());
    }
    // (literals left out, `…`, beside the literals: `"a" | "b" | … | str(?)`)
    if let Some(i) = parts.iter().position(|p| p == "…") {
        let dots = parts.remove(i);
        let at = parts.iter().position(|p| p.ends_with("(?)") || p == "?").unwrap_or(parts.len()).min(4);
        parts.insert(at, dots);
    }
    parts.join(" | ")
}

/// One object as Python text (`render_value`'s parts).
fn render_object(an: &Analyzer, state: &State, o: ObjId, depth: u32, seen: &mut Vec<ObjId>) -> String {
    let max_depth: u32 = VALUE_DEPTH.load(std::sync::atomic::Ordering::Relaxed);
    if !an.heap.has_obj(o) {
        return "?".into();
    }
    let kind = an.heap.kind(o);
    if seen.contains(&o) {
        // a cycle back to an object being printed
        return "…".into();
    }
    let deeper = depth + 1;
    match kind {
        Kind::Dict | Kind::Frame => render_mapping(an, state, o, kind, depth, seen),
        // objects of library models: their attributes when they have some (`Namespace(n=3)`),
        // else what made them (`re.compile('a+')`, `PosixPath('/tmp')`) or their type (`<Match>`)
        // a builtin exception as Python writes it: `KeyError("k")`
        Kind::Namespace if an.m.exc_objs.contains_key(&o) && state.has_slot(o, Ns::Attr, intern::key_str_lit("args")) => {
            let args = state.slot(o, Ns::Attr, intern::key_str_lit("args"));
            if depth >= max_depth {
                return format!("{}(…)", an.m.exc_objs[&o]);
            }
            seen.push(o);
            let items = args_items(an, state, &args, depth, seen);
            seen.pop();
            format!("{}({})", an.m.exc_objs[&o], items.join(", "))
        }
        Kind::Namespace => {
            let label = an.heap.obj(o).label.to_string();
            let name = opaque_name(&label);
            let row = state.row(o, Ns::Attr);
            // (attribute bags: argparse's and types' namespaces, a request's `state`; other model
            // objects keep their attributes for the model's own use)
            let bag = ["args@", "namespace@", "request.state", "row@"].iter().any(|p| label.starts_with(p));
            if bag && row.present.iter().any(|k| !is_internal_key(k)) {
                // (a frame's row from `itertuples`: the namedtuple it names, "Pandas" by default)
                let shown = match label.strip_prefix("row@") {
                    Some(rest) => rest.split_once(':').map(|x| x.1).unwrap_or("Pandas"),
                    None => name.trim_matches(|c| c == '<' || c == '>'),
                };
                if depth >= max_depth {
                    return format!("{}(…)", shown);
                }
                return render_record(an, state, o, Ns::Attr, Some(shown), depth, seen);
            }
            name
        }
        // a (key, value) pair of `items()` / `zip` / `enumerate`: a 2-tuple
        Kind::Pair => {
            let crate::interp::Payload::Pair(k, v) = &an.heap.obj(o).payload else { return "?".into() };
            if depth >= max_depth {
                return "(…)".into();
            }
            seen.push(o);
            let out = format!("({}, {})", render_value(an, state, k, deeper, seen), render_value(an, state, v, deeper, seen));
            seen.pop();
            out
        }
        Kind::List | Kind::Tuple | Kind::Set => render_sequence(an, state, o, kind, depth, seen),
        // (an enum member as the program names it: `Color.RED`)
        Kind::Instance if an.heap.keys.member_key.contains_key(&o) => intern::with_key(an.heap.keys.member_key[&o], |k| if let intern::Key::Member(q, _) = k { q.to_string() } else { "?".into() }),
        Kind::Instance => {
            let name = an.heap.instance_class(o).and_then(|c| an.heap.class_info_opt(c)).map(|ci| ci.name.clone()).unwrap_or_else(|| "object".into());
            if depth >= max_depth {
                return format!("{}(…)", name);
            }
            render_record(an, state, o, Ns::Attr, Some(&name), depth, seen)
        }
        Kind::Function => {
            let l = an.heap.obj(o).label.trim_start_matches("def ").to_string();
            match l.strip_prefix("lambda@L") {
                Some(line) => format!("<lambda on line {}>", line),
                None => l,
            }
        }
        Kind::Bound => {
            let l = an.heap.obj(o).label.clone();
            let name = l.trim_start_matches("def ").trim_start_matches("class ").split(" bound").next().unwrap_or(&l).to_string();
            format!("<bound method {}>", name)
        }
        Kind::Class => an.heap.obj(o).label.trim_start_matches("class ").to_string(),
        Kind::Module => an.heap.obj(o).label.to_string(),
        // an unknown named in the program's terms (`a`, `a(b)`, `cfg["db"]`), else `?`
        // (unnamed here: of its type when it has one, `int(?)`)
        Kind::Unknown => an.names.visible(o).cloned().unwrap_or_else(|| match (an.heap.ann.get(&o), an.heap.obj(o).ty) {
            (Some((a, _)), _) => crate::interp::annotations::annotation_text(a),
            (None, Some(t)) => format!("{}(?)", t),
            (None, None) => "?".into(),
        }),
        Kind::External => an.heap.external_qual(o).trim_start_matches("builtins.").to_string(),
        Kind::Super => "super()".into(),
        // iterators, views, groupbys, parsers, responses…: by their type, as Python prints them
        _ => opaque_name(&an.heap.obj(o).label),
    }
}

/// A list, tuple or set: its known positions in order (a set's known literal members sorted),
/// else what any element may be in typing notation (`list[int]`); a deque wrapped.
fn render_sequence(an: &Analyzer, state: &State, o: ObjId, kind: Kind, depth: u32, seen: &mut Vec<ObjId>) -> String {
    let max_depth: u32 = VALUE_DEPTH.load(std::sync::atomic::Ordering::Relaxed);
    const MAX_ITEMS: usize = 8;
    let deeper = depth + 1;
    let (open, close, ty) = match kind {
        Kind::List => ("[", "]", "list"),
        Kind::Tuple => ("(", ")", "tuple"),
        _ => ("{", "}", "set"),
    };
    seen.push(o);
    let out = match an.heap.llen(o, state) {
        // a generator: by its function (it runs when it is consumed)
        _ if an.heap.obj(o).label.starts_with("generator ") => format!("<{}>", an.heap.obj(o).label),
        // known positions: the elements in order
        Some(n) if kind != Kind::Set => {
            if depth >= max_depth {
                seen.pop();
                return if n == 0 { format!("{}{}", open, close) } else { format!("{}…{}", open, close) };
            }
            let mut items: Vec<String> = (0..n.min(MAX_ITEMS)).map(|i| match state.slot_opt(o, Ns::Pos, intern::key_int(i as i64)) {
                Some(sv) => render_value(an, state, &sv, deeper, seen),
                None => "?".into(),
            }).collect();
            if n > MAX_ITEMS {
                items.push("…".into());
            }
            if kind == Kind::Tuple && n == 1 {
                format!("({},)", items[0])
            } else {
                format!("{}{}{}", open, items.join(", "), close)
            }
        }
        // a set of known literal members: them, sorted (a set has no order)
        Some(n) if kind == Kind::Set && depth < max_depth && n <= MAX_ITEMS && set_members(state, o, n).is_some() => {
            let ms = set_members(state, o, n).unwrap();
            if ms.is_empty() {
                "set()".into()
            } else {
                format!("{{{}}}", ms.iter().map(|k| render_value(an, state, &Value::lit(*k), deeper, seen)).collect::<Vec<_>>().join(", "))
            }
        }
        // one of a few lengths (an append on some paths): each list (`["a"] | ["a", "b"]`)
        None if kind != Kind::Set && depth < max_depth && alt_lists(an, state, o).is_some() => {
            let alts = alt_lists(an, state, o).unwrap();
            let items: Vec<String> = (0..*alts.last().unwrap()).map(|i| render_value(an, state, &state.slot(o, Ns::Pos, intern::key_int(i as i64)), deeper, seen)).collect();
            alts.iter().map(|n| if kind == Kind::Tuple && *n == 1 { format!("({},)", items[0]) } else { format!("{}{}{}", open, items[..*n].join(", "), close) }).collect::<Vec<_>>().join(" | ")
        }
        // unknown length: what any element may be, in typing notation
        _ => {
            let elem = state.slot_opt(o, Ns::Elem, STAR);
            let ekv = state.elems.get(&o);
            let known_empty = matches!(&ekv, Some(KeyVal::Lits(ks)) if ks.is_empty());
            let mut ev = elem.unwrap_or_default();
            // (the scalar part of the elements is kept apart, as iterating and indexing read it:
            // literals, an unknown of a type, `int(?)`; the slot keeps plain data)
            if let Some(kv) = ekv {
                if ev.kv.is_bot() || ev.kv == DATA {
                    ev = Value::with_kv(ev.pts.clone(), kv);
                }
            }
            let inner = if depth >= max_depth { "…".to_string() } else if ev.empty() { "?".to_string() } else { render_value(an, state, &ev, deeper, seen) };
            if ev.empty() && known_empty {
                format!("{}{}", open, close)
            } else if kind == Kind::Tuple {
                format!("tuple[{}, ...]", inner)
            } else {
                format!("{}[{}]", ty, inner)
            }
        }
    };
    seen.pop();
    if an.heap.obj(o).label.starts_with("deque@") {
        return format!("deque({})", out);
    }
    out
}

/// The lengths a list may have, when they are 2 or 3 short ones whose positions are known.
fn alt_lists(an: &Analyzer, state: &State, o: ObjId) -> Option<Vec<usize>> {
    let alts = an.heap.len_alts(o, state)?;
    let max = *alts.last()?;
    ((2..=3).contains(&alts.len()) && max <= 4 && (0..max).all(|i| state.has_slot(o, Ns::Pos, intern::key_int(i as i64)))).then_some(alts)
}

/// A dict or frame: its keys and values (a frame by its columns; a Series / Counter /
/// defaultdict / OrderedDict wrapped as Python prints them).
fn render_mapping(an: &Analyzer, state: &State, o: ObjId, kind: Kind, depth: u32, seen: &mut Vec<ObjId>) -> String {
    let max_depth: u32 = VALUE_DEPTH.load(std::sync::atomic::Ordering::Relaxed);
    let row = state.row(o, Ns::Item);
    // (a frame by its columns, a Series / Counter / defaultdict by their keys, as
    // Python prints them)
    let label = an.heap.obj(o).label.clone();
    let wrap = if kind == Kind::Frame {
        Some("DataFrame")
    } else if crate::models::pandas::is_series_obj(an, o) {
        Some("Series")
    } else {
        ["Counter@", "defaultdict@", "OrderedDict@"].iter().find(|p| label.starts_with(**p)).map(|p| p.trim_end_matches('@'))
    };
    let body = if depth >= max_depth {
        if row.present.is_empty() && row.closed() { "{}".into() } else { "{…}".into() }
    } else if kind == Kind::Frame {
        let mut cols: Vec<String> = intern::sorted_by_str(row.present.iter()).iter().map(|k| format!("{}{}", key_display(*k), row.mark(*k))).collect();
        if !row.closed() {
            cols.push("**?".into());
        }
        return format!("DataFrame[{}]", cols.join(", "));
    } else {
        render_record(an, state, o, Ns::Item, None, depth, seen)
    };
    match wrap {
        // (nothing known about its keys: the type alone)
        Some(w) if body == "{**?}" => format!("<{}>", if w == "series" { "Series" } else { w }),
        Some("series") => format!("Series({})", body),
        Some(w) => format!("{}({})", w, body),
        None => body,
    }
}

/// A long string literal cut to its first 60 characters (`"abc…"`).
fn short_literal(t: String) -> String {
    const MAX: usize = 60;
    if t.chars().count() <= MAX || !t.starts_with('"') {
        return t;
    }
    format!("{}…\"", t.chars().take(MAX - 2).collect::<String>())
}

/// An object of a library model by its type (`<Match>`, `<sqlite3.Connection>`), or by the
/// call that made it when its label reads as one (`re.compile('a+')`, `PosixPath('/tmp')`).
fn opaque_name(label: &str) -> String {
    if label.contains('(') && !label.contains('@') {
        return label.to_string();
    }
    let base = label.split('@').next().unwrap_or(label).trim();
    let base = base.replace("stdobj.repattern.", "re.Pattern.").replace("stdobj.", "");
    let name = match base.as_str() {
        "sqlite3.connect" => "sqlite3.Connection",
        "fastapi.app" => "FastAPI",
        "fastapi.client" => "TestClient",
        "fastapi.response" | "response" => "Response",
        "items view" => "dict_items",
        "keys view" => "dict_keys",
        "values view" => "dict_values",
        "groupby" => "GroupBy",
        "args" => "Namespace",
        "request.state" => "State",
        "" => "object",
        b => b,
    };
    format!("<{}>", name)
}

/// Keys the models keep for themselves (`"\u{1}text"`): never shown.
pub(super) fn is_internal_key(k: KeyId) -> bool {
    intern::key_as_str(k).map(|s| s.starts_with('\u{1}')).unwrap_or(false)
}

thread_local! {
    /// The function whose names expressions are printed for (`expr:` data from elsewhere —
    /// a callee's `1 + b` returned to its caller — prints as `?`).
    static RENDER_SCOPE: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// The function whose names values are printed for (`in_scope`).
pub fn render_scope() -> u32 {
    RENDER_SCOPE.with(|c| c.get())
}

/// Run `f` printing values as seen inside function `scope` (0: module level).
pub fn in_scope<R>(scope: u32, f: impl FnOnce() -> R) -> R {
    let prev = RENDER_SCOPE.with(|c| c.replace(scope));
    let r = f();
    RENDER_SCOPE.with(|c| c.set(prev));
    r
}

/// An unknown value computed by an expression rowan names it by (`1 + b`), when that
/// expression is in the scope being printed.
pub fn data_expr(kv: &KeyVal) -> Option<String> {
    let KeyVal::Data(s, _) = kv else { return None };
    let label = intern::str_get(*s);
    let rest = label_name(&label).strip_prefix("expr:")?;
    let (scope, text) = rest.split_once(':')?;
    let here = RENDER_SCOPE.with(|c| c.get());
    (scope.parse::<u32>().ok()? == here).then(|| text.to_string())
}

/// The keys (`ns` Item: `{"a": 1, "b"?: ?, **?}`) or attributes (Attr: `Name(a=1, **?)`) of an
/// object, with their values.
fn render_record(an: &Analyzer, state: &State, o: ObjId, ns: Ns, name: Option<&str>, depth: u32, seen: &mut Vec<ObjId>) -> String {
    const MAX_ITEMS: usize = 8;
    let row = state.row(o, ns);
    seen.push(o);
    let mut items: Vec<String> = Vec::new();
    let mut keys: Vec<KeyId> = intern::sorted_by_str(row.present.iter()).into_iter().filter(|k| !is_internal_key(*k)).collect();
    // (an instance of a followed library's class: its public attributes, the rest as `…`)
    let library = ns == Ns::Attr && an.heap.instance_class(o).and_then(|c| an.heap.class_info_opt(c)).is_some_and(|ci| an.heap.module_info(ci.module).library);
    let private = library && keys.iter().any(|k| intern::key_as_str(*k).is_some_and(|n| n.starts_with('_')));
    if library {
        keys.retain(|k| !intern::key_as_str(*k).is_some_and(|n| n.starts_with('_')));
    }
    // an exception as Python writes it: its `args` first, by position (`AppError("too big",
    // code=400)`), its `__cause__` when one is set
    if let Some(args) = exception_args(an, state, o, ns, &row) {
        let cause = intern::key_str_lit("__cause__");
        let no_cause = |k: KeyId| k == cause && state.slot_opt(o, ns, k).is_some_and(|v| v.pts.is_empty() && v.kv.as_lit() == Some(intern::key_none()));
        keys.retain(|k| *k != intern::key_str_lit("args") && !no_cause(*k));
        items.extend(args_items(an, state, &args, depth, seen));
    }
    // (an open record's key that may be there, of a value not known, says no more than `**?`:
    // left out — a loop's item read on earlier rounds was `{"id"?: ?, "name"?: ?, **?}`)
    // (nor one holding what any key not known holds, a read under it found: `dict[?, int(?)]`)
    if !row.closed() {
        let star = state.slot_opt(o, ns, STAR);
        keys.retain(|k| row.likely.contains(*k) || state.slot_opt(o, ns, *k).is_some_and(|sv| Some(&sv) != star.as_ref() && render_value(an, state, &sv, depth + 1, &mut seen.clone()) != "?"));
    }
    for k in keys.iter().take(MAX_ITEMS) {
        let val = match state.slot_opt(o, ns, *k) {
            Some(sv) => render_value(an, state, &sv, depth + 1, seen),
            None => "?".into(),
        };
        if ns == Ns::Attr {
            items.push(format!("{}{}={}", intern::key_py_str(*k), row.mark(*k), val));
        } else {
            // (an object used as a key: the object, by name)
            let key = match an.heap.keys.identity_objs.borrow().get(k).copied() {
                Some(ko) => render_object(an, state, ko, VALUE_DEPTH.load(std::sync::atomic::Ordering::Relaxed), seen),
                None => key_display(*k),
            };
            items.push(format!("{}{}: {}", key, row.mark(*k), val));
        }
    }
    if keys.len() > MAX_ITEMS || private {
        items.push("…".into());
    }
    // (a dict whose keys only the program's stores under keys not known put there: what any
    // of them holds, in typing notation, `dict[?, int(?)]`)
    let stored_only = ns == Ns::Item && items.is_empty() && row.mutators.is_empty() && !row.top && !row.sources.is_empty() && row.sources.iter().all(|(src, _)| intern::with_source(src, |s| s.kind == crate::source::SourceKind::Data && s.site.starts_with("store@L")));
    if let (true, Some(v)) = (stored_only, state.slot_opt(o, ns, STAR)) {
        let shown = render_value(an, state, &v, depth + 1, seen);
        seen.pop();
        let key = state.keys_type(std::iter::once(o), ns).map_or("?".to_string(), |t| format!("{}(?)", t));
        return format!("dict[{}, {}]", key, shown);
    }
    if !row.closed() {
        items.push("**?".into());
    }
    seen.pop();
    match (ns, name) {
        (Ns::Attr, Some(n)) => format!("{}({})", n, items.join(", ")),
        (Ns::Attr, None) => format!("obj({})", items.join(", ")),
        _ => format!("{{{}}}", items.join(", ")),
    }
}

/// An exception's `args` as the arguments of a call: a tuple of known length by position,
/// else `*args`.
fn args_items(an: &Analyzer, state: &State, args: &Value, depth: u32, seen: &mut Vec<ObjId>) -> Vec<String> {
    let tuple = args.single().filter(|t| args.kv.is_bot() && an.heap.kind(*t) == Kind::Tuple);
    match tuple.and_then(|t| Some((t, an.heap.llen(t, state)?))) {
        Some((t, n)) if n <= 8 => (0..n)
            .map(|i| match state.slot_opt(t, Ns::Pos, intern::key_int(i as i64)) {
                Some(sv) => render_value(an, state, &sv, depth + 1, seen),
                None => "?".into(),
            })
            .collect(),
        _ => vec![format!("*{}", render_value(an, state, args, depth + 1, seen))],
    }
}

/// The `args` of `o` when it is an instance of an exception class of the program (its attribute
/// row has them).
fn exception_args(an: &Analyzer, state: &State, o: ObjId, ns: Ns, row: &crate::domain::Row) -> Option<Value> {
    let args = intern::key_str_lit("args");
    if ns != Ns::Attr || an.heap.kind(o) != Kind::Instance || !row.definite.contains(args) {
        return None;
    }
    let names = an.exc_class_names(an.heap.instance_class(o)?)?;
    if !names.0.iter().any(|n| crate::interp::exprs::is_exception_name(n)) {
        return None;
    }
    state.slot_opt(o, ns, args)
}

/// The symbols a value mentions (unknown objects printed by name) with what is known about
/// each: the keys / attributes seen on it, what an unknown call may return, else `?`.
pub fn value_legend(an: &Analyzer, state: &State, v: &Value) -> Vec<(String, String)> {
    fn walk(an: &Analyzer, state: &State, v: &Value, depth: u32, seen: &mut Vec<ObjId>, out: &mut Vec<(String, String)>) {
        if depth > VALUE_DEPTH.load(std::sync::atomic::Ordering::Relaxed) || out.len() >= 8 {
            return;
        }
        for o in v.pts.iter() {
            if seen.contains(&o) {
                continue;
            }
            seen.push(o);
            match an.heap.kind(o) {
                Kind::Unknown => {
                    let Some(sym) = an.names.visible(o) else { continue };
                    if out.iter().any(|(s, _)| s == sym) {
                        continue;
                    }
                    let mut parts: Vec<String> = Vec::new();
                    let item = state.row(o, Ns::Item);
                    if !item.present.is_empty() {
                        parts.push(render_record(an, state, o, Ns::Item, None, depth, &mut Vec::new()));
                    }
                    let attr = state.row(o, Ns::Attr);
                    if !attr.present.is_empty() {
                        parts.push(render_record(an, state, o, Ns::Attr, None, depth, &mut Vec::new()));
                    }
                    if let Some((_, _, given)) = an.names.origins.get(&o) {
                        parts.extend(given.iter().cloned());
                    }
                    // keys seen on it describe it; objects an unknown call was given are what it
                    // may also be (besides anything else)
                    let given = an.names.origins.get(&o).map(|(_, _, g)| !g.is_empty()).unwrap_or(false);
                    // (named by an expression, nothing more known of it than its type: `str(?)`; a
                    // parameter's type is the hover's own, `Fact` `type`)
                    let expr = !sym.chars().all(|c| c.is_alphanumeric() || c == '_');
                    let unknown = match an.heap.obj(o).ty {
                        Some(t) if parts.is_empty() && expr => format!("{}(?)", t),
                        _ => "?".to_string(),
                    };
                    let desc = if parts.is_empty() { unknown } else if given { format!("{} | ?", parts.join(" | ")) } else { parts.join(" | ") };
                    out.push((sym.clone(), desc));
                }
                // (what a container holds: under its keys, attributes, positions, elements)
                Kind::Dict | Kind::Frame | Kind::Namespace | Kind::Instance | Kind::List | Kind::Tuple | Kind::Set => {
                    for ns in [Ns::Item, Ns::Attr, Ns::Pos, Ns::Elem] {
                        for (_, sv) in state.obj_slots(o, ns) {
                            walk(an, state, &sv, depth + 1, seen, out);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(an, state, v, 0, &mut Vec::new(), &mut out);
    // in the order the value mentions them
    let text = render_value(an, state, v, 0, &mut Vec::new());
    out.sort_by_key(|(s0, _)| text.find(s0.as_str()).unwrap_or(usize::MAX));
    out
}

/// The members of set `o` of known length `n` when every position holds one literal, sorted.
fn set_members(state: &State, o: ObjId, n: usize) -> Option<Vec<KeyId>> {
    let mut ms = Vec::new();
    for i in 0..n {
        let sv = state.slot_opt(o, Ns::Pos, intern::key_int(i as i64))?;
        ms.push(sv.kv.as_lit().filter(|_| sv.pts.is_empty())?);
    }
    ms.sort_by(|a, b| crate::interp::compare::literal_order(*a, *b).unwrap_or_else(|| intern::key_repr(*a).cmp(&intern::key_repr(*b))));
    ms.dedup();
    Some(ms)
}

#[cfg(test)]
mod tests {
    #[test]
    fn join_alternatives_dedupes_top_level() {
        let t = |xs: &[&str]| super::join_alternatives(&xs.iter().map(|x| x.to_string()).collect::<Vec<_>>());
        assert_eq!(t(&["{\"a\": 1} | None", "None | {\"a\": 1}"]), "{\"a\": 1} | None");
        assert_eq!(t(&["{\"a\": 1 | 2}", "?", "3"]), "{\"a\": 1 | 2} | 3 | ?");
        assert_eq!(t(&["\"x | y\" | ?", "\"x | y\""]), "\"x | y\" | ?");
    }
}
