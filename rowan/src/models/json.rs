//! JSON: `json.dumps` text exactly as CPython writes it for known data, `json.loads` of known
//! text (and an open record of unknown text), and HTTP bodies through it (`requests`,
//! responses).

use super::*;

pub(super) fn json_tag(q: &str) -> String {
    format!("{}.{}", q.split('.').next().unwrap_or(""), last(q))
}

/// `json.loads("…")` of a literal: the parsed value (dicts with exactly their keys).
pub(super) fn json_literal(an: &mut Analyzer, st: &mut State, c: &Call, j: &serde_json::Value, path: &str) -> Value {
    let hook = c.kw("object_hook");
    json_literal_h(an, st, c, j, path, hook.as_ref())
}

/// (`object_hook=f`: each object, innermost first, is replaced by `f(dict)`)
pub(super) fn json_literal_h(an: &mut Analyzer, st: &mut State, c: &Call, j: &serde_json::Value, path: &str, hook: Option<&Value>) -> Value {
    let site = format!("{}#json{}", crate::interp::site_string(&c.node), path);
    match j {
        serde_json::Value::Null => Value::none(),
        serde_json::Value::Bool(b) => Value::lit(intern::key_bool(*b)),
        serde_json::Value::Number(n) => match n.as_i64() {
            Some(i) => Value::lit(intern::key_int(i)),
            None => Value::lit(intern::key_float(n.as_f64().unwrap_or(0.0))),
        },
        serde_json::Value::String(t) => Value::lit(key_str_lit(t)),
        serde_json::Value::Array(items) => {
            let vals: Vec<Value> = items.iter().enumerate().map(|(i, x)| json_literal_h(an, st, c, x, &format!("{}.{}", path, i), hook)).collect();
            let oid = an.alloc(Kind::List, &site, st, &format!("list@L{}", c.line), Payload::None, None, Ns::Item, None, None);
            let single = an.heap.obj(oid).single;
            let acc = join_values(vals.clone());
            st.set_slot(oid, Ns::Elem, STAR, Value::with_kv(acc.pts.clone(), crate::models::torch::elem_kv(&acc.kv)), single);
            st.elems.insert(oid, acc.kv.clone());
            if single {
                an.heap.set_llen(oid, st, Some(vals.len()));
                for (i, v) in vals.iter().enumerate() {
                    st.set_slot(oid, Ns::Pos, intern::key_int(i as i64), v.clone(), true);
                }
            }
            Value::obj(oid)
        }
        serde_json::Value::Object(m) => {
            let oid = an.alloc(Kind::Dict, &site, st, &format!("dict@L{}", c.line), Payload::None, Some(Row::new()), Ns::Item, None, None);
            let strong = an.heap.obj(oid).single;
            for (k, x) in m.iter() {
                let v = json_literal_h(an, st, c, x, &format!("{}.{}", path, k), hook);
                an.store_key(oid, Ns::Item, &Value::lit(key_str_lit(k)), &v, strong, &c.node, st);
            }
            match hook {
                Some(h) => {
                    // (a call context per object: the hook's allocations stay distinct)
                    let tag = path.bytes().fold(7u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32)) % 1_000_000;
                    let nd = c.node.sub(1_000_000 + tag);
                    an.call_value(h, &nd, None, st, Some(vec![Value::obj(oid)]))
                }
                _ => Value::obj(oid),
            }
        }
    }
}

