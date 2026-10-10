//! Library models (SPECIFICATION.md §11); port of `rowan/models.py`.  A model is
//! `fn(&mut Analyzer, &mut State, &Call) -> Value`; models are keyed by qualified name with
//! prefix fallbacks.

pub mod minire;
pub mod pandas;
pub mod pydantic;
pub mod fastapi;
pub mod counter;
pub mod csvio;
pub mod argparse;
pub mod configparser;
pub mod concurrency;
pub mod itertools;
pub mod stdfuncs;
pub mod pyvalues;
pub mod pure;
pub mod torch;

use indexmap::IndexMap;
use ruff_python_ast as ast;
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
    "Series.str.", "Series.dt.", "Series.cat.", "Series.sparse.", "Series.list.", "Series.struct.", "Series.", "str.", "DataFrame.", "list.", "set.", "tuple.", "dict.", "view.", "groupby.", "zip.", "enumerate.", "unknown.", "class.", "pair.",
    "parser.", "cfgparser.", "cfgsection.", "partial.", "response.", "pydantic.cls.", "pydantic.", "typeadapter.", "fieldinfo.", "pydverr.", "counter.", "chainmap.", "deque.", "stdobj.", "slice.", "memo.", "fastapi.", "starlette.", "torch.", "tensor.", "tensorq.", "nnmodule.", "math.", "numpy.", "np.",
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
        r(&["builtins.object.__setattr__"], m_object_setattr);
        r(&["builtins.delattr"], m_delattr);
        r(&["builtins.hasattr"], m_hasattr);
        r(&["builtins.vars", "attr.asdict", "attrs.asdict"], m_vars);
        r(&["dataclasses.asdict"], m_asdict);
        r(&["dataclasses.fields"], m_dc_fields);
        r(&["builtins.map", "builtins.filter"], m_map);
        r(&["builtins.str"], m_str);
        r(&["builtins.repr"], m_repr);
        r(
            &[
                "builtins.print",
                "builtins.id", "builtins.hash",
                "builtins.format", "builtins.input", "builtins.callable", "builtins.issubclass", "builtins.hex",
                "builtins.chr", "builtins.ord", "builtins.open", "builtins.divmod", "builtins.pow",
                "builtins.bytes", "builtins.object",
            ],
            noeffect,
        );
        r(&["builtins.int", "builtins.float"], m_int);
        r(&["builtins.round", "builtins.abs"], m_round_abs);
        r(&["builtins.super"], m_super);
        r(&["builtins.bool"], m_bool);
        r(&["builtins.type"], m_type);
        r(&["builtins.range"], m_range);
        r(&["types.MappingProxyType"], m_mappingproxy);
        r(&["typing.get_type_hints", "typing_extensions.get_type_hints", "inspect.get_annotations"], m_type_hints);
        r(&["builtins.any", "builtins.all"], m_any_all);
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
        r(&["dict.move_to_end"], m_move_to_end);
        r(&["dict.clear"], m_clear);
        r(&["dict.keys", "dict.values", "dict.items"], m_views);
        r(&["dict.copy"], m_dcopy);
        r(&["dict.*", "unknown.*", "class.*"], m_unknown_method);
        r(
            &[
                "set.add", "set.update", "set.union", "set.intersection", "set.difference", "set.symmetric_difference", "set.issubset", "set.issuperset",
                "set.isdisjoint", "set.discard", "set.remove", "set.intersection_update", "set.difference_update", "set.symmetric_difference_update", "set.copy",
                "set.clear",
            ],
            m_set,
        );
        r(&["list.append", "list.extend"], m_append);
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
        r(&["json.dump", "yaml.dump", "yaml.safe_dump", "pickle.dump", "pprint.pprint", "pprint.pformat", "orjson.dumps"], noeffect);
        r(&["json.dumps", "pickle.dumps"], m_json_dumps);
        r(&["copy.copy", "copy.deepcopy"], m_copy);
        r(&["dataclasses.replace"], m_dc_replace);
        r(&["dataclasses.astuple"], m_astuple);
        r(&["dataclasses.make_dataclass"], m_make_dataclass);
        r(&["namedtuple._asdict", "namedtuple._replace"], m_nt_method);
        r(&["operator.itemgetter", "operator.attrgetter"], m_getter);
        // a coroutine is analysed as its result: running / awaiting it is that value
        r(&["asyncio.run", "asyncio.create_task", "asyncio.ensure_future", "asyncio.wait_for", "asyncio.shield"], m_first_arg);
        r(&["asyncio.gather"], m_gather);
        r(&["re.match", "re.search", "re.fullmatch"], m_re_match);
        r(&["builtins.exec", "builtins.eval"], m_exec);
        r(&["builtins.globals"], m_globals);
        r(&["types.SimpleNamespace", "argparse.Namespace"], m_simplenamespace);
        r(&["collections.namedtuple", "typing.NamedTuple"], m_namedtuple);
        r(&["dataclasses.field", "typing.cast", "typing.TypeVar", "typing.NewType"], m_passthrough);
        r(&["collections.defaultdict", "collections.Counter", "collections.ChainMap"], m_defaultdict);
        r(&["collections.OrderedDict"], m_ordered_dict);
        r(&["csv.DictReader"], m_dictreader);
        r(&["csv.DictWriter"], noeffect);
        r(&["argparse.ArgumentParser"], m_argparser);
        r(&["parser.*"], m_parser_method);
        r(&["functools.partial"], m_partial);
        r(&["memo.cache_info", "memo.cache_clear"], m_memo);
        r(&["functools.reduce"], m_reduce);
        r(&["itertools.chain", "itertools.chain.from_iterable"], m_chain);
        r(&["partial.*"], m_partial_call);
        r(&["logging.getLogger", "logging.getLogger.getChild", "structlog.get_logger", "loguru.logger.bind"], m_get_logger);
        r(&["os.getenv", "os.environ.get"], m_getenv);
        pandas::install(&mut r);
        pydantic::install(&mut r);
        fastapi::install(&mut r);
        r(&["counter.*"], counter::m_counter);
        r(&["chainmap.*"], m_chainmap);
        r(&["deque.*"], counter::m_deque);
        r(&["collections.deque"], counter::deque_new);
        r(&["builtins.slice"], m_slice);
        r(&["slice.indices"], m_slice_indices);
        csvio::register(&mut r);
        argparse::register(&mut r);
        configparser::register(&mut r);
        concurrency::register(&mut r);
        itertools::register(&mut r);
        stdfuncs::register(&mut r);
        r(&["typeadapter.*"], m_typeadapter);
        r(&["torch.*"], torch::m_torch);
        r(&["numpy.*", "np.*"], torch::m_numpy);
        r(&["math.*"], m_math);
        r(&["tensor.*"], torch::m_tensor_method);
        r(&["tensorq.*"], torch::m_tensor_or_builtin);
        r(&["nnmodule.*"], torch::m_nnmodule);
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

/// Is there a model for anything in module `name` (so following its source is not wanted)?
pub fn has_module_models(name: &str) -> bool {
    let dotted = format!("{}.", name);
    if PREFIXES.iter().any(|p| p.starts_with(&dotted) || *p == dotted) || ALLOWLIST_PREFIXES.iter().any(|p| p.starts_with(&dotted) || *p == dotted) {
        return true;
    }
    let reg = Registry::new();
    reg.table.keys().any(|k| k.starts_with(&dotted))
}

impl Default for Registry {
    fn default() -> Self {
        Registry::new()
    }
}

fn lit_num(v: &Value) -> Option<f64> {
    if !v.pts.is_empty() {
        return None;
    }
    match intern::key_get(v.kv.as_lit()?) {
        intern::Key::Int(i) => Some(i as f64),
        intern::Key::Float(b) => Some(f64::from_bits(b)),
        intern::Key::Bool(b) => Some(b as i64 as f64),
        _ => None,
    }
}

/// `round(x)` (half to even) / `abs(x)` on known numbers; on a dim divided by an int
/// (`round(T / p)`): unknown. Otherwise a number.
fn m_round_abs(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(a) = c.args.first() else { return Value::data(&format!("L{}", c.line)) };
    let is_int = a.pts.is_empty() && a.kv.as_lit().map(|k| matches!(intern::key_get(k), intern::Key::Int(_) | intern::Key::Bool(_))).unwrap_or(false);
    if let Some(x) = lit_num(a) {
        if c.qual == "builtins.abs" {
            return if is_int { Value::lit(intern::key_int(x.abs() as i64)) } else { Value::lit(intern::key_float(x.abs())) };
        }
        if c.args.len() == 1 {
            let r = x.round();
            let r = if (x - x.trunc()).abs() == 0.5 { 2.0 * (x / 2.0).round() } else { r };
            return Value::lit(intern::key_int(r as i64));
        }
    }
    if c.qual == "builtins.abs" && an.torch_on {
        if let Some(d) = crate::models::torch::dim_of_label(a) {
            // dims are not negative
            if crate::shape::poly(d).iter().all(|(_, k)| *k > 0) {
                return a.clone();
            }
        }
    }
    let _ = st;
    Value::data(&format!("L{}", c.line))
}

/// `math.*`: numbers (a scalar for the tensor model)
fn m_math(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    // ceil / floor / trunc of known numbers, of a dim divided by an int
    if matches!(c.qual.as_str(), "math.ceil" | "math.floor" | "math.trunc") {
        if let Some(a) = c.args.first() {
            if let Some(x) = lit_num(a) {
                let r = match c.qual.as_str() {
                    "math.ceil" => x.ceil(),
                    "math.floor" => x.floor(),
                    _ => x.trunc(),
                };
                return Value::lit(intern::key_int(r as i64));
            }
            if let Some((d, k)) = crate::models::torch::quotient_of(a) {
                let r = if c.qual == "math.ceil" { crate::shape::ceildiv(d, crate::shape::lit(k)) } else { crate::shape::floordiv(d, crate::shape::lit(k)) };
                return crate::models::torch::dim_value(crate::shape::norm(r, &st.tf));
            }
        }
    }
    if an.torch_on && !matches!(c.qual.as_str(), "math.floor" | "math.ceil" | "math.isclose" | "math.isnan" | "math.isinf" | "math.prod" | "math.gcd" | "math.comb" | "math.factorial") {
        return crate::models::torch::num_value();
    }
    Value::data(&format!("L{}", c.line))
}

/// `OrderedDict(pairs)` / `OrderedDict(mapping)` is `dict(...)` (ordered, like every dict);
/// `OrderedDict()` the empty one.
fn m_ordered_dict(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    if c.args.is_empty() && c.kwargs.is_empty() && c.star_args.is_empty() && c.star_kwargs.is_empty() {
        return m_defaultdict(an, st, c);
    }
    let call = Call { qual: "builtins.dict".into(), recv: c.recv.clone(), args: c.args.clone(), kwargs: c.kwargs.clone(), star_args: c.star_args.clone(), star_kwargs: c.star_kwargs.clone(), node: c.node, call_ast: c.call_ast, site: c.site.clone(), line: c.line };
    m_dict(an, st, &call)
}

