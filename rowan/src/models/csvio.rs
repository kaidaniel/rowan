//! `io.StringIO` (its text on each path) and the `csv` module on known texts: `csv.writer` /
//! `csv.DictWriter` format rows the way CPython does (delimiter, minimal quoting, `\r\n`,
//! `restval`, `extrasaction`), `csv.reader` / `csv.DictReader` parse a known text into rows
//! with known positions and keys. A text whose header a `DictWriter` wrote but whose rows are
//! unknown (`csvhdr:`) still gives `DictReader` rows with exactly those keys.

use indexmap::IndexMap;

use crate::domain::{join_values, KeyVal, Ns, ObjId, Row, Source, SourceKind, Value};
use crate::frontend::Node;
use crate::intern::{self, key_str_lit};
use crate::interp::{Analyzer, Call, Payload, ViewKind};
use crate::state::{Kind, State};

/// The slot holding a StringIO's text.
pub fn text_key() -> intern::KeyId {
    key_str_lit("\u{1}text")
}

/// The slot holding the header a DictWriter wrote at the start (`csvhdr:` label), kept while
/// only that writer's rows follow: it survives joins where the text itself becomes unknown.
pub fn hdr_key() -> intern::KeyId {
    key_str_lit("\u{1}csvhdr")
}

#[derive(Clone, Debug)]
pub struct CsvWriter {
    pub file: Value,
    pub delim: char,
    pub quote: char,
    pub term: String,
    /// a DictWriter: its field names (None: not known), restval, extrasaction="raise"
    pub dict: bool,
    pub fields: Option<Vec<String>>,
    pub restval: Value,
    pub raise_extras: bool,
}

/// A text as far as it is known.
#[derive(Clone, Debug, PartialEq)]
enum Text {
    Lit(String),
    /// a header line with these fields (delimiter), then rows of that many fields
    Hdr(char, Vec<String>),
    Unknown,
}

fn text_of_value(v: &Value) -> Text {
    if !v.pts.is_empty() {
        return Text::Unknown;
    }
    match &v.kv {
        KeyVal::Lit(k) => match intern::key_as_str(*k) {
            Some(s) => Text::Lit(s.to_string()),
            None => Text::Unknown,
        },
        KeyVal::Data(d) => {
            let t = intern::str_get(*d);
            match t.strip_prefix("csvhdr:") {
                Some(rest) => {
                    let mut it = rest.chars();
                    let delim = it.next().unwrap_or(',');
                    let fields: Vec<String> = it.as_str().split('\u{1f}').map(|x| x.to_string()).collect();
                    Text::Hdr(delim, fields)
                }
                None => Text::Unknown,
            }
        }
        _ => Text::Unknown,
    }
}

fn text_value(t: &Text, line: u32) -> Value {
    match t {
        Text::Lit(s) => Value::lit_str(s),
        Text::Hdr(d, fs) => Value::data(&format!("csvhdr:{}{}", d, fs.join("\u{1f}"))),
        Text::Unknown => Value::data(&format!("text@L{}", line)),
    }
}

fn is_stringio(an: &Analyzer, o: ObjId) -> bool {
    an.kind(o) == Kind::Namespace && matches!(&an.obj(o).payload, Payload::Method(q, _) if q == "stdobj.stringio")
}

/// A csv reader over a parameter's text: decided where the caller passes a known one.
fn text_event(an: &mut Analyzer, st: &State, f: &Value) {
    let mut ps = Vec::new();
    if let Some(p) = f.kv.as_param() {
        ps.push(p);
    }
    for o in f.pts.iter() {
        if is_stringio(an, o) {
            if let Some(p) = st.slot(o, Ns::Attr, text_key()).kv.as_param() {
                ps.push(p);
            }
        }
    }
    for p in ps {
        an.record_event("concrete", p);
    }
}

/// The text a value of StringIO objects / strings holds (joined over the objects).
fn text_in(an: &Analyzer, st: &State, f: &Value) -> Text {
    if f.pts.is_empty() {
        return text_of_value(f);
    }
    let mut out: Option<Text> = None;
    for o in f.pts.iter() {
        let t = if is_stringio(an, o) {
            match text_of_value(&st.slot(o, Ns::Attr, text_key())) {
                Text::Unknown => text_of_value(&st.slot(o, Ns::Attr, hdr_key())),
                t => t,
            }
        } else {
            Text::Unknown
        };
        out = Some(match out {
            None => t,
            Some(cur) if cur == t => cur,
            _ => Text::Unknown,
        });
    }
    out.unwrap_or(Text::Unknown)
}

/// Appending `piece` to `cur`.
fn append(cur: &Text, piece: &Text) -> Text {
    match (cur, piece) {
        (Text::Lit(a), Text::Lit(b)) => Text::Lit(format!("{}{}", a, b)),
        _ => Text::Unknown,
    }
}