/// `json.loads(text)` (pydantic's `model_validate_json` reads its text the same way).
/// `json.dumps(x)` / `pickle.dumps(x)`: a text standing for a snapshot of x when x is plain
/// data whose round trip keeps its structure (json: dicts with str keys and lists, no tuples
/// or instances; pickle: any dicts / lists / tuples), so `loads` can give its structure back.
pub(super) fn m_json_dumps(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fallback = Value::data(&format!("L{}", c.line));
    let Some(x) = c.args.first() else { return fallback };
    let json = c.qual == "json.dumps";
    // a parameter's value: known at the call site
    // (placeholders inside the containers too: `{"meta": meta, …}`)
    // fully known data: the exact text
    if json {
        if let Some(opts) = JsonOpts::of(c) {
            if let Some(t) = json_text(an, st, x, &opts, 0) {
                return Value::lit_str(&t);
            }
        }
    }
    if x.pts.is_empty() || !x.kv.is_bot() {
        return fallback;
    }
    // (plain data only, a bounded walk)
    let mut work: Vec<u32> = x.pts.iter().collect();
    let mut seen: rustc_hash::FxHashSet<u32> = Default::default();
    while let Some(o) = work.pop() {
        if !seen.insert(o) || seen.len() > 200 {
            if seen.len() > 200 {
                return fallback;
            }
            continue;
        }
        match an.heap.kind(o) {
            Kind::Dict if !pandas::is_series_obj(an, o) => {
                // (json: int / float / bool / None keys become strings)
                let r = st.row(o, Ns::Item);
                if json && !r.present.iter().all(|k| intern::key_is_str(k) || json_key_text(k).is_some()) {
                    return fallback;
                }
                for (_, v) in st.obj_slots(o, Ns::Item) {
                    work.extend(v.pts.iter());
                }
            }
            Kind::List => {
                for (_, v) in st.obj_slots(o, Ns::Pos).into_iter().chain(st.obj_slots(o, Ns::Elem)) {
                    work.extend(v.pts.iter());
                }
            }
            Kind::Tuple if !json => {
                for (_, v) in st.obj_slots(o, Ns::Pos).into_iter().chain(st.obj_slots(o, Ns::Elem)) {
                    work.extend(v.pts.iter());
                }
            }
            // (data of unknown structure inside: kept as it is)
            Kind::Unknown if json => {}
            _ => return fallback,
        }
    }
    let snaps: Vec<String> = x.pts.iter().map(|o| {
        let mut seen = IndexMap::new();
        let n = an.copy_object(o, &c.node, st, true, &mut seen);
        if json {
            for (_, copy) in seen.iter() {
                json_str_keys(an, st, *copy);
            }
            json_str_keys(an, st, n);
        }
        n.to_string()
    }).collect();
    Value::data(&format!("{}:{}", if json { "pyjsontext" } else { "pickletext" }, snaps.join(",")))
}

/// The value a FastAPI client reads back from a handler's (already dumped) result: a copy with
/// the JSON text's keys (`1` → "1"), Enum members as their values, dates as ISO text and, when
/// `decimal_float` (jsonable_encoder of plain data), Decimals as floats.
pub fn json_wire(an: &mut Analyzer, st: &mut State, node: &crate::frontend::Node, v: &Value, decimal_float: bool) -> Value {
    let lit = |k: KeyId| -> KeyId {
        if let Some((kind, _, tz)) = crate::models::chrono::as_chrono(k) {
            if kind != crate::models::chrono::DELTA && kind != crate::models::chrono::TZ {
                if let Some(t) = crate::models::chrono::isoformat(k, "T", "auto") {
                    let _ = tz;
                    return key_str_lit(&t);
                }
            }
        }
        if decimal_float && crate::models::pynum::is_num_key(k) {
            if let Some(f) = crate::models::pynum::str_of(k).and_then(|t| t.parse::<f64>().ok()) {
                return intern::key_float(f);
            }
        }
        k
    };
    // (`orig`: a copy's original, for Enum members the deep copy duplicated)
    let conv = |an: &Analyzer, st: &State, x: &Value, orig: &rustc_hash::FxHashMap<u32, u32>| -> Value {
        let mut kv = x.kv.clone();
        if kv.is_literal() {
            kv = KeyVal::lits(KeySet::from_iter(crate::domain::kv_literals(&kv).iter().map(lit)));
        }
        let mut out = Value::kv_only(kv);
        for o in x.pts.iter() {
            let m = orig.get(&o).copied().unwrap_or(o);
            if an.heap.keys.member_key.contains_key(&m) {
                out = out.join(&st.slot(m, Ns::Attr, key_str_lit("value")));
            } else {
                out = out.join(&Value::obj(o));
            }
        }
        out
    };
    let top = conv(an, st, v, &Default::default());
    let mut out = Value::kv_only(top.kv.clone());
    for o in top.pts.iter() {
        if !matches!(an.heap.kind(o), Kind::Dict | Kind::List | Kind::Tuple) {
            out = out.join(&Value::obj(o));
            continue;
        }
        let mut seen = IndexMap::new();
        let n = an.copy_object(o, node, st, true, &mut seen);
        let orig: rustc_hash::FxHashMap<u32, u32> = seen.iter().map(|(a, b)| (*b, *a)).collect();
        let mut copies: Vec<u32> = seen.values().copied().collect();
        copies.push(n);
        for c in copies {
            if orig.get(&c).map(|m| an.heap.keys.member_key.contains_key(m)).unwrap_or(false) {
                continue;
            }
            for ns in [Ns::Item, Ns::Pos, Ns::Elem] {
                for (k, x) in st.obj_slots(c, ns) {
                    let y = conv(an, st, &x, &orig);
                    if y != x {
                        st.set_slot(c, ns, k, y, true);
                    }
                }
            }
            if let Some(e) = st.elems.get(&c) {
                if e.is_literal() {
                    st.elems.insert(c, KeyVal::lits(KeySet::from_iter(crate::domain::kv_literals(&e).iter().map(lit))));
                }
            }
            json_str_keys(an, st, c);
        }
        out = out.join(&Value::obj(n));
    }
    out
}