pub fn noeffect(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    // `print(…, file=buf)` writes into a StringIO; other writers (`json.dump(x, buf)`) make its
    // text unknown
    if c.qual == "builtins.print" {
        csvio::print_into(an, st, c);
    } else if !c.args.is_empty() || !c.kwargs.is_empty() {
        let vals: Vec<&Value> = c.args.iter().chain(c.kwargs.values()).collect();
        csvio::clobber(an, st, &vals, c.line);
    }
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
        // a parameter: a mapping or pairs, decided per call site
        for x in src.pts.iter() {
            if an.kind(x) == Kind::Ph && !crate::interp::disabled("dictph") {
                an.record_event("concrete", x);
            }
        }
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
                        // both lengths known: the pairs zip makes (it stops at the shorter)
                        if let Some(items) = an.iterate(&Value::obj(x), &c.node, st).items {
                            for item in items {
                                let parts = an.unpack(&item, 2, st, &c.node);
                                an.store_key(oid, Ns::Item, &parts[0].kv, &parts[1], true, &c.node, st);
                            }
                            continue;
                        }
                        let lits = an.lit_keys(&ks, st);
                        let vel = an.elem_of(&vs, &c.node, st);
                        if let Some(lits) = lits {
                            // the values' length is unknown: the header/row idiom pairs every
                            // key (design case d30), so the keys are definite
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
                    Kind::List | Kind::Tuple | Kind::Set | Kind::View | Kind::Enumerate => {
                        let it = an.iterate(&Value::obj(x), &c.node, st);
                        match (&it.elem, &it.items) {
                            (_, Some(items)) => {
                                for item in items {
                                    let parts = an.unpack(item, 2, st, &c.node);
                                    an.store_key(oid, Ns::Item, &parts[0].kv, &parts[1], true, &c.node, st);
                                }
                            }
                            (Elem::Pair(k, v), None) => an.store_key(oid, Ns::Item, &k.kv, v, false, &c.node, st),
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
    let mut ks = c.args.first().and_then(|a| an.lit_keys(a, st));
    if ks.is_none() {
        // known items that are literals (a user iterable, a generator)
        if let Some(a) = c.args.first() {
            let items = an.iterate(a, &c.node, st).items;
            let lits: Option<Vec<KeyId>> = items.and_then(|its| its.iter().map(|v| v.kv.as_lit().filter(|_| v.pts.is_empty())).collect());
            ks = lits.map(|l| l.into_iter().collect());
        }
    }
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
    if an.torch_on && matches!(c.qual.as_str(), "builtins.tuple" | "builtins.list") {
        if let Some(a) = c.args.first() {
            if crate::models::torch::size_shape(a).is_some() {
                return a.clone();
            }
        }
    }
    let kind = match c.qual.as_str() {
        "builtins.tuple" => Kind::Tuple,
        "builtins.set" | "builtins.frozenset" => Kind::Set,
        _ => Kind::List,
    };
    let oid = an.alloc_simple(kind, &c.node, st, &format!("{}@L{}", last(&c.qual), c.line));
    an.set_llen(oid, st, None);
    if let Some(a) = c.args.first() {
        let it = an.iterate(a, &c.node, st);
        if c.qual == "builtins.iter" {
            an.obj_mut(oid).iterator = true;
        } else {
            an.exhaust(a, st);
        }
        match &it.elem {
            Elem::Pair(k, v) => {
                let ctx = an.frame().ctx.clone();
                let pid = an.alloc_raw(Kind::Pair, &format!("pair:{}", c.site), &ctx, true, "pair", Payload::Pair(k.clone(), v.clone()));
                st.set_slot(oid, Ns::Elem, STAR, Value::obj(pid), true);
                st.elems.insert(oid, DATA);
                // known pairs (zip / enumerate / items of known inputs) keep their positions;
                // sorted by known literal keys, in that order
                let plain = matches!(c.qual.as_str(), "builtins.list" | "builtins.tuple" | "builtins.reversed" | "builtins.sorted");
                if let (true, Some(items)) = (plain, &it.items) {
                    if an.obj(oid).single && items.len() <= 64 {
                        let order: Option<Vec<usize>> = match c.qual.as_str() {
                            "builtins.sorted" => literal_sort_order(an, st, c, items),
                            "builtins.reversed" => Some((0..items.len()).rev().collect()),
                            _ => Some((0..items.len()).collect()),
                        };
                        if let Some(order) = order {
                            an.set_llen(oid, st, Some(items.len()));
                            for (i, j) in order.into_iter().enumerate() {
                                st.set_slot(oid, Ns::Pos, intern::key_int(i as i64), items[j].clone(), true);
                            }
                        } else {
                            an.set_llen(oid, st, Some(items.len()));
                            let all = join_values(items.clone());
                            for i in 0..items.len() {
                                st.set_slot(oid, Ns::Pos, intern::key_int(i as i64), all.clone(), true);
                            }
                        }
                    }
                }
            }
            Elem::One(elem) => {
                st.set_slot(oid, Ns::Elem, STAR, Value::with_kv(elem.pts.clone(), crate::models::torch::elem_kv(&elem.kv)), true);
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
                    if an.obj(oid).single && items.len() <= 64 {
                        an.set_llen(oid, st, Some(items.len()));
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
                        // the items of a set come in an order Python does not promise
                        let from_set = a.pts.iter().any(|o| an.kind(o) == Kind::Set);
                        if from_set && kind != Kind::Set && !sorted {
                            for i in 0..n {
                                st.set_slot(oid, Ns::Pos, intern::key_int(i as i64), all.clone(), true);
                            }
                        }
                        an.normalize_set(oid, st);
                    }
                }
            }
        }
    } else {
        st.elems.insert(oid, KeyVal::Lits(KeySet::new()));
        an.set_llen(oid, st, Some(0));
    }
    Value::obj(oid)
}

/// `isinstance(x, C)`: decided when every object of `x` has a known kind / class; a
/// placeholder records an event so the call site decides.
fn m_isinstance(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let (Some(v), Some(cv)) = (c.args.first(), c.args.get(1)) else { return Value::data("bool") };
    // isinstance(x, torch.Tensor): decided for tensors and for containers / literals
    if an.torch_on {
        if let Some(q) = cv.single().filter(|o| an.kind(*o) == Kind::External).map(|o| an.external_qual(o)) {
            if q == "torch.Tensor" || q == "torch.nn.Parameter" || q == "torch.nn.parameter.Parameter" {
                let tensor = crate::models::torch::is_tensor(v);
                let not_tensor = !tensor
                    && !v.pts.is_empty()
                    && v.pts.iter().all(|o| matches!(an.kind(o), Kind::List | Kind::Tuple | Kind::Dict | Kind::Set | Kind::Namespace | Kind::Function | Kind::Class) || (an.kind(o) == Kind::Instance && !an.t_is_module_instance(o)))
                    && (v.kv.is_bot() || v.kv.is_literal())
                    || v.pts.is_empty() && v.kv.is_literal();
                if tensor && q == "torch.Tensor" {
                    return Value::lit(intern::key_bool(true));
                }
                if not_tensor {
                    return Value::lit(intern::key_bool(false));
                }
                return Value::data("bool");
            }
        }
    }
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
    if an.torch_on {
        if let Some(v) = an.t_minmax(c.qual.ends_with("max"), &c.args, st) {
            return v;
        }
    }
    let is_max = c.qual.ends_with("max");
    // known items (or several arguments) whose keys are known literals: the first extreme one
    let cands: Option<Vec<Value>> = if c.args.len() == 1 { an.iterate(&c.args[0], &c.node, st).items } else { Some(c.args.clone()) };
    if c.args.len() == 1 {
        an.exhaust(&c.args[0], st);
    }
    let default = c.kw("default");
    // a known-empty iterable: the default, else ValueError
    if c.args.len() == 1 && matches!(&cands, Some(its) if its.is_empty()) && !c.args[0].pts.is_empty() {
        return match default {
            Some(d) => d,
            None => {
                an.raise_here("ValueError", st);
                Value::new()
            }
        };
    }
    if let Some(items) = cands.filter(|its| !its.is_empty() && its.len() <= crate::interp::UNROLL_MAX) {
        let keyf = c.kw("key").filter(|k| !k.pts.is_empty());
        let mut keys: Vec<Option<KeyId>> = Vec::new();
        for (i, item) in items.iter().enumerate() {
            let kv = match &keyf {
                Some(f) => {
                    an.frame_mut().unroll.push(i);
                    let r = an.call_value(f, &c.node, None, st, Some(vec![item.clone()]));
                    an.frame_mut().unroll.pop();
                    r
                }
                None => item.clone(),
            };
            keys.push(kv.kv.as_lit().filter(|_| kv.pts.is_empty()));
        }
        // items that order themselves (`__lt__`): compared one against the best so far
        if keys.iter().any(|k| k.is_none()) && keyf.is_none() && !crate::interp::disabled("instorder") && items.iter().all(|x| x.single().map(|o| an.kind(o) == Kind::Instance).unwrap_or(false)) {
            let mut best = 0usize;
            let mut ok = true;
            for i in 1..items.len() {
                let (x, y) = if is_max { (items[best].clone(), items[i].clone()) } else { (items[i].clone(), items[best].clone()) };
                // max: best < item → item; min: item < best → item
                match an.instance_less(&x, &y, &c.node, st) {
                    Some(true) => best = i,
                    Some(false) => {}
                    None => {
                        ok = false;
                        break;
                    }
                }
            }
            if ok {
                return items[best].clone();
            }
        }
        if keys.iter().all(|k| k.is_some()) {
            let mut best = 0usize;
            let mut ok = true;
            for i in 1..items.len() {
                match crate::interp::exprs::literal_order(keys[i].unwrap(), keys[best].unwrap()) {
                    Some(std::cmp::Ordering::Greater) if is_max => best = i,
                    Some(std::cmp::Ordering::Less) if !is_max => best = i,
                    Some(_) => {}
                    None => {
                        ok = false;
                        break;
                    }
                }
            }
            if ok {
                return items[best].clone();
            }
        }
        if c.args.len() > 1 {
            return join_values(c.args.clone());
        }
    }
    if c.args.len() == 1 {
        let it = an.iterate(&c.args[0], &c.node, st);
        let elem = it.elem.one_or_empty();
        if let Some(kf) = c.kw("key") {
            if !kf.pts.is_empty() {
                an.call_value(&kf, &c.node, None, st, Some(vec![elem.clone()]));
            }
        }
        // any number of items: one of them, or the default when there are none
        return match default {
            Some(d) => elem.join(&d),
            None => elem,
        };
    }
    join_values(c.args.clone())
}

fn m_zip(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    // `zip(*rows)` / `zip(a, b, c)`: every input with known items gives the tuples directly
    if (!c.star_args.is_empty() || c.args.len() != 2) && !crate::interp::disabled("zipn") {
        let mut inputs: Option<Vec<Value>> = Some(c.args.clone());
        for sa in &c.star_args {
            match (an.iterate(sa, &c.node, st).items, inputs.as_mut()) {
                (Some(its), Some(ins)) => ins.extend(its),
                _ => inputs = None,
            }
        }
        let cols: Option<Vec<Vec<Value>>> = inputs.and_then(|ins| ins.iter().map(|x| an.iterate(x, &c.node, st).items).collect());
        if let Some(cols) = cols {
            let n = cols.iter().map(|c0| c0.len()).min().unwrap_or(0);
            let mut tuples: Vec<Value> = Vec::new();
            for j in 0..n {
                an.frame_mut().unroll.push(j);
                let vals: Vec<Value> = cols.iter().map(|c0| c0[j].clone()).collect();
                let kv = KeyVal::lits(vals.iter().filter_map(|v| v.kv.as_lit()).collect());
                tuples.push(an.alloc_positional(Kind::Tuple, &c.node, Some(vals), Value::new(), kv, st));
                an.frame_mut().unroll.pop();
            }
            return an.alloc_positional(Kind::List, &c.node, Some(tuples), Value::new(), DATA, st);
        }
    }
    let a = c.args.first().cloned().unwrap_or_default();
    let b = c.args.get(1).cloned().unwrap_or_default();
    let oid = an.alloc_at(Kind::Zip, &c.node, st, &format!("zip@L{}", c.line), Payload::Zip(a.clone(), b.clone()), None, Ns::Item, None, None);
    an.obj_mut(oid).payload = Payload::Zip(a.clone(), b.clone());
    // `strict=True`: lengths that differ raise ValueError; otherwise both have the known one
    let strict = c.kwargs.get(&intern::s("strict")).and_then(|v| v.kv.as_lit()) == Some(intern::key_bool(true));
    if strict && !crate::interp::disabled("zipstrict") {
        an.zip_strict.insert(oid);
        let la = an.iterate(&a, &c.node, st).items.map(|x| x.len());
        let lb = an.iterate(&b, &c.node, st).items.map(|x| x.len());
        match (la, lb) {
            (Some(x), Some(y)) if x != y => {
                an.raise_here("ValueError", st);
                return Value::new();
            }
            (Some(_), Some(_)) => {}
            _ => an.may_raise_here("ValueError", st),
        }
    }
    Value::obj(oid)
}

fn m_enumerate(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let a = c.args.first().cloned().unwrap_or_default();
    // `start=` (or the second positional): indices are known only when it is a known int
    let start = match c.kw("start").or_else(|| c.args.get(1).cloned()) {
        None => Some(0),
        Some(v) => v.kv.as_lit().and_then(intern::key_as_int).filter(|_| v.pts.is_empty()),
    };
    let oid = an.alloc_at(Kind::Enumerate, &c.node, st, &format!("enumerate@L{}", c.line), Payload::Enumerate(a.clone(), start), None, Ns::Item, None, None);
    an.obj_mut(oid).payload = Payload::Enumerate(a, start);
    Value::obj(oid)
}

fn m_next(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(a) = c.args.first() else { return Value::data("next") };
    // a generator with known yields: the item at the cursor, which moves on
    if let Some(g) = a.single() {
        if matches!(an.kind(g), Kind::List) && an.obj(g).single {
            if let Some(n) = an.llen(g, st) {
                let cur = st.slot_opt(g, Ns::Attr, key_str_lit("__cursor")).and_then(|v| v.kv.as_lit()).and_then(intern::key_as_int).unwrap_or(0);
                if cur >= 0 && (cur as usize) < n {
                    st.set_slot(g, Ns::Attr, key_str_lit("__cursor"), Value::lit(intern::key_int(cur + 1)), true);
                    return st.slot(g, Ns::Pos, intern::key_int(cur));
                }
                if cur >= 0 && (cur as usize) >= n && !crate::interp::disabled("nextend") {
                    // exhausted: the default, else StopIteration
                    return match c.args.get(1) {
                        Some(d) => d.clone(),
                        None => {
                            an.raise_here("StopIteration", st);
                            Value::new()
                        }
                    };
                }
            }
        }
    }
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
                let recs = an.record_objects(&c.args[0]);
                let strong = recs.len() == 1 && an.strongable(recs[0]) && !crate::interp::disabled("setattrstrong");
                for o in recs {
                    an.store_key(o, Ns::Attr, &c.args[1].kv, &c.args[2], strong, &c.node, st);
                }
            }
        }
    }
    an.none_v.clone()
}

/// `object.__setattr__(obj, "name", v)`: the plain store (no user `__setattr__`).
fn m_object_setattr(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let saved = std::mem::replace(&mut an.bypass_setattr, true);
    m_setattr(an, st, c);
    an.bypass_setattr = saved;
    an.none_v.clone()
}

fn m_delattr(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    if c.args.len() >= 2 {
        if let Some(name) = c.args[1].kv.as_lit().filter(|_| c.args[1].pts.is_empty()).and_then(intern::key_as_str) {
            an.delete_item(&c.args[0], Ns::Attr, &Value::lit(key_str_lit(&name)), &c.node, st, true);
        }
    }
    an.none_v.clone()
}

/// `hasattr(obj, "name")` on instances of known classes: decided by the attribute rows and
/// the class (a class without `__getattr__`).
fn m_hasattr(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let unknown = Value::data("hasattr");
    if c.args.len() < 2 {
        return unknown;
    }
    let Some(name) = c.args[1].kv.as_lit().filter(|_| c.args[1].pts.is_empty()).and_then(intern::key_as_str) else { return unknown };
    // dunder attributes come from places not modelled (decorators, the type itself)
    if name.starts_with("__") {
        return unknown;
    }
    let k = key_str_lit(&name);
    let objs: Vec<u32> = c.args[0].pts.iter().collect();
    if objs.is_empty() {
        return unknown;
    }
    let mut verdicts: Vec<bool> = Vec::new();
    for o in objs {
        if an.kind(o) == Kind::Ph {
            an.record_event("narrow", o);
            return unknown;
        }
        if an.kind(o) != Kind::Instance {
            return unknown;
        }
        let Some(cls) = an.instance_class(o) else { return unknown };
        if !an.hierarchy_closed(cls) || an.class_lookup(cls, "__getattr__").is_some() {
            return unknown;
        }
        if an.class_lookup(cls, &name).is_some() {
            verdicts.push(true);
            continue;
        }
        let row = st.row(o, Ns::Attr);
        if row.definite.contains(k) {
            verdicts.push(true);
        } else if row.closed() && !row.present.contains(k) {
            verdicts.push(false);
        } else {
            return unknown;
        }
    }
    if verdicts.iter().all(|b| *b) {
        Value::lit(intern::key_bool(true))
    } else if verdicts.iter().all(|b| !*b) {
        Value::lit(intern::key_bool(false))
    } else {
        unknown
    }
}

/// `dataclasses.asdict(obj)`: a dict of the fields, nested dataclass instances (also in
/// lists / tuples) converted the same way.
fn m_asdict(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(a) = c.args.first().cloned() else { return Value::data("asdict") };
    fn conv(an: &mut Analyzer, st: &mut State, c: &Call, v: &Value, depth: u32) -> Value {
        let mut out = Value::with_kv(crate::domain::ObjSet::new(), v.kv.clone());
        for o in v.pts.iter() {
            let cls_of = match an.kind(o) {
                Kind::Instance => an.instance_class(o),
                Kind::Ph => an.ph_info(o).cls.get(),
                _ => None,
            };
            let is_dc = cls_of.map(|cl| !an.dataclass_fields_mro(cl).is_empty()).unwrap_or(false);
            if !is_dc || depth > 4 {
                out = out.join(&Value::obj(o));
                continue;
            }
            let cls = cls_of.unwrap();
            if an.kind(o) == Kind::Ph {
                // a placeholder typed with a dataclass: its fields read through it
                let d = an.alloc_at(Kind::Dict, &c.node, st, &format!("asdict@L{}", c.line), Payload::None, Some(Row::new()), Ns::Item, None, Some(&format!("asdict{}:{}", depth, o)));
                st.set_row(d, Ns::Item, Row::new());
                for (f, _) in an.dataclass_fields_mro(cls) {
                    let fv = an.load_attr(&Value::obj(o), &f, &c.node, st, false);
                    let cv = conv(an, st, c, &fv, depth + 1);
                    an.store_key(d, Ns::Item, &KeyVal::Lit(key_str_lit(&f)), &cv, true, &c.node, st);
                }
                out = out.join(&Value::obj(d));
                continue;
            }
            // one dict per converted instance (siblings at one depth must not share it)
            let d = an.alloc_at(Kind::Dict, &c.node, st, &format!("asdict@L{}", c.line), Payload::None, Some(Row::new()), Ns::Item, None, Some(&format!("asdict{}:{}", depth, o)));
            st.set_row(d, Ns::Item, Row::new());
            for (f, _) in an.dataclass_fields_mro(cls) {
                let fk = key_str_lit(&f);
                let row = st.row(o, Ns::Attr);
                if !row.present.contains(fk) {
                    continue;
                }
                let fv = st.slot(o, Ns::Attr, fk);
                let cv = conv(an, st, c, &fv, depth + 1);
                an.store_key(d, Ns::Item, &KeyVal::Lit(fk), &cv, row.definite.contains(fk), &c.node, st);
            }
            out = out.join(&Value::obj(d));
        }
        out
    }
    if a.pts.iter().all(|o| an.kind(o) == Kind::Instance && an.instance_class(o).map(|cl| !an.dataclass_fields_mro(cl).is_empty()).unwrap_or(false)) && !a.pts.is_empty() {
        return conv(an, st, c, &a, 0);
    }
    if a.pts.iter().all(|o| an.kind(o) == Kind::Ph && an.ph_info(o).cls.get().map(|cl| !an.dataclass_fields_mro(cl).is_empty()).unwrap_or(false)) && !a.pts.is_empty() {
        return conv(an, st, c, &a, 0);
    }
    m_vars(an, st, c)
}

