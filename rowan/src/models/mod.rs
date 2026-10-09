//! Library models (SPECIFICATION.md §11); port of `rowan/models.py`.  A model is
//! `fn(&mut Analyzer, &mut State, &Call) -> Value`; models are keyed by qualified name with
//! prefix fallbacks.

pub mod pandas;

use indexmap::IndexMap;
use rustc_hash::FxHashMap;

use crate::domain::{join_values, kv_join, FnKind, KeyVal, Ns, RenameFn, Row, RowData, Source, SourceKind, Value, DATA};
use crate::interp::access::append_slot_key;
use crate::interp::funcs::Elem;
use crate::interp::{Analyzer, Call, Payload, ViewKind};
use crate::intern::{KeyId, self, key_str_lit, s, KeySet, STAR};
use crate::state::{Kind, State};

pub type Model = fn(&mut Analyzer, &mut State, &Call) -> Value;

pub const ALLOWLIST_PREFIXES: &[&str] = &[
    "logging.", "print", "warnings.", "time.", "datetime.", "re.", "math.", "random.", "os.", "shutil.", "glob.", "subprocess.", "io.",
    "tempfile.", "csv.writer", "csv.reader", "sys.", "traceback.", "inspect.", "importlib.", "platform.", "socket.", "ssl.", "signal.",
    "threading.", "multiprocessing.", "concurrent.", "asyncio.", "queue.", "heapq.", "bisect.", "array.", "zlib.", "gzip.", "bz2.", "lzma.",
    "zipfile.", "tarfile.", "hmac.", "secrets.", "binascii.", "codecs.", "unicodedata.", "locale.", "gettext.", "calendar.", "difflib.",
    "pprint.pp", "textwrap.", "typing.", "pathlib.", "uuid.", "hashlib.", "base64.", "string.", "itertools.count", "numpy.", "np.", "pd.api",
    "pytest.", "unittest.", "abc.", "enum.", "decimal.", "fractions.", "statistics.", "operator.", "struct.", "builtins.NotImplementedError",
];

const PREFIXES: &[&str] = &[
    "Series.", "str.", "DataFrame.", "list.", "set.", "tuple.", "dict.", "view.", "groupby.", "zip.", "enumerate.", "unknown.", "class.", "pair.",
    "parser.", "partial.", "response.",
];

pub struct Registry {
    table: FxHashMap<&'static str, Model>,
}

impl Registry {
    pub fn new() -> Registry {
        let mut t: FxHashMap<&'static str, Model> = FxHashMap::default();
        let mut r = |names: &[&'static str], m: Model| {
            for n in names {
                t.insert(n, m);
            }
        };
        r(&["builtins.dict"], m_dict);
        r(&["builtins.dict.fromkeys"], m_fromkeys);
        r(&["builtins.list", "builtins.tuple", "builtins.set", "builtins.sorted", "builtins.reversed", "builtins.frozenset", "builtins.iter"], m_list);
        r(&["builtins.zip"], m_zip);
        r(&["builtins.enumerate"], m_enumerate);
        r(&["builtins.next"], m_next);
        r(&["builtins.getattr"], m_getattr);
        r(&["builtins.setattr"], m_setattr);
        r(&["builtins.vars", "dataclasses.asdict", "attr.asdict", "attrs.asdict"], m_vars);
        r(&["builtins.map", "builtins.filter"], m_map);
        r(&["builtins.str"], m_str);
        r(
            &[
                "builtins.int", "builtins.float", "builtins.print", "builtins.repr", 
                "builtins.type", "builtins.id", "builtins.hash", "builtins.abs", "builtins.round",
                "builtins.any", "builtins.all", "builtins.format", "builtins.input", "builtins.callable", "builtins.issubclass", "builtins.hex",
                "builtins.chr", "builtins.ord", "builtins.range", "builtins.open", "builtins.hasattr", "builtins.divmod", "builtins.pow",
                "builtins.bytes", "builtins.object",
            ],
            noeffect,
        );
        r(&["builtins.super"], m_super);
        r(&["builtins.bool"], m_bool);
        r(&["builtins.len"], m_len);
        r(&["builtins.sum"], m_sum);
        r(
            &["builtins.str.upper", "builtins.str.lower", "builtins.str.strip", "builtins.str.title", "builtins.str.capitalize", "builtins.str.casefold", "builtins.str.lstrip", "builtins.str.rstrip"],
            m_str_fn,
        );
        r(&["str.*"], m_str_method);
        r(&["dict.get"], m_get);
        r(&["dict.setdefault"], m_setdefault);
        r(&["dict.update"], m_update);
        r(&["dict.pop"], m_pop);
        r(&["builtins.max", "builtins.min"], m_minmax);
        r(&["builtins.isinstance"], m_isinstance);
        r(&["dict.popitem"], m_popitem);
        r(&["dict.clear"], m_clear);
        r(&["dict.keys", "dict.values", "dict.items"], m_views);
        r(&["dict.copy"], m_dcopy);
        r(&["dict.*", "unknown.*", "class.*"], m_unknown_method);
        r(&["list.append", "set.add", "list.insert", "list.extend", "set.update"], m_append);
        r(&["list.pop", "set.pop"], m_lpop);
        r(&["list.*", "set.*", "tuple.*", "zip.*", "enumerate.*", "pair.*"], m_list_other);
        r(&["view.*"], m_view);
        r(
            &[
                "json.load", "json.loads", "yaml.safe_load", "yaml.load", "yaml.full_load", "tomllib.load", "tomllib.loads", "toml.load", "toml.loads",
                "pickle.load", "pickle.loads", "orjson.loads", "ujson.loads", "simplejson.loads", "simplejson.load",
            ],
            m_json_load,
        );
        r(
            &[
                "requests.get", "requests.post", "requests.put", "requests.patch", "requests.delete", "requests.request", "requests.Session.get",
                "requests.Session.post", "httpx.get", "httpx.post", "httpx.Client.get", "httpx.Client.post", "urllib.request.urlopen",
            ],
            m_http,
        );
        r(&["response.*"], m_response_method);
        r(&["json.dump", "json.dumps", "yaml.dump", "yaml.safe_dump", "pickle.dump", "pickle.dumps", "pprint.pprint", "pprint.pformat", "orjson.dumps"], noeffect);
        r(&["copy.copy", "copy.deepcopy"], m_copy);
        r(&["dataclasses.replace"], m_dc_replace);
        r(&["dataclasses.field", "typing.cast", "typing.TypeVar", "typing.NewType"], m_passthrough);
        r(&["collections.defaultdict", "collections.Counter", "collections.OrderedDict", "collections.ChainMap"], m_defaultdict);
        r(&["csv.DictReader"], m_dictreader);
        r(&["csv.DictWriter"], noeffect);
        r(&["argparse.ArgumentParser"], m_argparser);
        r(&["parser.*"], m_parser_method);
        r(&["functools.partial"], m_partial);
        r(&["functools.reduce"], noeffect);
        r(&["itertools.chain", "itertools.chain.from_iterable"], m_chain);
        r(&["partial.*"], m_partial_call);
        r(&["logging.getLogger", "logging.getLogger.getChild", "structlog.get_logger", "loguru.logger.bind"], m_get_logger);
        r(&["os.getenv", "os.environ.get"], m_getenv);
        pandas::install(&mut r);
        Registry { table: t }
    }

    pub fn lookup(&self, qual: &str) -> Option<Model> {
        if let Some(m) = self.table.get(qual) {
            return Some(*m);
        }
        if qual.starts_with("pandas.DataFrame.") || qual.starts_with("pd.DataFrame.") {
            let q = format!("DataFrame.{}", qual.rsplit('.').next().unwrap_or(""));
            if let Some(m) = self.table.get(q.as_str()) {
                return Some(*m);
            }
        }
        for prefix in PREFIXES {
            if qual.starts_with(prefix) {
                let star = format!("{}*", prefix);
                if let Some(m) = self.table.get(star.as_str()) {
                    return Some(*m);
                }
            }
        }
        for prefix in ALLOWLIST_PREFIXES {
            if qual.starts_with(prefix) || qual.starts_with(&format!("builtins.{}", prefix)) {
                return Some(noeffect);
            }
        }
        if qual.starts_with("builtins.") {
            return Some(noeffect);
        }
        None
    }
}