/// `s` written into the StringIO objects of `f` (their text extended).
fn write_into(an: &mut Analyzer, st: &mut State, f: &Value, piece: &Text, line: u32, hdr_rows: bool) {
    let objs: Vec<ObjId> = f.pts.iter().filter(|o| is_stringio(an, *o)).collect();
    let strong = objs.len() == 1;
    for o in objs {
        let cur = text_of_value(&st.slot(o, Ns::Attr, text_key()));
        let next = append(&cur, piece);
        st.set_slot(o, Ns::Attr, text_key(), text_value(&next, line), strong);
        // (anything but a DictWriter's rows: the header fact no longer describes the text)
        if !hdr_rows {
            st.set_slot(o, Ns::Attr, hdr_key(), Value::data("nohdr"), strong);
        }
    }
}

/// A StringIO object read as a file by a library (pandas readers): its text.
pub fn unwrap_text(an: &Analyzer, st: &State, v: &Value) -> Value {
    if v.pts.is_empty() || !v.pts.iter().all(|o| is_stringio(an, o)) {
        return v.clone();
    }
    join_values(v.pts.iter().map(|o| st.slot(o, Ns::Attr, text_key())).collect::<Vec<_>>())
}

/// Unknown code may have written into these StringIO objects.
pub fn clobber(an: &mut Analyzer, st: &mut State, vals: &[&Value], line: u32) {
    for v in vals {
        for o in v.pts.iter() {
            if is_stringio(an, o) {
                st.set_slot(o, Ns::Attr, text_key(), Value::data(&format!("text@L{}", line)), true);
                st.set_slot(o, Ns::Attr, hdr_key(), Value::data("nohdr"), true);
            }
        }
    }
}

fn single_char(an: &Analyzer, c: &Call, name: &str, dflt: char) -> Option<char> {
    match c.kw(name) {
        None => Some(dflt),
        Some(v) => {
            let _ = an;
            let s = v.kv.as_lit().filter(|_| v.pts.is_empty()).and_then(intern::key_as_str)?;
            let mut it = s.chars();
            let ch = it.next()?;
            if it.next().is_some() {
                return None;
            }
            Some(ch)
        }
    }
}

/// A sequence of literal strings (`["a", "b"]`, `("a", "b")`).
fn str_seq(an: &mut Analyzer, st: &mut State, v: &Value, node: &Node) -> Option<Vec<String>> {
    let it = an.iterate(v, node, st);
    let items = it.items?;
    items.iter().map(|x| x.kv.as_lit().filter(|_| x.pts.is_empty()).and_then(intern::key_as_str).map(|s| s.to_string())).collect()
}

/// `str(v)` of a literal cell as csv writes it (None: "").
fn cell_text(v: &Value) -> Option<String> {
    if !v.pts.is_empty() {
        return None;
    }
    let k = v.kv.as_lit()?;
    if k == intern::key_none() {
        return Some(String::new());
    }
    match intern::key_get(k) {
        intern::Key::Str(s) => Some(s.to_string()),
        intern::Key::Int(_) | intern::Key::Float(_) | intern::Key::Bool(_) => Some(intern::key_py_str(k)),
        _ => None,
    }
}

/// One row as the excel dialect writes it (QUOTE_MINIMAL).
fn format_row(cells: &[String], w: &CsvWriter) -> String {
    let mut parts = Vec::new();
    for c in cells {
        let needs = c.contains(w.delim) || c.contains(w.quote) || c.contains('\r') || c.contains('\n') || (cells.len() == 1 && c.is_empty());
        if needs {
            let q = w.quote.to_string();
            parts.push(format!("{}{}{}", q, c.replace(&q, &format!("{}{}", q, q)), q));
        } else {
            parts.push(c.clone());
        }
    }
    format!("{}{}", parts.join(&w.delim.to_string()), w.term)
}

/// The excel dialect's parse of a whole text: rows of fields (an empty line is an empty row).
fn parse_text(text: &str, delim: char, quote: char) -> Option<Vec<Vec<String>>> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut in_q = false;
    let mut field_started = false;
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if in_q {
            if ch == quote {
                if i + 1 < chars.len() && chars[i + 1] == quote {
                    field.push(quote);
                    i += 2;
                    continue;
                }
                in_q = false;
            } else {
                field.push(ch);
            }
            i += 1;
            continue;
        }
        if ch == quote && !field_started {
            in_q = true;
            field_started = true;
        } else if ch == delim {
            row.push(std::mem::take(&mut field));
            field_started = false;
        } else if ch == '\r' || ch == '\n' {
            if ch == '\r' && i + 1 < chars.len() && chars[i + 1] == '\n' {
                i += 1;
            }
            if field_started || !row.is_empty() || !field.is_empty() {
                row.push(std::mem::take(&mut field));
            }
            rows.push(std::mem::take(&mut row));
            field_started = false;
        } else {
            field.push(ch);
            field_started = true;
        }
        i += 1;
    }
    if in_q {
        return None;
    }
    if field_started || !row.is_empty() || !field.is_empty() {
        row.push(field);
        rows.push(row);
    }
    Some(rows)
}