/// `model.model_dump()` / `model.dict()`: like `asdict` on the model.
fn m_dc_fields(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(a) = c.args.first() else { return Value::data("fields") };
    let cls = a.single().and_then(|o| match an.kind(o) {
        Kind::Class => Some(o),
        Kind::Instance => an.instance_class(o),
        _ => None,
    });
    let Some(cls) = cls else { return Value::data("fields") };
    let names: Vec<String> = an.dataclass_fields_mro(cls).into_iter().map(|(f, _)| f).collect();
    let mut vals: Vec<Value> = Vec::new();
    for (i, f) in names.iter().enumerate() {
        an.frame_mut().unroll.push(i);
        let row = Row::with_source(Source::new("dataclasses.Field", SourceKind::Data));
        let fo = an.alloc_at(Kind::Namespace, &c.node, st, &format!("Field {}", f), Payload::None, Some(row), Ns::Attr, None, None);
        an.frame_mut().unroll.pop();
        an.store_key(fo, Ns::Attr, &KeyVal::Lit(key_str_lit("name")), &Value::lit(key_str_lit(f)), true, &c.node, st);
        vals.push(Value::obj(fo));
    }
    an.alloc_positional(Kind::Tuple, &c.node, Some(vals), Value::new(), DATA, st)
}

fn m_vars(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(a) = c.args.first() else { return Value::data("vars") };
    // a parameter's vars(): the live view of its attributes (as `obj.__dict__`)
    if c.qual == "builtins.vars" && !crate::interp::disabled("varsview") {
        if let Some(p) = a.single().filter(|o| an.kind(*o) == Kind::Ph) {
            let vid = an.alloc_raw(Kind::View, &format!("view:attrs:{}", p), "", true, "__dict__", Payload::View(ViewKind::Attrs, Value::obj(p)));
            an.obj_mut(vid).payload = Payload::View(ViewKind::Attrs, Value::obj(p));
            return Value::obj(vid);
        }
    }
    let oid = an.alloc_simple(Kind::Dict, &c.node, st, &format!("vars@L{}", c.line));
    if c.qual == "builtins.vars" {
        if let Some(inst) = a.single().filter(|o| an.kind(*o) == Kind::Instance) {
            an.attr_mirrors.insert(oid, inst);
        }
    }
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
    an.set_llen(oid, st, None);
    if c.args.len() >= 2 {
        let it = an.iterate(&c.args[1], &c.node, st);
        if c.qual.ends_with("filter") && !crate::interp::disabled("filteritems") {
            if let Some(items) = it.items.as_ref().filter(|its| its.len() <= crate::interp::UNROLL_MAX) {
                // known items: the predicate per item (None: the item's truth); decided for all
                // of them, the kept items keep their positions
                let mut kept: Vec<Value> = Vec::new();
                let mut all_decided = true;
                for (i, item) in items.iter().enumerate() {
                    let truth = if c.args[0].pts.is_empty() {
                        an.truthiness(item, st)
                    } else {
                        an.frame_mut().unroll.push(i);
                        let r = an.call_value(&c.args[0], &c.node, None, st, Some(vec![item.clone()]));
                        an.frame_mut().unroll.pop();
                        an.truthiness(&r, st)
                    };
                    match truth {
                        Some(false) => {}
                        Some(true) => kept.push(item.clone()),
                        None => {
                            all_decided = false;
                            kept.push(item.clone());
                        }
                    }
                }
                let res = join_values(kept.clone());
                st.set_slot(oid, Ns::Elem, STAR, Value::with_kv(res.pts.clone(), crate::models::torch::elem_kv(&res.kv)), true);
                st.elems.insert(oid, res.kv);
                if all_decided && an.obj(oid).single {
                    an.set_llen(oid, st, Some(kept.len()));
                    for (i, v) in kept.into_iter().enumerate() {
                        st.set_slot(oid, Ns::Pos, intern::key_int(i as i64), v, true);
                    }
                }
                return Value::obj(oid);
            }
        }
        // `map(f, a, b, …)`: f gets one item of each (as many as the shortest has)
        let others: Vec<crate::interp::funcs::Iteration> = c.args.iter().skip(2).map(|a| an.iterate(a, &c.node, st)).collect();
        if !c.qual.ends_with("filter") && an.obj(oid).single {
            let n_known = std::iter::once(it.items.as_ref().map(|x| x.len())).chain(others.iter().map(|o| o.items.as_ref().map(|x| x.len()))).collect::<Option<Vec<usize>>>().and_then(|v| v.into_iter().min());
            if let (Some(items), Some(n)) = (&it.items, n_known) {
                if items.len() <= crate::interp::UNROLL_MAX {
                    // known items: the function per item, positions kept
                    let mut outs = Vec::new();
                    for (i, item) in items.iter().take(n).enumerate() {
                        an.frame_mut().unroll.push(i);
                        let mut call_args = vec![item.clone()];
                        call_args.extend(others.iter().map(|o| o.items.as_ref().unwrap()[i].clone()));
                        let r = an.call_value(&c.args[0], &c.node, None, st, Some(call_args));
                        an.frame_mut().unroll.pop();
                        st.set_slot(oid, Ns::Pos, intern::key_int(i as i64), r.clone(), true);
                        outs.push(r);
                    }
                    let res = join_values(outs);
                    st.set_slot(oid, Ns::Elem, STAR, Value::with_kv(res.pts.clone(), crate::models::torch::elem_kv(&res.kv)), true);
                    st.elems.insert(oid, res.kv);
                    an.set_llen(oid, st, Some(n.min(items.len())));
                    return Value::obj(oid);
                }
            }
        }
        let elem = it.elem.one_or_empty();
        let res = if c.qual.ends_with("filter") {
            // the predicate still runs on the items (its reads are the program's)
            if !c.args[0].pts.is_empty() {
                an.call_value(&c.args[0], &c.node, None, st, Some(vec![elem.clone()]));
            }
            elem
        } else {
            let mut call_args = vec![elem];
            call_args.extend(others.iter().map(|o| o.elem.one_or_empty()));
            an.call_value(&c.args[0], &c.node, None, st, Some(call_args))
        };
        st.set_slot(oid, Ns::Elem, STAR, Value::with_kv(res.pts.clone(), crate::models::torch::elem_kv(&res.kv)), true);
        st.elems.insert(oid, res.kv);
    }
    Value::obj(oid)
}

/// `reduce(f, xs[, init])`: over known items the function item by item; otherwise one
/// application on any element, joined with the start value.
fn m_reduce(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let (Some(f), Some(xs)) = (c.args.first().cloned(), c.args.get(1).cloned()) else { return Value::data("reduce") };
    let init = c.args.get(2).cloned();
    let it = an.iterate(&xs, &c.node, st);
    if let Some(items) = it.items.filter(|its| its.len() <= crate::interp::UNROLL_MAX) {
        let mut rest = items.into_iter();
        let mut acc = match init {
            Some(v) => v,
            None => match rest.next() {
                Some(v) => v,
                None => {
                    an.raise_here("TypeError", st);
                    return Value::new();
                }
            },
        };
        for (i, item) in rest.enumerate() {
            an.frame_mut().unroll.push(i);
            acc = an.call_value(&f, &c.node, None, st, Some(vec![acc, item]));
            an.frame_mut().unroll.pop();
        }
        return acc;
    }
    let elem = it.elem.one_or_empty();
    let start = init.unwrap_or_else(|| elem.clone());
    let r = an.call_value(&f, &c.node, None, st, Some(vec![start.clone(), elem]));
    r.join(&start)
}

/// `type(x)` with one argument: the class of an instance of a known class (else unknown).
fn m_type(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    // `type("Name", (), {...})`: a class whose attributes are the namespace's items
    if c.args.len() == 3 && !crate::interp::disabled("type3") {
        let name = c.args[0].kv.as_lit().filter(|_| c.args[0].pts.is_empty()).and_then(intern::key_as_str);
        let no_bases = c.args[1].single().map(|o| an.llen(o, st) == Some(0)).unwrap_or(false);
        let ns = c.args[2].single().filter(|o| an.kind(*o) == Kind::Dict);
        if let (Some(name), true, Some(ns)) = (name.filter(|n| n.chars().all(|ch| ch.is_alphanumeric() || ch == '_')), no_bases, ns) {
            let row = st.row(ns, Ns::Item);
            if row.closed() && row.present == row.definite {
                let modname = format!("<type {} {}>", name, c.site);
                let moid = match an.modules.get(&modname) {
                    Some(&m) => m,
                    None => {
                        let Ok((idx, tree)) = crate::frontend::parse(&modname, None, format!("class {}:\n    pass\n", name)) else { return Value::data("type") };
                        let m = an.make_module_pub(&modname, None, idx, tree);
                        an.run_module(m, st);
                        m
                    }
                };
                // (made in another state: run it in this one too)
                if st.slot_opt(moid, Ns::Attr, key_str_lit(&name)).is_none() {
                    an.run_module(moid, st);
                }
                let cv = st.slot(moid, Ns::Attr, key_str_lit(&name));
                if let Some(cls) = cv.single() {
                    for k in row.present.iter() {
                        if intern::key_as_str(k).is_some() {
                            let v = st.slot(ns, Ns::Item, k);
                            an.cls_slots.insert((cls, Ns::Attr, k), v);
                        }
                    }
                }
                return cv;
            }
        }
        return an.unknown_call(&c.qual, None, &c.args, &c.kwargs, &c.star_args, &c.star_kwargs, st, &c.node);
    }
    if c.args.len() == 1 {
        let a = &c.args[0];
        let classes: Option<Vec<u32>> = if a.pts.is_empty() { None } else { a.pts.iter().map(|o| if an.kind(o) == Kind::Instance { an.instance_class(o) } else { None }).collect() };
        if let Some(cs) = classes {
            return join_values(cs.into_iter().map(Value::obj).collect::<Vec<Value>>());
        }
        // builtin values: their builtin class (the object the builtin name is)
        if !crate::interp::disabled("typebuiltin") {
            let builtin = |an: &mut Analyzer, n: &str| -> Value { Value::obj(an.alloc_raw(Kind::External, &format!("ext:builtins.{}", n), "", true, n, Payload::Str(format!("builtins.{}", n)))) };
            let mut outs: Vec<Value> = Vec::new();
            let mut known = true;
            for o in a.pts.iter() {
                let n = match an.kind(o) {
                    Kind::Dict if !crate::models::pandas::is_series_obj(an, o) => "dict",
                    Kind::List => "list",
                    Kind::Tuple => "tuple",
                    Kind::Set => "set",
                    Kind::Ph => {
                        an.record_event("concrete", o);
                        known = false;
                        continue;
                    }
                    _ => {
                        known = false;
                        continue;
                    }
                };
                outs.push(builtin(an, n));
            }
            match &a.kv {
                KeyVal::Bot => {}
                kv if kv.is_literal() => {
                    for k in crate::domain::kv_literals(kv).iter() {
                        let n = intern::with_key(k, |kk| match kk {
                            intern::Key::Str(_) => "str",
                            intern::Key::Int(_) => "int",
                            intern::Key::Float(_) => "float",
                            intern::Key::Bool(_) => "bool",
                            intern::Key::None => "NoneType",
                            intern::Key::Bytes(_) => "bytes",
                            intern::Key::Tuple(_) => "tuple",
                            _ => "",
                        });
                        if n.is_empty() {
                            known = false;
                        } else {
                            outs.push(builtin(an, n));
                        }
                    }
                }
                KeyVal::Param(p) => {
                    an.record_event("concrete", *p);
                    known = false;
                }
                _ => known = false,
            }
            if known && !outs.is_empty() {
                return join_values(outs);
            }
        }
    }
    Value::data(&format!("type@L{}", c.line))
}

fn m_str(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    if let Some(a) = c.args.first() {
        if let Some(t) = an.enum_text(a, st) {
            return t;
        }
        if let Some(t) = an.exc_text(a, false, st) {
            return t;
        }
        return Value::kv_only(an.rename_kv(&a.kv, &RenameFn::new(FnKind::Str)));
    }
    Value::lit_str("")
}

fn m_repr(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    if let Some(t) = c.args.first().and_then(|a| an.exc_text(a, true, st)) {
        return t;
    }
    noeffect(an, st, c)
}

/// `int(x)` / `float(x)`: on a known number or numeric string the value; on a known string
/// that is not a number a ValueError raised here; on anything else a value that may raise.
fn m_int(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let is_float = c.qual == "builtins.float";
    let Some(a) = c.args.first() else { return Value::lit(intern::key_int(0)) };
    // int(d / k) of a (non-negative) dim: d // k
    if !is_float {
        if let Some((d, k)) = crate::models::torch::quotient_of(a) {
            return crate::models::torch::dim_value(crate::shape::norm(crate::shape::floordiv(d, crate::shape::lit(k)), &st.tf));
        }
        if crate::models::torch::dim_of_label(a).is_some() {
            return a.clone();
        }
    }
    if c.args.len() > 1 || !c.kwargs.is_empty() {
        an.may_raise_here("ValueError", st);
        return Value::data(&format!("L{}", c.line));
    }
    let num = |k: KeyId| -> Option<Result<intern::Key, ()>> {
        intern::with_key(k, |kk| match kk {
            intern::Key::Int(i) => Some(Ok(if is_float { intern::Key::Float((*i as f64).to_bits()) } else { intern::Key::Int(*i) })),
            intern::Key::Bool(b) => Some(Ok(intern::Key::Int(*b as i64))),
            intern::Key::Float(f) => Some(Ok(if is_float { intern::Key::Float(*f) } else { intern::Key::Int(f64::from_bits(*f).trunc() as i64) })),
            intern::Key::Str(s) => {
                let t = s.trim().replace('_', "");
                if is_float {
                    Some(t.parse::<f64>().map(|f| intern::Key::Float(f.to_bits())).map_err(|_| ()))
                } else {
                    Some(t.parse::<i64>().map(intern::Key::Int).map_err(|_| ()))
                }
            }
            _ => None,
        })
    };
    if a.pts.is_empty() {
        if let Some(k) = a.kv.as_lit() {
            match num(k) {
                Some(Ok(key)) => return Value::lit(intern::key(key)),
                Some(Err(())) => {
                    an.raise_here("ValueError", st);
                    return Value::new();
                }
                None => {}
            }
        }
    }
    for o in a.pts.iter() {
        if an.kind(o) == Kind::Ph {
            // decided per call site when the argument is a literal there
            an.record_event("narrow", o);
        }
    }
    if let KeyVal::Param(ph) = &a.kv {
        an.record_event("narrow", *ph);
    }
    an.may_raise_here("ValueError", st);
    Value::data(&format!("L{}", c.line))
}