impl Default for Registry {
    fn default() -> Self {
        Registry::new()
    }
}

pub fn noeffect(_an: &mut Analyzer, _st: &mut State, c: &Call) -> Value {
    Value::data(&format!("L{}", c.line))
}

fn last(q: &str) -> &str {
    q.rsplit('.').next().unwrap_or(q)
}

// ------------------------------------------------------------- externals that are records
pub fn external_record(an: &mut Analyzer, qual: &str, state: &mut State) -> Option<u32> {
    if qual == "os.environ" || qual == "os.environb" {
        let oid = an.alloc_raw(Kind::Dict, &format!("ext-record:{}", qual), "", true, qual, Payload::None);
        if !state.has_row(oid, Ns::Item) {
            state.set_row(oid, Ns::Item, Row::with_source(Source::new(qual, SourceKind::External)));
        }
        return Some(oid);
    }
    None
}

// -------------------------------------------------------------------------- builtins
fn m_dict(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let oid = an.alloc_simple(Kind::Dict, &c.node, st, &format!("dict@L{}", c.line));
    if let Some(src) = c.args.first() {
        let recs = an.record_objects(src);
        if !recs.is_empty() {
            an.copy_into(oid, Ns::Item, src, st, true, &KeySet::new());
            if let Some(s1) = src.single() {
                if an.is_entry_object(s1) {
                    an.obj_mut(oid).copied_from = Some(s1);
                }
            }
        } else {
            for x in src.pts.iter() {
                match an.kind(x) {
                    Kind::Zip => {
                        let Payload::Zip(ks, vs) = an.obj(x).payload.clone() else { continue };
                        let lits = an.lit_keys(&ks, st);
                        let vel = an.elem_of(&vs, &c.node, st);
                        if let Some(lits) = lits {
                            let mut row = st.row(oid, Ns::Item);
                            for k in lits.iter() {
                                row = row.store(k, true);
                                st.set_slot(oid, Ns::Item, k, Value::with_kv(vel.pts.clone(), DATA), false);
                            }
                            st.set_row(oid, Ns::Item, row);
                        } else {
                            let kel = an.elem_of(&ks, &c.node, st);
                            an.store_key(oid, Ns::Item, &kel.kv, &vel, false, &c.node, st);
                        }
                    }
                    Kind::List | Kind::Tuple | Kind::Set | Kind::View => {
                        let it = an.iterate(&Value::obj(x), &c.node, st);
                        match (&it.elem, &it.items) {
                            (Elem::Pair(k, v), _) => an.store_key(oid, Ns::Item, &k.kv, v, false, &c.node, st),
                            (_, Some(items)) => {
                                for item in items {
                                    let parts = an.unpack(item, 2, st, &c.node);
                                    an.store_key(oid, Ns::Item, &parts[0].kv, &parts[1], true, &c.node, st);
                                }
                            }
                            _ => {
                                let r = st.row(oid, Ns::Item).data_store(Source::new(&format!("dict@L{}", c.line), SourceKind::Data));
                                st.set_row(oid, Ns::Item, r);
                            }
                        }
                    }
                    _ => {
                        let r = st.row(oid, Ns::Item).data_store(Source::new(&format!("dict@L{}", c.line), SourceKind::Data));
                        st.set_row(oid, Ns::Item, r);
                    }
                }
            }
            if src.pts.is_empty() {
                let r = st.row(oid, Ns::Item).data_store(Source::new(&format!("dict@L{}", c.line), SourceKind::Data));
                st.set_row(oid, Ns::Item, r);
            }
        }
    }
    for (k, v) in c.kwargs.iter() {
        an.store_key(oid, Ns::Item, &KeyVal::Lit(intern::key(intern::Key::Str(intern::str_get(*k)))), v, true, &c.node, st);
    }
    for sk in &c.star_kwargs {
        an.copy_into(oid, Ns::Item, sk, st, true, &KeySet::new());
    }
    Value::obj(oid)
}

fn m_fromkeys(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let oid = an.alloc_simple(Kind::Dict, &c.node, st, &format!("dict.fromkeys@L{}", c.line));
    let ks = c.args.first().and_then(|a| an.lit_keys(a, st));
    let val = c.args.get(1).cloned().unwrap_or_else(|| an.none_v.clone());
    if let Some(ks) = ks {
        let mut row = st.row(oid, Ns::Item);
        for k in ks.iter() {
            row = row.store(k, true);
            st.set_slot(oid, Ns::Item, k, val.clone(), true);
        }
        st.set_row(oid, Ns::Item, row);
    } else if let Some(a) = c.args.first() {
        let el = an.elem_of(a, &c.node, st);
        an.store_key(oid, Ns::Item, &el.kv, &val, false, &c.node, st);
    }
    Value::obj(oid)
}

fn m_list(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let kind = match c.qual.as_str() {
        "builtins.tuple" => Kind::Tuple,
        "builtins.set" | "builtins.frozenset" => Kind::Set,
        _ => Kind::List,
    };
    let oid = an.alloc_simple(kind, &c.node, st, &format!("{}@L{}", last(&c.qual), c.line));
    an.obj_mut(oid).literal_len = None;
    if let Some(a) = c.args.first() {
        let it = an.iterate(a, &c.node, st);
        match &it.elem {
            Elem::Pair(k, v) => {
                let ctx = an.frame().ctx.clone();
                let pid = an.alloc_raw(Kind::Pair, &format!("pair:{}", c.site), &ctx, true, "pair", Payload::Pair(k.clone(), v.clone()));
                st.set_slot(oid, Ns::Elem, STAR, Value::obj(pid), true);
                st.elems.insert(oid, DATA);
            }
            Elem::One(elem) => {
                st.set_slot(oid, Ns::Elem, STAR, Value::with_kv(elem.pts.clone(), DATA), true);
                st.elems.insert(oid, elem.kv.clone());
                let reorders = matches!(c.qual.as_str(), "builtins.sorted" | "builtins.reversed");
                if reorders {
                    if let Some(keyfn) = c.kw("key") {
                        if !keyfn.pts.is_empty() {
                            an.call_value(&keyfn, &c.node, None, st, Some(vec![elem.clone()]));
                        }
                    }
                }
                if let Some(items) = &it.items {
                    let reversed = c.qual.as_str() == "builtins.reversed";
                    let sorted = c.qual.as_str() == "builtins.sorted";
                    if an.obj(oid).single && items.len() <= 64 && kind != Kind::Set {
                        an.obj_mut(oid).literal_len = Some(items.len());
                        let n = items.len();
                        // sorted: when every sort key is a known literal (or a tuple of
                        // them) the order is known (stable sort); otherwise the count is
                        // known and every position may hold any item
                        let order: Option<Vec<usize>> = if sorted { literal_sort_order(an, st, c, items) } else { None };
                        let all = join_values(items.clone());
                        for (i, item) in items.iter().enumerate() {
                            let pos = if reversed { n - 1 - i } else { i };
                            let v = if sorted {
                                match &order {
                                    Some(ord) => items[ord[i]].clone(),
                                    None => all.clone(),
                                }
                            } else {
                                item.clone()
                            };
                            st.set_slot(oid, Ns::Pos, intern::key_int(pos as i64), v, true);
                        }
                    }
                }
            }
        }
    } else {
        st.elems.insert(oid, KeyVal::Lits(KeySet::new()));
        an.obj_mut(oid).literal_len = Some(0);
    }
    Value::obj(oid)
}