/// A json object key's text for a non-str literal key (`1` → "1", True → "true").
pub(super) fn json_key_text(k: KeyId) -> Option<String> {
    if k == intern::key_none() {
        return Some("null".into());
    }
    match intern::key_get(k) {
        intern::Key::Bool(b) => Some(if b { "true".into() } else { "false".into() }),
        intern::Key::Int(i) => Some(i.to_string()),
        intern::Key::Float(_) => Some(intern::key_py_str(k)),
        _ => None,
    }
}

/// A copied dict's keys as json makes them (strings).
pub(super) fn json_str_keys(an: &mut Analyzer, st: &mut State, o: u32) {
    if an.heap.kind(o) != Kind::Dict {
        return;
    }
    let row = st.row(o, Ns::Item);
    if row.present.iter().all(intern::key_is_str) {
        return;
    }
    let keys: Vec<KeyId> = row.ordered_keys().unwrap_or_else(|| row.present.iter().collect());
    let mut nr = Row::new();
    for k in keys {
        let v = st.slot(o, Ns::Item, k);
        let nk = match json_key_text(k) {
            Some(t) => key_str_lit(&t),
            None => k,
        };
        if nk != k {
            st.pop_slot(o, Ns::Item, k);
        }
        nr = nr.store(nk, row.definite.contains(k));
        st.set_slot(o, Ns::Item, nk, v, true);
    }
    if row.open() {
        nr = nr.data_store(Source::new("json keys", SourceKind::Data));
    }
    st.set_row(o, Ns::Item, nr);
}

/// `json.dumps` options that decide its text.
pub(super) struct JsonOpts {
    sort_keys: bool,
    indent: Option<String>,
    item_sep: String,
    key_sep: String,
    ensure_ascii: bool,
}

impl JsonOpts {
    fn of(c: &Call) -> Option<JsonOpts> {
        let lit = |name: &str| c.kw(name).map(|v| v.kv.as_lit().filter(|_| v.pts.is_empty()));
        let flag = |name: &str, dflt: bool| -> Option<bool> {
            match lit(name) {
                None => Some(dflt),
                Some(Some(k)) => intern::with_key(k, |kk| if let intern::Key::Bool(b) = kk { Some(*b) } else { None }),
                Some(None) => None,
            }
        };
        if c.kwargs.keys().any(|k| !matches!(&*intern::str_get(*k), "sort_keys" | "indent" | "separators" | "ensure_ascii")) || c.args.len() > 1 {
            return None;
        }
        let indent = match lit("indent") {
            None => None,
            Some(Some(k)) if k == intern::key_none() => None,
            Some(Some(k)) => match intern::key_get(k) {
                intern::Key::Int(i) if i >= 0 => Some(" ".repeat(i as usize)),
                intern::Key::Str(s) => Some(s.to_string()),
                _ => return None,
            },
            Some(None) => return None,
        };
        let (item_sep, key_sep) = match c.kw("separators") {
            None => (if indent.is_some() { ",".to_string() } else { ", ".to_string() }, ": ".to_string()),
            Some(v) => {
                let k = v.kv.as_lit().filter(|_| v.pts.is_empty())?;
                match intern::key_get(k) {
                    intern::Key::Tuple(ks) if ks.len() == 2 => (intern::key_as_str(ks[0])?.to_string(), intern::key_as_str(ks[1])?.to_string()),
                    _ => return None,
                }
            }
        };
        Some(JsonOpts { sort_keys: flag("sort_keys", false)?, indent, item_sep, key_sep, ensure_ascii: flag("ensure_ascii", true)? })
    }
}