fn m_super(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(foid) = an.frame().func else { return Value::data("super") };
    let fi = an.func_info(foid);
    let Some(cls) = fi.cls else { return Value::data("super") };
    // the function's first parameter (`self`, or `cls` in a classmethod / __init_subclass__)
    let first = match fi.node {
        crate::interp::FuncNode::Def(f) => crate::scopes::all_params(&f.parameters).first().map(|p| p.name.id.to_string()).unwrap_or_else(|| "self".into()),
        _ => "self".into(),
    };
    let mut self_v = st.env.get(&s(&first)).cloned().unwrap_or_default();
    let cells = an.frame().cells.clone();
    for (name, cid) in cells.iter() {
        if *intern::str_get(*name) == *first {
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

/// `"{} {name:>4}".format(...)` with a known template and known arguments.
/// `t.format(*args, **kwargs)` / `t.format_map(mapping)`: every field is looked up (names
/// not given as keywords from `**mapping` / the mapping, `.attr` / `[key]` accessors read
/// there, so a missing key is reported); the text when every piece is a known literal.
fn format_fields(an: &mut Analyzer, st: &mut State, t: &str, c: &Call, mapping: Option<&Value>) -> Option<String> {
    let lit_of = |v: &Value| -> Option<KeyId> { v.kv.as_lit().filter(|_| v.pts.is_empty()) };
    let chars: Vec<char> = t.chars().collect();
    let mut out = Some(String::new());
    let push = |out: &mut Option<String>, p: Option<String>| {
        match (out.as_mut(), p) {
            (Some(o), Some(p)) => o.push_str(&p),
            _ => *out = None,
        }
    };
    let mut auto = 0usize;
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '{' && chars.get(i + 1) == Some(&'{') {
            push(&mut out, Some("{".into()));
            i += 2;
            continue;
        }
        if ch == '}' && chars.get(i + 1) == Some(&'}') {
            push(&mut out, Some("}".into()));
            i += 2;
            continue;
        }
        if ch == '}' {
            return None;
        }
        if ch != '{' {
            push(&mut out, Some(ch.to_string()));
            i += 1;
            continue;
        }
        // the field runs to the matching `}` (a nested `{w}` in the spec counts)
        let mut depth = 0;
        let mut end = None;
        for (j, x) in chars.iter().enumerate().skip(i + 1) {
            match x {
                '{' => depth += 1,
                '}' if depth == 0 => {
                    end = Some(j);
                    break;
                }
                '}' => depth -= 1,
                _ => {}
            }
        }
        let end = end?;
        let field: String = chars[i + 1..end].iter().collect();
        i = end + 1;
        // field_name [!conv] [:spec]; `[` … `]` may hold `:` / `!`
        let mut name_end = field.len();
        let mut in_br = false;
        for (j, x) in field.char_indices() {
            match x {
                '[' => in_br = true,
                ']' => in_br = false,
                '!' | ':' if !in_br => {
                    name_end = j;
                    break;
                }
                _ => {}
            }
        }
        let name = &field[..name_end];
        let rest = &field[name_end..];
        let (conv, spec) = if let Some(r) = rest.strip_prefix('!') {
            let (cv, sp) = r.split_at(r.find(':').unwrap_or(r.len()));
            (Some(cv.to_string()), sp.strip_prefix(':').unwrap_or("").to_string())
        } else {
            (None, rest.strip_prefix(':').unwrap_or("").to_string())
        };
        let head_end = name.find(|x| x == '.' || x == '[').unwrap_or(name.len());
        let head = &name[..head_end];
        let mut v: Value = if head.is_empty() || head.chars().all(|x| x.is_ascii_digit()) {
            let n = if head.is_empty() {
                auto += 1;
                auto - 1
            } else {
                head.parse::<usize>().ok()?
            };
            match c.args.get(n) {
                Some(v) if c.star_args.is_empty() || n < c.args.len() => v.clone(),
                _ => {
                    push(&mut out, None);
                    continue;
                }
            }
        } else if let Some(v) = c.kwargs.get(&s(head)) {
            v.clone()
        } else if let Some(m) = mapping {
            an.load_item(m, Ns::Item, &Value::lit(key_str_lit(head)), &c.node, st, true)
        } else if !c.star_kwargs.is_empty() && !crate::interp::disabled("fmtkw") {
            let mut acc = Value::new();
            for sk in c.star_kwargs.clone() {
                acc = acc.join(&an.load_item(&sk, Ns::Item, &Value::lit(key_str_lit(head)), &c.node, st, true));
            }
            acc
        } else {
            push(&mut out, None);
            continue;
        };
        // accessors
        let mut acc = &name[head_end..];
        while !acc.is_empty() {
            if let Some(r) = acc.strip_prefix('.') {
                let e = r.find(|x| x == '.' || x == '[').unwrap_or(r.len());
                v = an.load_attr(&v, &r[..e], &c.node, st, true);
                acc = &r[e..];
            } else if let Some(r) = acc.strip_prefix('[') {
                let e = r.find(']')?;
                let k = &r[..e];
                let key = match k.parse::<i64>() {
                    Ok(n) if k.chars().all(|x| x.is_ascii_digit()) => intern::key_int(n),
                    _ => key_str_lit(k),
                };
                v = an.load_item(&v, Ns::Item, &Value::lit(key), &c.node, st, true);
                acc = &r[e + 1..];
            } else {
                return None;
            }
        }
        if out.is_none() || spec.contains('{') {
            out = None;
            continue;
        }
        let Some(k) = lit_of(&v) else {
            out = None;
            continue;
        };
        let k = match conv.as_deref() {
            None | Some("s") => k,
            Some("r") => match crate::models::pyvalues::builtin("repr", &[k], &[]) {
                Some(x) => x,
                None => {
                    out = None;
                    continue;
                }
            },
            Some("a") => match crate::models::pyvalues::builtin("ascii", &[k], &[]) {
                Some(x) => x,
                None => {
                    out = None;
                    continue;
                }
            },
            _ => return None,
        };
        let piece = if spec.is_empty() { Some(intern::key_py_str(k)) } else { crate::interp::exprs::format_spec_literal(k, &spec) };
        push(&mut out, piece);
    }
    out
}

fn m_str_method(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = last(&c.qual);
    let kv = c.recv.as_ref().map(|r| r.kv.clone()).unwrap_or(DATA);
    match name {
        "upper" | "lower" | "strip" | "title" => Value::kv_only(an.rename_kv(&kv, &RenameFn::new(FnKind::from_name(name).unwrap()))),
        "lstrip" | "rstrip" => Value::kv_only(an.rename_kv(&kv, &RenameFn::new(FnKind::Strip))),
        // positions / counts of a known substring in a known string
        "find" | "index" | "rfind" | "rindex" | "count" if c.args.len() == 1 => {
            let recv_s = kv.as_lit().and_then(intern::key_as_str);
            let arg_s = c.args[0].kv.as_lit().filter(|_| c.args[0].pts.is_empty()).and_then(intern::key_as_str);
            match (recv_s, arg_s) {
                (Some(h), Some(n)) => {
                    let pos = |p: Option<usize>| p.map(|b| h[..b].chars().count() as i64);
                    let r = match name {
                        "find" | "index" => pos(h.find(&*n)),
                        "rfind" | "rindex" => pos(h.rfind(&*n)),
                        _ => Some(if n.is_empty() { h.chars().count() as i64 + 1 } else { h.matches(&*n).count() as i64 }),
                    };
                    match r {
                        Some(i) => Value::lit(intern::key_int(i)),
                        None if name.ends_with("index") => {
                            an.raise_here("ValueError", st);
                            Value::new()
                        }
                        None => Value::lit(intern::key_int(-1)),
                    }
                }
                _ => Value::data(&format!("L{}", c.line)),
            }
        }
        "capitalize" | "casefold" | "swapcase" | "removeprefix" | "removesuffix" | "zfill" | "center" | "ljust" | "rjust" => {
            // on known strings (and known arguments) the result is known
            let arg_s = c.args.first().and_then(|a| a.kv.as_lit().filter(|_| a.pts.is_empty())).and_then(intern::key_as_str);
            let arg_i = c.args.first().and_then(|a| a.kv.as_lit().filter(|_| a.pts.is_empty())).and_then(intern::key_as_int);
            let f = |s: &str| -> Option<String> {
                Some(match name {
                    "capitalize" => {
                        let mut cs = s.chars();
                        match cs.next() {
                            Some(f0) => f0.to_uppercase().collect::<String>() + &cs.as_str().to_lowercase(),
                            None => String::new(),
                        }
                    }
                    "casefold" => s.to_lowercase(),
                    "swapcase" => s.chars().map(|ch| if ch.is_uppercase() { ch.to_lowercase().collect::<String>() } else { ch.to_uppercase().collect() }).collect(),
                    "removeprefix" => s.strip_prefix(&*arg_s.clone()?).unwrap_or(s).to_string(),
                    "removesuffix" => s.strip_suffix(&*arg_s.clone()?).unwrap_or(s).to_string(),
                    _ => {
                        let w = arg_i? as usize;
                        let n = s.chars().count();
                        if n >= w || c.args.len() > 1 {
                            if c.args.len() > 1 {
                                return None;
                            }
                            s.to_string()
                        } else {
                            let pad = w - n;
                            match name {
                                "zfill" => {
                                    let (sign, rest) = if s.starts_with('-') || s.starts_with('+') { s.split_at(1) } else { ("", s) };
                                    format!("{}{}{}", sign, "0".repeat(pad), rest)
                                }
                                "ljust" => format!("{}{}", s, " ".repeat(pad)),
                                "rjust" => format!("{}{}", " ".repeat(pad), s),
                                _ => format!("{}{}{}", " ".repeat(pad / 2), s, " ".repeat(pad - pad / 2)),
                            }
                        }
                    }
                })
            };
            let lits = crate::domain::kv_literals(&kv);
            if kv.is_literal() && !lits.is_empty() && c.recv.as_ref().map(|r| r.pts.is_empty()).unwrap_or(false) {
                let mut out: Vec<KeyId> = Vec::new();
                for k in lits.iter() {
                    match intern::key_as_str(k).and_then(|s| f(&s)) {
                        Some(r) => out.push(key_str_lit(&r)),
                        None => return Value::data("str"),
                    }
                }
                return Value::kv_only(KeyVal::lits(out.into_iter().collect()));
            }
            if let Some(ph) = kv.as_param() {
                an.record_event("narrow", ph);
            }
            Value::data("str")
        }
        "startswith" | "endswith" if c.args.len() == 1 && kv.as_lit().and_then(intern::key_as_str).is_some() && c.args[0].kv.as_lit().and_then(intern::key_as_str).is_some() => {
            let s = intern::key_as_str(kv.as_lit().unwrap()).unwrap();
            let p = intern::key_as_str(c.args[0].kv.as_lit().unwrap()).unwrap();
            Value::lit(intern::key_bool(if name == "startswith" { s.starts_with(&*p) } else { s.ends_with(&*p) }))
        }
        "startswith" | "endswith" if c.args.len() == 1 => {
            if let Some(ph) = kv.as_param() {
                an.record_event("narrow", ph);
            }
            Value::data("bool")
        }
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
            an.set_llen(oid, st, Some(3));
            Value::obj(oid)
        }
        "replace" if c.args.len() >= 2 && c.args[0].kv.as_lit().is_some() && c.args[1].kv.as_lit().is_some() => {
            let arg = intern::key_tuple(vec![c.args[0].kv.as_lit().unwrap(), c.args[1].kv.as_lit().unwrap()]);
            Value::kv_only(an.rename_kv(&kv, &RenameFn::with_arg(FnKind::Replace, arg)))
        }
        "format" | "format_map" => {
            let mapping = if name == "format_map" { c.args.first().cloned() } else { None };
            if name == "format_map" && mapping.is_none() {
                return Value::data(&format!("L{}", c.line));
            }
            if let Some(t) = kv.as_lit().and_then(intern::key_as_str) {
                if let Some(out) = format_fields(an, st, &t, c, mapping.as_ref()) {
                    return Value::lit(key_str_lit(&out));
                }
                // fields from parameters: decided per call site
                an.concrete_args(c.args.iter().chain(c.kwargs.values()).chain(c.star_kwargs.iter()), st);
            }
            if name == "format_map" {
                return Value::data(&format!("L{}", c.line));
            }
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
        "join" => {
            // a known separator joining known strings
            if let (Some(sep), Some(arg)) = (kv.as_lit().and_then(intern::key_as_str), c.args.first()) {
                // a literal tuple key's parts; a column key (`for c in df.columns`): the key renamed
                if arg.pts.is_empty() {
                    let step = crate::domain::Step::RenameFn(crate::domain::RenameFn::with_arg(crate::domain::FnKind::Join, key_str_lit(&sep)));
                    if let crate::domain::KeyVal::KeysOf(ko) = &arg.kv {
                        let mut ko = ko.clone();
                        ko.chain = crate::domain::lens_push(ko.chain, step);
                        return Value::kv_only(crate::domain::KeyVal::KeysOf(ko));
                    }
                    let ks = crate::domain::kv_literals(&arg.kv);
                    if !ks.is_empty() && ks.iter().all(|k| intern::with_key(k, |kk| matches!(kk, intern::Key::Tuple(_)))) {
                        let out = intern::KeySet::from_iter(ks.iter().map(|k| step.apply(k)));
                        return Value::kv_only(crate::domain::KeyVal::lits(out));
                    }
                }
                let it = an.iterate(arg, &c.node, st);
                if let Some(items) = it.items {
                    let mut parts: Vec<String> = Vec::new();
                    let mut ok = true;
                    for item in &items {
                        match item.kv.as_lit().filter(|_| item.pts.is_empty()).and_then(intern::key_as_str) {
                            Some(s) => parts.push(s.to_string()),
                            None => {
                                ok = false;
                                break;
                            }
                        }
                    }
                    if ok {
                        return Value::lit(key_str_lit(&parts.join(&*sep)));
                    }
                }
                if let Some(ph) = arg.kv.as_param() {
                    an.record_event("narrow", ph);
                }
            }
            Value::data(&format!("L{}", c.line))
        }
        "split" | "splitlines" | "partition" | "rsplit" => {
            let oid = an.alloc_simple(Kind::List, &c.node, st, &format!("split@L{}", c.line));
            an.set_llen(oid, st, None);
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
                        an.set_llen(oid, st, Some(parts.len()));
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
    // (a key that may be an object: an unknown key)
    let okey = an.opaque_key(key, st);
    let key = &okey;
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
    let okey = an.opaque_key(key, st);
    let key = &okey;
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
    let ll = an.llen(oid, st);
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
                an.set_llen(oid, st, Some(n - 1));
                return v;
            }
        }
    }
    an.set_llen(oid, st, None);
    an.elem_of(&Value::obj(oid), &c.node, st)
}

fn m_pop(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    st.drop_name_keys(None);
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
    st.drop_name_keys(None);
    let recs = dict_recv(an, c);
    // one closed dict whose insertion order is known: the last key is removed (the first
    // with `last=False`)
    let first = c.kw("last").or_else(|| c.args.first().cloned()).map(|v| v.kv.as_lit() == Some(intern::key_bool(false))).unwrap_or(false);
    if let [oid] = recs[..] {
        let row = st.row(oid, Ns::Item);
        if an.obj(oid).single && row.closed() && row.present == row.definite {
            if let Some(last_k) = row.ordered_keys().and_then(|ks| if first { ks.first().copied() } else { ks.last().copied() }) {
                let v = st.slot(oid, Ns::Item, last_k);
                st.set_row(oid, Ns::Item, row.delete(last_k, true));
                let tid = an.alloc_positional(Kind::Tuple, &c.node, Some(vec![Value::lit(last_k), v]), Value::new(), DATA, st);
                return tid;
            }
        }
    }
    for oid in recs {
        let row = st.row(oid, Ns::Item);
        st.set_row(oid, Ns::Item, Row::from_data(RowData { present: row.present.clone(), sources: row.sources.clone(), mutators: row.mutators.clone(), top: row.top, ..Default::default() }));
    }
    Value::data("popitem")
}

/// `od.move_to_end(k, last=True)` on one closed dict with a known order.
fn m_move_to_end(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let recs = dict_recv(an, c);
    // the key is looked up: an access (KeyError when it is not there)
    if let (Some(kv), Some(recv)) = (c.args.first(), c.recv.clone()) {
        if !crate::interp::disabled("mtaccess") {
            let row0 = recs.first().map(|o| st.row(*o, Ns::Item));
            an.load_item(&recv, Ns::Item, kv, &c.node, st, true);
            if let (Some(k), Some(r0)) = (kv.kv.as_lit().filter(|_| kv.pts.is_empty()), row0) {
                if recs.len() == 1 && r0.closed() && !r0.present.contains(k) {
                    an.raise_here("KeyError", st);
                    return Value::new();
                }
            }
        }
    }
    let k = c.args.first().and_then(|a| a.kv.as_lit().filter(|_| a.pts.is_empty()));
    let to_front = c.kw("last").or_else(|| c.args.get(1).cloned()).map(|v| v.kv.as_lit() == Some(intern::key_bool(false))).unwrap_or(false);
    if let ([oid], Some(k)) = (&recs[..], k) {
        let row = st.row(*oid, Ns::Item);
        if let Some(mut ks) = row.ordered_keys().filter(|ks| ks.contains(&k) && row.definite.contains(k)) {
            ks.retain(|x| *x != k);
            if to_front {
                ks.insert(0, k);
            } else {
                ks.push(k);
            }
            st.set_row(*oid, Ns::Item, row.with_order(ks));
            return an.none_v.clone();
        }
        if row.closed() && !row.present.contains(k) {
            an.raise_here("KeyError", st);
            return Value::new();
        }
    }
    for o in recs {
        // the key order is no longer known
        let row = st.row(o, Ns::Item);
        st.set_row(o, Ns::Item, row.with_order(Vec::new()));
    }
    an.none_v.clone()
}

fn m_clear(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    st.drop_name_keys(None);
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
            let ll = an.llen(oid, st);
            if let Some(n) = ll {
                let cap = if an.torch_on { 64 } else { 16 };
                if an.obj(oid).single && an.frame().loop_depth == 0 && n + items.len() <= cap {
                    for (i, item) in items.iter().enumerate() {
                        st.set_slot(oid, Ns::Pos, intern::key_int((n + i) as i64), item.clone(), true);
                        st.set_slot(oid, Ns::Elem, STAR, Value::with_kv(item.pts.clone(), crate::models::torch::elem_kv(&item.kv)), false);
                        let nk = match st.elems.get(&oid) {
                            Some(cur) => kv_join(&cur, &item.kv),
                            None => item.kv.clone(),
                        };
                        st.elems.insert(oid, nk);
                    }
                    an.set_llen(oid, st, Some(n + items.len()));
                    continue;
                }
            }
        }
        st.set_slot(oid, Ns::Elem, STAR, Value::with_kv(elem.pts.clone(), crate::models::torch::elem_kv(&elem.kv)), false);
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
        let ll = an.llen(oid, st);
        let single = an.obj(oid).single;
        if let Some(n) = ll {
            if single && name == "append" && an.frame().loop_depth == 0 && n < if an.torch_on { 64 } else { 16 } {
                st.set_slot(oid, Ns::Pos, intern::key_int(n as i64), elem, true);
                an.set_llen(oid, st, Some(n + 1));
                continue;
            }
            // appends of unknown count: the positions so far stay valid
            if name == "append" || name == "extend" {
                an.set_prefix(oid, st, n);
                continue;
            }
            st.drop_obj_ns(oid, Ns::Pos);
        } else if name == "append" || name == "extend" {
            // the length was unknown already: the known prefix stays
            continue;
        }
        an.set_llen(oid, st, None);
    }
    // the last element after an append (whatever the length)
    if name == "append" && !crate::interp::disabled("lastelem") {
        if let (Some(oid), Some(x)) = (recv.single(), c.args.last()) {
            if (an.kind(oid) == Kind::List && an.obj(oid).single) || an.kind(oid) == Kind::Ph {
                st.set_last(oid, x.clone());
            }
        }
    }
    an.none_v.clone()
}