impl Analyzer {
    /// A list with these items at `node`, its own object per `ctx`.
    fn csv_list(&mut self, vals: Vec<Value>, node: &Node, st: &mut State, ctx: &str, iterator: bool) -> Value {
        let oid = self.alloc_at(Kind::List, node, st, &format!("list@L{}", node.line), Payload::None, None, Ns::Item, None, Some(ctx));
        let acc = join_values(vals.clone());
        let single = self.obj(oid).single;
        st.set_slot(oid, Ns::Elem, intern::STAR, Value::with_kv(acc.pts.clone(), crate::models::torch::elem_kv(&acc.kv)), single);
        st.elems.insert(oid, acc.kv.clone());
        if single {
            for (i, v) in vals.iter().enumerate() {
                st.set_slot(oid, Ns::Pos, intern::key_int(i as i64), v.clone(), true);
            }
            self.set_llen(oid, st, Some(vals.len()));
        } else {
            self.set_llen(oid, st, None);
        }
        if iterator {
            self.obj_mut(oid).iterator = true;
        }
        Value::obj(oid)
    }

    fn csv_str_list(&mut self, items: &[String], node: &Node, st: &mut State, ctx: &str) -> Value {
        let vals: Vec<Value> = items.iter().map(|x| Value::lit_str(x)).collect();
        self.csv_list(vals, node, st, ctx, false)
    }
}

pub fn stringio_new(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let init = c.arg(0, Some("initial_value")).unwrap_or_else(|| Value::lit_str(""));
    let oid = an.alloc_at(Kind::Namespace, &c.node, st, &format!("StringIO@L{}", c.line), Payload::Method("stdobj.stringio".into(), Value::new()), Some(Row::new()), Ns::Attr, None, None);
    an.obj_mut(oid).payload = Payload::Method("stdobj.stringio".into(), Value::new());
    // (the text as given: a literal, or a library's text label such as pandas' to_json)
    let t = if init.pts.is_empty() || init.kv.as_param().is_some() || init.pts.iter().all(|o| an.kind(o) == Kind::Ph) { init } else { Value::data(&format!("text@L{}", c.line)) };
    st.set_slot(oid, Ns::Attr, text_key(), t, true);
    Value::obj(oid)
}

/// `csv.writer(f, …)` / `csv.DictWriter(f, fieldnames, restval="", extrasaction="raise", …)`.
pub fn writer_new(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let dict = c.qual.ends_with("DictWriter");
    let file = c.arg(0, Some("f")).unwrap_or_default();
    let delim = single_char(an, c, "delimiter", ',');
    let quote = single_char(an, c, "quotechar", '"');
    let term = match c.kw("lineterminator") {
        None => Some("\r\n".to_string()),
        Some(v) => v.kv.as_lit().filter(|_| v.pts.is_empty()).and_then(intern::key_as_str).map(|s| s.to_string()),
    };
    // (other dialect options: the texts are not known)
    let plain = c.kwargs.keys().all(|k| matches!(&*intern::str_get(*k), "delimiter" | "quotechar" | "lineterminator" | "fieldnames" | "restval" | "extrasaction"));
    if let Some(p) = c.arg(1, Some("fieldnames")).and_then(|f| f.kv.as_param().or_else(|| f.single().filter(|o| an.kind(*o) == Kind::Ph))) {
        an.record_event("concrete", p);
    }
    let fields = if dict { c.arg(1, Some("fieldnames")).and_then(|f| str_seq(an, st, &f, &c.node)) } else { None };
    let restval = c.arg(2, Some("restval")).unwrap_or_else(|| Value::lit_str(""));
    let raise_extras = match c.arg(3, Some("extrasaction")).and_then(|v| v.kv.as_lit()).and_then(intern::key_as_str) {
        Some(s) => &*s != "ignore",
        None => true,
    };
    let oid = an.alloc_at(Kind::Namespace, &c.node, st, &format!("{}@L{}", if dict { "DictWriter" } else { "writer" }, c.line), Payload::Method("stdobj.csvwriter".into(), Value::new()), Some(Row::new()), Ns::Attr, None, None);
    an.obj_mut(oid).payload = Payload::Method("stdobj.csvwriter".into(), Value::new());
    let known = plain && delim.is_some() && quote.is_some() && term.is_some();
    an.csv_writers.insert(
        oid,
        CsvWriter { file: if known { file.clone() } else { Value::new() }, delim: delim.unwrap_or(','), quote: quote.unwrap_or('"'), term: term.unwrap_or_default(), dict, fields: if known { fields } else { None }, restval, raise_extras },
    );
    if !known {
        clobber(an, st, &[&file], c.line);
    }
    if dict {
        if let Some(fv) = c.arg(1, Some("fieldnames")) {
            let k = key_str_lit("fieldnames");
            st.set_slot(oid, Ns::Attr, k, fv, true);
            st.set_row(oid, Ns::Attr, Row::new().store(k, true));
        }
    }
    Value::obj(oid)
}