/// `isinstance(x, C)`: decided when every object of `x` has a known kind / class; a
/// placeholder records an event so the call site decides.
fn m_isinstance(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let (Some(v), Some(cv)) = (c.args.first(), c.args.get(1)) else { return Value::data("bool") };
    if crate::interp::disabled("isinst") {
        return Value::data("bool");
    }
    let mut classes: Vec<u32> = Vec::new();
    for o in cv.pts.iter() {
        match an.kind(o) {
            Kind::Class | Kind::External => classes.push(o),
            Kind::Tuple => {
                for (_, cs) in st.slots_of(o) {
                    classes.extend(cs.pts.iter().filter(|x| matches!(an.kind(*x), Kind::Class | Kind::External)));
                }
            }
            _ => return Value::data("bool"),
        }
    }
    if classes.is_empty() {
        return Value::data("bool");
    }
    let mut verdicts: Vec<bool> = Vec::new();
    let builtin = |an: &Analyzer, cls: u32| -> Option<String> { if an.kind(cls) == Kind::External { Some(an.external_qual(cls).rsplit('.').next().unwrap_or("").to_string()) } else { None } };
    // literal values (no objects)
    if v.pts.is_empty() {
        let lits = crate::domain::kv_literals(&v.kv);
        if !v.kv.is_literal() || lits.is_empty() {
            if let Some(ph) = v.kv.as_param() {
                an.record_event("isinstance", ph);
            }
            return Value::data("bool");
        }
        for k in lits.iter() {
            let kind = intern::with_key(k, |kk| match kk {
                intern::Key::Str(_) => "str",
                intern::Key::Bool(_) => "bool",
                intern::Key::Int(_) => "int",
                intern::Key::Float(_) => "float",
                intern::Key::None => "NoneType",
                intern::Key::Tuple(_) => "tuple",
                intern::Key::Bytes(_) => "bytes",
                _ => "?",
            });
            let hit = classes.iter().any(|cls| match builtin(an, *cls) {
                Some(b) => b == kind || (b == "int" && kind == "bool") || (b == "object"),
                None => false,
            });
            verdicts.push(hit);
        }
    }
    for o in v.pts.iter() {
        let k = an.kind(o);
        if matches!(k, Kind::Ph | Kind::Unknown) {
            if k == Kind::Ph {
                an.record_event("isinstance", o);
            }
            return Value::data("bool");
        }
        let hit = classes.iter().any(|cls| match builtin(an, *cls) {
            Some(b) => match b.as_str() {
                "dict" | "defaultdict" | "OrderedDict" | "Counter" => matches!(k, Kind::Dict),
                "list" => k == Kind::List,
                "tuple" => k == Kind::Tuple,
                "set" | "frozenset" => k == Kind::Set,
                "object" => true,
                _ => false,
            },
            None => k == Kind::Instance && an.instance_class(o).map(|ic| an.is_subclass(ic, *cls)).unwrap_or(false),
        });
        verdicts.push(hit);
    }
    if verdicts.iter().all(|b| *b) {
        Value::lit(intern::key_bool(true))
    } else if verdicts.iter().all(|b| !*b) {
        Value::lit(intern::key_bool(false))
    } else {
        Value::data("bool")
    }
}

/// `max(xs, key=f)` / `min(xs)`: an element of the container (the key function runs on it);
/// with several arguments, any of them.
fn m_minmax(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    if c.args.len() == 1 {
        let it = an.iterate(&c.args[0], &c.node, st);
        let elem = it.elem.one_or_empty();
        if let Some(kf) = c.kw("key") {
            if !kf.pts.is_empty() {
                an.call_value(&kf, &c.node, None, st, Some(vec![elem.clone()]));
            }
        }
        return elem;
    }
    join_values(c.args.clone())
}

fn m_zip(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let a = c.args.first().cloned().unwrap_or_default();
    let b = c.args.get(1).cloned().unwrap_or_default();
    let oid = an.alloc_at(Kind::Zip, &c.node, st, &format!("zip@L{}", c.line), Payload::Zip(a.clone(), b.clone()), None, Ns::Item, None, None);
    an.obj_mut(oid).payload = Payload::Zip(a, b);
    Value::obj(oid)
}

fn m_enumerate(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let a = c.args.first().cloned().unwrap_or_default();
    let oid = an.alloc_at(Kind::Enumerate, &c.node, st, &format!("enumerate@L{}", c.line), Payload::Enumerate(a.clone()), None, Ns::Item, None, None);
    an.obj_mut(oid).payload = Payload::Enumerate(a);
    Value::obj(oid)
}

fn m_next(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(a) = c.args.first() else { return Value::data("next") };
    let it = an.iterate(a, &c.node, st);
    match it.elem {
        Elem::Pair(k, v) => {
            let ctx = an.frame().ctx.clone();
            let pid = an.alloc_raw(Kind::Pair, &format!("pair:{}", c.site), &ctx, true, "pair", Payload::Pair(k, v));
            Value::obj(pid)
        }
        Elem::One(elem) => match it.items {
            Some(items) if !items.is_empty() => items[0].clone(),
            _ => elem,
        },
    }
}

fn m_getattr(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    if c.args.len() >= 2 {
        if let Some(k) = c.args[1].kv.as_lit() {
            if let Some(name) = intern::key_as_str(k) {
                return an.load_attr(&c.args[0], &name, &c.node, st, c.args.len() == 2);
            }
        }
    }
    if let Some(a) = c.args.first() {
        return join_values(a.pts.iter().map(|o| an.all_values(o, Ns::Attr, st)));
    }
    Value::data("getattr")
}

fn m_setattr(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    if c.args.len() >= 3 {
        match c.args[1].kv.as_lit().and_then(intern::key_as_str) {
            Some(name) => an.store_attr(&c.args[0], &name, &c.args[2], &c.node, st),
            None => {
                for o in an.record_objects(&c.args[0]) {
                    an.store_key(o, Ns::Attr, &c.args[1].kv, &c.args[2], false, &c.node, st);
                }
            }
        }
    }
    an.none_v.clone()
}

fn m_vars(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(a) = c.args.first() else { return Value::data("vars") };
    let oid = an.alloc_simple(Kind::Dict, &c.node, st, &format!("vars@L{}", c.line));
    for x in an.record_objects(a) {
        let r = st.row(oid, Ns::Item).copy_from(&st.row(x, Ns::Attr), true, &KeySet::new(), intern::EMPTY_LENS);
        st.set_row(oid, Ns::Item, r);
        for (k, v) in st.obj_slots(x, Ns::Attr) {
            st.set_slot(oid, Ns::Item, k, v, false);
        }
    }
    Value::obj(oid)
}

fn m_map(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let oid = an.alloc_simple(Kind::List, &c.node, st, &format!("map@L{}", c.line));
    an.obj_mut(oid).literal_len = None;
    if c.args.len() >= 2 {
        let it = an.iterate(&c.args[1], &c.node, st);
        if !c.qual.ends_with("filter") && an.obj(oid).single {
            if let Some(items) = &it.items {
                if items.len() <= crate::interp::UNROLL_MAX {
                    // known items: the function per item, positions kept
                    let mut outs = Vec::new();
                    for (i, item) in items.iter().enumerate() {
                        an.frame_mut().unroll.push(i);
                        let r = an.call_value(&c.args[0], &c.node, None, st, Some(vec![item.clone()]));
                        an.frame_mut().unroll.pop();
                        st.set_slot(oid, Ns::Pos, intern::key_int(i as i64), r.clone(), true);
                        outs.push(r);
                    }
                    let res = join_values(outs);
                    st.set_slot(oid, Ns::Elem, STAR, Value::with_kv(res.pts.clone(), DATA), true);
                    st.elems.insert(oid, res.kv);
                    an.obj_mut(oid).literal_len = Some(items.len());
                    return Value::obj(oid);
                }
            }
        }
        let elem = it.elem.one_or_empty();
        let res = if c.qual.ends_with("filter") { elem } else { an.call_value(&c.args[0], &c.node, None, st, Some(vec![elem])) };
        st.set_slot(oid, Ns::Elem, STAR, Value::with_kv(res.pts.clone(), DATA), true);
        st.elems.insert(oid, res.kv);
    }
    Value::obj(oid)
}

fn m_str(an: &mut Analyzer, _st: &mut State, c: &Call) -> Value {
    if let Some(a) = c.args.first() {
        return Value::kv_only(an.rename_kv(&a.kv, &RenameFn::new(FnKind::Str)));
    }
    Value::lit_str("")
}