/// `slice(stop)` / `slice(start, stop[, step])`: the label a `a:b:c` subscript makes (known
/// int / None bounds), else an unknown slice.
fn m_slice(an: &mut Analyzer, _st: &mut State, c: &Call) -> Value {
    let none = intern::key_none();
    // a parameter bound: known at the call site
    for a in &c.args {
        if let Some(p) = a.kv.as_param() {
            an.record_event("concrete", p);
        }
    }
    let b = |v: &Value| -> Option<String> {
        if !v.pts.is_empty() {
            return None;
        }
        let k = v.kv.as_lit()?;
        if k == none {
            return Some(String::new());
        }
        intern::key_as_int(k).filter(|_| !intern::with_key(k, |kk| matches!(kk, intern::Key::Bool(_)))).map(|i| i.to_string())
    };
    let parts: Option<Vec<String>> = match c.args.len() {
        1 => b(&c.args[0]).map(|h| vec![String::new(), h, String::new()]),
        2 | 3 => {
            let mut v: Vec<Option<String>> = c.args.iter().map(b).collect();
            if v.len() == 2 {
                v.push(Some(String::new()));
            }
            v.into_iter().collect()
        }
        _ => None,
    };
    match parts {
        Some(p) => Value::data(&format!("slice[{}]@L{}", p.join(":"), c.line)),
        None => Value::data(&format!("slice@L{}", c.line)),
    }
}

fn m_slice_indices(an: &mut Analyzer, _st: &mut State, c: &Call) -> Value {
    let recv = c.recv.clone().unwrap_or_default();
    let n = c.args.first().and_then(|v| v.kv.as_lit().filter(|_| v.pts.is_empty())).and_then(intern::key_as_int);
    match (Analyzer::slice_label_bounds(&recv.kv), n) {
        (Some((lo, hi, st)), Some(n)) if n >= 0 && st != Some(0) => {
            let (a, b, s) = crate::interp::exprs::slice_indices(n, lo, hi, st.unwrap_or(1));
            Value::lit(intern::key_tuple(vec![intern::key_int(a), intern::key_int(b), intern::key_int(s)]))
        }
        _ => {
            let _ = an;
            Value::data("indices")
        }
    }
}

/// Set methods: decided on sets with known (literal) members, else the element model.
fn m_set(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = last(&c.qual).to_string();
    let recv = c.recv.clone().unwrap_or_default();
    let mine = an.known_members(&recv, st);
    let args: Vec<Option<KeySet>> = c.args.iter().map(|a| an.known_members(a, st)).collect();
    let all_args = args.iter().all(|a| a.is_some());
    let fold = |base: &KeySet, f: &dyn Fn(&KeySet, &KeySet) -> KeySet| -> KeySet { args.iter().flatten().fold(base.clone(), |acc, a| f(&acc, a)) };
    let union = |a: &KeySet, b: &KeySet| a.union(b);
    let inter = |a: &KeySet, b: &KeySet| a.intersection(b);
    let diff = |a: &KeySet, b: &KeySet| a.difference(b);
    let sym = |a: &KeySet, b: &KeySet| a.difference(b).union(&b.difference(a));
    let op: Option<&dyn Fn(&KeySet, &KeySet) -> KeySet> = match name.as_str() {
        "union" | "update" => Some(&union),
        "intersection" | "intersection_update" => Some(&inter),
        "difference" | "difference_update" => Some(&diff),
        "symmetric_difference" | "symmetric_difference_update" => Some(&sym),
        _ => None,
    };
    match name.as_str() {
        "union" | "intersection" | "difference" | "symmetric_difference" | "copy" => {
            if let (Some(m), true) = (&mine, all_args) {
                let r = match op {
                    Some(f) => fold(m, f),
                    None => m.clone(),
                };
                return an.literal_set(&c.node, r, st);
            }
            // a new set: the elements it may hold
            let mut kv = KeyVal::Bot;
            let mut elem = Value::new();
            let srcs: Vec<Value> = std::iter::once(recv.clone()).chain(if name == "union" { c.args.clone() } else { vec![] }).collect();
            for v in &srcs {
                let it = an.iterate(v, &c.node, st);
                let e = it.elem.one_or_empty();
                kv = kv_join(&kv, &e.kv);
                elem = elem.join(&e);
            }
            an.alloc_positional(Kind::Set, &c.node, None, elem, kv, st)
        }
        "issubset" | "issuperset" | "isdisjoint" => {
            if let (Some(m), Some(Some(o))) = (&mine, args.first()) {
                let b = match name.as_str() {
                    "issubset" => m.is_subset(o),
                    "issuperset" => o.is_subset(m),
                    _ => !m.intersects(o),
                };
                return Value::lit(intern::key_bool(b));
            }
            Value::data("set")
        }
        "add" | "discard" | "remove" | "update" | "intersection_update" | "difference_update" | "symmetric_difference_update" | "clear" => {
            let item = c.args.first().and_then(|a| a.kv.as_lit().filter(|_| a.pts.is_empty()));
            for oid in recv.pts.iter().filter(|o| an.kind(*o) == Kind::Set).collect::<Vec<_>>() {
                let strong = recv.pts.len() == 1 && an.obj(oid).single;
                let known = an.known_members(&Value::obj(oid), st);
                let next: Option<KeySet> = match (name.as_str(), &known, item) {
                    ("clear", _, _) if strong => Some(KeySet::new()),
                    ("add", Some(m), Some(k)) => Some(m.insert(k)),
                    ("discard", Some(m), Some(k)) => Some(m.remove(k)),
                    ("remove", Some(m), Some(k)) => {
                        if !m.contains(k) {
                            an.raise_here("KeyError", st);
                            return Value::new();
                        }
                        Some(m.remove(k))
                    }
                    (_, Some(m), _) if all_args && op.is_some() => Some(fold(m, op.unwrap())),
                    _ => None,
                };
                match next {
                    Some(r) if strong => an.set_members(oid, r, st),
                    _ => {
                        // members not known any more: the elements it may hold
                        let mut kv = st.elems.get(&oid).unwrap_or(KeyVal::Bot);
                        if matches!(name.as_str(), "add" | "update") {
                            for a in &c.args {
                                let e = if name == "add" { a.clone() } else { an.iterate(a, &c.node, st).elem.one_or_empty() };
                                kv = kv_join(&kv, &e.kv);
                                let cur = st.slot(oid, Ns::Elem, STAR);
                                st.set_slot(oid, Ns::Elem, STAR, cur.join(&Value::with_kv(e.pts.clone(), crate::models::torch::elem_kv(&e.kv))), true);
                            }
                        }
                        st.elems.insert(oid, kv);
                        an.set_llen(oid, st, None);
                        st.drop_obj_ns(oid, Ns::Pos);
                    }
                }
            }
            an.none_v.clone()
        }
        _ => Value::data("set"),
    }
}

/// `ChainMap` attributes and methods: `maps`, `parents`, `new_child(m)`.
fn m_chainmap(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = last(&c.qual).to_string();
    let Some(oid) = c.recv.as_ref().and_then(|r| r.single()) else { return Value::data("chainmap") };
    let maps = an.chainmaps.get(&oid).cloned().unwrap_or_default();
    match name.as_str() {
        "maps" => an.alloc_positional(Kind::List, &c.node, Some(maps), Value::new(), DATA, st),
        "parents" | "new_child" => {
            let mut ms: Vec<Value> = if name == "parents" { maps.iter().skip(1).cloned().collect() } else { maps.clone() };
            if name == "new_child" {
                let first = match c.arg(0, Some("m")) {
                    Some(m) => m,
                    None => {
                        let d = an.alloc_at(Kind::Dict, &c.node, st, &format!("dict@L{}", c.line), Payload::None, Some(Row::new()), Ns::Item, None, Some("chainchild"));
                        st.set_row(d, Ns::Item, Row::new());
                        Value::obj(d)
                    }
                };
                ms.insert(0, first);
            }
            let call = Call { qual: "collections.ChainMap".into(), recv: None, args: ms, kwargs: IndexMap::new(), star_args: vec![], star_kwargs: vec![], node: c.node, call_ast: None, site: format!("{}:{}", c.site, name), line: c.line };
            an.frame_mut().unroll.push(7000);
            let v = m_defaultdict(an, st, &call);
            an.frame_mut().unroll.pop();
            v
        }
        _ => Value::data("chainmap"),
    }
}

/// Library classes written in Python that are analysed as such (generated once): their
/// methods run as user code (`collections.UserDict`: `self.data`, `__missing__`, …).
pub fn generated_class(an: &mut Analyzer, st: &mut State, qual: &str) -> Option<Value> {
    let (modname, cname, src) = match qual {
        "collections.UserDict" => ("<collections.UserDict>", "UserDict", USERDICT_SRC),
        _ => return None,
    };
    let moid = match an.modules.get(modname) {
        Some(&m) => m,
        None => {
            let (idx, tree) = crate::frontend::parse(modname, None, src.to_string()).ok()?;
            let m = an.make_module_pub(modname, None, idx, tree);
            an.run_module(m, st);
            m
        }
    };
    if st.slot_opt(moid, Ns::Attr, key_str_lit(cname)).is_none() {
        an.run_module(moid, st);
    }
    let v = st.slot(moid, Ns::Attr, key_str_lit(cname));
    if let Some(c) = v.single() {
        an.userdict_classes.insert(c);
    }
    Some(v)
}

const USERDICT_SRC: &str = r#"
class UserDict:
    def __init__(self, dict=None, /, **kwargs):
        self.data = {}
        if dict is not None:
            self.update(dict)
        if kwargs:
            self.update(kwargs)

    def __len__(self):
        return len(self.data)

    def __getitem__(self, key):
        if key in self.data:
            return self.data[key]
        if hasattr(self.__class__, "__missing__"):
            return self.__class__.__missing__(self, key)
        raise KeyError(key)

    def __setitem__(self, key, item):
        self.data[key] = item

    def __delitem__(self, key):
        del self.data[key]

    def __iter__(self):
        return iter(self.data)

    def __contains__(self, key):
        return key in self.data

    def update(self, other=(), /, **kwds):
        for key in other:
            self[key] = other[key]
        for key in kwds:
            self[key] = kwds[key]

    def get(self, key, default=None):
        if key in self:
            return self[key]
        return default

    def keys(self):
        return self.data.keys()

    def values(self):
        return self.data.values()

    def items(self):
        return self.data.items()

    def pop(self, key, *default):
        return self.data.pop(key, *default)

    def setdefault(self, key, default=None):
        if key not in self:
            self[key] = default
        return self[key]

    def copy(self):
        return dict(self.data)
"#;

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
            // one list with known positions: updated in place
            if let Some(oid) = recv.single() {
                if let Some(mut ps) = an.known_positions(oid, st).filter(|_| !crate::interp::disabled("listinplace")) {
                    let done = match name {
                        "clear" => {
                            ps.clear();
                            true
                        }
                        "reverse" => {
                            ps.reverse();
                            true
                        }
                        "insert" if c.args.len() == 2 => match c.args[0].kv.as_lit().filter(|_| c.args[0].pts.is_empty()).and_then(intern::key_as_int) {
                            Some(i) => {
                                let n = ps.len() as i64;
                                let at = if i < 0 { (n + i).max(0) } else { i.min(n) } as usize;
                                ps.insert(at, c.args[1].clone());
                                true
                            }
                            None => false,
                        },
                        "sort" => match literal_sort_order(an, st, c, &ps) {
                            Some(order) => {
                                ps = order.into_iter().map(|j| ps[j].clone()).collect();
                                true
                            }
                            None => false,
                        },
                        _ => false,
                    };
                    if done {
                        an.set_positions(oid, ps, st);
                        return an.none_v.clone();
                    }
                }
            }
            for oid in recv.pts.iter() {
                an.set_llen(oid, st, None);
                st.drop_obj_ns(oid, Ns::Pos);
                if name == "insert" {
                    if let Some(x) = c.args.get(1) {
                        st.set_slot(oid, Ns::Elem, STAR, Value::with_kv(x.pts.clone(), crate::models::torch::elem_kv(&x.kv)), false);
                        let e = st.elems.get(&oid).unwrap_or_else(|| x.kv.clone());
                        st.elems.insert(oid, kv_join(&e, &x.kv));
                    }
                }
            }
            an.none_v.clone()
        }
        _ => Value::data(&format!("L{}", c.line)),
    }
}