/// `stdobj.stringio.<m>` / `stdobj.csvwriter.<m>`.
pub fn m_stdobj(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let q = c.qual.as_str();
    if q.starts_with("stdobj.thread.") || q.starts_with("stdobj.executor.") || q.starts_with("stdobj.future.") {
        return crate::models::concurrency::m_conc(an, st, c);
    }
    if q.starts_with("stdobj.path.") {
        return crate::models::pathtext::m_path_method(an, st, c);
    }
    if q.starts_with("stdobj.repattern.") {
        return crate::models::remodel::m_re(an, st, c);
    }
    if q.starts_with("stdobj.rematch.") {
        return crate::models::remodel::m_match(an, st, c);
    }
    if q.starts_with("stdobj.template.") {
        return crate::models::stdfuncs::m_template(an, st, c);
    }
    if q.starts_with("stdobj.exitstack.") {
        let name = q.rsplit('.').next().unwrap_or("").to_string();
        let recv = c.recv.clone().unwrap_or_default();
        return m_exitstack(an, st, c, &name, &recv);
    }
    let name = q.rsplit('.').next().unwrap_or("");
    let recv = c.recv.clone().unwrap_or_default();
    if q.starts_with("stdobj.stringio.") {
        return match name {
            "getvalue" | "read" => {
                let mut outs = Vec::new();
                for o in recv.pts.iter() {
                    let t = st.slot(o, Ns::Attr, text_key());
                    // (an unknown text whose header a DictWriter wrote: that fact)
                    outs.push(match text_of_value(&t) {
                        Text::Unknown => match text_of_value(&st.slot(o, Ns::Attr, hdr_key())) {
                            h @ Text::Hdr(..) => text_value(&h, c.line),
                            _ => t,
                        },
                        _ => t,
                    });
                }
                if outs.is_empty() {
                    Value::data("text")
                } else {
                    join_values(outs)
                }
            }
            "write" => {
                let piece = c.args.first().map(text_of_value).unwrap_or(Text::Unknown);
                write_into(an, st, &recv, &piece, c.line, false);
                match &piece {
                    Text::Lit(s) => Value::lit(intern::key_int(s.chars().count() as i64)),
                    _ => Value::data("written"),
                }
            }
            "writelines" => {
                let parts = c.args.first().and_then(|a| str_seq(an, st, a, &c.node));
                let piece = match parts {
                    Some(p) => Text::Lit(p.concat()),
                    None => Text::Unknown,
                };
                write_into(an, st, &recv, &piece, c.line, false);
                an.none_v.clone()
            }
            "seek" | "tell" => Value::lit(intern::key_int(0)),
            "truncate" => {
                // (truncate(0) after seek(0): an empty text)
                for o in recv.pts.iter() {
                    st.set_slot(o, Ns::Attr, text_key(), Value::lit_str(""), recv.pts.len() == 1);
                }
                Value::lit(intern::key_int(0))
            }
            "close" | "flush" => an.none_v.clone(),
            "readlines" => {
                let t = text_in(an, st, &recv);
                match t {
                    Text::Lit(s) => {
                        let lines: Vec<String> = s.split_inclusive('\n').map(|x| x.to_string()).collect();
                        an.csv_str_list(&lines, &c.node, st, "lines")
                    }
                    _ => Value::data("lines"),
                }
            }
            _ => Value::data(&format!("StringIO.{}", name)),
        };
    }
    if q.starts_with("stdobj.csvwriter.") {
        let ws: Vec<(ObjId, CsvWriter)> = recv.pts.iter().filter_map(|o| an.csv_writers.get(&o).cloned().map(|w| (o, w))).collect();
        for (_, w) in &ws {
            match name {
                "writeheader" => {
                    if let Some(fs) = &w.fields {
                        let line = format_row(fs, w);
                        let file = w.file.clone();
                        // a header at the start of the text (default quoting, plain names)
                        let at_start: Vec<ObjId> = file.pts.iter().filter(|o| is_stringio(an, *o) && text_of_value(&st.slot(*o, Ns::Attr, text_key())) == Text::Lit(String::new())).collect();
                        write_into(an, st, &file, &Text::Lit(line), c.line, false);
                        let plain = w.term == "\r\n" && w.quote == '"' && fs.iter().all(|f| !f.contains(w.delim) && !f.contains('"') && !f.contains('\n') && !f.contains('\r'));
                        if plain {
                            for o in at_start {
                                st.set_slot(o, Ns::Attr, hdr_key(), text_value(&Text::Hdr(w.delim, fs.clone()), c.line), file.pts.len() == 1);
                            }
                        }
                    } else {
                        clobber(an, st, &[&w.file.clone()], c.line);
                    }
                }
                "writerow" | "writerows" => {
                    let rows: Vec<Value> = if name == "writerow" {
                        c.args.first().cloned().into_iter().collect()
                    } else {
                        let it = c.args.first().map(|a| an.iterate(a, &c.node, st));
                        match it.and_then(|i| i.items) {
                            Some(items) => items,
                            None => {
                                // rows of unknown count
                                let e = c.args.first().map(|a| an.iterate(a, &c.node, st).elem.one_or_empty()).unwrap_or_default();
                                vec![e, Value::data("rows")]
                            }
                        }
                    };
                    let file = w.file.clone();
                    for r in rows {
                        let piece = if w.dict { dict_row_text(an, st, &r, w, c) } else { list_row_text(an, st, &r, w, &c.node) };
                        // (a DictWriter's rows keep the header fact)
                        write_into(an, st, &file, &piece, c.line, w.dict && w.fields.is_some());
                    }
                }
                _ => {}
            }
        }
        if ws.is_empty() {
            return Value::data("csvwriter");
        }
        return an.none_v.clone();
    }
    Value::data(name)
}