fn m_super(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(foid) = an.frame().func else { return Value::data("super") };
    let fi = an.func_info(foid);
    let Some(cls) = fi.cls else { return Value::data("super") };
    let mut self_v = st.env.get(&s("self")).cloned().unwrap_or_default();
    let cells = an.frame().cells.clone();
    for (name, cid) in cells.iter() {
        if &*intern::str_get(*name) == "self" {
            self_v = st.slot(*cid, Ns::Cell, key_str_lit("v"));
        }
    }
    if !c.args.is_empty() {
        if c.args.len() > 1 {
            self_v = c.args[1].clone();
        }
    }
    let ctx = an.frame().ctx.clone();
    let oid = an.alloc_raw(Kind::Super, &format!("super:{}:{}", foid, c.site), &ctx, true, "super()", Payload::Super(cls, self_v.clone()));
    an.obj_mut(oid).payload = Payload::Super(cls, self_v);
    Value::obj(oid)
}

fn m_str_fn(an: &mut Analyzer, _st: &mut State, c: &Call) -> Value {
    let fname = last(&c.qual);
    if let Some(a) = c.args.first() {
        let f = FnKind::from_name(fname).filter(|f| matches!(f, FnKind::Upper | FnKind::Lower | FnKind::Strip | FnKind::Title)).unwrap_or(FnKind::Strip);
        return Value::kv_only(an.rename_kv(&a.kv, &RenameFn::new(f)));
    }
    Value::data("str")
}

fn m_str_method(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = last(&c.qual);
    let kv = c.recv.as_ref().map(|r| r.kv.clone()).unwrap_or(DATA);
    match name {
        "upper" | "lower" | "strip" | "title" => Value::kv_only(an.rename_kv(&kv, &RenameFn::new(FnKind::from_name(name).unwrap()))),
        "lstrip" | "rstrip" => Value::kv_only(an.rename_kv(&kv, &RenameFn::new(FnKind::Strip))),
        "capitalize" | "casefold" | "swapcase" => Value::data("str"),
        "partition" | "rpartition" if c.args.len() == 1 && kv.as_lit().and_then(intern::key_as_str).is_some() && c.args[0].kv.as_lit().and_then(intern::key_as_str).is_some() => {
            let s = intern::key_as_str(kv.as_lit().unwrap()).unwrap();
            let sep = intern::key_as_str(c.args[0].kv.as_lit().unwrap()).unwrap();
            let parts: [String; 3] = if name == "partition" {
                match s.split_once(&*sep) {
                    Some((a, b)) => [a.to_string(), sep.to_string(), b.to_string()],
                    None => [s.to_string(), String::new(), String::new()],
                }
            } else {
                match s.rsplit_once(&*sep) {
                    Some((a, b)) => [a.to_string(), sep.to_string(), b.to_string()],
                    None => [String::new(), String::new(), s.to_string()],
                }
            };
            let oid = an.alloc_simple(Kind::Tuple, &c.node, st, &format!("tuple@L{}", c.line));
            let vals: Vec<Value> = parts.iter().map(|x| Value::lit(key_str_lit(x))).collect();
            for (i, v) in vals.iter().enumerate() {
                st.set_slot(oid, Ns::Pos, intern::key_int(i as i64), v.clone(), true);
            }
            st.set_slot(oid, Ns::Elem, STAR, Value::new(), true);
            st.elems.insert(oid, join_values(vals.clone()).kv);
            an.obj_mut(oid).literal_len = Some(3);
            Value::obj(oid)
        }
        "replace" if c.args.len() >= 2 && c.args[0].kv.as_lit().is_some() && c.args[1].kv.as_lit().is_some() => {
            let arg = intern::key_tuple(vec![c.args[0].kv.as_lit().unwrap(), c.args[1].kv.as_lit().unwrap()]);
            Value::kv_only(an.rename_kv(&kv, &RenameFn::with_arg(FnKind::Replace, arg)))
        }
        "format" => {
            if let Some(k) = kv.as_lit() {
                if let Some(sk) = intern::key_as_str(k) {
                    if sk.matches("{}").count() == 1 && c.args.len() == 1 {
                        let mut parts = sk.splitn(2, "{}");
                        let pre = parts.next().unwrap_or("").to_string();
                        let post = parts.next().unwrap_or("").to_string();
                        let mut out = c.args[0].kv.clone();
                        if let Some(ok) = out.as_lit() {
                            if !intern::key_is_str(ok) {
                                out = KeyVal::Lit(key_str_lit(&intern::key_py_str(ok)));
                            }
                        }
                        if !pre.is_empty() {
                            out = an.rename_kv(&out, &RenameFn::with_arg(FnKind::Prefix, key_str_lit(&pre)));
                        }
                        if !post.is_empty() {
                            out = an.rename_kv(&out, &RenameFn::with_arg(FnKind::Suffix, key_str_lit(&post)));
                        }
                        return Value::kv_only(out);
                    }
                }
            }
            Value::data(&format!("L{}", c.line))
        }
        "join" => Value::data(&format!("L{}", c.line)),
        "split" | "splitlines" | "partition" | "rsplit" => {
            let oid = an.alloc_simple(Kind::List, &c.node, st, &format!("split@L{}", c.line));
            an.obj_mut(oid).literal_len = None;
            // a literal string split by a literal separator: the parts, in order; a parameter
            // string is decided per call site
            if let Some(ph) = kv.as_param() {
                an.record_event("narrow", ph);
            }
            let sep: Option<Option<String>> = match c.args.first() {
                None => Some(None),
                Some(a) => a.kv.as_lit().and_then(intern::key_as_str).map(|s| Some(s.to_string())),
            };
            if let (Some(k), Some(sep), true) = (kv.as_lit(), sep, name == "split" && an.obj(oid).single && !crate::interp::disabled("split")) {
                if let Some(text) = intern::key_as_str(k) {
                    let parts: Vec<String> = match sep {
                        Some(sep) if !sep.is_empty() => text.split(sep.as_str()).map(|p| p.to_string()).collect(),
                        _ => text.split_whitespace().map(|p| p.to_string()).collect(),
                    };
                    if parts.len() <= 16 && c.args.len() <= 1 {
                        let mut ks = KeySet::new();
                        for (i, p) in parts.iter().enumerate() {
                            let v = Value::lit(key_str_lit(p));
                            st.set_slot(oid, Ns::Pos, intern::key_int(i as i64), v, true);
                            ks = ks.insert(key_str_lit(p));
                        }
                        an.obj_mut(oid).literal_len = Some(parts.len());
                        st.set_slot(oid, Ns::Elem, STAR, Value::new(), true);
                        st.elems.insert(oid, KeyVal::Lits(ks));
                        return Value::obj(oid);
                    }
                }
            }
            st.set_slot(oid, Ns::Elem, STAR, Value::new(), true);
            st.elems.insert(oid, DATA);
            Value::obj(oid)
        }
        _ => Value::data(&format!("L{}", c.line)),
    }
}

// ----------------------------------------------------------- dict.*
fn dict_recv(an: &Analyzer, c: &Call) -> Vec<u32> {
    match &c.recv {
        Some(r) => r.pts.iter().filter(|o| an.kind(*o).is_record()).collect(),
        None => Vec::new(),
    }
}

fn m_get(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(key) = c.args.first() else { return Value::data("get") };
    let default = c.args.get(1).cloned().unwrap_or_else(|| an.none_v.clone());
    let recv = c.recv.clone().unwrap_or_default();
    if key.kv.as_lit().is_some() {
        // whether the default applies is decided per call site when the receiver is concrete there
        for o in recv.pts.iter() {
            if an.kind(o) == Kind::Ph {
                an.record_event("narrow", o);
            } else if let Some(src) = an.obj(o).copied_from {
                an.record_event("narrow", src);
            }
        }
    } else if let Some(ph) = key.kv.as_param() {
        an.record_event("narrow", ph);
    }
    if let Some(k) = key.kv.as_lit() {
        let recs = an.record_objects(&recv);
        if !recs.is_empty() && recs.len() == recv.pts.len() && recs.iter().all(|o| an.kind(*o) != Kind::Frame) {
            // absent from every closed target: the default alone, no load
            if recs.iter().all(|o| { let r = st.row(*o, Ns::Item); r.closed() && !r.present.contains(k) }) {
                return default;
            }
        }
    }
    let out = an.load_item(&recv, Ns::Item, key, &c.node, st, false);
    // the default is only possible when the key may be absent on some target
    if let Some(k) = key.kv.as_lit() {
        let recs = an.record_objects(&recv);
        if !recs.is_empty() && recs.len() == recv.pts.len() && recs.iter().all(|o| an.kind(*o) != Kind::Frame && st.row(*o, Ns::Item).definite.contains(k)) {
            return out;
        }
    }
    out.join(&default)
}