fn m_view(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = last(&c.qual);
    if name == "map" {
        // `df.columns.map(f)`: the column names renamed by f
        if let Some(v) = pandas::columns_map(an, st, c) {
            return v;
        }
    }
    // `obj.__dict__.update(...)`: stores on the instance's attributes
    let attrs_of: Vec<u32> = c.recv.iter().flat_map(|r| r.pts.iter()).filter_map(|v| match &an.obj(v).payload {
        Payload::View(ViewKind::Attrs, inst) => Some(inst.pts.to_vec()),
        _ => None,
    }).flatten().collect();
    if !attrs_of.is_empty() {
        if name == "get" && !c.args.is_empty() {
            // `obj.__dict__.get(name, default)`: the attribute, or the default when it is not set
            let dflt = c.args.get(1).cloned().unwrap_or_else(|| an.none_v.clone());
            let Some(k) = c.args[0].kv.as_lit().filter(|_| c.args[0].pts.is_empty()) else { return an.all_values(attrs_of[0], Ns::Attr, st).join(&dflt) };
            let mut out = Value::new();
            for o in attrs_of {
                let row = st.row(o, Ns::Attr);
                if row.present.contains(k) {
                    out = out.join(&st.slot(o, Ns::Attr, k));
                }
                if !row.definite.contains(k) {
                    out = out.join(&dflt);
                }
            }
            return out;
        }
        if name != "update" {
            return an.unknown_call(&c.qual, c.recv.as_ref(), &c.args, &c.kwargs, &c.star_args, &c.star_kwargs, st, &c.node);
        }
        let strong = attrs_of.len() == 1 && an.obj(attrs_of[0]).single;
        for oid in attrs_of {
            if let Some(src) = c.args.first() {
                if !an.record_objects(src).is_empty() {
                    an.copy_into(oid, Ns::Attr, src, st, strong, &KeySet::new());
                } else {
                    let r = st.row(oid, Ns::Attr).data_store(Source::new(&format!("update@L{}", c.line), SourceKind::Data));
                    st.set_row(oid, Ns::Attr, r);
                }
            }
            for (k, v) in c.kwargs.iter() {
                an.store_key(oid, Ns::Attr, &KeyVal::Lit(intern::key(intern::Key::Str(intern::str_get(*k)))), v, strong, &c.node, st);
            }
            for sk in &c.star_kwargs {
                an.copy_into(oid, Ns::Attr, sk, st, strong, &KeySet::new());
            }
        }
        return an.none_v.clone();
    }
    if matches!(name, "tolist" | "to_list" | "copy") {
        if let Some(recv) = &c.recv {
            let oid = an.alloc_simple(Kind::List, &c.node, st, &format!("{}@L{}", name, c.line));
            an.set_llen(oid, st, None);
            let it = an.iterate(recv, &c.node, st);
            let elem = it.elem.one_or_empty();
            st.set_slot(oid, Ns::Elem, STAR, Value::with_kv(elem.pts.clone(), crate::models::torch::elem_kv(&elem.kv)), true);
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

/// `json.loads("…")` of a literal: the parsed value (dicts with exactly their keys).
fn json_literal(an: &mut Analyzer, st: &mut State, c: &Call, j: &serde_json::Value, path: &str) -> Value {
    let hook = c.kw("object_hook");
    json_literal_h(an, st, c, j, path, hook.as_ref())
}

/// (`object_hook=f`: each object, innermost first, is replaced by `f(dict)`)
fn json_literal_h(an: &mut Analyzer, st: &mut State, c: &Call, j: &serde_json::Value, path: &str, hook: Option<&Value>) -> Value {
    let site = format!("{}#json{}", crate::interp::site_string(&c.node), path);
    match j {
        serde_json::Value::Null => an.none_v.clone(),
        serde_json::Value::Bool(b) => Value::lit(intern::key_bool(*b)),
        serde_json::Value::Number(n) => match n.as_i64() {
            Some(i) => Value::lit(intern::key_int(i)),
            None => Value::lit(intern::key_float(n.as_f64().unwrap_or(0.0))),
        },
        serde_json::Value::String(t) => Value::lit(key_str_lit(t)),
        serde_json::Value::Array(items) => {
            let vals: Vec<Value> = items.iter().enumerate().map(|(i, x)| json_literal_h(an, st, c, x, &format!("{}.{}", path, i), hook)).collect();
            let oid = an.alloc(Kind::List, &site, st, &format!("list@L{}", c.line), Payload::None, None, Ns::Item, None, None);
            let single = an.obj(oid).single;
            let acc = join_values(vals.clone());
            st.set_slot(oid, Ns::Elem, STAR, Value::with_kv(acc.pts.clone(), crate::models::torch::elem_kv(&acc.kv)), single);
            st.elems.insert(oid, acc.kv.clone());
            if single {
                an.set_llen(oid, st, Some(vals.len()));
                for (i, v) in vals.iter().enumerate() {
                    st.set_slot(oid, Ns::Pos, intern::key_int(i as i64), v.clone(), true);
                }
            }
            Value::obj(oid)
        }
        serde_json::Value::Object(m) => {
            let oid = an.alloc(Kind::Dict, &site, st, &format!("dict@L{}", c.line), Payload::None, Some(Row::new()), Ns::Item, None, None);
            let strong = an.obj(oid).single;
            for (k, x) in m.iter() {
                let v = json_literal_h(an, st, c, x, &format!("{}.{}", path, k), hook);
                an.store_key(oid, Ns::Item, &KeyVal::Lit(key_str_lit(k)), &v, strong, &c.node, st);
            }
            match hook {
                Some(h) if !crate::interp::disabled("objecthook") => {
                    // (a call context per object: the hook's allocations stay distinct)
                    let mut nd = c.node;
                    let tag = path.bytes().fold(7u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32)) % 1_000_000;
                    nd.col = c.node.col.wrapping_add(1_000_000 + tag);
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
fn m_json_dumps(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fallback = Value::data(&format!("L{}", c.line));
    let Some(x) = c.args.first() else { return fallback };
    let json = c.qual == "json.dumps";
    // a parameter's value: known at the call site
    if let Some(p) = x.kv.as_param() {
        an.record_event("concrete", p);
    }
    for o in x.pts.iter() {
        if an.kind(o) == Kind::Ph {
            an.record_event("concrete", o);
        }
    }
    // fully known data: the exact text
    if json && !crate::interp::disabled("jsontext") {
        if let Some(opts) = JsonOpts::of(c) {
            if let Some(t) = json_text(an, st, x, &opts, 0) {
                return Value::lit_str(&t);
            }
        }
    }
    if crate::interp::disabled("jsonround") || x.pts.is_empty() || !x.kv.is_bot() {
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
        match an.kind(o) {
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

/// A json object key's text for a non-str literal key (`1` → "1", True → "true").
fn json_key_text(k: KeyId) -> Option<String> {
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
fn json_str_keys(an: &mut Analyzer, st: &mut State, o: u32) {
    if an.kind(o) != Kind::Dict {
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
struct JsonOpts {
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

fn json_str(s: &str, ascii: bool) -> String {
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

/// The exact text `json.dumps` makes of fully known data (None: anything unknown in it).
fn json_text(an: &Analyzer, st: &State, v: &Value, o: &JsonOpts, depth: usize) -> Option<String> {
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
    match an.kind(x) {
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
            let n = an.llen(x, st)?;
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

/// The snapshots a `json.dumps` / `pickle.dumps` text stands for.
fn dumped_snapshots(v: &Value, json: bool) -> Vec<u32> {
    let mut out = Vec::new();
    if let (true, KeyVal::Data(d)) = (v.pts.is_empty(), &v.kv) {
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
fn m_typeadapter(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let mut it = c.qual.splitn(3, '.');
    let (_, idx, method) = (it.next(), it.next().and_then(|x| x.parse::<usize>().ok()), it.next().unwrap_or(""));
    match idx {
        Some(i) => pydantic::adapter_call(an, st, i, method, c),
        None => Value::data("TypeAdapter"),
    }
}

fn m_json_load(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
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
        if let Some(a) = c.args.first() {
            if let KeyVal::Param(p) = &a.kv {
                an.record_event("concrete", *p);
            }
            for o in a.pts.iter() {
                if an.kind(o) == Kind::Ph {
                    an.record_event("concrete", o);
                }
            }
        }
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
        if let Some(text) = c.args.first().filter(|a| a.pts.is_empty()).and_then(|a| a.kv.as_lit()).and_then(intern::key_as_str) {
            if let Ok(j) = serde_json::from_str::<serde_json::Value>(&text) {
                return json_literal(an, st, c, &j, "");
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

/// `namedtuple("P", ["x", "y"])` / `NamedTuple("P", [("x", int)])` with known names: the
/// class an equivalent `class P(NamedTuple)` (with `_asdict`, `_replace`, `_fields`) defines,
/// analysed as a small generated module.
fn m_namedtuple(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fallback = |an: &mut Analyzer, st: &mut State, c: &Call| an.unknown_call(&c.qual, None, &c.args, &c.kwargs, &c.star_args, &c.star_kwargs, st, &c.node);
    let Some(tname) = c.args.first().and_then(|a| a.kv.as_lit()).and_then(intern::key_as_str) else { return fallback(an, st, c) };
    let Some(spec) = c.args.get(1).cloned().or_else(|| c.kw("field_names")).or_else(|| c.kw("fields")) else { return fallback(an, st, c) };
    let ident = |s: &str| !s.is_empty() && s.chars().all(|ch| ch.is_alphanumeric() || ch == '_') && !s.starts_with(|ch: char| ch.is_ascii_digit());
    let fields: Option<Vec<String>> = if let Some(s0) = spec.kv.as_lit().filter(|_| spec.pts.is_empty()).and_then(intern::key_as_str) {
        Some(s0.replace(',', " ").split_whitespace().map(|x| x.to_string()).collect())
    } else {
        an.iterate(&spec, &c.node, st).items.and_then(|items| {
            items
                .iter()
                .map(|it| {
                    // a name, or a (name, type) pair
                    let v = if it.pts.is_empty() { it.clone() } else { an.unpack(it, 2, st, &c.node).remove(0) };
                    v.kv.as_lit().filter(|_| v.pts.is_empty()).and_then(intern::key_as_str).map(|s| s.to_string())
                })
                .collect()
        })
    };
    let Some(fields) = fields.filter(|fs| !fs.is_empty() && fs.iter().all(|f| ident(f)) && ident(&tname)) else { return fallback(an, st, c) };
    // the defaults (of the last fields), as module globals the generated class refers to
    let dvals: Vec<Value> = c.kw("defaults").map(|d| an.iterate(&d, &c.node, st).items.unwrap_or_default()).unwrap_or_default();
    let ndefaults = dvals.len();
    let modname = format!("<namedtuple {} {}>", tname, c.site);
    if let Some(&moid) = an.modules.get(&modname) {
        return st.slot(moid, Ns::Attr, key_str_lit(&tname));
    }
    let mut src = format!("from typing import NamedTuple\n\n\nclass {}(NamedTuple):\n", tname);
    let first_default = fields.len().saturating_sub(ndefaults);
    for (i, f) in fields.iter().enumerate() {
        let dflt = if i >= first_default { format!(" = _rowan_default_{}", i - first_default) } else { String::new() };
        src.push_str(&format!("    {}: \"_field\"{}\n", f, dflt));
    }
    let tuple_src = fields.iter().map(|f| format!("{:?}", f)).collect::<Vec<_>>().join(", ");
    src.push_str(&format!("    _fields = ({},)\n", tuple_src));
    let fd = fields.iter().enumerate().filter(|(i, _)| *i >= first_default).map(|(i, f)| format!("{:?}: _rowan_default_{}", f, i - first_default)).collect::<Vec<_>>().join(", ");
    src.push_str(&format!("    _field_defaults = {{{}}}\n\n", fd));
    let items = fields.iter().map(|f| format!("{:?}: self.{}", f, f)).collect::<Vec<_>>().join(", ");
    src.push_str(&format!("    def _asdict(self):\n        return {{{}}}\n\n", items));
    let args = fields.iter().map(|f| format!("kw.get({:?}, self.{})", f, f)).collect::<Vec<_>>().join(", ");
    src.push_str(&format!("    def _replace(self, **kw):\n        for k in kw:\n            if k not in {:?}:\n                raise ValueError(k)\n        return {}({})\n", fields, tname, args).replace("[", "(").replace("]", ",)"));
    let Ok((idx, tree)) = crate::frontend::parse(&modname, None, src) else { return fallback(an, st, c) };
    let moid = an.make_module_pub(&modname, None, idx, tree);
    for (i, v) in dvals.iter().enumerate() {
        st.set_slot(moid, Ns::Attr, key_str_lit(&format!("_rowan_default_{}", i)), v.clone(), true);
    }
    an.run_module(moid, st);
    st.slot(moid, Ns::Attr, key_str_lit(&tname))
}

/// `p._asdict()` / `p._replace(**kw)` on an instance of a class-syntax NamedTuple.
/// `itemgetter(k, ...)` / `attrgetter("a", ...)` with known keys: the function they stand for,
/// analysed as a small generated module (`def g(obj): return obj[k]`).
fn m_getter(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let item = c.qual.ends_with("itemgetter");
    let keys: Option<Vec<KeyId>> = c.args.iter().map(|a| a.kv.as_lit().filter(|_| a.pts.is_empty())).collect();
    let Some(keys) = keys.filter(|k| !k.is_empty() && c.star_args.is_empty()) else {
        return an.unknown_call(&c.qual, None, &c.args, &c.kwargs, &c.star_args, &c.star_kwargs, st, &c.node);
    };
    let mut parts: Vec<String> = Vec::new();
    for k in &keys {
        if item {
            parts.push(format!("obj[{}]", intern::key_repr(*k)));
        } else {
            let Some(a) = intern::key_as_str(*k).filter(|a| a.split('.').all(|p| !p.is_empty() && p.chars().all(|ch| ch.is_alphanumeric() || ch == '_'))) else {
                return an.unknown_call(&c.qual, None, &c.args, &c.kwargs, &c.star_args, &c.star_kwargs, st, &c.node);
            };
            parts.push(format!("obj.{}", a));
        }
    }
    let body = if parts.len() == 1 { parts[0].clone() } else { format!("({},)", parts.join(", ")) };
    let modname = format!("<{} {}>", if item { "itemgetter" } else { "attrgetter" }, body);
    let moid = match an.modules.get(&modname) {
        Some(&m) => m,
        None => {
            let src = format!("def getter(obj):\n    return {}\n", body);
            let Ok((idx, tree)) = crate::frontend::parse(&modname, None, src) else { return Value::data("getter") };
            let m = an.make_module_pub(&modname, None, idx, tree);
            an.run_module(m, st);
            m
        }
    };
    // (made in another state — a summary's —: its function is bound in this one too)
    if st.slot_opt(moid, Ns::Attr, key_str_lit("getter")).is_none() {
        an.run_module(moid, st);
    }
    st.slot(moid, Ns::Attr, key_str_lit("getter"))
}

/// `asyncio.gather(a, b, *rest)`: the list of their results, in order.
/// `re.match(pattern, s)` with a known pattern: a match object whose `groupdict()` has the
/// pattern's named groups (or None: whether it matches is not decided here).
/// `SimpleNamespace(a=1, **d)`: attributes from the keywords and the mapping (reading one of the
/// mapping's is a read of the mapping).
fn m_simplenamespace(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let oid = an.alloc_at(Kind::Namespace, &c.node, st, &format!("namespace@L{}", c.line), Payload::None, Some(Row::new()), Ns::Attr, None, None);
    for sk in &c.star_kwargs {
        an.copy_into(oid, Ns::Attr, sk, st, true, &KeySet::new());
    }
    for (k, v) in c.kwargs.iter() {
        an.store_key(oid, Ns::Attr, &KeyVal::Lit(key_str_lit(&intern::str_get(*k))), v, true, &c.node, st);
    }
    Value::obj(oid)
}

/// `exec(code)` / `eval(code)`: a literal code string is run here, in this frame; anything
/// else may change every record this frame reaches (and gives an unknown value).
/// `globals()`: the module's names as a dict (stores through it bind names).
fn m_globals(an: &mut Analyzer, _st: &mut State, _c: &Call) -> Value {
    let m = an.frame().module;
    let vid = an.alloc_raw(Kind::View, &format!("view:attrs:{}", m), "", true, "globals()", Payload::View(ViewKind::Attrs, Value::obj(m)));
    an.obj_mut(vid).payload = Payload::View(ViewKind::Attrs, Value::obj(m));
    Value::obj(vid)
}

fn m_exec(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let is_eval = c.qual.ends_with("eval");
    let code = c.args.first().and_then(|a| a.kv.as_lit().filter(|_| a.pts.is_empty())).and_then(intern::key_as_str);
    if let (Some(code), 1) = (code, c.args.len()) {
        if is_eval {
            if let Some(e) = crate::frontend::parse_expression(&code) {
                return an.eval(e, st);
            }
        } else if let Ok((_, tree)) = crate::frontend::parse(&format!("<exec L{}>", c.line), None, code.to_string()) {
            an.exec_block(&tree.body, st);
            st.reachable = true;
            return an.none_v.clone();
        }
    }
    // unknown code: everything reachable from the frame's names (and the module's) may change
    let mut reach: Vec<Value> = st.env.values().cloned().collect();
    let module = an.frame().module;
    reach.push(Value::obj(module));
    reach.extend(c.args.iter().cloned());
    an.unknown_call(&c.qual, None, &reach, &c.kwargs, &c.star_args, &c.star_kwargs, st, &c.node)
}

fn m_re_match(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(pat) = c.args.first().and_then(|a| a.kv.as_lit().filter(|_| a.pts.is_empty())).and_then(intern::key_as_str) else {
        return an.unknown_call(&c.qual, None, &c.args, &c.kwargs, &c.star_args, &c.star_kwargs, st, &c.node);
    };
    let mut names: Vec<String> = Vec::new();
    let mut rest: &str = &pat;
    while let Some(i) = rest.find("(?P<") {
        let after = &rest[i + 4..];
        let Some(j) = after.find('>') else { break };
        let n = &after[..j];
        if !n.is_empty() && n.chars().all(|ch| ch.is_alphanumeric() || ch == '_') && !names.iter().any(|x| x == n) {
            names.push(n.to_string());
        }
        rest = &after[j..];
    }
    let ngroups = pat.matches('(').count().saturating_sub(pat.matches("(?").count()) + names.len();
    let modname = format!("<re.Match {:?}>", &*pat);
    let moid = match an.modules.get(&modname) {
        Some(&m) => m,
        None => {
            let items = names.iter().map(|n| format!("{:?}: _GROUP", n)).collect::<Vec<_>>().join(", ");
            let groups = (0..ngroups).map(|_| "_GROUP").collect::<Vec<_>>().join(", ");
            let src = format!(
                "class Match:\n    def groupdict(self, default=None):\n        return {{{}}}\n\n    def group(self, *names):\n        return _GROUP\n\n    def groups(self, default=None):\n        return ({}{})\n\n    def start(self, g=0):\n        return _POS\n\n    def end(self, g=0):\n        return _POS\n\n    def span(self, g=0):\n        return (_POS, _POS)\n\n\ndef matched():\n    return Match()\n",
                items,
                groups,
                if ngroups == 1 { "," } else { "" }
            );
            let Ok((idx, tree)) = crate::frontend::parse(&modname, None, src) else { return Value::data("re") };
            let m = an.make_module_pub(&modname, None, idx, tree);
            an.run_module(m, st);
            m
        }
    };
    if st.slot_opt(moid, Ns::Attr, key_str_lit("matched")).is_none() {
        an.run_module(moid, st);
    }
    let f = st.slot(moid, Ns::Attr, key_str_lit("matched"));
    let m = an.call_value(&f, &c.node, None, st, Some(vec![]));
    m.join(&an.none_v.clone())
}

fn m_gather(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let mut vals: Option<Vec<Value>> = Some(c.args.clone());
    for sa in &c.star_args {
        match (an.iterate(sa, &c.node, st).items, vals.as_mut()) {
            (Some(its), Some(vs)) => vs.extend(its),
            _ => vals = None,
        }
    }
    match vals {
        Some(vs) => an.alloc_positional(Kind::List, &c.node, Some(vs), Value::new(), DATA, st),
        None => {
            let e = join_values(c.args.iter().cloned().chain(c.star_args.iter().map(|sa| an.elem_of(sa, &c.node, st))).collect::<Vec<_>>());
            an.alloc_positional(Kind::List, &c.node, None, e, DATA, st)
        }
    }
}

fn m_nt_method(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(recv) = c.recv.clone() else { return Value::data("namedtuple") };
    let mut outs: Vec<Value> = Vec::new();
    for o in recv.pts.iter() {
        let Some(cls) = an.instance_class(o) else { continue };
        let fields: Vec<String> = an.dataclass_fields_mro(cls).into_iter().map(|(f, _)| f).collect();
        if c.qual.ends_with("_asdict") {
            let d = an.alloc_simple(Kind::Dict, &c.node, st, &format!("dict@L{}", c.line));
            for f in &fields {
                let v = st.slot(o, Ns::Attr, key_str_lit(f));
                an.store_key(d, Ns::Item, &KeyVal::Lit(key_str_lit(f)), &v, true, &c.node, st);
            }
            outs.push(Value::obj(d));
        } else {
            let n = an.copy_object(o, &c.node, st, false, &mut IndexMap::new());
            for (k, v) in c.kwargs.iter() {
                let name = intern::str_get(*k);
                if fields.iter().any(|f| *f == *name) {
                    an.store_key(n, Ns::Attr, &KeyVal::Lit(key_str_lit(&name)), v, true, &c.node, st);
                }
            }
            outs.push(Value::obj(n));
        }
    }
    if outs.is_empty() {
        return Value::data("namedtuple");
    }
    join_values(outs)
}

/// `dataclasses.astuple(obj)`: the fields' values in order (nested dataclasses as tuples).
fn m_astuple(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    fn conv(an: &mut Analyzer, st: &mut State, c: &Call, v: &Value, depth: u32) -> Value {
        let mut out = Value::with_kv(crate::domain::ObjSet::new(), v.kv.clone());
        for o in v.pts.iter() {
            let cls = match an.kind(o) {
                Kind::Instance => an.instance_class(o),
                Kind::Ph => an.ph_info(o).cls.get(),
                _ => None,
            };
            let Some(cls) = cls.filter(|cl| !an.dataclass_fields_mro(*cl).is_empty()).filter(|_| depth < 4) else {
                out = out.join(&Value::obj(o));
                continue;
            };
            let fields: Vec<String> = an.dataclass_fields_mro(cls).into_iter().map(|(f, _)| f).collect();
            let mut vals = Vec::new();
            for f in fields {
                let x = if an.kind(o) == Kind::Ph { an.load_attr(&Value::obj(o), &f, &c.node, st, false) } else { st.slot(o, Ns::Attr, key_str_lit(&f)) };
                vals.push(conv(an, st, c, &x, depth + 1));
            }
            an.frame_mut().unroll.push(9200 + depth as usize);
            let t = an.alloc_positional(Kind::Tuple, &c.node, Some(vals), Value::new(), DATA, st);
            an.frame_mut().unroll.pop();
            out = out.join(&t);
        }
        out
    }
    let Some(a) = c.args.first().cloned() else { return Value::data("astuple") };
    conv(an, st, c, &a, 0)
}

/// `make_dataclass(name, fields)`: a generated `@dataclass` class (`name`, `(name, type)`,
/// `(name, type, field(default=…))` with literal defaults read from the call's text).
fn m_make_dataclass(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let fallback = |an: &mut Analyzer, st: &mut State, c: &Call| an.unknown_call(&c.qual, None, &c.args, &c.kwargs, &c.star_args, &c.star_kwargs, st, &c.node);
    let Some(tname) = c.args.first().and_then(|a| a.kv.as_lit()).and_then(intern::key_as_str) else { return fallback(an, st, c) };
    let Some(ast_call) = c.call_ast else { return fallback(an, st, c) };
    let mut fields: Vec<(String, Option<&'static ast::Expr>)> = Vec::new();
    let elts: &'static [ast::Expr] = match &ast_call.arguments.args[1] {
        ast::Expr::List(l) => &l.elts,
        _ => return fallback(an, st, c),
    };
    for e in elts {
        match e {
            ast::Expr::StringLiteral(s0) => fields.push((s0.value.to_str().to_string(), None)),
            ast::Expr::Tuple(t) => {
                let Some(ast::Expr::StringLiteral(n)) = t.elts.first() else { return fallback(an, st, c) };
                let d: Option<&'static ast::Expr> = match t.elts.get(2) {
                    Some(ast::Expr::Call(fc)) if crate::interp::dotted_last(&fc.func) == "field" => fc.arguments.keywords.iter().find(|k| k.arg.as_ref().map(|x| x.as_str() == "default").unwrap_or(false)).map(|k| &k.value),
                    Some(other) => Some(other),
                    None => None,
                };
                fields.push((n.value.to_str().to_string(), d));
            }
            _ => return fallback(an, st, c),
        }
    }
    let modname = format!("<make_dataclass {} {}>", tname, c.site);
    if let Some(&moid) = an.modules.get(&modname) {
        return st.slot(moid, Ns::Attr, key_str_lit(&tname));
    }
    let mut src = format!("from dataclasses import dataclass\n\n\n@dataclass\nclass {}:\n", tname);
    let mut dvals: Vec<Value> = Vec::new();
    for (f, d) in &fields {
        match d {
            Some(e) => {
                src.push_str(&format!("    {}: \"_field\" = _rowan_default_{}\n", f, dvals.len()));
                dvals.push(an.eval(e, st));
            }
            None => src.push_str(&format!("    {}: \"_field\"\n", f)),
        }
    }
    if fields.is_empty() {
        src.push_str("    pass\n");
    }
    let Ok((idx, tree)) = crate::frontend::parse(&modname, None, src) else { return fallback(an, st, c) };
    let moid = an.make_module_pub(&modname, None, idx, tree);
    for (i, v) in dvals.iter().enumerate() {
        st.set_slot(moid, Ns::Attr, key_str_lit(&format!("_rowan_default_{}", i)), v.clone(), true);
    }
    an.run_module(moid, st);
    st.slot(moid, Ns::Attr, key_str_lit(&tname))
}

fn m_dc_replace(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(a) = c.args.first() else { return Value::data("replace") };
    let mut outs = Vec::new();
    for o in a.pts.iter() {
        // a new instance through `__init__`: the init fields from the object (the changes
        // win); non-init fields and attributes added later are not carried over
        let cls = match an.kind(o) {
            Kind::Instance => an.instance_class(o),
            Kind::Ph => an.ph_info(o).cls.get(),
            _ => None,
        };
        if let Some(cls) = cls.filter(|cl| an.class_info_opt(*cl).map(|ci| ci.has_fields() && !ci.named_tuple).unwrap_or(false) && !crate::interp::disabled("dcreplace")) {
            let specs = an.dc_specs(cls);
            let mut kw: IndexMap<intern::StrId, Value> = IndexMap::new();
            for sp in specs.iter().filter(|sp| sp.init && !sp.initvar) {
                let v = if an.kind(o) == Kind::Ph { an.load_attr(&Value::obj(o), &sp.name, &c.node, st, false) } else { st.slot(o, Ns::Attr, key_str_lit(&sp.name)) };
                kw.insert(intern::s(&sp.name), v);
            }
            for (k, v) in c.kwargs.iter() {
                kw.insert(*k, v.clone());
            }
            an.frame_mut().unroll.push(9100);
            outs.push(an.construct(cls, &[], &kw, &[], &[], st, &c.node));
            an.frame_mut().unroll.pop();
            continue;
        }
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

fn m_first_arg(_an: &mut Analyzer, _st: &mut State, c: &Call) -> Value {
    c.args.first().cloned().unwrap_or_else(|| Value::data(&format!("L{}", c.line)))
}

fn m_passthrough(_an: &mut Analyzer, _st: &mut State, c: &Call) -> Value {
    if c.qual == "typing.cast" && c.args.len() >= 2 {
        return c.args[1].clone();
    }
    Value::data(&format!("L{}", c.line))
}

fn m_defaultdict(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = last(&c.qual).to_string();
    if name == "Counter" && !crate::interp::disabled("counter") {
        return counter::construct(an, st, c);
    }
    let oid = an.alloc_simple(Kind::Dict, &c.node, st, &format!("{}@L{}", name, c.line));
    let src_arg: Option<Value>;
    if name == "defaultdict" || name == "Counter" {
        if name == "Counter" {
            if c.args.is_empty() {
                // an empty Counter is a closed dict whose missing keys read as 0
                st.set_slot(oid, Ns::Item, STAR, Value::lit(intern::key_int(0)), false);
            } else {
                st.set_slot(oid, Ns::Item, STAR, Value::lit(intern::key_int(0)), false);
                // known items with literal values: those are the keys (counts unknown)
                let it = if an.record_objects(&c.args[0]).iter().all(|o| an.kind(*o) != Kind::Dict) { an.iterate(&c.args[0], &c.node, st).items } else { None };
                let lits: Option<Vec<KeyId>> = it.and_then(|items| items.iter().map(|v| if v.pts.is_empty() { v.kv.as_lit() } else { None }).collect());
                match lits {
                    Some(ks) => {
                        let mut r = st.row(oid, Ns::Item);
                        for k in ks {
                            r = r.store(k, true);
                            st.set_slot(oid, Ns::Item, k, Value::data("count"), true);
                        }
                        st.set_row(oid, Ns::Item, r);
                    }
                    None => {
                        let r = st.row(oid, Ns::Item).data_store(Source::new(&format!("{}@L{}", name, c.line), SourceKind::Data));
                        st.set_row(oid, Ns::Item, r);
                    }
                }
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
    } else if name == "ChainMap" {
        // the maps it looks through (writes go to the first; `.maps`, `new_child`)
        let maps: Vec<Value> = if c.args.is_empty() {
            let d = an.alloc_at(Kind::Dict, &c.node, st, &format!("dict@L{}", c.line), Payload::None, Some(Row::new()), Ns::Item, None, Some("chainmap0"));
            st.set_row(d, Ns::Item, Row::new());
            vec![Value::obj(d)]
        } else {
            c.args.clone()
        };
        an.chainmaps.insert(oid, maps.clone());
        // every map, the first one winning: copied from the last to the first
        for sa in maps.iter().rev() {
            if !an.record_objects(sa).is_empty() {
                an.copy_into(oid, Ns::Item, sa, st, true, &KeySet::new());
            } else {
                let r = st.row(oid, Ns::Item).data_store(Source::new(&format!("{}@L{}", name, c.line), SourceKind::Data));
                st.set_row(oid, Ns::Item, r);
            }
        }
        src_arg = None;
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
                an.set_llen(tid, st, Some(2));
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

/// `f.cache_info()` (hits, misses, maxsize, currsize) / `f.cache_clear()` of an lru_cache'd f.
fn m_memo(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(f) = c.recv.as_ref().and_then(|r| r.single()) else { return Value::data("cache") };
    let memo = an.alloc_raw(Kind::Dict, &format!("memo:{}", f), "", true, "lru_cache", Payload::None);
    let zero = Value::lit(intern::key_int(0));
    if c.qual.ends_with("cache_clear") {
        st.set_row(memo, Ns::Item, Row::new());
        st.drop_obj_ns(memo, Ns::Item);
        st.set_slot(memo, Ns::Attr, key_str_lit("hits"), zero.clone(), true);
        st.set_slot(memo, Ns::Attr, key_str_lit("misses"), zero, true);
        return an.none_v.clone();
    }
    let oid = an.alloc_at(Kind::Namespace, &c.node, st, "CacheInfo", Payload::None, Some(Row::new()), Ns::Attr, None, Some("cacheinfo"));
    let r = st.row(memo, Ns::Item);
    let size = if r.closed() && r.present == r.definite { Value::lit(intern::key_int(r.present.len() as i64)) } else { Value::data("currsize") };
    let mut row = Row::new();
    for (k, v) in [
        ("hits", st.slot_opt(memo, Ns::Attr, key_str_lit("hits")).unwrap_or_else(|| zero.clone())),
        ("misses", st.slot_opt(memo, Ns::Attr, key_str_lit("misses")).unwrap_or_else(|| zero.clone())),
        ("maxsize", Value::data("maxsize")),
        ("currsize", size),
    ] {
        let kk = key_str_lit(k);
        row = row.store(kk, true);
        st.set_slot(oid, Ns::Attr, kk, v, true);
    }
    st.set_row(oid, Ns::Attr, row);
    Value::obj(oid)
}

fn m_partial(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(f) = c.args.first() else { return Value::data("partial") };
    let mut func = f.clone();
    let mut args: Vec<Value> = c.args[1..].to_vec();
    let mut kw = c.kwargs.clone();
    let mut star: Vec<Value> = c.star_kwargs.clone();
    // `partial(partial(f, a, k=1), b, k=2)` is `partial(f, a, b, k=2)`
    if let Some(inner) = f.single().filter(|o| an.kind(*o) == Kind::Partial && !crate::interp::disabled("partialflat")) {
        if let Payload::Partial(f0, a0, k0) = an.obj(inner).payload.clone() {
            func = f0;
            let mut a = a0;
            a.extend(args);
            args = a;
            let mut k = k0;
            for (kk, v) in kw.iter() {
                k.insert(*kk, v.clone());
            }
            kw = k;
            let mut s0 = an.partial_star.get(&inner).cloned().unwrap_or_default();
            s0.extend(star);
            star = s0;
        }
    }
    // `**opts`: its literal keys become bound keywords; an open mapping is passed on as it is
    let mut rest_star: Vec<Value> = Vec::new();
    for sk in &star {
        let recs = an.record_objects(sk);
        let closed = recs.len() == 1 && { let r = st.row(recs[0], Ns::Item); r.closed() && r.present == r.definite && r.present.iter().all(intern::key_is_str) };
        if closed && !crate::interp::disabled("partialstar") {
            let d = recs[0];
            for k in st.row(d, Ns::Item).ordered_keys().unwrap_or_else(|| st.row(d, Ns::Item).present.iter().collect()) {
                if let Some(name) = intern::key_as_str(k) {
                    kw.insert(intern::s(&name), st.slot(d, Ns::Item, k));
                }
            }
        } else {
            rest_star.push(sk.clone());
        }
    }
    let payload = Payload::Partial(func, args, kw);
    let oid = an.alloc_at(Kind::Partial, &c.node, st, &format!("partial@L{}", c.line), payload.clone(), None, Ns::Item, None, None);
    an.obj_mut(oid).payload = payload;
    if !rest_star.is_empty() {
        an.partial_star.insert(oid, rest_star);
    } else {
        an.partial_star.remove(&oid);
    }
    Value::obj(oid)
}

fn m_chain(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let oid = an.alloc_simple(Kind::List, &c.node, st, &format!("chain@L{}", c.line));
    an.set_llen(oid, st, None);
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
        // (bound `**opts` of unknown keys: passed on before the call's own)
        let mut star = an.partial_star.get(&x).cloned().unwrap_or_default();
        star.extend(c.star_kwargs.iter().cloned());
        for fo in f.pts.iter() {
            outs.push(an.call_one(fo, &fake_args, &kw, &c.star_args, &star, st, &c.node, c.call_ast));
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

/// `range(...)` with known int bounds: a list whose items are known when it is short; its
/// bounds decide membership at any length.
/// The names a class (its MRO for `get_type_hints`) or a function annotates, in order.
pub fn annotated_names(an: &Analyzer, o: crate::domain::ObjId, mro: bool) -> Option<Vec<String>> {
    match an.kind(o) {
        Kind::Class => {
            let mut out: Vec<String> = Vec::new();
            let classes: Vec<crate::domain::ObjId> = if mro { an.mro(o).iter().rev().copied().collect() } else { vec![o] };
            let mut any = false;
            for c in classes {
                let Some(ci) = an.class_info_opt(c) else { continue };
                any = true;
                for stmt in &ci.node.body {
                    if let ruff_python_ast::Stmt::AnnAssign(a) = stmt {
                        if let ruff_python_ast::Expr::Name(n) = &*a.target {
                            let n = n.id.to_string();
                            if !out.contains(&n) {
                                out.push(n);
                            }
                        }
                    }
                }
            }
            any.then_some(out)
        }
        Kind::Function => {
            let fi = an.func_info(o);
            let crate::interp::FuncNode::Def(f) = fi.node else { return Some(vec![]) };
            let p = &f.parameters;
            let mut out: Vec<String> = Vec::new();
            for a in p.posonlyargs.iter().chain(p.args.iter()).chain(p.kwonlyargs.iter()) {
                if a.parameter.annotation.is_some() {
                    out.push(a.parameter.name.id.to_string());
                }
            }
            for a in p.vararg.iter().chain(p.kwarg.iter()) {
                if a.annotation.is_some() {
                    out.push(a.name.id.to_string());
                }
            }
            if f.returns.is_some() {
                out.push("return".into());
            }
            Some(out)
        }
        _ => None,
    }
}

/// A closed dict of the given keys, each holding unknown data.
pub fn names_dict(an: &mut Analyzer, st: &mut State, node: &crate::frontend::Node, names: &[String], vals: Option<Vec<Value>>, what: &str) -> Value {
    let did = an.alloc_at(Kind::Dict, node, st, &format!("{}@L{}", what, node.line), Payload::None, Some(Row::new()), Ns::Item, None, None);
    let mut row = Row::new();
    for (i, n) in names.iter().enumerate() {
        let k = key_str_lit(n);
        row = row.store(k, true);
        let v = vals.as_ref().and_then(|vs| vs.get(i).cloned()).unwrap_or_else(|| Value::data(what));
        st.set_slot(did, Ns::Item, k, v, true);
    }
    st.set_row(did, Ns::Item, row);
    Value::obj(did)
}

fn m_type_hints(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(o) = c.args.first().and_then(|v| v.single()) else { return Value::data("hints") };
    let mro = !c.qual.starts_with("inspect.");
    match annotated_names(an, o, mro) {
        Some(names) => names_dict(an, st, &c.node, &names, None, "hints"),
        None => Value::data("hints"),
    }
}

/// `MappingProxyType(d)`: a live read-only view of d.
fn m_mappingproxy(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(d) = c.args.first().cloned() else { return Value::data("proxy") };
    if d.pts.is_empty() || !d.pts.iter().all(|o| matches!(an.kind(o), Kind::Dict | Kind::Ph | Kind::Unknown)) {
        return an.unknown_call(&c.qual, None, &c.args, &c.kwargs, &c.star_args, &c.star_kwargs, st, &c.node);
    }
    let _ = st;
    let vid = an.alloc_raw(Kind::View, &format!("view:proxy:{}:{}", c.site, d.pts.iter().map(|o| o.to_string()).collect::<Vec<_>>().join(",")), "", true, "mappingproxy", Payload::View(ViewKind::Proxy, d.clone()));
    an.obj_mut(vid).payload = Payload::View(ViewKind::Proxy, d);
    Value::obj(vid)
}

fn m_range(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let ints: Option<Vec<i64>> = c.args.iter().map(|a| if a.pts.is_empty() { a.kv.as_lit().and_then(intern::key_as_int) } else { None }).collect();
    let bounds = match ints.as_deref() {
        Some([stop]) => Some((0, *stop, 1)),
        Some([start, stop]) => Some((*start, *stop, 1)),
        Some([start, stop, step]) if *step != 0 => Some((*start, *stop, *step)),
        _ => None,
    };
    let Some((lo, hi, step)) = bounds.filter(|_| c.kwargs.is_empty() && !crate::interp::disabled("range")) else {
        an.concrete_args(c.args.iter(), st);
        let oid = an.alloc_simple(Kind::List, &c.node, st, &format!("range@L{}", c.line));
        an.set_llen(oid, st, None);
        // (for the tensor model: an int of unknown value)
        st.elems.insert(oid, if an.torch_on { KeyVal::data(&crate::shape::dim_label(crate::shape::unknown())) } else { DATA });
        return Value::obj(oid);
    };
    let len = if step > 0 { (hi - lo + step - 1).max(0) / step } else { (lo - hi - step - 1).max(0) / -step };
    let vals: Option<Vec<Value>> = if len <= 64 { Some((0..len).map(|i| Value::lit(intern::key_int(lo + i * step))).collect()) } else { None };
    let kv = match &vals {
        Some(vs) => KeyVal::lits(vs.iter().filter_map(|v| v.kv.as_lit()).collect()),
        None => DATA,
    };
    let v = an.alloc_positional(Kind::List, &c.node, vals, Value::new(), kv, st);
    if let Some(oid) = v.single() {
        an.obj_mut(oid).payload = Payload::Range(lo, hi, step);
    }
    v
}

/// `any(xs)` / `all(xs)` over known items: decided when the items' truthiness decides it
/// (`any([])` is False, `all([])` is True).
fn m_any_all(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let is_any = c.qual == "builtins.any";
    let Some(a) = c.args.first() else { return Value::data("any") };
    let Some(items) = an.iterate(a, &c.node, st).items else { return Value::data("any") };
    let mut unknown = false;
    for v in &items {
        match an.truthiness(v, st) {
            Some(b) if b == is_any => return Value::lit(intern::key(intern::Key::Bool(is_any))),
            Some(_) => {}
            None => unknown = true,
        }
    }
    if unknown {
        return Value::data("any");
    }
    Value::lit(intern::key(intern::Key::Bool(!is_any)))
}

/// `len(x)`: the count when it is known (a literal string, a container with a known
/// length, a closed record whose keys are all definite), else unknown data.
fn m_len(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(v) = c.args.first() else { return Value::data("len") };
    if an.torch_on {
        if let Some(r) = an.t_len(v, &c.node, st) {
            return r;
        }
    }
    // the length of a placeholder: decided per call site where it is a known container
    for o in v.pts.iter() {
        if an.kind(o) == Kind::Ph {
            an.record_event("iter", o);
        }
    }
    if v.pts.is_empty() {
        if let Some(k) = v.kv.as_lit() {
            if let Some(s) = intern::key_as_str(k) {
                return Value::lit(intern::key_int(s.chars().count() as i64));
            }
        }
        return Value::data("len");
    }
    if let Some(o) = v.single() {
        // a view of one dict: the dict's length
        if let Payload::View(ViewKind::Keys | ViewKind::Values | ViewKind::Items | ViewKind::Proxy, inner) = an.obj(o).payload.clone() {
            if let Some(d) = inner.single().filter(|d| matches!(an.kind(*d), Kind::Dict | Kind::Frame)) {
                let row = st.row(d, Ns::Item);
                if row.closed() && row.present == row.definite && !row.symbolic() {
                    return Value::lit(intern::key_int(row.definite.len() as i64));
                }
                // a closed frame: between its definite and its possible columns
                if an.kind(d) == Kind::Frame && row.closed() && !row.symbolic() && row.present.len() - row.definite.len() <= 8 {
                    let ks = intern::KeySet::from_iter((row.definite.len()..=row.present.len()).map(|n| intern::key_int(n as i64)));
                    return Value::kv_only(crate::domain::KeyVal::lits(ks));
                }
            }
            return Value::data("len");
        }
        if let Some(n) = an.llen(o, st) {
            return Value::lit(intern::key_int(n as i64));
        }
        if matches!(an.kind(o), Kind::Dict) {
            let row = st.row(o, Ns::Item);
            if row.closed() && row.present == row.definite && !row.symbolic() {
                return Value::lit(intern::key_int(row.definite.len() as i64));
            }
            // keys from a parameter (`d.update(rec)`): decided per call site
            if row.symbolic() {
                for r in row.refs().to_vec() {
                    let (p, _) = crate::domain::unrho(r);
                    an.record_event("concrete", p);
                }
            }
            // keys set on some paths: between the sure and the possible count
            if row.closed() && !row.symbolic() && row.present.len() - row.definite.len() <= 8 && !crate::interp::disabled("lenrange") {
                let ks = intern::KeySet::from_iter((row.definite.len()..=row.present.len()).map(|n| intern::key_int(n as i64)));
                return Value::kv_only(crate::domain::KeyVal::lits(ks));
            }
        }
    }
    Value::data("len")
}

/// `sum(xs)`: the literal sum when every item is a known number, else unknown data.
fn m_sum(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(v) = c.args.first() else { return Value::data("sum") };
    let it = an.iterate(v, &c.node, st);
    an.exhaust(v, st);
    // a sum of tensors: the elements' (broadcast) shape
    if an.torch_on {
        let e = it.elem.flat();
        if crate::models::torch::is_tensor(&e) {
            if let Some(s) = an.t_shape(&e, st) {
                let start = c.args.get(1).and_then(|x| an.t_shape(x, st)).unwrap_or_else(|| crate::shape::Shape::known(vec![]));
                return an.t_broadcast(&s, &start, &c.node, st, "sum");
            }
        }
    }
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
    if an.kind(o) == Kind::Pair {
        // a (key, value) pair compares like a 2-tuple
        let Payload::Pair(a, b) = an.obj(o).payload.clone() else { return None };
        let lit = |x: &Value| x.kv.as_lit().filter(|_| x.pts.is_empty());
        let mut ks = vec![lit(&a)?];
        if let Some(y) = lit(&b) {
            ks.push(y);
        }
        return Some(ks);
    }
    if an.kind(o) != Kind::Tuple {
        return None;
    }
    let n = an.llen(o, st)?;
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
    // instances ordering themselves (no key=): a stable sort on their `__lt__`
    if c.kw("key").is_none() && !items.is_empty() && !crate::interp::disabled("instorder") && items.iter().all(|x| x.single().map(|o| an.kind(o) == Kind::Instance).unwrap_or(false)) {
        let n = items.len();
        let mut less = vec![vec![false; n]; n];
        for i in 0..n {
            for j in 0..n {
                if i != j {
                    less[i][j] = an.instance_less(&items[i], &items[j], &c.node, st)?;
                }
            }
        }
        let reverse = c.kw("reverse").map(|r| r.kv.as_lit() == Some(intern::key_bool(true))).unwrap_or(false);
        let mut idx: Vec<usize> = (0..n).collect();
        idx.sort_by(|&a, &b| {
            let o = if less[a][b] { std::cmp::Ordering::Less } else if less[b][a] { std::cmp::Ordering::Greater } else { std::cmp::Ordering::Equal };
            if reverse { o.reverse() } else { o }
        });
        return Some(idx);
    }
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