/// The text of `writer.writerow(row)` for a row of literal cells.
fn list_row_text(an: &mut Analyzer, st: &mut State, r: &Value, w: &CsvWriter, node: &Node) -> Text {
    let it = an.iterate(r, node, st);
    let Some(items) = it.items else { return Text::Unknown };
    let cells: Option<Vec<String>> = items.iter().map(cell_text).collect();
    match cells {
        Some(cs) => Text::Lit(format_row(&cs, w)),
        None => Text::Unknown,
    }
}

/// The text of `DictWriter.writerow(d)`: its fields (restval for missing ones); keys outside the
/// fields raise ValueError with extrasaction="raise".
fn dict_row_text(an: &mut Analyzer, st: &mut State, r: &Value, w: &CsvWriter, _c: &Call) -> Text {
    let Some(fs) = &w.fields else {
        // (unknown field names: any row may have keys outside them)
        if w.raise_extras {
            an.may_raise_here("ValueError", st);
        }
        return Text::Unknown;
    };
    let recs: Vec<ObjId> = r.pts.iter().filter(|o| an.kind(*o) == Kind::Dict).collect();
    if recs.is_empty() || recs.len() != r.pts.len() {
        return Text::Unknown;
    }
    let fset: rustc_hash::FxHashSet<String> = fs.iter().cloned().collect();
    if w.raise_extras {
        let mut sure = recs.len() == 1;
        let mut maybe = false;
        let mut any_sure = false;
        for o in &recs {
            let row = st.row(*o, Ns::Item);
            let extra_def = row.definite.iter().any(|k| !intern::key_as_str(k).map(|s| fset.contains(&*s)).unwrap_or(false));
            let extra_maybe = row.present.iter().any(|k| !intern::key_as_str(k).map(|s| fset.contains(&*s)).unwrap_or(false)) || row.open();
            any_sure |= extra_def;
            sure &= extra_def;
            maybe |= extra_maybe;
        }
        if sure && any_sure {
            an.raise_here("ValueError", st);
            return Text::Unknown;
        }
        if maybe {
            an.may_raise_here("ValueError", st);
        }
    }
    if recs.len() != 1 {
        return Text::Unknown;
    }
    let o = recs[0];
    let row = st.row(o, Ns::Item);
    let mut cells = Vec::new();
    for f in fs {
        let k = key_str_lit(f);
        let v = if row.definite.contains(k) {
            st.slot(o, Ns::Item, k)
        } else if !row.present.contains(k) && row.closed() {
            w.restval.clone()
        } else {
            return Text::Unknown;
        };
        match cell_text(&v) {
            Some(t) => cells.push(t),
            None => return Text::Unknown,
        }
    }
    Text::Lit(format_row(&cells, w))
}

/// `csv.reader(f, delimiter=…)`: on a known text its rows (lists of str), in order.
pub fn reader_new(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let f = c.arg(0, Some("csvfile")).unwrap_or_default();
    text_event(an, st, &f);
    let delim = single_char(an, c, "delimiter", ',');
    let quote = single_char(an, c, "quotechar", '"');
    let plain = c.kwargs.keys().all(|k| matches!(&*intern::str_get(*k), "delimiter" | "quotechar"));
    if let (Text::Lit(text), Some(d), Some(q), true) = (text_in(an, st, &f), delim, quote, plain) {
        if let Some(rows) = parse_text(&text, d, q) {
            let vals: Vec<Value> = rows.iter().enumerate().map(|(i, r)| an.csv_str_list(r, &c.node, st, &format!("csvr{}", i))).collect();
            return an.csv_list(vals, &c.node, st, "csvrows", true);
        }
    }
    // unknown rows: lists of strings
    let e = Value::data(&format!("csv field@L{}", c.line));
    let row = an.csv_list(vec![], &c.node, st, "csvrow", false);
    if let Some(o) = row.single() {
        st.set_slot(o, Ns::Elem, intern::STAR, e.clone(), true);
        st.elems.insert(o, e.kv.clone());
        an.set_llen(o, st, None);
    }
    let lid = an.csv_list(vec![], &c.node, st, "csvrows", true);
    if let Some(o) = lid.single() {
        st.set_slot(o, Ns::Elem, intern::STAR, row.clone(), true);
        an.set_llen(o, st, None);
    }
    lid
}