fn m_setdefault(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(key) = c.args.first() else { return Value::data("setdefault") };
    let default = c.args.get(1).cloned().unwrap_or_else(|| an.none_v.clone());
    let recv = c.recv.clone().unwrap_or_default();
    let recs = an.record_objects(&recv);
    if let Some(k) = key.kv.as_lit() {
        if !recs.is_empty() && recs.len() == recv.pts.len() {
            let rows: Vec<Row> = recs.iter().map(|o| st.row(*o, Ns::Item)).collect();
            if rows.iter().all(|r| r.definite.contains(k)) {
                // present everywhere: the stored value, nothing is written
                return an.load_item(&recv, Ns::Item, key, &c.node, st, false);
            }
            if rows.iter().all(|r| r.closed() && !r.present.contains(k)) {
                // absent everywhere: the default is stored and returned
                an.store_item(&recv, Ns::Item, key, &default, &c.node, st);
                return default;
            }
        }
    } else if let Some(ph) = key.kv.as_param() {
        // presence of a parameter key: decided per call site
        an.record_event("narrow", ph);
    }
    let cur = an.load_item(&recv, Ns::Item, key, &c.node, st, false);
    let nv = if !cur.empty() { cur.join(&default) } else { default };
    an.store_item(&recv, Ns::Item, key, &nv, &c.node, st);
    an.load_item(&recv, Ns::Item, key, &c.node, st, false)
}

fn m_update(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let recs = dict_recv(an, c);
    let strong = recs.len() == 1 && an.obj(recs[0]).single;
    for oid in recs {
        if let Some(src) = c.args.first() {
            if !an.record_objects(src).is_empty() {
                an.copy_into(oid, Ns::Item, src, st, strong, &KeySet::new());
            } else if !src.pts.is_empty() {
                let it = an.iterate(src, &c.node, st);
                match (&it.elem, &it.items) {
                    (Elem::Pair(k, v), _) => an.store_key(oid, Ns::Item, &k.kv, v, false, &c.node, st),
                    (_, Some(items)) => {
                        for item in items {
                            let parts = an.unpack(item, 2, st, &c.node);
                            an.store_key(oid, Ns::Item, &parts[0].kv, &parts[1], strong, &c.node, st);
                        }
                    }
                    _ => {
                        let r = st.row(oid, Ns::Item).data_store(Source::new(&format!("update@L{}", c.line), SourceKind::Data));
                        st.set_row(oid, Ns::Item, r);
                    }
                }
            } else {
                let r = st.row(oid, Ns::Item).data_store(Source::new(&format!("update@L{}", c.line), SourceKind::Data));
                st.set_row(oid, Ns::Item, r);
            }
        }
        for (k, v) in c.kwargs.iter() {
            an.store_key(oid, Ns::Item, &KeyVal::Lit(intern::key(intern::Key::Str(intern::str_get(*k)))), v, strong, &c.node, st);
        }
        for sk in &c.star_kwargs {
            an.copy_into(oid, Ns::Item, sk, st, strong, &KeySet::new());
        }
    }
    an.none_v.clone()
}

/// `xs.pop()` / `xs.pop(i)` on one list: positional when the list's positions are known.
fn list_pop_one(an: &mut Analyzer, st: &mut State, oid: u32, c: &Call) -> Value {
    let ll = an.obj(oid).literal_len;
    let single = an.obj(oid).single;
    let idx = c.args.first().and_then(|a| a.kv.as_lit()).and_then(intern::key_as_int);
    let keyed = c.args.first().map(|a| a.kv.as_lit().is_none()).unwrap_or(false);
    if let (Some(n), false) = (ll, keyed) {
        if single && n > 0 && an.frame().loop_depth == 0 {
            let i = match idx {
                None => n as i64 - 1,
                Some(i) if i < 0 => n as i64 + i,
                Some(i) => i,
            };
            if 0 <= i && (i as usize) < n {
                let v = st.slot(oid, Ns::Pos, intern::key_int(i));
                for j in (i as usize)..(n - 1) {
                    let nv = st.slot(oid, Ns::Pos, intern::key_int(j as i64 + 1));
                    st.set_slot(oid, Ns::Pos, intern::key_int(j as i64), nv, true);
                }
                st.pop_slot(oid, Ns::Pos, intern::key_int(n as i64 - 1));
                an.obj_mut(oid).literal_len = Some(n - 1);
                return v;
            }
        }
    }
    an.obj_mut(oid).literal_len = None;
    an.elem_of(&Value::obj(oid), &c.node, st)
}

fn m_pop(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let recv = c.recv.clone().unwrap_or_default();
    let int_key = c.args.first().and_then(|a| a.kv.as_lit()).map(|k| intern::key_as_int(k).is_some()).unwrap_or(false);
    let listy = recv.pts.iter().all(|o| matches!(an.kind(o), Kind::Ph | Kind::List | Kind::Tuple));
    if !recv.pts.is_empty() && listy && (c.args.is_empty() || int_key) {
        // list.pop through a placeholder (or with no key): an element
        let mut outs = Vec::new();
        for oid in recv.pts.iter() {
            match an.kind(oid) {
                Kind::Ph => outs.push(an.iterate(&Value::obj(oid), &c.node, st).elem.flat()),
                _ => outs.push(list_pop_one(an, st, oid, c)),
            }
        }
        return join_values(outs);
    }
    let Some(key) = c.args.first() else { return Value::data("pop") };
    let has_default = c.args.len() > 1;
    if key.kv.as_lit().is_some() {
        // presence on a placeholder (or a copy of one): decided per call site
        for o in recv.pts.iter() {
            if an.kind(o) == Kind::Ph {
                an.record_event("narrow", o);
            } else if let Some(src) = an.obj(o).copied_from {
                an.record_event("narrow", src);
            }
        }
    }
    if let Some(k) = key.kv.as_lit() {
        let recs = an.record_objects(&recv);
        if !recs.is_empty() && recs.len() == recv.pts.len() && recs.iter().all(|o| an.kind(*o) != Kind::Frame) {
            let rows: Vec<Row> = recs.iter().map(|o| st.row(*o, Ns::Item)).collect();
            if has_default && rows.iter().all(|r| r.closed() && !r.present.contains(k)) {
                // absent everywhere: the default alone
                return c.args[1].clone();
            }
            if rows.iter().all(|r| r.definite.contains(k)) {
                // present everywhere: the value alone
                let out = an.load_item(&recv, Ns::Item, key, &c.node, st, !has_default);
                an.delete_item(&recv, Ns::Item, key, &c.node, st, false);
                return out;
            }
        }
    }
    let mut out = an.load_item(&recv, Ns::Item, key, &c.node, st, !has_default);
    an.delete_item(&recv, Ns::Item, key, &c.node, st, false);
    if has_default {
        out = out.join(&c.args[1]);
    }
    out
}

fn m_popitem(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    for oid in dict_recv(an, c) {
        let row = st.row(oid, Ns::Item);
        st.set_row(oid, Ns::Item, Row::from_data(RowData { present: row.present.clone(), sources: row.sources.clone(), mutators: row.mutators.clone(), top: row.top, ..Default::default() }));
    }
    Value::data("popitem")
}