pub(super) fn json_str(s: &str, ascii: bool) -> String {
    let mut out = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c if ascii && (c as u32) > 0x7e => {
                let mut buf = [0u16; 2];
                for u in c.encode_utf16(&mut buf) {
                    out.push_str(&format!("\\u{:04x}", u));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The JSON text httpx sends for a request's `json=` body (compact separators, no ASCII
/// escaping), when the value is known.
pub fn json_body_text(an: &Analyzer, st: &State, v: &Value) -> Option<String> {
    let o = JsonOpts { sort_keys: false, indent: None, item_sep: ",".into(), key_sep: ":".into(), ensure_ascii: false };
    json_text(an, st, v, &o, 0)
}

/// The exact text `json.dumps` makes of fully known data (None: anything unknown in it).
pub(super) fn json_text(an: &Analyzer, st: &State, v: &Value, o: &JsonOpts, depth: usize) -> Option<String> {
    if depth > 32 {
        return None;
    }
    let nl = |d: usize| -> String { match &o.indent { Some(ind) => format!("\n{}", ind.repeat(d)), None => String::new() } };
    if v.pts.is_empty() {
        let k = v.kv.as_lit()?;
        if k == intern::key_none() {
            return Some("null".into());
        }
        return match intern::key_get(k) {
            intern::Key::Bool(b) => Some(if b { "true".into() } else { "false".into() }),
            intern::Key::Int(i) => Some(i.to_string()),
            intern::Key::Float(bits) => {
                let f = f64::from_bits(bits);
                Some(if f.is_nan() { "NaN".into() } else if f.is_infinite() { if f > 0.0 { "Infinity".into() } else { "-Infinity".into() } } else { intern::key_py_str(k) })
            }
            intern::Key::Str(s) => Some(json_str(&s, o.ensure_ascii)),
            intern::Key::Tuple(ks) => {
                let parts: Option<Vec<String>> = ks.iter().map(|k| json_text(an, st, &Value::lit(*k), o, depth + 1)).collect();
                let parts = parts?;
                if parts.is_empty() {
                    return Some("[]".into());
                }
                Some(format!("[{}{}{}]", nl(depth + 1), parts.join(&format!("{}{}", o.item_sep, nl(depth + 1))), nl(depth)))
            }
            _ => None,
        };
    }
    if !v.kv.is_bot() || v.pts.len() != 1 {
        return None;
    }
    let x = v.pts.first()?;
    match an.heap.kind(x) {
        Kind::Dict if !pandas::is_series_obj(an, x) => {
            let r = st.row(x, Ns::Item);
            if !r.closed() || r.present != r.definite {
                return None;
            }
            let mut keys: Vec<KeyId> = r.ordered_keys()?;
            let ktext = |k: KeyId| -> Option<String> { intern::key_as_str(k).map(|s| s.to_string()).or_else(|| json_key_text(k)) };
            if o.sort_keys {
                if !keys.iter().all(|k| intern::key_is_str(*k)) {
                    return None;
                }
                keys.sort_by_key(|k| intern::key_as_str(*k).map(|s| s.to_string()).unwrap_or_default());
            }
            if keys.is_empty() {
                return Some("{}".into());
            }
            let mut parts = Vec::new();
            for k in keys {
                let vt = json_text(an, st, &st.slot(x, Ns::Item, k), o, depth + 1)?;
                parts.push(format!("{}{}{}", json_str(&ktext(k)?, o.ensure_ascii), o.key_sep, vt));
            }
            Some(format!("{{{}{}{}}}", nl(depth + 1), parts.join(&format!("{}{}", o.item_sep, nl(depth + 1))), nl(depth)))
        }
        Kind::List | Kind::Tuple => {
            let n = an.heap.llen(x, st)?;
            let mut parts = Vec::new();
            for i in 0..n {
                parts.push(json_text(an, st, &st.slot(x, Ns::Pos, intern::key_int(i as i64)), o, depth + 1)?);
            }
            if parts.is_empty() {
                return Some("[]".into());
            }
            Some(format!("[{}{}{}]", nl(depth + 1), parts.join(&format!("{}{}", o.item_sep, nl(depth + 1))), nl(depth)))
        }
        _ => None,
    }
}

/// Compact JSON text (pydantic's `model_dump_json`: no spaces, non-ASCII kept) of a value
/// with known contents.
pub fn compact_json_text(an: &Analyzer, st: &State, v: &Value) -> Option<String> {
    let o = JsonOpts { sort_keys: false, indent: None, item_sep: ",".into(), key_sep: ":".into(), ensure_ascii: false };
    json_text(an, st, v, &o, 0)
}

/// The snapshots a `json.dumps` / `pickle.dumps` text stands for.
pub(super) fn dumped_snapshots(v: &Value, json: bool) -> Vec<u32> {
    let mut out = Vec::new();
    if let (true, KeyVal::Data(d, _)) = (v.pts.is_empty(), &v.kv) {
        let t = intern::str_get(*d);
        if let Some(rest) = t.strip_prefix(if json { "pyjsontext:" } else { "pickletext:" }) {
            out.extend(rest.split(',').filter_map(|x| x.parse::<u32>().ok()));
        }
    }
    out
}

pub fn json_loads(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    m_json_load(an, st, c)
}

/// `TypeAdapter` methods: `typeadapter.<idx>.<method>`.
pub(super) fn m_typeadapter(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let mut it = c.qual.splitn(3, '.');
    let (_, idx, method) = (it.next(), it.next().and_then(|x| x.parse::<usize>().ok()), it.next().unwrap_or(""));
    match idx {
        Some(i) => pydantic::adapter_call(an, st, i, method, c),
        None => Value::data("TypeAdapter"),
    }
}

pub(super) fn m_json_load(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    // the text of a dumps(): a copy of what was dumped
    if matches!(c.qual.as_str(), "json.loads" | "pickle.loads") {
        let snaps = c.args.first().map(|a| dumped_snapshots(a, c.qual == "json.loads")).unwrap_or_default();
        // (only while the snapshot is part of this state: not a text made inside a summary)
        let here = |o: u32| st.has_row(o, Ns::Item) || st.lens.get(&o).is_some();
        if !snaps.is_empty() && snaps.iter().all(|o| here(*o)) {
            return join_values(snaps.iter().map(|o| Value::obj(an.copy_object(*o, &c.node, st, true, &mut IndexMap::new()))).collect::<Vec<_>>());
        }
    }
    if c.qual == "json.loads" {
        // a parameter's text: decided where the caller passes a literal one
        // the text of a model's model_dump_json(): its dump
        let ms = c.args.first().map(pydantic::json_models).unwrap_or_default();
        if !ms.is_empty() {
            return pydantic::json_of_models(an, st, c, &ms);
        }
        // the text of a frame's to_json: its structure
        let texts = c.args.first().map(pandas::json_texts).unwrap_or_default();
        if !texts.is_empty() {
            let mut out = Value::new();
            for (f, orient) in texts {
                out = out.join(&pandas::json_of_frame(an, st, c, f, &orient));
            }
            return out;
        }
        // (json.loads reports its own raises: a text that does not parse raises here)
        // (a str, or bytes / bytearray in UTF-8)
        // (bytes in UTF-8, a BOM dropped as `utf-8-sig` does; NUL bytes may mean UTF-16 / 32)
        let lit_text = |k: KeyId| -> Option<String> {
            intern::key_as_str(k).map(|s| s.to_string()).or_else(|| intern::with_key(k, |kk| if let intern::Key::Bytes(b) = kk { if b.contains(&0) { None } else { String::from_utf8(b.strip_prefix(b"\xef\xbb\xbf".as_slice()).unwrap_or(b).to_vec()).ok() } } else { None }))
        };
        if let Some(text) = c.args.first().filter(|a| a.pts.is_empty()).and_then(|a| a.kv.as_lit()).and_then(lit_text) {
            // (where CPython's decoder fails: the JSONDecodeError with its msg / pos / lineno /
            // colno; a text it accepts that serde does not, NaN / Infinity, stays unknown)
            match (serde_json::from_str::<serde_json::Value>(&text), crate::models::pytext::py_json_error(&text)) {
                (Ok(j), None) => return json_literal(an, st, c, &j, ""),
                (_, Some((msg, pos))) => {
                    let (lineno, colno, full) = crate::models::pytext::py_json_error_parts(&text, &msg, pos);
                    let e = crate::models::exceptions::exception_object(an, "JSONDecodeError", vec![Value::lit(intern::key_str_lit(&full))], &c.node, st);
                    if let Some(o) = e.single() {
                        let strong = an.heap.obj(o).single;
                        for (name, v) in [("msg", Value::lit(intern::key_str_lit(&msg))), ("doc", Value::lit(intern::key_str_lit(&text))), ("pos", Value::lit(intern::key_int(pos as i64))), ("lineno", Value::lit(intern::key_int(lineno))), ("colno", Value::lit(intern::key_int(colno)))] {
                            an.store_key(o, Ns::Attr, &Value::lit(intern::key_str_lit(name)), &v, strong, &c.node, st);
                        }
                    }
                    an.raise_exc("JSONDecodeError", Some(e), None, true, st);
                    return Value::new();
                }
                _ => {}
            }
        }
        // one of several literal texts: the join of their structures
        if let Some(KeyVal::Lits(ks)) = c.args.first().filter(|a| a.pts.is_empty()).map(|a| a.kv.clone()) {
            let parsed: Option<Vec<serde_json::Value>> = ks.iter().map(|k| intern::key_as_str(k).and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())).collect();
            if let Some(js) = parsed.filter(|v| v.len() <= 16) {
                let outs: Vec<Value> = js.iter().enumerate().map(|(i, j)| json_literal(an, st, c, j, &format!("#{}", i))).collect();
                return join_values(outs);
            }
        }
        an.may_raise_here("JSONDecodeError", st);
    }
    let tag = json_tag(&c.qual);
    let src = Source::new(&format!("{}@L{}", tag, c.line), SourceKind::External);
    let oid = an.alloc_at(Kind::Unknown, &c.node, st, &format!("{}@L{}", tag, c.line), Payload::None, Some(Row::with_source(src)), Ns::Item, None, None);
    st.set_row(oid, Ns::Attr, Row::with_source(src));
    let text = crate::frontend::text_at(&c.node);
    if !text.is_empty() {
        let scope = an.frame().func.unwrap_or(0);
        an.set_symbol(oid, text.split_whitespace().collect::<Vec<_>>().join(" "), scope);
    }
    Value::obj(oid)
}

pub(super) fn m_http(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let tag = format!("{}@L{}", json_tag(&c.qual), c.line);
    let oid = an.alloc_at(Kind::Response, &c.node, st, &tag, Payload::Str(tag.clone()), None, Ns::Item, None, None);
    an.heap.obj_mut(oid).payload = Payload::Str(tag);
    Value::obj(oid)
}

pub(super) fn m_response_method(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = last(&c.qual);
    if name == "json" || name == "read" {
        let mut outs = Vec::new();
        let recv = c.recv.clone().unwrap_or_default();
        for x in recv.pts.iter() {
            let tag = an.heap.external_qual(x);
            let src = Source::new(&tag, SourceKind::External);
            let oid = an.alloc_at(Kind::Unknown, &c.node, st, &format!("{}.json()", tag), Payload::None, Some(Row::with_source(src)), Ns::Item, None, None);
            st.set_row(oid, Ns::Attr, Row::with_source(src));
            let text = crate::frontend::text_at(&c.node);
            if !text.is_empty() {
                let scope = an.frame().func.unwrap_or(0);
        an.set_symbol(oid, text.split_whitespace().collect::<Vec<_>>().join(" "), scope);
            }
            outs.push(Value::obj(oid));
        }
        return if outs.is_empty() { Value::data("json") } else { join_values(outs) };
    }
    Value::data(&format!("L{}", c.line))
}