/// `csv.DictReader(f, fieldnames=None, restkey=None, restval=None, …)`.
pub fn dictreader_new(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let f = c.arg(0, Some("f")).unwrap_or_default();
    text_event(an, st, &f);
    let fieldnames = c.arg(1, Some("fieldnames"));
    let restkey = c.arg(2, Some("restkey")).unwrap_or_else(|| an.none_v.clone());
    let restval = c.arg(3, Some("restval")).unwrap_or_else(|| an.none_v.clone());
    let delim = single_char(an, c, "delimiter", ',');
    let quote = single_char(an, c, "quotechar", '"');
    let plain = c.kwargs.keys().all(|k| matches!(&*intern::str_get(*k), "delimiter" | "quotechar" | "fieldnames" | "restkey" | "restval"));
    let given: Option<Vec<String>> = match &fieldnames {
        Some(fv) => str_seq(an, st, fv, &c.node),
        None => None,
    };
    let rk = restkey.kv.as_lit().filter(|_| restkey.pts.is_empty());
    let text = text_in(an, st, &f);
    if let (Some(d), Some(q), true, Some(rk)) = (delim, quote, plain, rk) {
        // a known text: each row a dict of its fields
        if let Text::Lit(text) = &text {
            if fieldnames.is_none() || given.is_some() {
                if let Some(rows) = parse_text(text, d, q) {
                    let mut rows: Vec<Vec<String>> = rows.into_iter().filter(|r| !r.is_empty()).collect();
                    let fields: Option<Vec<String>> = match &given {
                        Some(g) => Some(g.clone()),
                        None if rows.is_empty() => None,
                        None => Some(rows.remove(0)),
                    };
                    let mut vals: Vec<Value> = Vec::new();
                    if let Some(fields) = &fields {
                        for (i, r) in rows.iter().enumerate() {
                            let did = an.alloc_at(Kind::Dict, &c.node, st, &format!("DictReader row@L{}", c.line), Payload::None, Some(Row::new()), Ns::Item, None, Some(&format!("csvrow{}", i)));
                            let mut row = Row::new();
                            for (j, fname) in fields.iter().enumerate() {
                                let k = key_str_lit(fname);
                                let v = match r.get(j) {
                                    Some(x) => Value::lit_str(x),
                                    None => restval.clone(),
                                };
                                row = row.store(k, true);
                                st.set_slot(did, Ns::Item, k, v, true);
                            }
                            if r.len() > fields.len() {
                                let rest = an.csv_str_list(&r[fields.len()..].to_vec(), &c.node, st, &format!("csvrest{}", i));
                                row = row.store(rk, true);
                                st.set_slot(did, Ns::Item, rk, rest, true);
                            }
                            st.set_row(did, Ns::Item, row);
                            vals.push(Value::obj(did));
                        }
                    }
                    let lv = an.csv_list(vals, &c.node, st, "csvdrows", true);
                    if let Some(o) = lv.single() {
                        let fv = match &fields {
                            Some(fs) => an.csv_str_list(fs, &c.node, st, "csvfields"),
                            None => an.none_v.clone(),
                        };
                        an.csv_readers.insert(o, fv);
                    }
                    return lv;
                }
            }
        }
    }
    // rows of unknown count: the header's fields when a DictWriter wrote it, the field names
    // given (plus restkey for longer rows), else open
    let row = match (&text, &given) {
        (Text::Hdr(hd, fs), None) if Some(*hd) == delim && plain => Row::closed_with(intern::KeySet::from_iter(fs.iter().map(|x| key_str_lit(x)))),
        (_, Some(g)) => {
            let mut r = Row::closed_with(intern::KeySet::from_iter(g.iter().map(|x| key_str_lit(x))));
            match rk {
                Some(k) => r = r.store(k, false),
                None => r = r.data_store(Source::new(&format!("csv.DictReader@L{}", c.line), SourceKind::External)),
            }
            r
        }
        _ => Row::with_source(Source::new(&format!("csv.DictReader@L{}", c.line), SourceKind::External)),
    };
    let rid = an.alloc_at(Kind::Dict, &c.node, st, &format!("DictReader row@L{}", c.line), Payload::None, Some(row.clone()), Ns::Item, Some(false), None);
    an.obj_mut(rid).iter_elem = true;
    for k in row.present.iter() {
        let v = if Some(k) == rk { Value::data("csv rest") } else { Value::data(&format!("csv field@L{}", c.line)) };
        st.set_slot(rid, Ns::Item, k, v, false);
    }
    let ctx = an.frame().ctx.clone();
    let vid = an.alloc_raw(Kind::View, &format!("view:elems:{}", c.site), &ctx, true, "DictReader", Payload::View(ViewKind::Elems, Value::obj(rid)));
    an.obj_mut(vid).payload = Payload::View(ViewKind::Elems, Value::obj(rid));
    let fv = match (&text, &given, &fieldnames) {
        (_, Some(g), _) => an.csv_str_list(g, &c.node, st, "csvfields"),
        (Text::Hdr(_, fs), None, None) => an.csv_str_list(fs, &c.node, st, "csvfields"),
        (_, None, Some(fv)) => fv.clone(),
        _ => Value::data("fieldnames"),
    };
    an.csv_readers.insert(vid, fv);
    Value::obj(vid)
}