fn m_clear(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let recs = dict_recv(an, c);
    for &oid in &recs {
        let strong = an.obj(oid).single && recs.len() == 1;
        if strong {
            st.set_row(oid, Ns::Item, Row::new());
            st.drop_obj_ns(oid, Ns::Item);
        } else {
            let row = st.row(oid, Ns::Item);
            st.set_row(oid, Ns::Item, Row::from_data(RowData { present: row.present.clone(), sources: row.sources.clone(), mutators: row.mutators.clone(), ..Default::default() }));
        }
    }
    an.none_v.clone()
}

fn m_views(an: &mut Analyzer, _st: &mut State, c: &Call) -> Value {
    let kind = ViewKind::from_name(last(&c.qual));
    let recv = c.recv.clone().unwrap_or_default();
    let ids: Vec<String> = recv.pts.iter().map(|x| x.to_string()).collect();
    let oid = an.alloc_raw(Kind::View, &format!("view:{}:{}", kind.name(), ids.join(",")), "", true, &format!("{} view", kind.name()), Payload::View(kind, recv.clone()));
    an.obj_mut(oid).payload = Payload::View(kind, recv);
    Value::obj(oid)
}

fn m_dcopy(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let outs: Vec<Value> = dict_recv(an, c).into_iter().map(|o| Value::obj(an.copy_object(o, &c.node, st, false, &mut IndexMap::new()))).collect();
    if outs.is_empty() {
        Value::data("copy")
    } else {
        join_values(outs)
    }
}

fn m_unknown_method(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    an.unknown_call(&c.qual, c.recv.as_ref(), &c.args, &c.kwargs, &c.star_args, &c.star_kwargs, st, &c.node)
}

// ----------------------------------------------------------- list.*
fn m_append(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = last(&c.qual);
    let recv = c.recv.clone().unwrap_or_default();
    for oid in recv.pts.iter() {
        if !matches!(an.kind(oid), Kind::List | Kind::Set | Kind::Tuple | Kind::Ph) {
            continue;
        }
        let mut ext_items: Option<Vec<Value>> = None;
        let elem = if (name == "extend" || name == "update") && !c.args.is_empty() {
            let it = an.iterate(&c.args[0], &c.node, st);
            if name == "extend" {
                ext_items = it.items.clone();
            }
            it.elem.one_or_empty()
        } else {
            c.args.last().cloned().unwrap_or_default()
        };
        // extend by known items in straight-line code: each item gets its position
        if let (Some(items), Kind::List) = (&ext_items, an.kind(oid)) {
            let ll = an.obj(oid).literal_len;
            if let Some(n) = ll {
                if an.obj(oid).single && an.frame().loop_depth == 0 && an.cond_depth == 0 && n + items.len() <= 16 {
                    for (i, item) in items.iter().enumerate() {
                        st.set_slot(oid, Ns::Pos, intern::key_int((n + i) as i64), item.clone(), true);
                        st.set_slot(oid, Ns::Elem, STAR, Value::with_kv(item.pts.clone(), DATA), false);
                        let nk = match st.elems.get(&oid) {
                            Some(cur) => kv_join(&cur, &item.kv),
                            None => item.kv.clone(),
                        };
                        st.elems.insert(oid, nk);
                    }
                    an.obj_mut(oid).literal_len = Some(n + items.len());
                    continue;
                }
            }
        }
        st.set_slot(oid, Ns::Elem, STAR, Value::with_kv(elem.pts.clone(), DATA), false);
        let nk = match st.elems.get(&oid) {
            Some(cur) => kv_join(&cur, &elem.kv),
            None => elem.kv.clone(),
        };
        st.elems.insert(oid, nk);
        if an.kind(oid) == Kind::Ph {
            // through a placeholder: the i-th append, positional at instantiation
            let positional = name == "append" && an.frame().loop_depth == 0 && an.cond_depth == 0;
            let i = if positional { (0..16).find(|i| !st.has_slot(oid, Ns::Pos, append_slot_key(*i))).unwrap_or(-1) } else { -1 };
            st.set_slot(oid, Ns::Pos, append_slot_key(i), elem.clone(), true);
            continue;
        }
        let ll = an.obj(oid).literal_len;
        let single = an.obj(oid).single;
        if let Some(n) = ll {
            if single && name == "append" && an.frame().loop_depth == 0 && an.cond_depth == 0 && n < 16 {
                st.set_slot(oid, Ns::Pos, intern::key_int(n as i64), elem, true);
                an.obj_mut(oid).literal_len = Some(n + 1);
                continue;
            }
            // appends of unknown count: the positions so far stay valid
            if name == "append" || name == "extend" {
                an.obj_mut(oid).known_prefix = Some(n);
            } else {
                st.drop_obj_ns(oid, Ns::Pos);
            }
        }
        an.obj_mut(oid).literal_len = None;
    }
    an.none_v.clone()
}

fn m_lpop(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let recv = c.recv.clone().unwrap_or_default();
    let mut outs = Vec::new();
    for oid in recv.pts.iter() {
        if an.kind(oid) == Kind::Ph {
            outs.push(an.iterate(&Value::obj(oid), &c.node, st).elem.flat());
            continue;
        }
        outs.push(list_pop_one(an, st, oid, c));
    }
    if outs.is_empty() {
        Value::data("pop")
    } else {
        join_values(outs)
    }
}

fn m_list_other(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = last(&c.qual);
    let recv = c.recv.clone().unwrap_or_default();
    match name {
        "copy" => join_values(recv.pts.iter().map(|o| Value::obj(an.copy_object(o, &c.node, st, false, &mut IndexMap::new()))).collect::<Vec<_>>()),
        "remove" | "discard" | "clear" | "sort" | "reverse" | "insert" => {
            for oid in recv.pts.iter() {
                an.obj_mut(oid).literal_len = None;
                an.obj_mut(oid).known_prefix = None;
                st.drop_obj_ns(oid, Ns::Pos);
            }
            an.none_v.clone()
        }
        _ => Value::data(&format!("L{}", c.line)),
    }
}

fn m_view(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = last(&c.qual);
    if matches!(name, "tolist" | "to_list" | "copy") {
        if let Some(recv) = &c.recv {
            let oid = an.alloc_simple(Kind::List, &c.node, st, &format!("{}@L{}", name, c.line));
            an.obj_mut(oid).literal_len = None;
            let it = an.iterate(recv, &c.node, st);
            let elem = it.elem.one_or_empty();
            st.set_slot(oid, Ns::Elem, STAR, Value::with_kv(elem.pts.clone(), DATA), true);
            st.elems.insert(oid, elem.kv);
            return Value::obj(oid);
        }
    }
    Value::data(&format!("L{}", c.line))
}

// ---------------------------------------------------------- json/copy
fn json_tag(q: &str) -> String {
    format!("{}.{}", q.split('.').next().unwrap_or(""), last(q))
}

fn m_json_load(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let tag = json_tag(&c.qual);
    let src = Source::new(&format!("{}@L{}", tag, c.line), SourceKind::External);
    let oid = an.alloc_at(Kind::Unknown, &c.node, st, &format!("{}@L{}", tag, c.line), Payload::None, Some(Row::with_source(src)), Ns::Item, None, None);
    st.set_row(oid, Ns::Attr, Row::with_source(src));
    Value::obj(oid)
}

fn m_http(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let tag = format!("{}@L{}", json_tag(&c.qual), c.line);
    let oid = an.alloc_at(Kind::Response, &c.node, st, &tag, Payload::Str(tag.clone()), None, Ns::Item, None, None);
    an.obj_mut(oid).payload = Payload::Str(tag);
    Value::obj(oid)
}

fn m_response_method(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = last(&c.qual);
    if name == "json" || name == "read" {
        let mut outs = Vec::new();
        let recv = c.recv.clone().unwrap_or_default();
        for x in recv.pts.iter() {
            let tag = an.external_qual(x);
            let src = Source::new(&tag, SourceKind::External);
            let oid = an.alloc_at(Kind::Unknown, &c.node, st, &format!("{}.json()", tag), Payload::None, Some(Row::with_source(src)), Ns::Item, None, None);
            st.set_row(oid, Ns::Attr, Row::with_source(src));
            outs.push(Value::obj(oid));
        }
        return if outs.is_empty() { Value::data("json") } else { join_values(outs) };
    }
    Value::data(&format!("L{}", c.line))
}