/// `print(*args, sep=" ", end="\n", file=f)` into a StringIO: its text extended (without
/// `file=`: where `redirect_stdout` sends it).
pub fn print_into(an: &mut Analyzer, st: &mut State, c: &Call) {
    let f = match c.kw("file") {
        Some(f) => f,
        None => match an.stdout_redirect.last() {
            Some(f) => f.clone(),
            None => return,
        },
    };
    let sep = c.kw("sep").map(|v| cell_text(&v).unwrap_or_else(|| "\u{0}".into())).unwrap_or_else(|| " ".into());
    let end = c.kw("end").map(|v| cell_text(&v).unwrap_or_else(|| "\u{0}".into())).unwrap_or_else(|| "\n".into());
    let parts: Option<Vec<String>> = c.args.iter().map(|a| if a.pts.is_empty() { a.kv.as_lit().map(intern::key_py_str) } else { None }).collect();
    let piece = match parts {
        Some(p) if !sep.contains('\u{0}') && !end.contains('\u{0}') && c.star_args.is_empty() => Text::Lit(format!("{}{}", p.join(&sep), end)),
        _ => Text::Unknown,
    };
    write_into(an, st, &f, &piece, c.line, false);
}

/// `contextlib.redirect_stdout(f)` / `redirect_stderr(f)`: a manager holding its target.
pub fn redirect_new(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let oid = an.alloc_at(Kind::Namespace, &c.node, st, "redirect", Payload::Method("stdobj.redirect".into(), Value::new()), Some(Row::new()), Ns::Attr, None, None);
    an.obj_mut(oid).payload = Payload::Method("stdobj.redirect".into(), Value::new());
    let target = c.arg(0, Some("new_target")).unwrap_or_default();
    st.set_slot(oid, Ns::Attr, key_str_lit("\u{1}target"), target, true);
    st.set_slot(oid, Ns::Attr, key_str_lit("\u{1}stderr"), Value::lit(intern::key_bool(c.qual.ends_with("stderr"))), true);
    Value::obj(oid)
}

/// The StringIO targets of the `redirect_stdout` managers among `managers`.
pub fn redirect_targets(an: &Analyzer, st: &State, managers: &[Value]) -> Vec<Value> {
    let mut out = Vec::new();
    for m in managers {
        for o in m.pts.iter() {
            if an.kind(o) == Kind::Namespace && matches!(&an.obj(o).payload, Payload::Method(q, _) if q == "stdobj.redirect") {
                let err = st.slot(o, Ns::Attr, key_str_lit("\u{1}stderr")).kv.as_lit() == Some(intern::key_bool(true));
                if !err {
                    out.push(st.slot(o, Ns::Attr, key_str_lit("\u{1}target")));
                }
            }
        }
    }
    out
}

/// `contextlib.closing(x)` (closes x at exit) / `contextlib.nullcontext(x=None)`: `as` gives x.
pub fn closing_new(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let kind = if c.qual.ends_with("closing") { "stdobj.closing" } else { "stdobj.nullcontext" };
    let oid = an.alloc_at(Kind::Namespace, &c.node, st, kind, Payload::Method(kind.into(), Value::new()), Some(Row::new()), Ns::Attr, None, None);
    an.obj_mut(oid).payload = Payload::Method(kind.into(), Value::new());
    let target = c.arg(0, Some("enter_result")).or_else(|| c.arg(0, Some("thing"))).unwrap_or_else(|| an.none_v.clone());
    st.set_slot(oid, Ns::Attr, key_str_lit("\u{1}target"), target, true);
    Value::obj(oid)
}

/// What `with m as x` binds for these managers (None: not one of them).
pub fn enter_value(an: &Analyzer, st: &State, v: &Value) -> Option<Value> {
    let mut outs = Vec::new();
    for o in v.pts.iter() {
        match &an.obj(o).payload {
            Payload::Method(q, _) if an.kind(o) == Kind::Namespace && matches!(q.as_str(), "stdobj.closing" | "stdobj.nullcontext" | "stdobj.redirect") => outs.push(st.slot(o, Ns::Attr, key_str_lit("\u{1}target"))),
            Payload::Method(q, _) if an.kind(o) == Kind::Namespace && q.starts_with("stdobj.") => outs.push(Value::obj(o)),
            _ => return None,
        }
    }
    if outs.is_empty() {
        None
    } else {
        Some(join_values(outs))
    }
}

/// `closing(x)`'s exit: `x.close()`.
pub fn closing_exit(an: &mut Analyzer, st: &mut State, v: &Value, node: &Node) {
    for o in v.pts.iter() {
        if an.kind(o) == Kind::Namespace && matches!(&an.obj(o).payload, Payload::Method(q, _) if q == "stdobj.closing") {
            let t = st.slot(o, Ns::Attr, key_str_lit("\u{1}target"));
            let m = an.load_attr(&t, "close", node, st, false);
            an.call_value(&m, node, None, st, Some(vec![]));
        }
    }
}

/// `contextlib.ExitStack()`.
pub fn exitstack_new(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let oid = an.alloc_at(Kind::Namespace, &c.node, st, "ExitStack", Payload::Method("stdobj.exitstack".into(), Value::new()), Some(Row::new()), Ns::Attr, None, None);
    an.obj_mut(oid).payload = Payload::Method("stdobj.exitstack".into(), Value::new());
    an.exit_callbacks.insert(oid, Vec::new());
    Value::obj(oid)
}