fn m_copy(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(a) = c.args.first() else { return Value::data("copy") };
    let deep = c.qual.ends_with("deepcopy");
    let mut outs = Vec::new();
    for o in a.pts.iter() {
        if matches!(an.kind(o), Kind::Dict | Kind::List | Kind::Tuple | Kind::Set | Kind::Instance | Kind::Frame) {
            outs.push(Value::obj(an.copy_object(o, &c.node, st, deep, &mut IndexMap::new())));
        } else {
            outs.push(Value::obj(o));
        }
    }
    if outs.is_empty() {
        a.clone()
    } else {
        join_values(outs)
    }
}

fn m_dc_replace(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(a) = c.args.first() else { return Value::data("replace") };
    let mut outs = Vec::new();
    for o in a.pts.iter() {
        if an.kind(o) == Kind::Instance {
            let nid = an.copy_object(o, &c.node, st, false, &mut IndexMap::new());
            for (k, v) in c.kwargs.iter() {
                an.store_key(nid, Ns::Attr, &KeyVal::Lit(intern::key(intern::Key::Str(intern::str_get(*k)))), v, true, &c.node, st);
            }
            outs.push(Value::obj(nid));
        }
    }
    if outs.is_empty() {
        a.clone()
    } else {
        join_values(outs)
    }
}

fn m_passthrough(_an: &mut Analyzer, _st: &mut State, c: &Call) -> Value {
    if c.qual == "typing.cast" && c.args.len() >= 2 {
        return c.args[1].clone();
    }
    Value::data(&format!("L{}", c.line))
}

fn m_defaultdict(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = last(&c.qual).to_string();
    let oid = an.alloc_simple(Kind::Dict, &c.node, st, &format!("{}@L{}", name, c.line));
    let src_arg: Option<Value>;
    if name == "defaultdict" || name == "Counter" {
        if name == "Counter" {
            if c.args.is_empty() {
                // an empty Counter is a closed dict whose missing keys read as 0
                st.set_slot(oid, Ns::Item, STAR, Value::lit(intern::key_int(0)), false);
            } else {
                let r = st.row(oid, Ns::Item).data_store(Source::new(&format!("{}@L{}", name, c.line), SourceKind::Data));
                st.set_row(oid, Ns::Item, r);
            }
        }
        if name == "defaultdict" && !c.args.is_empty() {
            if !c.args[0].pts.is_empty() && !crate::interp::disabled("ddfactory") {
                // the factory runs per inserted key (see `load_item`); no shared default
                an.dd_factories.insert(oid, c.args[0].clone());
            } else if !c.args[0].pts.is_empty() {
                let dflt = an.call_value(&c.args[0], &c.node, None, st, Some(vec![]));
                st.set_slot(oid, Ns::Item, STAR, Value::with_kv(dflt.pts, DATA), false);
            } else {
                st.set_slot(oid, Ns::Item, STAR, Value::data("default"), false);
            }
        }
        src_arg = if name == "defaultdict" { c.args.get(1).cloned() } else { c.args.first().cloned() };
    } else {
        src_arg = c.args.first().cloned();
    }
    if let Some(sa) = src_arg {
        if !an.record_objects(&sa).is_empty() {
            an.copy_into(oid, Ns::Item, &sa, st, true, &KeySet::new());
        } else if name != "Counter" {
            let r = st.row(oid, Ns::Item).data_store(Source::new(&format!("{}@L{}", name, c.line), SourceKind::Data));
            st.set_row(oid, Ns::Item, r);
        }
    }
    for (k, v) in c.kwargs.iter() {
        an.store_key(oid, Ns::Item, &KeyVal::Lit(intern::key(intern::Key::Str(intern::str_get(*k)))), v, true, &c.node, st);
    }
    Value::obj(oid)
}

fn m_dictreader(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fields = c.kw("fieldnames").and_then(|f| an.lit_keys(&f, st));
    let row = match fields {
        Some(f) => Row::closed_with(f),
        None => Row::with_source(Source::new(&format!("csv.DictReader@L{}", c.line), SourceKind::External)),
    };
    let rid = an.alloc_at(Kind::Dict, &c.node, st, &format!("DictReader row@L{}", c.line), Payload::None, Some(row), Ns::Item, Some(false), None);
    an.obj_mut(rid).iter_elem = true;
    let ctx = an.frame().ctx.clone();
    let vid = an.alloc_raw(Kind::View, &format!("view:elems:{}", c.site), &ctx, true, "DictReader", Payload::View(ViewKind::Elems, Value::obj(rid)));
    an.obj_mut(vid).payload = Payload::View(ViewKind::Elems, Value::obj(rid));
    Value::obj(vid)
}

fn m_argparser(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let oid = an.alloc_simple(Kind::Parser, &c.node, st, &format!("ArgumentParser@L{}", c.line));
    st.set_row(oid, Ns::Attr, Row::new());
    Value::obj(oid)
}

fn m_parser_method(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = last(&c.qual);
    let recv = c.recv.clone().unwrap_or_default();
    let parsers: Vec<u32> = recv.pts.iter().filter(|o| an.kind(*o) == Kind::Parser).collect();
    match name {
        "add_argument" => {
            let mut dest: Option<String> = None;
            if let Some(dv) = c.kw("dest") {
                if let Some(k) = dv.kv.as_lit() {
                    dest = Some(intern::key_py_str(k));
                }
            } else {
                for a in &c.args {
                    if let Some(k) = a.kv.as_lit() {
                        if let Some(sk) = intern::key_as_str(k) {
                            if let Some(rest) = sk.strip_prefix("--") {
                                dest = Some(rest.replace('-', "_"));
                                break;
                            }
                            if !sk.starts_with('-') {
                                dest = Some(sk.replace('-', "_"));
                                break;
                            }
                            if dest.is_none() {
                                dest = Some(sk.trim_start_matches('-').replace('-', "_"));
                            }
                        }
                    }
                }
            }
            for p in parsers {
                match &dest {
                    Some(d) => {
                        let dk = key_str_lit(d);
                        let single = an.obj(p).single;
                        let r = st.row(p, Ns::Attr).store(dk, single);
                        st.set_row(p, Ns::Attr, r);
                        st.set_slot(p, Ns::Attr, dk, Value::data(&format!("arg {}", d)), true);
                    }
                    None => {
                        let r = st.row(p, Ns::Attr).data_store(Source::new(&format!("add_argument@L{}", c.line), SourceKind::Data));
                        st.set_row(p, Ns::Attr, r);
                    }
                }
            }
            Value::data("action")
        }
        "parse_args" | "parse_known_args" | "parse_intermixed_args" => {
            let nid = an.alloc_at(Kind::Namespace, &c.node, st, &format!("args@L{}", c.line), Payload::None, None, Ns::Attr, None, None);
            for p in parsers {
                let r = st.row(nid, Ns::Attr).copy_from(&st.row(p, Ns::Attr), true, &KeySet::new(), intern::EMPTY_LENS);
                st.set_row(nid, Ns::Attr, r);
                for (k, v) in st.obj_slots(p, Ns::Attr) {
                    st.set_slot(nid, Ns::Attr, k, v, false);
                }
            }
            if name == "parse_known_args" {
                let tid = an.alloc_simple(Kind::Tuple, &c.node, st, "parse_known_args result");
                an.obj_mut(tid).literal_len = Some(2);
                st.set_slot(tid, Ns::Pos, intern::key_int(0), Value::obj(nid), true);
                st.set_slot(tid, Ns::Pos, intern::key_int(1), Value::data("rest"), true);
                return Value::obj(tid);
            }
            Value::obj(nid)
        }
        "add_subparsers" | "add_argument_group" | "add_mutually_exclusive_group" => recv,
        _ => Value::data(&format!("L{}", c.line)),
    }
}

fn m_partial(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(f) = c.args.first() else { return Value::data("partial") };
    let payload = Payload::Partial(f.clone(), c.args[1..].to_vec(), c.kwargs.clone());
    let oid = an.alloc_at(Kind::Partial, &c.node, st, &format!("partial@L{}", c.line), payload.clone(), None, Ns::Item, None, None);
    an.obj_mut(oid).payload = payload;
    Value::obj(oid)
}