/// An ExitStack's exit: its callbacks, last registered first.
pub fn exitstack_close(an: &mut Analyzer, st: &mut State, v: &Value, node: &Node) {
    for o in v.pts.iter() {
        let Some(cbs) = an.exit_callbacks.get(&o).cloned() else { continue };
        an.exit_callbacks.insert(o, Vec::new());
        for (i, (f, args, kwargs)) in cbs.into_iter().enumerate().collect::<Vec<_>>().into_iter().rev() {
            let mut nd = *node;
            nd.col = node.col.wrapping_add(3_000_000 + i as u32);
            for fo in f.pts.iter() {
                if an.kind(fo) == Kind::Function {
                    an.call_user(fo, None, args.clone(), kwargs.clone(), vec![], vec![], st, &nd, None, false);
                } else {
                    an.call_value(&Value::obj(fo), &nd, None, st, Some(args.clone()));
                }
            }
        }
    }
}

fn m_exitstack(an: &mut Analyzer, st: &mut State, c: &Call, name: &str, recv: &Value) -> Value {
    match name {
        "callback" | "push" => {
            let Some(f) = c.args.first().cloned() else { return an.none_v.clone() };
            let args: Vec<Value> = if name == "push" { vec![an.none_v.clone(), an.none_v.clone(), an.none_v.clone()] } else { c.args.iter().skip(1).cloned().collect() };
            for o in recv.pts.iter() {
                an.exit_callbacks.entry(o).or_default().push((f.clone(), args.clone(), c.kwargs.clone()));
            }
            f
        }
        "enter_context" => {
            // (a @contextmanager generator ran to its end when called: its yielded value)
            let Some(cm) = c.args.first().cloned() else { return Value::data("context") };
            let mut outs = Vec::new();
            for o in cm.pts.iter() {
                match an.kind(o) {
                    Kind::List if an.obj(o).iterator => {
                        let it = an.iterate(&Value::obj(o), &c.node, st);
                        outs.push(match it.items {
                            Some(items) if !items.is_empty() => items[0].clone(),
                            _ => it.elem.one_or_empty(),
                        });
                    }
                    Kind::Instance => {
                        let v = an.call_method_if_defined(&Value::obj(o), "__enter__", &c.node, st).unwrap_or_else(|| Value::obj(o));
                        // its __exit__ at the stack's exit
                        let none = an.none_v.clone();
                        let ex = an.load_attr(&Value::obj(o), "__exit__", &c.node, st, false);
                        for s in recv.pts.iter() {
                            an.exit_callbacks.entry(s).or_default().push((ex.clone(), vec![none.clone(), none.clone(), none.clone()], IndexMap::new()));
                        }
                        outs.push(v);
                    }
                    _ => outs.push(Value::obj(o)),
                }
            }
            join_values(outs)
        }
        "close" => {
            exitstack_close(an, st, recv, &c.node);
            an.none_v.clone()
        }
        "__enter__" => recv.clone(),
        _ => Value::data(name),
    }
}

pub fn register(r: &mut dyn FnMut(&[&'static str], crate::models::Model)) {
    r(&["io.StringIO"], stringio_new);
    r(&["contextlib.redirect_stdout", "contextlib.redirect_stderr"], redirect_new);
    r(&["contextlib.ExitStack"], exitstack_new);
    r(&["contextlib.closing", "contextlib.nullcontext"], closing_new);
    r(&["csv.writer", "csv.DictWriter"], writer_new);
    r(&["csv.reader"], reader_new);
    r(&["csv.DictReader"], dictreader_new);
    r(&["stdobj.*"], m_stdobj);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn excel() -> CsvWriter {
        CsvWriter { file: Value::new(), delim: ',', quote: '"', term: "\r\n".into(), dict: false, fields: None, restval: Value::new(), raise_extras: true }
    }

    fn strs(rows: &[&[&str]]) -> Vec<Vec<String>> {
        rows.iter().map(|r| r.iter().map(|x| x.to_string()).collect()).collect()
    }

    // expected values from CPython's csv module
    #[test]
    fn writes_like_cpython() {
        let w = excel();
        let rows = strs(&[&["a", "b"], &["x,y", "q\"r", ""], &[""]]);
        let text: String = rows.iter().map(|r| format_row(r, &w)).collect();
        assert_eq!(text, "a,b\r\n\"x,y\",\"q\"\"r\",\r\n\"\"\r\n");
    }

    #[test]
    fn reads_like_cpython() {
        assert_eq!(parse_text("p|q\n1|2\n", '|', '"').unwrap(), strs(&[&["p", "q"], &["1", "2"]]));
        assert_eq!(parse_text("a,\"b,c\",\"\"\n", ',', '"').unwrap(), strs(&[&["a", "b,c", ""]]));
        assert_eq!(parse_text("a\n\nb\n", ',', '"').unwrap(), strs(&[&["a"], &[], &["b"]]));
        assert_eq!(parse_text("a,b\r\nc", ',', '"').unwrap(), strs(&[&["a", "b"], &["c"]]));
    }
}