fn m_chain(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let oid = an.alloc_simple(Kind::List, &c.node, st, &format!("chain@L{}", c.line));
    an.obj_mut(oid).literal_len = None;
    let mut acc = Value::new();
    let mut kv: Option<KeyVal> = None;
    for a in &c.args {
        let it = an.iterate(a, &c.node, st);
        let Elem::One(elem) = it.elem else { continue };
        acc = acc.join(&Value::with_kv(elem.pts.clone(), DATA));
        kv = Some(match kv {
            None => elem.kv.clone(),
            Some(k) => kv_join(&k, &elem.kv),
        });
    }
    st.set_slot(oid, Ns::Elem, STAR, acc, true);
    st.elems.insert(oid, kv.unwrap_or(DATA));
    Value::obj(oid)
}

pub fn partial_call(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    m_partial_call(an, st, c)
}

fn m_partial_call(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let recv = c.recv.clone().unwrap_or_default();
    let mut outs = Vec::new();
    for x in recv.pts.iter() {
        let Payload::Partial(f, bound_args, bound_kw) = an.obj(x).payload.clone() else { continue };
        let mut kw = bound_kw.clone();
        for (k, v) in c.kwargs.iter() {
            kw.insert(*k, v.clone());
        }
        let mut fake_args = bound_args.clone();
        fake_args.extend(c.args.iter().cloned());
        for fo in f.pts.iter() {
            outs.push(an.call_one(fo, &fake_args, &kw, &c.star_args, &c.star_kwargs, st, &c.node, c.call_ast));
        }
    }
    if outs.is_empty() {
        Value::data("partial")
    } else {
        join_values(outs)
    }
}

fn m_get_logger(an: &mut Analyzer, _st: &mut State, _c: &Call) -> Value {
    Value::obj(an.alloc_raw(Kind::External, "ext:logging.Logger", "", true, "logging.Logger", Payload::Str("logging.Logger".into())))
}

fn m_getenv(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let env = external_record(an, "os.environ", st).unwrap();
    let call = Call { qual: "dict.get".into(), recv: Some(Value::obj(env)), args: c.args.clone(), kwargs: c.kwargs.clone(), star_args: c.star_args.clone(), star_kwargs: c.star_kwargs.clone(), node: c.node, call_ast: c.call_ast, site: c.site.clone(), line: c.line };
    m_get(an, st, &call)
}

/// `bool(x)`: the literal when the truthiness of `x` is decided, else unknown data.
fn m_bool(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(v) = c.args.first() else { return Value::lit(intern::key(intern::Key::Bool(false))) };
    match an.truthiness(v, st) {
        Some(b) => Value::lit(intern::key(intern::Key::Bool(b))),
        None => Value::data("bool"),
    }
}

/// `len(x)`: the count when it is known (a literal string, a container with a known
/// length, a closed record whose keys are all definite), else unknown data.
fn m_len(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(v) = c.args.first() else { return Value::data("len") };
    if v.pts.is_empty() {
        if let Some(k) = v.kv.as_lit() {
            if let Some(s) = intern::key_as_str(k) {
                return Value::lit(intern::key_int(s.chars().count() as i64));
            }
        }
        return Value::data("len");
    }
    if let Some(o) = v.single() {
        if let Some(n) = an.obj(o).literal_len {
            return Value::lit(intern::key_int(n as i64));
        }
        if matches!(an.kind(o), Kind::Dict) {
            let row = st.row(o, Ns::Item);
            if row.closed() && row.present == row.definite && !row.symbolic() {
                return Value::lit(intern::key_int(row.definite.len() as i64));
            }
        }
    }
    Value::data("len")
}

/// `sum(xs)`: the literal sum when every item is a known number, else unknown data.
fn m_sum(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(v) = c.args.first() else { return Value::data("sum") };
    let it = an.iterate(v, &c.node, st);
    if let Some(items) = it.items {
        let mut acc_i: i64 = 0;
        let mut acc_f: f64 = 0.0;
        let mut float = false;
        for item in &items {
            let Some(k) = item.kv.as_lit().filter(|_| item.pts.is_empty()) else { return Value::data("sum") };
            match intern::with_key(k, |kk| match kk {
                intern::Key::Int(i) => Some((*i as f64, false, *i)),
                intern::Key::Bool(b) => Some((if *b { 1.0 } else { 0.0 }, false, *b as i64)),
                intern::Key::Float(b) => Some((f64::from_bits(*b), true, 0)),
                _ => None,
            }) {
                Some((f, is_f, i)) => {
                    acc_f += f;
                    if is_f {
                        float = true;
                    } else {
                        acc_i = acc_i.wrapping_add(i);
                    }
                }
                None => return Value::data("sum"),
            }
        }
        if let Some(start) = c.args.get(1) {
            match start.kv.as_lit().filter(|_| start.pts.is_empty()).and_then(|k| intern::with_key(k, |kk| match kk {
                intern::Key::Int(i) => Some((*i as f64, false, *i)),
                intern::Key::Float(b) => Some((f64::from_bits(*b), true, 0)),
                _ => None,
            })) {
                Some((f, is_f, i)) => {
                    acc_f += f;
                    if is_f {
                        float = true;
                    } else {
                        acc_i = acc_i.wrapping_add(i);
                    }
                }
                None => return Value::data("sum"),
            }
        }
        return if float { Value::lit(intern::key_float(acc_f)) } else { Value::lit(intern::key_int(acc_i)) };
    }
    Value::data("sum")
}

/// The sort key of one item as literals: the item itself, or `key(item)`, when it is a
/// single literal or a tuple of single literals.
fn literal_sort_key(an: &mut Analyzer, st: &mut State, c: &Call, item: &Value) -> Option<Vec<KeyId>> {
    let v = match c.kw("key") {
        Some(kf) if !kf.pts.is_empty() => an.call_value(&kf, &c.node, None, st, Some(vec![item.clone()])),
        Some(_) => return None,
        None => item.clone(),
    };
    if v.pts.is_empty() {
        return v.kv.as_lit().map(|k| vec![k]);
    }
    let o = v.single()?;
    if an.kind(o) != Kind::Tuple {
        return None;
    }
    let n = an.obj(o).literal_len?;
    let mut ks = Vec::new();
    for i in 0..n {
        let s = st.slot(o, Ns::Pos, intern::key_int(i as i64));
        if !s.pts.is_empty() {
            return None;
        }
        ks.push(s.kv.as_lit()?);
    }
    Some(ks)
}

/// The permutation `sorted` applies when every key is known (stable), else None.
fn literal_sort_order(an: &mut Analyzer, st: &mut State, c: &Call, items: &[Value]) -> Option<Vec<usize>> {
    let mut keys: Vec<Vec<KeyId>> = Vec::new();
    for item in items {
        keys.push(literal_sort_key(an, st, c, item)?);
    }
    let reverse = c.kw("reverse").map(|r| matches!(r.kv.as_lit().map(|k| intern::with_key(k, |kk| matches!(kk, intern::Key::Bool(true)))), Some(true))).unwrap_or(false);
    let cmp = |a: &Vec<KeyId>, b: &Vec<KeyId>| -> Option<std::cmp::Ordering> {
        for (x, y) in a.iter().zip(b.iter()) {
            match crate::interp::exprs::literal_order(*x, *y)? {
                std::cmp::Ordering::Equal => continue,
                o => return Some(o),
            }
        }
        Some(a.len().cmp(&b.len()))
    };
    // every pair must be comparable
    for i in 0..keys.len() {
        for j in (i + 1)..keys.len() {
            cmp(&keys[i], &keys[j])?;
        }
    }
    let mut idx: Vec<usize> = (0..items.len()).collect();
    idx.sort_by(|&a, &b| {
        let o = cmp(&keys[a], &keys[b]).unwrap_or(std::cmp::Ordering::Equal);
        if reverse { o.reverse() } else { o }
    });
    Some(idx)
}
