//! Library models; port of `rowan/models.py`.  A model is
//! `fn(&mut Analyzer, &mut State, &Call) -> Value`; models are keyed by qualified name with
//! prefix fallbacks.

pub mod minire;
pub mod shape;
pub mod singledispatch;
pub mod dataclasses;
pub mod pyre;
pub mod chrono;
pub mod sqlite;
pub mod pathtext;
pub mod pynum;
pub mod remodel;
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
pub mod pytext;
pub mod torch;
pub mod enums;
pub mod exceptions;
pub mod tables;

use indexmap::IndexMap;
use ruff_python_ast as ast;
use rustc_hash::FxHashMap;

use crate::domain::{join_values, kv_join, FnKind, KeyVal, Ns, RenameFn, Row, RowData, Source, SourceKind, Value, DATA};
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
    "parser.", "num.", "dcinit.", "statistics.", "cfgparser.", "cfgsection.", "partial.", "response.", "pydantic.cls.", "pydantic.", "typeadapter.", "fieldinfo.", "pydverr.", "counter.", "chainmap.", "deque.", "stdobj.", "threadsub.", "slice.", "memo.", "fastapi.", "starlette.", "torch.", "tensor.", "tensorq.", "nnmodule.", "math.", "numpy.", "np.",
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
        r(&["random.shuffle"], m_shuffle);
        r(&["reprlib.repr"], m_reprlib_repr);
        r(&["operator.setitem"], m_op_setitem);
        r(&["builtins.object.__setattr__"], m_object_setattr);
        r(&["builtins.object.__new__"], m_object_new);
        r(&["builtins.object.__init__"], m_object_init);
        r(&["builtins.delattr"], m_delattr);
        r(&["builtins.hasattr"], m_hasattr);
        r(&["builtins.vars", "attr.asdict", "attrs.asdict"], m_vars);
        r(&["dataclasses.asdict"], m_asdict);
        r(&["excinit"], exceptions::m_excinit);
        r(&["dcinit.*"], dataclasses::m_dcinit);
        r(&["dataclasses.fields"], m_dc_fields);
        r(&["builtins.map", "builtins.filter"], m_map);
        r(&["builtins.str"], m_str);
        r(&["builtins.repr"], m_repr);
        r(
            &[
                "builtins.print",
                "builtins.id", "builtins.hash",
                "builtins.format", "builtins.input", "builtins.issubclass", "builtins.hex",
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
        r(&["decimal.Decimal", "fractions.Fraction"], m_number_new);
        r(&["datetime.date", "datetime.datetime", "datetime.time", "datetime.timedelta", "datetime.timezone"], m_chrono_new);
        r(
            &[
                "datetime.date.fromisoformat", "datetime.datetime.fromisoformat", "datetime.time.fromisoformat", "datetime.datetime.strptime", "datetime.date.fromordinal",
                "datetime.datetime.fromordinal", "datetime.datetime.combine",
            ],
            m_chrono_class,
        );
        r(
            &[
                "datetime.datetime.now", "datetime.datetime.today", "datetime.datetime.utcnow", "datetime.date.today", "datetime.datetime.fromtimestamp",
                "datetime.datetime.utcfromtimestamp", "datetime.date.fromtimestamp",
            ],
            m_chrono_clock,
        );
        r(&["decimal.getcontext", "decimal.localcontext", "decimal.setcontext", "decimal.Context"], m_decimal_context);
        r(&["num.*"], m_num_method);
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
        r(&["builtins.callable"], m_callable);
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
        r(&["builtins.exec", "builtins.eval"], m_exec);
        r(&["builtins.globals"], m_globals);
        r(&["builtins.locals"], m_locals);
        r(&["builtins.__import__", "importlib.import_module"], m_import);
        r(&["builtins.exit", "builtins.quit", "sys.exit"], m_exit);
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
        remodel::register(&mut r);
        sqlite::register(&mut r);
        pathtext::register(&mut r);
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

    /// A model registered under exactly this name (no fallback).
    pub fn has_exact(&self, qual: &str) -> bool {
        self.table.contains_key(qual)
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
        // (functions of those modules that change what they are given are unknown calls)
        const MUTATING: &[&str] = &["random.shuffle", "operator.setitem", "operator.delitem", "struct.pack_into", "heapq.heapreplace", "heapq.heappushpop", "bisect.insort_right"];
        let in_place_op = qual.strip_prefix("operator.i").is_some_and(|r| !matches!(r, "ndex" | "s_" | "s_not" | "temgetter" | "nv" | "nvert"));
        if MUTATING.contains(&qual) || in_place_op || qual.starts_with("array.") {
            return None;
        }
        for prefix in ALLOWLIST_PREFIXES {
            if qual.starts_with(prefix) || qual.starts_with(&format!("builtins.{}", prefix)) {
                return Some(noeffect);
            }
        }
        // (the builtins that change nothing they are given and run none of the program's code;
        // any other without a model of its own is an unknown call)
        match qual.strip_prefix("builtins.") {
            Some("help" | "copyright" | "credits" | "license" | "breakpoint" | "compile" | "ascii" | "bin" | "oct" | "complex" | "dir" | "classmethod" | "staticmethod" | "property" | "bytearray" | "memoryview") => Some(noeffect),
            _ => None,
        }
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
    if c.qual == "builtins.abs" && an.m.torch_on {
        if let Some(d) = crate::models::torch::dim_of_label(a) {
            // dims are not negative
            if crate::models::shape::poly(d).iter().all(|(_, k)| *k > 0) {
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
                let r = if c.qual == "math.ceil" { crate::models::shape::ceildiv(d, crate::models::shape::lit(k)) } else { crate::models::shape::floordiv(d, crate::models::shape::lit(k)) };
                return crate::models::torch::dim_value(crate::models::shape::norm(r, &st.tf));
            }
        }
    }
    if an.m.torch_real && !matches!(c.qual.as_str(), "math.floor" | "math.ceil" | "math.isclose" | "math.isnan" | "math.isinf" | "math.prod" | "math.gcd" | "math.comb" | "math.factorial") {
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
    // (a library function given the program's functions may call them: `inspect.getmembers(x,
    // pred)`, `asyncio.to_thread(f, …)`, `os.walk(top, onerror=f)`)
    let code = |v: &Value| v.pts.iter().any(|o| matches!(an.heap.kind(o), Kind::Function | Kind::Bound | Kind::Partial));
    if !c.qual.starts_with("builtins.") && c.args.iter().chain(c.kwargs.values()).chain(c.star_args.iter()).any(code) {
        return an.unknown_call(&c.qual, c.recv.as_ref(), &c.args, &c.kwargs, &c.star_args, &c.star_kwargs, st, &c.node);
    }
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

/// A builtin or container method taken for one that raises only on arguments of the wrong
/// type (`len`, `sorted`, `str.join`, …) may still raise a TypeError where an argument (or a
/// builtin's result) is not known: a running try records that.
pub fn may_type_error(an: &mut Analyzer, qual: &str, args: &[Value], kwargs: &IndexMap<StrId, Value>, r: &Value, state: &mut State) {
    if !state.reachable {
        return;
    }
    if let ("builtins.hash", [a]) = (qual, args) {
        an.check_hashable(a, state);
        return;
    }
    if sure_type_error(an, qual, args, kwargs, state) {
        an.raise_here("TypeError", state);
        return;
    }
    if !an.in_try() {
        return;
    }
    // (known: literals and the program's containers of literals; not an unknown value, an
    // instance, the items of a list of those)
    let known_kv = |kv: &KeyVal| matches!(kv, KeyVal::Bot | KeyVal::Lit(_) | KeyVal::Lits(_));
    let known = |an: &Analyzer, v: &Value| {
        known_kv(&v.kv)
            && v.pts.iter().all(|o| match an.heap.kind(o) {
                Kind::Unknown | Kind::Instance => false,
                Kind::List | Kind::Tuple | Kind::Set => state.elems.get(&o).is_none_or(|kv| known_kv(&kv)) && state.slot(o, Ns::Elem, intern::STAR).pts.iter().all(|x| !matches!(an.heap.kind(x), Kind::Unknown | Kind::Instance)),
                _ => true,
            })
    };
    let unknown = match qual.strip_prefix("builtins.") {
        Some(n) if crate::models::tables::SAFE_BUILTINS.contains(&n) => {
            if matches!(n, "str" | "repr" | "print" | "id" | "bool" | "callable" | "type" | "super") {
                return;
            }
            // (isinstance / issubclass / hasattr take anything first)
            let skip = usize::from(matches!(n, "isinstance" | "issubclass" | "hasattr"));
            args.iter().skip(skip).chain(kwargs.values()).any(|a| !known(an, a)) || (skip == 0 && !known_kv(&r.kv))
        }
        Some(_) => return,
        // (a str method given arguments whose result is not known may have failed on them:
        // `",".join(items)` of non-strs)
        None if crate::interp::safe_method(qual) => args.iter().chain(kwargs.values()).any(|a| !known(an, a)) || (qual.starts_with("str.") && !args.is_empty() && !known_kv(&r.kv)),
        None => return,
    };
    if unknown {
        an.may_raise_here("TypeError", state);
    }
}

/// The type of a value that is surely a literal of one kind: 0 a number (bool, int, float),
/// 1 a str, 2 bytes, 3 None.
fn lit_kind(v: &Value) -> Option<u8> {
    if !v.pts.is_empty() {
        return None;
    }
    let ks: Vec<KeyId> = match &v.kv {
        KeyVal::Lit(k) => vec![*k],
        KeyVal::Lits(ks) if !ks.is_empty() => ks.to_vec(),
        _ => return None,
    };
    let kind = |k: KeyId| {
        intern::with_key(k, |k| match k {
            intern::Key::Int(_) | intern::Key::Float(_) | intern::Key::Bool(_) => Some(0),
            intern::Key::Str(_) => Some(1),
            intern::Key::Bytes(_) => Some(2),
            intern::Key::None => Some(3),
            _ => None,
        })
    };
    let first = kind(ks[0])?;
    ks.iter().all(|k| kind(*k) == Some(first)).then_some(first)
}

/// A builtin given arguments of a type it rejects on every run: `len(5)`,
/// `sorted([1, "a"])` (a sort that compares no int with a str cannot order them),
/// `",".join([1])`, `sum(["a"])` (`0 + "a"`).
fn sure_type_error(an: &Analyzer, qual: &str, args: &[Value], kwargs: &IndexMap<StrId, Value>, st: &State) -> bool {
    // the items of one list / tuple of known length
    let items = |v: &Value| -> Option<Vec<Value>> {
        let o = v.single().filter(|o| v.kv.is_bot() && matches!(an.heap.kind(*o), Kind::List | Kind::Tuple) && !an.heap.obj(*o).iterator)?;
        let n = an.heap.llen(o, st)?;
        Some((0..n).map(|i| st.slot(o, Ns::Pos, intern::key_int(i as i64))).collect())
    };
    let [a] = args else { return false };
    match qual {
        "builtins.len" => matches!(lit_kind(a), Some(0 | 3)),
        "builtins.sorted" | "builtins.min" | "builtins.max" if kwargs.keys().all(|k| *k == s("reverse")) => items(a).is_some_and(|xs| {
            let ks: Vec<Option<u8>> = xs.iter().map(lit_kind).collect();
            xs.len() >= 2 && (ks.contains(&Some(3)) || ks.iter().flatten().any(|k| ks.iter().flatten().any(|j| j != k)))
        }),
        "builtins.sum" if kwargs.is_empty() => items(a).is_some_and(|xs| {
            let ks: Vec<Option<u8>> = xs.iter().map(lit_kind).collect();
            ks.iter().all(|k| k.is_some()) && ks.iter().any(|k| *k != Some(0))
        }),
        "str.join" if kwargs.is_empty() => items(a).is_some_and(|xs| xs.iter().any(|x| matches!(lit_kind(x), Some(0 | 2 | 3)))),
        _ => false,
    }
}

/// A builtin Python carries out by a dunder of its argument, given instances of the
/// program's classes that define it: `str` / `repr` / `format` / `print` (`__str__`,
/// `__repr__`, `__format__`), `hash`, `len`, `int`, `float`, `abs`, `bool`; `pprint`'s
/// printers (`__repr__`). The dunders run
/// (their effects happen) and give the result; the rest of the argument goes to the model.
#[allow(clippy::too_many_arguments)]
pub fn builtin_dunder(an: &mut Analyzer, qual: &str, args: &[Value], kwargs: &IndexMap<StrId, Value>, star_args: &[Value], star_kwargs: &[Value], state: &mut State, node: &crate::frontend::Node) -> Option<Value> {
    let names: &[&str] = match qual.strip_prefix("builtins.").unwrap_or(qual) {
        "str" | "print" => &["__str__", "__repr__"],
        "pprint.pp" | "pprint.pprint" | "pprint.pformat" => &["__repr__"],
        "repr" | "ascii" => &["__repr__"],
        "format" => &["__format__"],
        "hash" => &["__hash__"],
        "len" => &["__len__"],
        "int" => &["__int__", "__index__"],
        "float" => &["__float__", "__index__"],
        "complex" => &["__complex__", "__float__", "__index__"],
        "bin" | "oct" | "hex" => &["__index__"],
        "abs" => &["__abs__"],
        "round" => &["__round__"],
        "divmod" => &["__divmod__"],
        "pow" => &["__pow__"],
        "dir" => &["__dir__"],
        "bytes" | "bytearray" => &["__bytes__", "__index__", "__iter__"],
        "bool" => &["__bool__", "__len__"],
        _ => return None,
    };
    let insts = |an: &Analyzer, v: &Value| v.pts.iter().any(|o| an.heap.kind(o) == Kind::Instance && an.heap.instance_class(o).is_some_and(|c| names.iter().any(|n| an.class_lookup(c, n).is_some())));
    if !star_args.is_empty() || !star_kwargs.is_empty() || !args.iter().any(|a| insts(an, a)) {
        return None;
    }
    let rest_call = |an: &mut Analyzer, rest: Value, state: &mut State| -> Value {
        if rest.pts.is_empty() && rest.kv.is_bot() {
            return Value::new();
        }
        let mut a = args.to_vec();
        a[0] = rest;
        an.call_external(qual, &a, kwargs, &[], &[], state, node, None)
    };
    match qual {
        // (each argument's text; the call writes those)
        "builtins.print" | "pprint.pp" | "pprint.pprint" | "pprint.pformat" => {
            let mut texts = Vec::new();
            for a in args {
                texts.push(match an.run_dunder(a, names, vec![], node, state) {
                    Some((t, rest)) if rest.pts.is_empty() && rest.kv.is_bot() => t,
                    Some((t, rest)) => t.join(&rest),
                    None => a.clone(),
                });
            }
            Some(an.call_external(qual, &texts, kwargs, &[], &[], state, node, None))
        }
        "builtins.bool" if args.len() == 1 => Some(match an.truth_of(&args[0], state, node) {
            Some(b) => Value::lit(intern::key(intern::Key::Bool(b))),
            None => Value::data(&format!("L{}", node.line)),
        }),
        // `format(x, spec)`: `__format__(spec)`; without one, `object.__format__` is `str(x)`
        // for an empty spec
        "builtins.format" => {
            let spec = args.get(1).cloned().unwrap_or_else(|| Value::lit_str(""));
            let (r, rest) = format_dunder(an, &args[0], spec, node, state)?;
            let tail = rest_call(an, rest, state);
            Some(r.join(&tail))
        }
        // (the other arguments go to the dunder: `round(x, 2)` is `x.__round__(2)`; the text of
        // what `__index__` gives for `bin`, `hex`, …; the items `__iter__` gives for `bytes`)
        _ if !args.is_empty() => {
            let (r, rest) = an.run_dunder(&args[0], names, args[1..].to_vec(), node, state)?;
            let r = match qual {
                "builtins.bin" | "builtins.oct" | "builtins.hex" | "builtins.complex" => an.call_external(qual, &[r], kwargs, &[], &[], state, node, None),
                "builtins.bytes" | "builtins.bytearray" => {
                    an.iterate(&r, node, state);
                    Value::data(&format!("L{}", node.line))
                }
                _ => r,
            };
            let tail = rest_call(an, rest, state);
            Some(r.join(&tail))
        }
        _ => None,
    }
}

/// `format(v, spec)` for the instances among `v` whose class defines `__format__`, or (an
/// empty spec) `__str__` / `__repr__`: their texts joined, and the rest of `v`.
pub fn format_dunder(an: &mut Analyzer, v: &Value, spec: Value, node: &crate::frontend::Node, state: &mut State) -> Option<(Value, Value)> {
    let mut ran = an.run_dunder(v, &["__format__"], vec![spec.clone()], node, state);
    if spec.kv.as_lit() == Some(key_str_lit("")) && spec.pts.is_empty() {
        let rest = ran.as_ref().map_or(v, |(_, r)| r).clone();
        if let Some((r, rest2)) = an.run_dunder(&rest, &["__str__", "__repr__"], vec![], node, state) {
            ran = Some((ran.map_or(r.clone(), |(o, _)| o.join(&r)), rest2));
        }
    }
    ran
}

fn last(q: &str) -> &str {
    q.rsplit('.').next().unwrap_or(q)
}

/// Module constants with known values (`re.IGNORECASE`, `math.pi`, `string.digits`, …).
pub fn external_constant(an: &Analyzer, qual: &str) -> Option<Value> {
    let int = |i: i64| Some(Value::lit(intern::key_int(i)));
    let st = |s: &str| Some(Value::lit(key_str_lit(s)));
    match qual {
        "re.I" | "re.IGNORECASE" => int(2),
        "re.L" | "re.LOCALE" => int(4),
        "re.M" | "re.MULTILINE" => int(8),
        "re.S" | "re.DOTALL" => int(16),
        "re.U" | "re.UNICODE" => int(32),
        "re.X" | "re.VERBOSE" => int(64),
        "re.A" | "re.ASCII" => int(256),
        "re.NOFLAG" => int(0),
        "sys.maxsize" => int(i64::MAX),
        // (False when the program runs: what it imports only for type checkers is not imported)
        "typing.TYPE_CHECKING" | "typing_extensions.TYPE_CHECKING" => Some(Value::lit(intern::key_bool(false))),
        "string.ascii_letters" => st("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"),
        "string.ascii_lowercase" => st("abcdefghijklmnopqrstuvwxyz"),
        "string.ascii_uppercase" => st("ABCDEFGHIJKLMNOPQRSTUVWXYZ"),
        "string.digits" => st("0123456789"),
        "string.hexdigits" => st("0123456789abcdefABCDEF"),
        "string.octdigits" => st("01234567"),
        "string.punctuation" => st("!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~"),
        "string.whitespace" => st(" \t\n\r\x0b\x0c"),
        "string.printable" => st("0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~ \t\n\r\x0b\x0c"),
        q if q.starts_with("datetime.") && chrono::constant(q).is_some() => Some(Value::lit(chrono::constant(q).unwrap())),
        "decimal.ROUND_HALF_EVEN" | "decimal.ROUND_HALF_UP" | "decimal.ROUND_HALF_DOWN" | "decimal.ROUND_UP" | "decimal.ROUND_DOWN" | "decimal.ROUND_CEILING" | "decimal.ROUND_FLOOR" | "decimal.ROUND_05UP" => st(qual.trim_start_matches("decimal.")),
        "os.sep" | "os.path.sep" => st("/"),
        "os.linesep" => st("\n"),
        "os.pathsep" => st(":"),
        "os.curdir" => st("."),
        "os.pardir" => st(".."),
        _ if an.m.torch_real => None,
        "math.pi" => Some(Value::lit(intern::key_float(std::f64::consts::PI))),
        "math.e" => Some(Value::lit(intern::key_float(std::f64::consts::E))),
        "math.tau" => Some(Value::lit(intern::key_float(std::f64::consts::TAU))),
        "math.inf" => Some(Value::lit(intern::key_float(f64::INFINITY))),
        _ => None,
    }
}

// ------------------------------------------------------------- externals that are records
pub fn external_record(an: &mut Analyzer, qual: &str, state: &mut State) -> Option<u32> {
    if qual == "os.environ" || qual == "os.environb" {
        let oid = an.heap.alloc_raw(Kind::Dict, &format!("ext-record:{}", qual), "", true, qual, Payload::None);
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
        let recs = an.record_objects(src);
        if !recs.is_empty() {
            an.copy_into(oid, Ns::Item, src, st, true, &KeySet::new());
        } else if let Some(items) = src.single().filter(|_| src.kv.is_bot()).and_then(|x| an.m.sorted_items.get(&x).cloned()) {
            // `dict(sorted(d.items()))`: d's keys (the sure ones sure) in sorted order
            let mut d = st.row(oid, Ns::Item).data();
            let mut order = d.order.to_vec();
            for (k, val, sure) in items.iter() {
                d.present = d.present.insert(*k);
                d.stored = d.stored.insert(*k);
                if *sure {
                    d.definite = d.definite.insert(*k);
                    d.likely = d.likely.insert(*k);
                }
                if !order.contains(k) {
                    order.push(*k);
                }
                st.set_slot(oid, Ns::Item, *k, val.clone(), true);
            }
            d.order = order.into();
            st.set_row(oid, Ns::Item, Row::from_data(d));
        } else {
            for x in src.pts.iter() {
                match an.heap.kind(x) {
                    Kind::Zip => {
                        let Payload::Zip(ks, vs) = an.heap.obj(x).payload.clone() else { continue };
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
                        // values that are one of several sequences of known lengths: the keys
                        // the shortest pairs are definite, those of the longer ones possible
                        let klist = an.iterate(&ks, &c.node, st).items.and_then(|its| its.iter().map(|v| v.kv.as_lit().filter(|_| v.pts.is_empty())).collect::<Option<Vec<KeyId>>>());
                        let lens: Option<Vec<usize>> = if vs.pts.len() > 1 && vs.kv.is_bot() { vs.pts.iter().map(|o| if matches!(an.heap.kind(o), Kind::List | Kind::Tuple) && !an.heap.obj(o).iterator { an.heap.llen(o, st) } else { None }).collect() } else { None };
                        if let (Some(kl), Some(lens)) = (klist, lens) {
                            let (lo, hi) = (lens.iter().copied().min().unwrap_or(0), lens.iter().copied().max().unwrap_or(0));
                            let mut row = st.row(oid, Ns::Item);
                            for (i, k) in kl.into_iter().enumerate().take(hi) {
                                row = row.store(k, i < lo);
                                st.set_slot(oid, Ns::Item, k, Value::with_kv(vel.pts.clone(), DATA), false);
                            }
                            st.set_row(oid, Ns::Item, row);
                        } else if let Some(lits) = lits {
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
    let val = c.args.get(1).cloned().unwrap_or_else(|| an.none_v.clone());
    let Some(a) = c.args.first() else { return Value::obj(oid) };
    // known items that are literals, in order (a list, a string's characters, a generator)
    let items = an.iterate(a, &c.node, st).items;
    let lits: Option<Vec<KeyId>> = items.and_then(|its| its.iter().map(|v| v.kv.as_lit().filter(|_| v.pts.is_empty())).collect());
    let mut row = st.row(oid, Ns::Item);
    if let Some(ls) = lits {
        for k in ls {
            row = row.store(k, true);
            st.set_slot(oid, Ns::Item, k, val.clone(), true);
        }
        st.set_row(oid, Ns::Item, row);
    } else if let Some(ks) = if a.pts.is_empty() { None } else { an.lit_keys(a, st) } {
        // the literals the iterable may hold (how many is not known): possible keys
        for k in ks.iter() {
            row = row.store(k, false);
            st.set_slot(oid, Ns::Item, k, val.clone(), true);
        }
        st.set_row(oid, Ns::Item, row);
    } else {
        let el = an.elem_of(a, &c.node, st);
        an.store_key(oid, Ns::Item, &el.kv, &val, false, &c.node, st);
    }
    Value::obj(oid)
}

fn m_list(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    if an.m.torch_on && matches!(c.qual.as_str(), "builtins.tuple" | "builtins.list") {
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
    an.heap.set_llen(oid, st, None);
    an.m.sorted_items.remove(&oid);
    if c.qual == "builtins.sorted" && c.kw("key").is_none() && an.heap.obj(oid).single {
        if let Some(items) = c.args.first().and_then(|a| sorted_dict_items(an, st, a, c)) {
            an.m.sorted_items.insert(oid, std::rc::Rc::new(items));
        }
    }
    if let Some(a) = c.args.first() {
        let it = an.iterate(a, &c.node, st);
        if kind == Kind::Set {
            match &it.items {
                Some(items) => items.iter().for_each(|x| an.check_hashable(x, st)),
                None => an.check_hashable_if(&it.elem.flat(), false, st),
            }
        }
        if c.qual == "builtins.iter" {
            an.heap.obj_mut(oid).iterator = true;
        } else {
            an.exhaust(a, st);
        }
        match &it.elem {
            Elem::Pair(k, v) => {
                let ctx = an.frame().ctx.clone();
                let pid = an.heap.alloc_raw(Kind::Pair, &format!("pair:{}", c.site), &ctx, true, "pair", Payload::Pair(k.clone(), v.clone()));
                st.set_slot(oid, Ns::Elem, STAR, Value::obj(pid), true);
                st.elems.insert(oid, DATA);
                // `sorted(d.items(), key=f)` with unknown items: f reads every pair
                if c.qual == "builtins.sorted" && it.items.is_none() {
                    if let Some(keyfn) = c.kw("key") {
                        if !keyfn.pts.is_empty() {
                            an.call_value(&keyfn, &c.node, None, st, Some(vec![Value::obj(pid)]));
                        }
                    }
                }
                // known pairs (zip / enumerate / items of known inputs) keep their positions;
                // sorted by known literal keys, in that order
                let plain = matches!(c.qual.as_str(), "builtins.list" | "builtins.tuple" | "builtins.reversed" | "builtins.sorted");
                if let (true, Some(items)) = (plain, &it.items) {
                    if an.heap.obj(oid).single && items.len() <= 64 {
                        let order: Option<Vec<usize>> = match c.qual.as_str() {
                            "builtins.sorted" => literal_sort_order(an, st, c, items),
                            "builtins.reversed" => Some((0..items.len()).rev().collect()),
                            _ => Some((0..items.len()).collect()),
                        };
                        if order.is_none() && c.qual == "builtins.sorted" {
                            // (items in no known order may not compare: a TypeError)
                            an.may_raise_here("TypeError", st);
                        }
                        if let Some(order) = order {
                            an.heap.set_llen(oid, st, Some(items.len()));
                            for (i, j) in order.into_iter().enumerate() {
                                st.set_slot(oid, Ns::Pos, intern::key_int(i as i64), items[j].clone(), true);
                            }
                        } else {
                            an.heap.set_llen(oid, st, Some(items.len()));
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
                // (a set of known literals, `set(text)`: its distinct items, however many the
                // iterable had)
                let text_chars: Option<Vec<Value>> = a.kv.as_lit().and_then(intern::key_as_str).filter(|_| a.pts.is_empty()).map(|t| t.chars().map(|ch| Value::lit(key_str_lit(&ch.to_string()))).collect());
                let distinct: Option<Vec<Value>> = it.items.as_ref().or(text_chars.as_ref()).filter(|_| kind == Kind::Set).and_then(|items| {
                    let mut seen: Vec<KeyId> = Vec::new();
                    for v in items {
                        match (&v.kv, v.pts.is_empty()) {
                            (KeyVal::Lit(k), true) if !seen.contains(k) => seen.push(*k),
                            (KeyVal::Lit(_), true) => {}
                            _ => return None,
                        }
                    }
                    Some(seen.into_iter().map(Value::lit).collect())
                });
                if let Some(items) = distinct.as_ref().or(it.items.as_ref()) {
                    let reversed = c.qual.as_str() == "builtins.reversed";
                    let sorted = c.qual.as_str() == "builtins.sorted";
                    if an.heap.obj(oid).single && items.len() <= 64 {
                        an.heap.set_llen(oid, st, Some(items.len()));
                        let n = items.len();
                        // sorted: when every sort key is a known literal (or a tuple of
                        // them) the order is known (stable sort); otherwise the count is
                        // known and every position may hold any item
                        let order: Option<Vec<usize>> = if sorted { literal_sort_order(an, st, c, items) } else { None };
                        if sorted && order.is_none() {
                            an.may_raise_here("TypeError", st);
                        }
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
                        let from_set = a.pts.iter().any(|o| an.heap.kind(o) == Kind::Set);
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
        // a dict with some keys only maybe there: the sure keys before the first maybe one
        // are the first items (the length is not known)
        if matches!(c.qual.as_str(), "builtins.list" | "builtins.tuple") && an.heap.llen(oid, st).is_none() && an.heap.obj(oid).single {
            if let Some(m) = an.maybe_dict_items(a, st) {
                let lead: Vec<Value> = m.iter().take_while(|(_, sure)| *sure).map(|(v, _)| v.clone()).collect();
                if !lead.is_empty() {
                    for (i, v) in lead.iter().enumerate() {
                        st.set_slot(oid, Ns::Pos, intern::key_int(i as i64), v.clone(), true);
                    }
                    st.set_lens(oid, (None, lead.len(), 0));
                }
            }
        }
    } else {
        st.elems.insert(oid, KeyVal::Lits(KeySet::new()));
        an.heap.set_llen(oid, st, Some(0));
    }
    // (a copy of a list / tuple whose length is one of a few: the same alternatives)
    if matches!(c.qual.as_str(), "builtins.list" | "builtins.tuple" | "builtins.sorted" | "builtins.reversed") && an.heap.llen(oid, st).is_none() {
        if let Some(src) = c.args.first().and_then(|a| a.single()).filter(|s| matches!(an.heap.kind(*s), Kind::List | Kind::Tuple)) {
            if let Some((None, _, m)) = st.lens.get(&src).filter(|l| l.2 != 0) {
                let p = an.heap.prefix(oid, st).unwrap_or(0);
                st.set_lens(oid, (None, p, m));
            }
        }
    }
    Value::obj(oid)
}

/// `isinstance(x, C)`: decided when every object of `x` has a known kind / class; a
/// placeholder records an event so the call site decides.
/// `callable(x)`: decided where every value `x` may be is: functions, classes and bound
/// methods are, literals are not, an instance is when its class defines `__call__`.
fn m_callable(an: &mut Analyzer, _st: &mut State, c: &Call) -> Value {
    let Some(v) = c.args.first() else { return Value::data("callable") };
    let mut seen: Vec<bool> = Vec::new();
    if !v.kv.is_bot() {
        if !v.kv.is_literal() || crate::domain::kv_literals(&v.kv).is_empty() {
            return Value::data("callable");
        }
        seen.push(false);
    }
    for o in v.pts.iter() {
        let b = match an.heap.kind(o) {
            Kind::Function | Kind::Bound | Kind::Class | Kind::Partial | Kind::Method => true,
            Kind::Instance => match an.heap.instance_class(o) {
                Some(cls) if !an.heap.mro(cls).iter().any(|x| an.heap.class_info_opt(*x).is_none_or(|ci| ci.open_base)) => an.class_lookup(cls, "__call__").is_some(),
                _ => return Value::data("callable"),
            },
            Kind::Dict | Kind::List | Kind::Tuple | Kind::Set => false,
            // (a builtin function or class; other library names may be constants)
            Kind::External if an.heap.external_qual(o).strip_prefix("builtins.").is_some_and(|n| crate::interp::is_builtin_name(n) && !n.starts_with("__") && !matches!(n, "NotImplemented" | "Ellipsis" | "None" | "True" | "False")) => true,
            _ => return Value::data("callable"),
        };
        seen.push(b);
    }
    match (seen.iter().all(|b| *b), seen.iter().all(|b| !*b)) {
        (true, false) => Value::lit(intern::key_bool(true)),
        (false, true) => Value::lit(intern::key_bool(false)),
        _ => Value::data("callable"),
    }
}

fn m_isinstance(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let (Some(v), Some(cv)) = (c.args.first(), c.args.get(1)) else { return Value::data("bool") };
    // isinstance(x, torch.Tensor): decided for tensors and for containers / literals
    if an.m.torch_on {
        if let Some(q) = cv.single().filter(|o| an.heap.kind(*o) == Kind::External).map(|o| an.heap.external_qual(o)) {
            if q == "torch.Tensor" || q == "torch.nn.Parameter" || q == "torch.nn.parameter.Parameter" {
                let tensor = crate::models::torch::is_tensor(v);
                let not_tensor = !tensor
                    && !v.pts.is_empty()
                    && v.pts.iter().all(|o| matches!(an.heap.kind(o), Kind::List | Kind::Tuple | Kind::Dict | Kind::Set | Kind::Namespace | Kind::Function | Kind::Class) || (an.heap.kind(o) == Kind::Instance && !crate::models::torch::t_is_module_instance(an, o)))
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
    let Some(classes) = an.isinstance_classes(cv, st) else { return Value::data("bool") };
    let builtin = |an: &Analyzer, cls: u32| -> Option<String> { if an.heap.kind(cls) == Kind::External { Some(an.heap.external_qual(cls).rsplit('.').next().unwrap_or("").to_string()) } else { None } };
    let mut verdicts: Vec<bool> = Vec::new();
    // literal values (with or without objects: both parts count)
    if v.pts.is_empty() || (v.kv.is_literal() && !crate::domain::kv_literals(&v.kv).is_empty()) {
        let lits = crate::domain::kv_literals(&v.kv);
        if !v.kv.is_literal() || lits.is_empty() {
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
                intern::Key::Dec(..) => "Decimal",
                intern::Key::Frac(..) => "Fraction",
                intern::Key::Frozen(_) => "frozenset",
                _ => "?",
            });
            // (an abstract class like numbers.Number or collections.abc.Hashable: not decided)
            const KNOWN: &[&str] = &["str", "bool", "int", "float", "NoneType", "tuple", "bytes", "Decimal", "Fraction", "frozenset", "object", "list", "dict", "set", "complex", "bytearray", "type", "range", "defaultdict", "OrderedDict", "Counter", "deque"];
            if kind == "?" || classes.iter().any(|cls| builtin(an, *cls).map(|b| !KNOWN.contains(&b.as_str())).unwrap_or(false) || protocol_members(an, *cls).is_some()) {
                return Value::data("bool");
            }
            let hit = classes.iter().any(|cls| match builtin(an, *cls) {
                Some(b) => b == kind || (b == "int" && kind == "bool") || (b == "object"),
                None => false,
            });
            verdicts.push(hit);
        }
    }
    for o in v.pts.iter() {
        match an.instance_of(o, &classes, st) {
            Some(b) => verdicts.push(b),
            None => return Value::data("bool"),
        }
    }
    if verdicts.iter().all(|b| *b) {
        Value::lit(intern::key_bool(true))
    } else if verdicts.iter().all(|b| !*b) {
        Value::lit(intern::key_bool(false))
    } else {
        Value::data("bool")
    }
}

/// `a op b` (an ordering or equality) where a library value decides it: pathlib paths are
/// equal when their texts are (never equal to a str); Decimal / Fraction / datetime literals
/// compare by value (`Decimal("1.0") == 1`).
pub fn library_compare(an: &Analyzer, op: ast::CmpOp, a: &Value, b: &Value) -> Option<Value> {
    let lit = |t: bool| Value::lit(intern::key_bool(t));
    if matches!(op, ast::CmpOp::Eq | ast::CmpOp::NotEq) {
        let pt = |v: &Value| v.single().filter(|_| v.kv.is_bot()).and_then(|o| crate::models::pathtext::path_text(an, o));
        let (pa, pb) = (pt(a), pt(b));
        let decided = match (&pa, &pb) {
            (Some(x), Some(y)) => Some(x == y),
            (Some(_), None) | (None, Some(_)) if (a.pts.is_empty() && a.kv.is_literal()) || (b.pts.is_empty() && b.kv.is_literal()) => Some(false),
            _ => None,
        };
        if let Some(eq) = decided {
            return Some(lit(if op == ast::CmpOp::Eq { eq } else { !eq }));
        }
    }
    let has_num = |v: &Value| v.kv.is_literal() && crate::domain::kv_literals(&v.kv).iter().any(|k| pynum::is_num_key(k) || chrono::is_chrono_key(k));
    if !(has_num(a) || has_num(b)) || !a.pts.is_empty() || !b.pts.is_empty() {
        return None;
    }
    let opn = match op {
        ast::CmpOp::Eq => "eq",
        ast::CmpOp::NotEq => "ne",
        ast::CmpOp::Lt => "lt",
        ast::CmpOp::LtE => "le",
        ast::CmpOp::Gt => "gt",
        _ => "ge",
    };
    let mut outs: Vec<bool> = Vec::new();
    for x in crate::domain::kv_literals(&a.kv).iter() {
        for y in crate::domain::kv_literals(&b.kv).iter() {
            match pynum::binop(opn, x, y).or_else(|| chrono::binop(opn, x, y)).flatten().and_then(|k| intern::with_key(k, |kk| if let intern::Key::Bool(t) = kk { Some(*t) } else { None })) {
                Some(t) => outs.push(t),
                None => return Some(Value::data("bool")),
            }
        }
    }
    outs.dedup();
    Some(if outs.len() == 1 { lit(outs[0]) } else { Value::data("bool") })
}

/// An attribute of a value without objects (a str, a number, a Decimal / Fraction / datetime,
/// None, a slice, an unknown scalar): its value, or the bound method.
pub fn scalar_attr(an: &mut Analyzer, obj: &Value, name: &str, report: bool, state: &mut State) -> Value {
    if name == "str" && matches!(obj.kv, KeyVal::KeysOf(_)) {
        // `df.columns.str.strip().str.lower()`: the accessor of renamed column names
        return obj.clone();
    }
    // a Decimal / Fraction: its attributes and methods
    let numlike = |k: KeyId| crate::models::pynum::is_num_key(k) || crate::models::chrono::is_chrono_key(k);
    if obj.kv.is_literal() && !crate::domain::kv_literals(&obj.kv).is_empty() && crate::domain::kv_literals(&obj.kv).iter().all(numlike) {
        let ks = crate::domain::kv_literals(&obj.kv);
        let attr_of = |k: KeyId, n: &str| if crate::models::chrono::is_chrono_key(k) { crate::models::chrono::attr(k, n) } else { crate::models::pynum::attr(k, n) };
        let mut out: Option<Vec<Value>> = Some(Vec::new());
        for k in ks.iter() {
            match (attr_of(k, name), out.as_mut()) {
                (Some(Some(v)), Some(o)) => o.push(Value::lit(v)),
                _ => out = None,
            }
        }
        if let Some(o) = out {
            return join_values(o);
        }
        if attr_of(ks.iter().next().unwrap(), name).is_some() {
            return Value::data(name);
        }
        return an.method_value(&format!("num.{}", name), obj);
    }
    // an attribute of None (but a dunder every object has): an AttributeError
    let none_only = obj.kv.is_literal() && crate::domain::kv_literals(&obj.kv).iter().all(|k| k == intern::key_none()) && !crate::domain::kv_literals(&obj.kv).is_empty();
    if none_only && report && !name.starts_with("__") {
        an.raise_here("AttributeError", state);
        return Value::new();
    }
    if matches!(obj.kv, KeyVal::Lit(_) | KeyVal::Lits(_) | KeyVal::KeysOf(_)) {
        return an.method_value(&format!("str.{}", name), obj);
    }
    if let KeyVal::Data(d) = &obj.kv {
        let label = intern::str_get(*d);
        if matches!(&*label, "datetime" | "date" | "time" | "timedelta" | "timezone") && crate::models::chrono::NAMES.contains(&name) {
            return match crate::models::chrono::attr(crate::models::chrono::chrono_key(crate::models::chrono::DELTA, 0, crate::models::chrono::NAIVE), name) {
                _ if matches!(name, "year" | "month" | "day" | "hour" | "minute" | "second" | "microsecond" | "days" | "seconds" | "microseconds" | "tzinfo") => Value::data(name),
                _ => an.method_value(&format!("num.{}", name), obj),
            };
        }
        // a string whose text is not known (a validated `str` field, a str method's result)
        if (&*label == "str" || crate::domain::label_type(&label) == Some("str")) && crate::interp::STR_METHOD_NAMES.contains(&name) {
            return an.method_value(&format!("str.{}", name), obj);
        }
        if matches!(&*label, "Decimal" | "Fraction") && crate::models::pynum::NAMES.contains(&name) {
            return if matches!(name, "numerator" | "denominator") { Value::data(name) } else { an.method_value(&format!("num.{}", name), obj) };
        }
    }
    // a slice object with known bounds
    if let Some((lo, hi, st)) = Analyzer::slice_label_bounds(&obj.kv) {
        let lit = |b: Option<i64>, an: &Analyzer| match b {
            Some(i) => Value::lit(intern::key_int(i)),
            None => an.none_v.clone(),
        };
        match name {
            "start" => return lit(lo, an),
            "stop" => return lit(hi, an),
            "step" => return lit(st, an),
            "indices" => return an.method_value("slice.indices", obj),
            _ => {}
        }
    }
    an.method_value(&format!("unknown.{}", name), obj)
}

/// The members of a `typing.Protocol` class (its own and its protocol bases'), as
/// `isinstance` checks them on a runtime-checkable one; None for any other class.
pub fn protocol_members(an: &Analyzer, cls: u32) -> Option<Vec<String>> {
    const SPECIAL: &[&str] = &["__init__", "__new__", "__module__", "__qualname__", "__doc__", "__dict__", "__weakref__", "__slots__", "__annotations__", "__class_getitem__", "__init_subclass__", "__subclasshook__", "__abstractmethods__", "__parameters__", "__orig_bases__", "__match_args__"];
    let ci = an.heap.class_info_opt(cls)?;
    // (`Protocol` / `typing.Protocol` / `Protocol[T]` among the bases as written)
    let is_proto = |e: &ast::Expr| match e {
        ast::Expr::Subscript(sb) => matches!(&*sb.value, ast::Expr::Name(n) if n.id.as_str() == "Protocol") || matches!(&*sb.value, ast::Expr::Attribute(a) if a.attr.as_str() == "Protocol"),
        ast::Expr::Name(n) => n.id.as_str() == "Protocol",
        ast::Expr::Attribute(a) => a.attr.as_str() == "Protocol",
        _ => false,
    };
    if !ci.node.bases().iter().any(is_proto) {
        return None;
    }
    let proto_bases: Vec<u32> = ci.bases.clone();
    let mut ms: Vec<String> = Vec::new();
    for st in ci.node.body.iter() {
        let name = match st {
            ast::Stmt::FunctionDef(f) => Some(f.name.id.to_string()),
            ast::Stmt::AnnAssign(a) => a.target.as_name_expr().map(|n| n.id.to_string()),
            ast::Stmt::Assign(a) => a.targets.first().and_then(|t| t.as_name_expr()).map(|n| n.id.to_string()),
            _ => None,
        };
        if let Some(n) = name.filter(|n| !SPECIAL.contains(&n.as_str()) && !n.starts_with("_abc_")) {
            ms.push(n);
        }
    }
    for b in proto_bases {
        ms.extend(protocol_members(an, b).unwrap_or_default());
    }
    Some(ms)
}

/// `max(xs, key=f)` / `min(xs)`: an element of the container (the key function runs on it);
/// with several arguments, any of them.
fn m_minmax(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    if an.m.torch_on {
        if let Some(v) = crate::models::torch::t_minmax(an, c.qual.ends_with("max"), &c.args, st) {
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
        let mut kvals: Vec<Value> = Vec::new();
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
            kvals.push(kv);
        }
        // items that order themselves (`__lt__`): compared one against the best so far
        if keys.iter().any(|k| k.is_none()) && keyf.is_none() && items.iter().all(|x| x.single().map(|o| an.heap.kind(o) == Kind::Instance).unwrap_or(false)) {
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
                match crate::interp::compare::literal_order(keys[i].unwrap(), keys[best].unwrap()) {
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
        // keys that are tuples / pairs of known items: compared item by item
        if keys.iter().any(|k| k.is_none()) {
            let mut best = 0usize;
            let mut ok = true;
            for i in 1..items.len() {
                match crate::models::stdfuncs::value_order(an, st, &kvals[i], &kvals[best]) {
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
        let elem = an.elem_value(&it.elem, &c.node, "el");
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
    if !c.star_args.is_empty() || c.args.len() != 2 {
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
        // (not every input's items known: tuples of their items, how many not known)
        if c.star_args.is_empty() {
            let elems: Vec<Value> = c.args.clone().iter().map(|x| {
                let e = an.iterate(x, &c.node, st).elem;
                an.elem_value(&e, &c.node, "zn")
            }).collect();
            let t = an.alloc_positional_sfx(Kind::Tuple, &c.node, "#t", Some(elems), Value::new(), DATA, st);
            return an.alloc_positional(Kind::List, &c.node, None, t, DATA, st);
        }
    }
    let a = c.args.first().cloned().unwrap_or_default();
    let b = c.args.get(1).cloned().unwrap_or_default();
    let oid = an.alloc_at(Kind::Zip, &c.node, st, &format!("zip@L{}", c.line), Payload::Zip(a.clone(), b.clone()), None, Ns::Item, None, None);
    an.heap.obj_mut(oid).payload = Payload::Zip(a.clone(), b.clone());
    // `strict=True`: lengths that differ raise ValueError; otherwise both have the known one
    let strict = c.kwargs.get(&intern::s("strict")).and_then(|v| v.kv.as_lit()) == Some(intern::key_bool(true));
    if strict {
        an.m.zip_strict.insert(oid);
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
    an.heap.obj_mut(oid).payload = Payload::Enumerate(a, start);
    Value::obj(oid)
}

fn m_next(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(a) = c.args.first() else { return Value::data("next") };
    // a generator with known yields: the item at the cursor, which moves on
    if let Some(g) = a.single() {
        if matches!(an.heap.kind(g), Kind::List) && an.heap.obj(g).single {
            // (an endless generator: the positions it is known to reach)
            if let (None, Some(p), Some(cur)) = (an.heap.llen(g, st), an.heap.prefix(g, st), an.cursor_of(g, st)) {
                if cur < p && an.heap.obj(g).iterator {
                    an.set_cursor(g, Some(cur as usize + 1), st);
                    return st.slot(g, Ns::Pos, intern::key_int(cur as i64));
                }
            }
            if let Some(n) = an.heap.llen(g, st) {
                let cur = an.cursor_of(g, st).map(|c| c as i64).unwrap_or(-1);
                if cur >= 0 && (cur as usize) < n {
                    an.set_cursor(g, Some(cur as usize + 1), st);
                    return st.slot(g, Ns::Pos, intern::key_int(cur));
                }
                if cur >= 0 && (cur as usize) >= n {
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
    // (a generator at a position not known: it ran to one of its yields at least)
    if let Some(g) = a.single() {
        an.gen_took_item(g, st);
    }
    match it.elem {
        Elem::Pair(k, v) => {
            let ctx = an.frame().ctx.clone();
            let pid = an.heap.alloc_raw(Kind::Pair, &format!("pair:{}", c.site), &ctx, true, "pair", Payload::Pair(k, v));
            Value::obj(pid)
        }
        Elem::One(elem) => match it.items {
            Some(items) if !items.is_empty() => items[0].clone(),
            _ => elem,
        },
    }
}

fn m_getattr(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(obj) = c.args.first().cloned() else { return Value::data("getattr") };
    let name_v = c.args.get(1).cloned().unwrap_or_default();
    let lit = name_v.kv.as_lit().and_then(intern::key_as_str).map(|name| {
        // (with a default: the default where the attribute is surely not there, both where it
        // may not be)
        let Some(d) = c.args.get(2) else { return an.load_attr(&obj, &name, &c.node, st, true) };
        let has = m_hasattr(an, st, &Call { args: c.args[..2].to_vec(), ..c.clone() });
        let found = |an: &mut Analyzer, st: &mut State| an.load_attr(&obj, &name, &c.node, st, false);
        match has.kv.as_lit().filter(|_| has.pts.is_empty()).map(|b| b == intern::key_bool(true)) {
            Some(false) => d.clone(),
            Some(true) => found(an, st),
            None => found(an, st).join(d),
        }
    });
    if lit.is_some() && name_v.pts.is_empty() {
        return lit.unwrap();
    }
    // a name not known: any of the object's attributes (of a library object, anything), or the
    // default; known in part: that attribute, or one whose name fits the text known of it
    // (`f"run_{mode}"`: `run_*`), else one rowan does not follow (AGENTS.md: getattr by unknown
    // names is not followed; every attribute and method of `self` was too costly)
    let affixes: Option<Vec<(String, String)>> = name_v.pts.iter().map(|o| match &an.heap.obj(o).payload {
        Payload::Affix(p, s) => Some((p.clone(), s.clone())),
        _ => None,
    }).collect();
    let mut v = match (lit.is_some(), affixes) {
        (true, Some(afs)) if !afs.is_empty() => {
            let names = attr_names_like(an, &obj, &afs, st);
            let mut v = Value::new();
            for n in names {
                let has = m_hasattr(an, st, &Call { args: vec![obj.clone(), Value::lit_str(&n)], ..c.clone() });
                if has.kv.as_lit() != Some(intern::key_bool(false)) || !has.pts.is_empty() {
                    v = v.join(&an.load_attr(&obj, &n, &c.node, st, false));
                }
            }
            // (an object of unknown class may have others)
            if obj.pts.iter().any(|o| !matches!(an.heap.kind(o), Kind::Instance | Kind::Class | Kind::Module)) {
                v = v.join(&Value::obj(crate::interp::any_object(&mut an.heap)));
            }
            v
        }
        (true, _) => Value::obj(crate::interp::any_object(&mut an.heap)),
        _ => join_values(obj.pts.iter().map(|o| an.all_values(o, Ns::Attr, st))),
    };
    if obj.pts.iter().any(|o| an.heap.kind(o) == Kind::External) {
        let lib: Vec<Value> = vec![Value::with_kv(obj.pts.filter(|o| an.heap.kind(o) == Kind::External), crate::domain::BOT)];
        v = v.join(&an.unknown_call(&c.qual, None, &lib, &IndexMap::new(), &[], &[], st, &c.node));
    }
    for x in lit.iter().chain(c.args.get(2)) {
        v = v.join(x);
    }
    v
}

/// The attribute names of `obj` (its own and its classes') that start and end with one of the
/// affix pairs.
fn attr_names_like(an: &Analyzer, obj: &Value, afs: &[(String, String)], st: &State) -> Vec<String> {
    let fits = |n: &str| afs.iter().any(|(p, s)| n.len() >= p.len() + s.len() && n.starts_with(p.as_str()) && n.ends_with(s.as_str()));
    let mut owners: Vec<u32> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    for o in obj.pts.iter() {
        for k in st.row(o, Ns::Attr).present.iter() {
            names.extend(intern::key_as_str(k).map(|n| n.to_string()));
        }
        match an.heap.kind(o) {
            Kind::Instance => owners.extend(an.heap.instance_class(o).map(|c| an.heap.mro(c)).unwrap_or_default()),
            Kind::Class => owners.extend(an.heap.mro(o)),
            _ => {}
        }
    }
    if !owners.is_empty() {
        for (c, ns, k) in an.heap.cls_slots.keys() {
            if *ns == Ns::Attr && owners.contains(c) {
                names.extend(intern::key_as_str(*k).map(|n| n.to_string()));
            }
        }
    }
    names.retain(|n| fits(n));
    names.sort();
    names.dedup();
    names
}

fn m_setattr(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    // (setattr reports its own raises: frozen classes, slots, setter-less properties raise in
    // the store; an object of unknown class may)
    if let Some(o) = c.args.first() {
        if o.pts.is_empty() || o.pts.iter().any(|x| !matches!(an.heap.kind(x), Kind::Instance | Kind::Namespace | Kind::Class | Kind::Module | Kind::Function)) || c.args.get(1).map(|k| k.kv.as_lit().is_none()).unwrap_or(true) {
            an.may_raise_here("AttributeError", st);
        }
    }
    if c.args.len() >= 3 {
        match c.args[1].kv.as_lit().and_then(intern::key_as_str) {
            Some(name) => an.store_attr(&c.args[0], &name, &c.args[2], &c.node, st),
            None => {
                let recs = an.record_objects(&c.args[0]);
                let strong = recs.len() == 1 && an.heap.obj(recs[0]).single;
                for o in recs {
                    // a user `__setattr__` takes the store, whatever the name
                    let hook = an.heap.kind(o) == Kind::Instance && !an.setattr_bypassed() && an.heap.instance_class(o).is_some_and(|cls| an.class_lookup(cls, "__setattr__").is_some());
                    if hook {
                        an.call_method_with(&Value::obj(o), "__setattr__", vec![c.args[1].clone(), c.args[2].clone()], &c.node, st);
                        continue;
                    }
                    an.store_key(o, Ns::Attr, &c.args[1].kv, &c.args[2], strong, &c.node, st);
                }
            }
        }
    }
    an.none_v.clone()
}

/// `random.shuffle(xs)` of lists of known length: each position may hold any of the items
/// (the same objects); anything else is an unknown call.
fn m_shuffle(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let xs = c.args.first().cloned().unwrap_or_default();
    let lists: Vec<(ObjId, usize)> = xs.pts.iter().filter_map(|o| (an.heap.kind(o) == Kind::List && !an.heap.obj(o).iterator).then(|| an.heap.llen(o, st).map(|n| (o, n))).flatten()).collect();
    if c.args.len() != 1 || xs.pts.is_empty() || !xs.kv.is_bot() || lists.len() != xs.pts.len() {
        return an.unknown_call(&c.qual, None, &c.args, &c.kwargs, &c.star_args, &c.star_kwargs, st, &c.node);
    }
    for (o, n) in lists {
        let all = join_values((0..n).map(|i| st.slot(o, Ns::Pos, intern::key_int(i as i64))).collect::<Vec<_>>());
        for i in 0..n {
            st.set_slot(o, Ns::Pos, intern::key_int(i as i64), all.clone(), xs.pts.len() == 1);
        }
    }
    an.none_v.clone()
}

/// `reprlib.repr(x)`: a str; it runs the `__repr__` of what x reaches, so with an instance of
/// a class defining one (or anything unknown, or more than a few hundred objects) it is an
/// unknown call.
fn m_reprlib_repr(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let mut work: Vec<ObjId> = c.args.iter().flat_map(|a| a.pts.iter()).collect();
    let mut seen: FxHashSet<ObjId> = FxHashSet::default();
    let mut own_code = !c.kwargs.is_empty() || c.args.len() != 1;
    while let Some(o) = work.pop() {
        if !seen.insert(o) || matches!(an.heap.kind(o), Kind::Class | Kind::Module | Kind::Function | Kind::External) {
            continue;
        }
        own_code |= seen.len() > 512 || matches!(an.heap.kind(o), Kind::Unknown | Kind::Frame | Kind::Series) || an.heap.instance_class(o).is_some_and(|cls| an.class_lookup(cls, "__repr__").is_some());
        if own_code {
            break;
        }
        an.obj_edges(o, st, &mut work);
    }
    if own_code {
        return an.unknown_call(&c.qual, None, &c.args, &c.kwargs, &c.star_args, &c.star_kwargs, st, &c.node);
    }
    Value::data(&format!("L{}", c.line))
}

/// `operator.setitem(d, k, v)` into dicts by a key of no objects is `d[k] = v`; anything else
/// is an unknown call.
fn m_op_setitem(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    match &c.args[..] {
        [d, k, v] if !d.pts.is_empty() && d.kv.is_bot() && k.pts.is_empty() && d.pts.iter().all(|o| an.heap.kind(o) == Kind::Dict) => {
            an.store_item(d, Ns::Item, k, v, &c.node, st);
            an.none_v.clone()
        }
        _ => an.unknown_call(&c.qual, None, &c.args, &c.kwargs, &c.star_args, &c.star_kwargs, st, &c.node),
    }
}

/// `object.__setattr__(obj, "name", v)`: the plain store (no user `__setattr__`).
fn m_object_setattr(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    an.plain_setattr(|an| m_setattr(an, st, c));
    an.none_v.clone()
}

/// `object.__init__(self)` (`super().__init__()` in a class whose bases are the program's):
/// nothing; given more, as CPython decides by the class (unknown code).
fn m_object_init(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    if c.args.len() == 1 && c.kwargs.is_empty() && c.star_args.is_empty() && c.star_kwargs.is_empty() {
        return an.none_v.clone();
    }
    an.unknown_call(&c.qual, None, &c.args, &c.kwargs, &c.star_args, &c.star_kwargs, st, &c.node)
}

/// `object.__new__(cls)`: a fresh instance of each user class `cls` may be, without attributes
/// (a NamedTuple's: its fields from the other arguments, as `super().__new__(cls, …)` in a
/// subclass's `__new__` makes it); anything else (a class only a call site knows, an external
/// class) is unknown.
fn m_object_new(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(cv) = c.args.first().cloned() else { return Value::new() };
    let mut out = Value::new();
    let mut other = !cv.kv.is_bot();
    for o in cv.pts.iter() {
        if an.heap.kind(o) == Kind::Class && an.heap.class_info_opt(o).is_some() {
            let inst = an.alloc_instance(o, &c.node, st);
            if crate::models::dataclasses::is_named_tuple_class(an, o) {
                crate::models::dataclasses::dataclass_init(an, o, inst, &c.args[1..], &c.star_args, &c.kwargs, &c.star_kwargs, st, &c.node);
            }
            out = out.join(&Value::obj(inst));
        } else {
            other = true;
        }
    }
    if other {
        out = out.join(&an.unknown_call(&c.qual, None, &c.args, &c.kwargs, &c.star_args, &c.star_kwargs, st, &c.node));
    }
    out
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
/// `hasattr(o, name)` for a builtin dict / list / tuple / set: its type's methods are there;
/// what only records of user classes have (dataclass / pydantic / namedtuple API, `__dict__`)
/// is not; anything else is not decided here (Counter, defaultdict, OrderedDict add methods).
pub fn builtin_has_attr(an: &Analyzer, o: u32, name: &str) -> Option<bool> {
    const DICT_M: &[&str] = &["keys", "values", "items", "get", "pop", "popitem", "setdefault", "update", "clear", "copy", "fromkeys"];
    const LIST_M: &[&str] = &["append", "extend", "insert", "remove", "pop", "clear", "index", "count", "sort", "reverse", "copy"];
    const SET_M: &[&str] = &["add", "discard", "remove", "pop", "clear", "copy", "union", "intersection", "difference", "symmetric_difference", "issubset", "issuperset", "isdisjoint", "update"];
    const ABSENT: &[&str] = &["__dataclass_fields__", "__dataclass_params__", "__pydantic_fields__", "model_fields", "model_dump", "model_validate", "__fields__", "_fields", "_asdict", "_replace", "__dict__", "__slots__", "dict", "json"];
    let methods: &[&str] = match an.heap.kind(o) {
        Kind::Dict => DICT_M,
        Kind::List => LIST_M,
        Kind::Tuple => &["index", "count"],
        Kind::Set => SET_M,
        _ => return None,
    };
    if methods.contains(&name) {
        Some(true)
    } else if ABSENT.contains(&name) {
        Some(false)
    } else {
        None
    }
}

/// Run `o.__getattr__(name)` on a copy of the state under a try catching AttributeError:
/// Some(false) when it surely raises it, Some(true) when it surely returns, None otherwise (or
/// when it changes the heap).
fn getattr_hook_outcome(an: &mut Analyzer, st: &mut State, o: u32, k: KeyId, node: &crate::frontend::Node) -> Option<bool> {
    let mut s2 = st.copy();
    let mark = an.heap.len() as u32;
    let caught: rustc_hash::FxHashSet<String> = ["AttributeError".to_string()].into_iter().collect();
    an.enter_try(caught, &mut s2);
    an.call_method_with(&Value::obj(o), "__getattr__", vec![Value::lit(k)], node, &mut s2);
    let out = an.leave_try();
    let (flagged, raised) = (out.any(), out.raised());
    let recs = out.recs;
    if flagged {
        return None;
    }
    // (objects it made, the exception among them, do not count)
    let fp = s2.footprint(st);
    if fp.rows.iter().any(|(x, _)| *x < mark) || fp.slots.iter().any(|(x, _)| *x < mark) || fp.elems.iter().any(|x| *x < mark) {
        return None;
    }
    let all_attr = recs.iter().all(|(cls, _, _)| cls.as_ref().is_some_and(|(n, _)| n.iter().any(|x| x == "AttributeError")));
    match (s2.reachable, raised.is_some()) {
        (false, true) if all_attr && !recs.is_empty() => Some(false),
        (true, false) => Some(true),
        _ => None,
    }
}

/// The attributes of `str` (CPython 3.14 `dir(str)`).
const STR_ATTRS: &[&str] = &[
    "__add__", "__class__", "__contains__", "__delattr__", "__dir__", "__doc__", "__eq__", "__format__", "__ge__",
    "__getattribute__", "__getitem__", "__getnewargs__", "__getstate__", "__gt__", "__hash__", "__init__", "__init_subclass__",
    "__iter__", "__le__", "__len__", "__lt__", "__mod__", "__mul__", "__ne__", "__new__", "__reduce__", "__reduce_ex__",
    "__repr__", "__rmod__", "__rmul__", "__setattr__", "__sizeof__", "__str__", "__subclasshook__",
    "capitalize", "casefold", "center", "count", "encode", "endswith", "expandtabs", "find", "format", "format_map", "index",
    "isalnum", "isalpha", "isascii", "isdecimal", "isdigit", "isidentifier", "islower", "isnumeric", "isprintable", "isspace",
    "istitle", "isupper", "join", "ljust", "lower", "lstrip", "maketrans", "partition", "removeprefix", "removesuffix", "replace",
    "rfind", "rindex", "rjust", "rpartition", "rsplit", "rstrip", "split", "splitlines", "startswith", "strip", "swapcase", "title",
    "translate", "upper", "zfill",
];

fn m_hasattr(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let unknown = Value::data("hasattr");
    if c.args.len() < 2 {
        return unknown;
    }
    let Some(name) = c.args[1].kv.as_lit().filter(|_| c.args[1].pts.is_empty()).and_then(intern::key_as_str) else { return unknown };
    let k = key_str_lit(&name);
    let v0 = &c.args[0];
    // (a str literal: its type's attributes are known exactly — CPython 3.14's `dir(str)`)
    if v0.pts.is_empty() && v0.kv.is_literal() && crate::domain::kv_literals(&v0.kv).iter().all(intern::key_is_str) {
        return Value::lit(intern::key_bool(STR_ATTRS.contains(&&*name)));
    }
    // dunder attributes come from places not modelled (decorators, the type itself)
    if name.starts_with("__") && !(c.args[0].kv.is_bot() && !c.args[0].pts.is_empty() && c.args[0].pts.iter().all(|o| builtin_has_attr(an, o, &name) == Some(false))) {
        return unknown;
    }
    let objs: Vec<u32> = c.args[0].pts.iter().collect();
    if objs.is_empty() || !c.args[0].kv.is_bot() {
        return unknown;
    }
    let mut verdicts: Vec<bool> = Vec::new();
    for o in objs {
        if let Some(b) = builtin_has_attr(an, o, &name) {
            verdicts.push(b);
            continue;
        }
        if an.heap.kind(o) != Kind::Instance {
            return unknown;
        }
        let Some(cls) = an.heap.instance_class(o) else { return unknown };
        if !an.hierarchy_closed(cls) {
            return unknown;
        }
        if an.class_lookup(cls, "__getattr__").is_some() {
            // the normal lookup first; then hasattr is `__getattr__` under `except AttributeError`
            if an.class_lookup(cls, &name).is_some() || st.row(o, Ns::Attr).definite.contains(k) {
                verdicts.push(true);
                continue;
            }
            match getattr_hook_outcome(an, st, o, k, &c.node) {
                Some(b) => {
                    verdicts.push(b);
                    continue;
                }
                None => return unknown,
            }
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
/// `dataclasses.asdict(v)` at `node` (a dataclass inside a pydantic dump).
pub fn asdict_of(an: &mut Analyzer, st: &mut State, node: &crate::frontend::Node, v: &Value) -> Value {
    let c = Call { qual: "dataclasses.asdict".into(), recv: None, args: vec![v.clone()], kwargs: IndexMap::new(), star_args: vec![], star_kwargs: vec![], node: *node, call_ast: None, site: format!("L{}", node.line), line: node.line };
    m_asdict(an, st, &c)
}

fn m_asdict(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(a) = c.args.first().cloned() else { return Value::data("asdict") };
    fn conv(an: &mut Analyzer, st: &mut State, c: &Call, v: &Value, depth: u32) -> Value {
        let mut out = Value::with_kv(crate::domain::ObjSet::new(), v.kv.clone());
        for o in v.pts.iter() {
            let cls_of = match an.heap.kind(o) {
                Kind::Instance => an.heap.instance_class(o),
                _ => None,
            };
            let is_dc = cls_of.map(|cl| !crate::models::dataclasses::dataclass_fields_mro(an, cl).is_empty()).unwrap_or(false);
            if !is_dc || depth > 4 {
                out = out.join(&Value::obj(o));
                continue;
            }
            let cls = cls_of.unwrap();
            // one dict per converted instance (siblings at one depth must not share it)
            let d = an.alloc_at(Kind::Dict, &c.node, st, &format!("asdict@L{}", c.line), Payload::None, Some(Row::new()), Ns::Item, None, Some(&format!("asdict{}:{}", depth, o)));
            st.set_row(d, Ns::Item, Row::new());
            for (f, _) in crate::models::dataclasses::dataclass_fields_mro(an, cls) {
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
    if a.pts.iter().all(|o| an.heap.kind(o) == Kind::Instance && an.heap.instance_class(o).map(|cl| !crate::models::dataclasses::dataclass_fields_mro(an, cl).is_empty()).unwrap_or(false)) && !a.pts.is_empty() {
        return conv(an, st, c, &a, 0);
    }
    m_vars(an, st, c)
}

/// `model.model_dump()` / `model.dict()`: like `asdict` on the model.
fn m_dc_fields(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(a) = c.args.first() else { return Value::data("fields") };
    let cls = a.single().and_then(|o| match an.heap.kind(o) {
        Kind::Class => Some(o),
        Kind::Instance => an.heap.instance_class(o),
        _ => None,
    });
    let Some(cls) = cls else { return Value::data("fields") };
    let names: Vec<String> = crate::models::dataclasses::dataclass_fields_mro(an, cls).into_iter().map(|(f, _)| f).collect();
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
    if c.qual == "builtins.vars" {
    }
    let oid = an.alloc_simple(Kind::Dict, &c.node, st, &format!("vars@L{}", c.line));
    if c.qual == "builtins.vars" {
        if let Some(inst) = a.single().filter(|o| an.heap.kind(*o) == Kind::Instance) {
            an.m.attr_mirrors.insert(oid, inst);
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
    an.heap.set_llen(oid, st, None);
    if c.args.len() >= 2 {
        let it = an.iterate(&c.args[1], &c.node, st);
        if c.qual.ends_with("filter") {
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
                if all_decided && an.heap.obj(oid).single {
                    an.heap.set_llen(oid, st, Some(kept.len()));
                    for (i, v) in kept.into_iter().enumerate() {
                        st.set_slot(oid, Ns::Pos, intern::key_int(i as i64), v, true);
                    }
                }
                return Value::obj(oid);
            }
        }
        // `map(f, a, b, …)`: f gets one item of each (as many as the shortest has)
        let others: Vec<crate::interp::funcs::Iteration> = c.args.iter().skip(2).map(|a| an.iterate(a, &c.node, st)).collect();
        if !c.qual.ends_with("filter") && an.heap.obj(oid).single {
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
                    an.heap.set_llen(oid, st, Some(n.min(items.len())));
                    return Value::obj(oid);
                }
            }
        }
        let elem = an.elem_value(&it.elem, &c.node, "el");
        let res = if c.qual.ends_with("filter") {
            // the predicate still runs on the items (its reads are the program's)
            if !c.args[0].pts.is_empty() {
                an.call_value(&c.args[0], &c.node, None, st, Some(vec![elem.clone()]));
            }
            elem
        } else {
            let mut call_args = vec![elem];
            call_args.extend(others.iter().map(|o| an.elem_value(&o.elem, &c.node, "el")));
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
    let elem = an.elem_value(&it.elem, &c.node, "el");
    let start = init.unwrap_or_else(|| elem.clone());
    let r = an.call_value(&f, &c.node, None, st, Some(vec![start.clone(), elem]));
    r.join(&start)
}

/// `type(x)` with one argument: the class of an instance of a known class (else unknown).
fn m_type(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    // `type(name, bases, {...})`: a class whose attributes are the namespace's items, made
    // by a synthetic module per call site and choice of bases (`class N(__b0, __b1): pass`
    // with `__bi` bound to the bases); a base that is one of several classes makes one class
    // per choice
    if c.args.len() == 3 {
        let lit_name = c.args[0].kv.as_lit().filter(|_| c.args[0].pts.is_empty()).and_then(intern::key_as_str).filter(|n| !n.is_empty() && n.chars().all(|ch| ch.is_alphanumeric() || ch == '_'));
        let bases: Option<Vec<Vec<ObjId>>> = c.args[1].single().filter(|o| an.heap.kind(*o) == Kind::Tuple && c.args[1].kv.is_bot()).and_then(|t| {
            let n = an.heap.llen(t, st)?;
            (0..n)
                .map(|i| {
                    let b = st.slot(t, Ns::Pos, intern::key_int(i as i64));
                    (b.kv.is_bot() && !b.pts.is_empty() && b.pts.iter().all(|o| matches!(an.heap.kind(o), Kind::Class | Kind::External))).then(|| b.pts.to_vec())
                })
                .collect()
        });
        let ns = c.args[2].single().filter(|o| an.heap.kind(*o) == Kind::Dict).map(|o| (o, st.row(o, Ns::Item)));
        let mut choices: Vec<Vec<ObjId>> = vec![Vec::new()];
        for alts in bases.iter().flatten() {
            choices = choices.iter().flat_map(|ch| alts.iter().map(move |b| ch.iter().copied().chain([*b]).collect::<Vec<ObjId>>())).collect();
        }
        if let (Some(_), Some((ns, row))) = (bases.as_ref().filter(|_| choices.len() <= 32), ns.filter(|(_, r)| r.closed() && r.present == r.definite)) {
            let name: String = lit_name.as_deref().unwrap_or("type").to_string();
            let mut out = Value::new();
            for ch in choices {
                let names: Vec<String> = (0..ch.len()).map(|i| format!("__b{}", i)).collect();
                let modname = format!("<type {} {:?}>", crate::interp::site_string(&c.node), ch);
                let moid = match an.module(&modname) {
                    Some(m) => m,
                    None => {
                        let Ok((idx, tree)) = crate::frontend::parse(&modname, None, format!("class {}({}):\n    pass\n", name, names.join(", "))) else { return Value::data("type") };
                        an.make_module(&modname, None, idx, tree)
                    }
                };
                // (made in another state, or not yet: run it in this one)
                if st.slot_opt(moid, Ns::Attr, key_str_lit(&name)).is_none() {
                    for (n, b) in names.iter().zip(&ch) {
                        st.set_slot(moid, Ns::Attr, key_str_lit(n), Value::obj(*b), true);
                    }
                    an.run_module(moid, st);
                }
                let cv = st.slot(moid, Ns::Attr, key_str_lit(&name));
                if let Some(cls) = cv.single() {
                    for k in row.present.iter().filter(|k| intern::key_as_str(*k).is_some()) {
                        an.heap.cls_slots.insert((cls, Ns::Attr, k), st.slot(ns, Ns::Item, k));
                    }
                    if lit_name.is_none() {
                        for k in ["__name__", "__qualname__"] {
                            an.heap.cls_slots.insert((cls, Ns::Attr, key_str_lit(k)), c.args[0].clone());
                        }
                    }
                }
                out = out.join(&cv);
            }
            return out;
        }
        return an.unknown_call(&c.qual, None, &c.args, &c.kwargs, &c.star_args, &c.star_kwargs, st, &c.node);
    }
    if c.args.len() == 1 {
        let a = &c.args[0];
        let classes: Option<Vec<u32>> = if a.pts.is_empty() { None } else { a.pts.iter().map(|o| if an.heap.kind(o) == Kind::Instance { an.heap.instance_class(o) } else { None }).collect() };
        if let Some(cs) = classes {
            return join_values(cs.into_iter().map(Value::obj).collect::<Vec<Value>>());
        }
        // builtin values: their builtin class (the object the builtin name is)
        let builtin = |an: &mut Analyzer, n: &str| -> Value { Value::obj(an.heap.alloc_raw(Kind::External, &format!("ext:builtins.{}", n), "", true, n, Payload::Str(format!("builtins.{}", n)))) };
        let mut outs: Vec<Value> = Vec::new();
        let mut known = true;
        for o in a.pts.iter() {
            // a builtin exception object: its class
            if let Some(en) = an.m.exc_objs.get(&o).cloned().filter(|_| an.heap.kind(o) == Kind::Namespace) {
                outs.push(builtin(an, &en));
                continue;
            }
            let n = match an.heap.kind(o) {
                // (a torch container — ModuleList, Sequential, ModuleDict — is not a list or dict)
                _ if crate::models::torch::t_layer_type(an, o).is_some() => {
                    known = false;
                    continue;
                }
                Kind::Dict if !crate::models::pandas::is_series_obj(an, o) => "dict",
                Kind::List => "list",
                Kind::Tuple => "tuple",
                Kind::Set => "set",
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
            _ => known = false,
        }
        if known && !outs.is_empty() {
            return join_values(outs);
        }
    }
    Value::data(&format!("type@L{}", c.line))
}

/// The builtin type a library function's result has, whatever its arguments (`len`: an int;
/// `abs` of an int: an int), when it returns: None when that depends on more than this knows.
pub fn result_type(an: &Analyzer, qual: &str, args: &[Value]) -> Option<&'static str> {
    let arg_type = || args.first().and_then(|a| an.value_type(a));
    match qual.strip_prefix("builtins.").unwrap_or(qual) {
        "len" | "ord" | "hash" | "id" | "int" => Some("int"),
        "str" | "repr" | "ascii" | "chr" | "format" | "json.dumps" => Some("str"),
        "float" => Some("float"),
        "bool" | "isinstance" | "issubclass" | "callable" | "hasattr" => Some("bool"),
        "abs" => match arg_type()? {
            "bool" | "int" => Some("int"),
            "float" => Some("float"),
            _ => None,
        },
        // (`round(x)` of a number: an int; `round(x, n)`: of x's type)
        "round" if matches!(arg_type()?, "int" | "bool" | "float") => Some(if args.len() == 1 || arg_type() != Some("float") { "int" } else { "float" }),
        _ => None,
    }
}

/// The builtin type of what a method of a str (`ty`: or bytes) returns (`count`: an int).
pub fn str_method_type(qual: &str, ty: &'static str) -> Option<&'static str> {
    let name = last(qual);
    if matches!(name, "count" | "find" | "rfind" | "index" | "rindex") {
        return Some("int");
    }
    if name == "startswith" || name == "endswith" || name.starts_with("is") {
        return Some("bool");
    }
    const SAME: &[&str] = &["upper", "lower", "strip", "lstrip", "rstrip", "title", "capitalize", "casefold", "swapcase", "removeprefix", "removesuffix", "zfill", "center", "ljust", "rjust", "replace", "expandtabs", "join", "format", "format_map", "translate"];
    if SAME.contains(&name) {
        return Some(if matches!(name, "format" | "format_map" | "casefold") { "str" } else { ty });
    }
    match (name, ty) {
        ("encode", "str") => Some("bytes"),
        ("decode", "bytes") => Some("str"),
        _ => None,
    }
}

fn m_str(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    // `str(b, encoding[, errors])` decodes (known bytes: models::pytext)
    if c.args.len() > 1 || c.kwargs.keys().any(|k| matches!(&*intern::str_get(*k), "encoding" | "errors")) {
        return Value::data("str");
    }
    if let Some(a) = c.args.first() {
        if let Some(t) = crate::models::enums::enum_text(an, a, st) {
            return t;
        }
        if let Some(t) = crate::models::exceptions::exc_text(an, a, false, st) {
            return t;
        }
        let mut out = if a.kv.is_bot() { Value::new() } else { Value::kv_only(an.rename_kv(&a.kv, &RenameFn::new(FnKind::Str))) };
        // (a text not known: a string all the same, its str methods apply)
        if matches!(out.kv, KeyVal::Data(_)) {
            out = Value::data("str");
        }
        // the objects' part (an unknown value, an instance's __str__, …): some string
        if !a.pts.is_empty() {
            out = out.join(&Value::data("str"));
        }
        return out;
    }
    Value::lit_str("")
}

fn m_repr(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    if let Some(t) = c.args.first().and_then(|a| crate::models::exceptions::exc_text(an, a, true, st)) {
        return t;
    }
    if let Some(r) = c.args.first().and_then(|a| a.single()).and_then(|o| pathtext::path_repr(an, o)) {
        return Value::lit_str(&r);
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
            return crate::models::torch::dim_value(crate::models::shape::norm(crate::models::shape::floordiv(d, crate::models::shape::lit(k)), &st.tf));
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
    an.may_raise_here("ValueError", st);
    Value::data(&format!("L{}", c.line))
}

fn m_super(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(foid) = an.frame().func else { return Value::data("super") };
    let fi = an.heap.func_info(foid);
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
    let oid = an.heap.alloc_raw(Kind::Super, &format!("super:{}:{}", foid, c.site), &ctx, true, "super()", Payload::Super(cls, self_v.clone()));
    an.heap.obj_mut(oid).payload = Payload::Super(cls, self_v);
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
        } else if !c.star_kwargs.is_empty() {
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
        let piece = if spec.is_empty() { Some(intern::key_py_str(k)) } else { crate::interp::ops::format_spec_literal(k, &spec) };
        push(&mut out, piece);
    }
    out
}

/// A str method; one returning a string, on a string whose text is not known, gives a
/// string whose text is not known (its str methods apply in turn: `v.strip().title()`).
fn m_str_method(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let out = m_str_method_inner(an, st, c);
    // (`format` / `format_map` report their own raises: their field reads do; a result not
    // computed here may raise anywhere)
    if matches!(last(&c.qual), "format" | "format_map") && !(out.pts.is_empty() && out.kv.is_literal()) {
        an.flag_may_raise(st);
    }
    const STR_OUT: &[&str] = &["upper", "lower", "strip", "lstrip", "rstrip", "title", "capitalize", "casefold", "swapcase", "removeprefix", "removesuffix", "zfill", "center", "ljust", "rjust", "replace", "expandtabs", "join", "format"];
    if !STR_OUT.contains(&last(&c.qual)) || !out.pts.is_empty() {
        return out;
    }
    match &out.kv {
        KeyVal::Data(_) | KeyVal::Bot => Value::data("str"),
        _ => out,
    }
}

fn m_str_method_inner(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = last(&c.qual);
    let kv = c.recv.as_ref().map(|r| r.kv.clone()).unwrap_or(DATA);
    match name {
        // (`strip(chars)`, `lstrip()`, `rstrip()` are not `strip()`: a str not known)
        "upper" | "lower" | "title" | "strip" if c.args.is_empty() && c.kwargs.is_empty() => Value::kv_only(an.rename_kv(&kv, &RenameFn::new(FnKind::from_name(name).unwrap()))),
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
            Value::data("str")
        }
        "startswith" | "endswith" if c.args.len() == 1 && kv.as_lit().and_then(intern::key_as_str).is_some() && c.args[0].kv.as_lit().and_then(intern::key_as_str).is_some() => {
            let s = intern::key_as_str(kv.as_lit().unwrap()).unwrap();
            let p = intern::key_as_str(c.args[0].kv.as_lit().unwrap()).unwrap();
            Value::lit(intern::key_bool(if name == "startswith" { s.starts_with(&*p) } else { s.ends_with(&*p) }))
        }
        "startswith" | "endswith" if c.args.len() == 1 => {
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
            an.heap.set_llen(oid, st, Some(3));
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
            // a template from a parameter: decided per call site (with its fields)
            if let Some(t) = kv.as_lit().and_then(intern::key_as_str) {
                if let Some(out) = format_fields(an, st, &t, c, mapping.as_ref()) {
                    return Value::lit(key_str_lit(&out));
                }
                // fields from parameters: decided per call site
            }
            if name == "format_map" {
                return Value::data(&format!("L{}", c.line));
            }
            if let Some(k) = kv.as_lit() {
                if let Some(sk) = intern::key_as_str(k) {
                    if sk.matches("{}").count() == 1 && c.args.len() == 1 && c.args[0].pts.is_empty() {
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
                if let Some(items) = it.ordered_items() {
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
            }
            Value::data(&format!("L{}", c.line))
        }
        "split" | "splitlines" | "partition" | "rsplit" => {
            let oid = an.alloc_simple(Kind::List, &c.node, st, &format!("split@L{}", c.line));
            an.heap.set_llen(oid, st, None);
            // a literal string split by a literal separator: the parts, in order; a parameter
            // string is decided per call site
            let sep: Option<Option<String>> = match c.args.first() {
                None => Some(None),
                Some(a) => a.kv.as_lit().and_then(intern::key_as_str).map(|s| Some(s.to_string())),
            };
            if let (Some(k), Some(sep), true) = (kv.as_lit(), sep, name == "split" && an.heap.obj(oid).single) {
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
                        an.heap.set_llen(oid, st, Some(parts.len()));
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
        Some(r) => r.pts.iter().filter(|o| an.heap.kind(*o).is_record()).collect(),
        None => Vec::new(),
    }
}

fn m_get(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(key) = c.args.first() else { return Value::data("get") };
    an.check_hashable(key, st);
    // (a key that may be an object: an unknown key)
    let okey = an.opaque_key(key, st);
    let key = &okey;
    let default = c.args.get(1).cloned().unwrap_or_else(|| an.none_v.clone());
    let recv = c.recv.clone().unwrap_or_default();
    if let Some(k) = key.kv.as_lit() {
        let recs = an.record_objects(&recv);
        if !recs.is_empty() && recs.len() == recv.pts.len() && recs.iter().all(|o| an.heap.kind(*o) != Kind::Frame) {
            // absent from every closed target: the default alone, no load
            if recs.iter().all(|o| { let r = st.row(*o, Ns::Item); r.lacks(k) }) {
                return default;
            }
        }
    }
    let out = an.load_item(&recv, Ns::Item, key, &c.node, st, false);
    // the default is only possible when the key may be absent on some target
    if let Some(k) = key.kv.as_lit() {
        let recs = an.record_objects(&recv);
        if !recs.is_empty() && recs.len() == recv.pts.len() && recs.iter().all(|o| an.heap.kind(*o) != Kind::Frame && st.row(*o, Ns::Item).definite.contains(k)) {
            return out;
        }
    }
    out.join(&default)
}

fn m_setdefault(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(key) = c.args.first() else { return Value::data("setdefault") };
    an.check_hashable(key, st);
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
            if rows.iter().all(|r| r.lacks(k)) {
                // absent everywhere: the default is stored and returned
                an.store_item(&recv, Ns::Item, key, &default, &c.node, st);
                return default;
            }
        }
    }
    let cur = an.load_item(&recv, Ns::Item, key, &c.node, st, false);
    let nv = if !cur.empty() { cur.join(&default) } else { default };
    an.store_item(&recv, Ns::Item, key, &nv, &c.node, st);
    an.load_item(&recv, Ns::Item, key, &c.node, st, false)
}

/// `d.update(obj)` / `dict(obj)` with an instance of a user class that is no dict: an object
/// with `keys()` is read as `obj[k]` for each key it lists; otherwise it is an iterable of
/// pairs (unknown here: the row opens). Returns false when `src` holds no such instance.
fn update_from_instances(an: &mut Analyzer, st: &mut State, oid: u32, src: &Value, strong: bool, c: &Call) -> bool {
    let src = &an.unkey(src);
    if src.pts.is_empty() {
        return false;
    }
    let insts: Vec<u32> = src.pts.iter().filter(|o| an.heap.kind(*o) == Kind::Instance && an.heap.instance_class(*o).is_some_and(|cl| !an.heap.class_info_opt(cl).map(|ci| ci.lib.dict_base || ci.lib.typed_dict || ci.lib.pydantic).unwrap_or(true))).collect();
    if insts.is_empty() || insts.len() != src.pts.len() {
        return false;
    }
    let strong = strong && insts.len() == 1;
    for inst in insts {
        let cls = an.heap.instance_class(inst).unwrap();
        if an.class_lookup(cls, "keys").is_some() {
            let ks = an.call_method_with(&Value::obj(inst), "keys", vec![], &c.node, st).unwrap_or_else(|| Value::data("keys"));
            let it = an.iterate(&ks, &c.node, st);
            match it.items {
                Some(items) => {
                    for k in items {
                        let v = an.call_method_with(&Value::obj(inst), "__getitem__", vec![k.clone()], &c.node, st).unwrap_or_else(|| Value::data("getitem"));
                        an.store_key(oid, Ns::Item, &k.kv, &v, strong, &c.node, st);
                    }
                }
                None => {
                    let k = it.elem.flat();
                    let v = an.call_method_with(&Value::obj(inst), "__getitem__", vec![k.clone()], &c.node, st).unwrap_or_else(|| Value::data("getitem"));
                    an.store_key(oid, Ns::Item, &KeyVal::Data(intern::s("mapkeys")), &v, false, &c.node, st);
                }
            }
        } else {
            let r = st.row(oid, Ns::Item).data_store(Source::new(&format!("update@L{}", c.line), SourceKind::Data));
            st.set_row(oid, Ns::Item, r);
        }
    }
    true
}

fn m_update(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    // a parameter's mapping (or keywords): decided per call site
    let recs = dict_recv(an, c);
    let strong = recs.len() == 1 && an.heap.obj(recs[0]).single;
    for oid in recs {
        if let Some(src) = c.args.first() {
            if update_from_instances(an, st, oid, src, strong, c) {
            } else if !an.record_objects(src).is_empty() {
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
    let ll = an.heap.llen(oid, st);
    let single = an.heap.obj(oid).single;
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
                an.heap.set_llen(oid, st, Some(n - 1));
                return v;
            }
        }
    }
    an.heap.set_llen(oid, st, None);
    an.elem_of(&Value::obj(oid), &c.node, st)
}

fn m_pop(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    st.drop_name_keys(None);
    let recv = c.recv.clone().unwrap_or_default();
    let int_key = c.args.first().and_then(|a| a.kv.as_lit()).map(|k| intern::key_as_int(k).is_some()).unwrap_or(false);
    let listy = recv.pts.iter().all(|o| matches!(an.heap.kind(o), Kind::List | Kind::Tuple));
    if !recv.pts.is_empty() && listy && (c.args.is_empty() || int_key) {
        // list.pop through a placeholder (or with no key)
        return m_lpop(an, st, c);
    }
    let Some(key) = c.args.first() else { return Value::data("pop") };
    if recv.pts.iter().any(|o| an.hashes_keys(o)) {
        an.check_hashable(key, st);
    }
    let has_default = c.args.len() > 1;
    if let Some(k) = key.kv.as_lit() {
        let recs = an.record_objects(&recv);
        if !recs.is_empty() && recs.len() == recv.pts.len() && recs.iter().all(|o| an.heap.kind(*o) != Kind::Frame) {
            let rows: Vec<Row> = recs.iter().map(|o| st.row(*o, Ns::Item)).collect();
            if has_default && rows.iter().all(|r| r.lacks(k)) {
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
    // (pop reports its own raises: without a default a key that may be missing is a KeyError)
    if !has_default {
        an.may_raise_here("KeyError", st);
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
        if an.heap.obj(oid).single && row.closed() && row.present == row.definite {
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
        let row0 = recs.first().map(|o| st.row(*o, Ns::Item));
        an.load_item(&recv, Ns::Item, kv, &c.node, st, true);
        if let (Some(k), Some(r0)) = (kv.kv.as_lit().filter(|_| kv.pts.is_empty()), row0) {
            if recs.len() == 1 && r0.closed() && !r0.present.contains(k) {
                an.raise_here("KeyError", st);
                return Value::new();
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
        let strong = an.heap.obj(oid).single && recs.len() == 1;
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
    let oid = an.heap.alloc_raw(Kind::View, &format!("view:{}:{}", kind.name(), ids.join(",")), "", true, &format!("{} view", kind.name()), Payload::View(kind, recv.clone()));
    an.heap.obj_mut(oid).payload = Payload::View(kind, recv);
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
/// `lst.append(x)` / `lst.extend(known items)` on a list whose length is one of a few: the
/// alternatives shifted by the number added (a length past 15 voids them all).
fn m_append(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let added: Option<usize> = match last(&c.qual) {
        "append" => Some(1),
        "extend" => c.args.first().and_then(|a| a.single()).filter(|o| matches!(an.heap.kind(*o), Kind::List | Kind::Tuple)).and_then(|o| an.heap.llen(o, st)),
        _ => None,
    };
    let before: Vec<(crate::domain::ObjId, u16)> = match (&c.recv, added) {
        (Some(r), Some(_)) => r.pts.iter().filter_map(|o| st.lens.get(&o).filter(|l| l.0.is_none() && l.2 != 0).map(|l| (o, l.2))).collect(),
        _ => Vec::new(),
    };
    // (a length that is a dim: that many more)
    let sym = c.recv.as_ref().and_then(|r| r.single()).and_then(|o| Some((o, st.sym_len(o)?))).filter(|_| added.is_some());
    let r = m_append_inner(an, st, c);
    if let (Some((o, l)), Some(k)) = (sym, added) {
        if let Some(d) = crate::models::shape::parse_dim_label(&intern::str_get(l)) {
            st.set_sym_len(o, intern::s(&crate::models::shape::dim_label(crate::models::shape::add(d, crate::models::shape::lit(k as i64)))));
        }
    }
    // (one of several lists: each may also not have grown)
    let alone = c.recv.as_ref().is_some_and(|r| r.pts.len() == 1 && r.kv.is_bot());
    if let Some(k) = added {
        for (o, m) in before {
            if let Some((None, p, 0)) = st.lens.get(&o) {
                let shifted = ((m as u32) << k) | if alone { 0 } else { m as u32 };
                if shifted <= 0xFFFF {
                    st.set_lens(o, (None, p, shifted as u16));
                }
            }
        }
    }
    r
}

fn m_append_inner(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = last(&c.qual);
    let recv = c.recv.clone().unwrap_or_default();
    // (a receiver that is one of several lists: each may or may not be the one that grows)
    let alone = recv.pts.len() == 1 && recv.kv.is_bot();
    for oid in recv.pts.iter() {
        if !matches!(an.heap.kind(oid), Kind::List | Kind::Set | Kind::Tuple) {
            continue;
        }
        let mut ext_items: Option<Vec<Value>> = None;
        // (the lengths it may have, before a store of an element voids them)
        let alts = an.heap.len_alts(oid, st);
        let elem = if (name == "extend" || name == "update") && !c.args.is_empty() {
            let it = an.iterate(&c.args[0], &c.node, st);
            if name == "extend" {
                ext_items = it.items.clone();
            }
            an.elem_value(&it.elem, &c.node, "el")
        } else {
            c.args.last().cloned().unwrap_or_default()
        };
        // extend by known items in straight-line code: each item gets its position
        if let (Some(items), Kind::List) = (&ext_items, an.heap.kind(oid)) {
            let ll = an.heap.llen(oid, st);
            if let Some(n) = ll {
                let cap = if an.m.torch_on { 64 } else { 16 };
                if alone && an.heap.obj(oid).single && an.frame().loop_depth == 0 && n + items.len() <= cap {
                    for (i, item) in items.iter().enumerate() {
                        st.set_slot(oid, Ns::Pos, intern::key_int((n + i) as i64), item.clone(), true);
                        an.add_elem(oid, item, st);
                    }
                    an.heap.set_llen(oid, st, Some(n + items.len()));
                    continue;
                }
            }
        }
        an.add_elem(oid, &elem, st);
        let ll = an.heap.llen(oid, st);
        let single = an.heap.obj(oid).single;
        if let Some(n) = ll {
            if alone && single && name == "append" && an.frame().loop_depth == 0 && n < if an.m.torch_on { 64 } else { 16 } {
                st.set_slot(oid, Ns::Pos, intern::key_int(n as i64), elem, true);
                an.heap.set_llen(oid, st, Some(n + 1));
                continue;
            }
            // one of several lists: this one as it was, or longer by the items
            let items = if name == "append" { Some(vec![elem.clone()]) } else { ext_items.clone() };
            if let Some(items) = items.filter(|its| !alone && an.frame().loop_depth == 0 && n + its.len() < 16) {
                for (i, item) in items.iter().enumerate() {
                    st.set_slot(oid, Ns::Pos, intern::key_int((n + i) as i64), item.clone(), false);
                }
                st.set_lens(oid, (None, n, (1u16 << n) | (1u16 << (n + items.len()))));
                continue;
            }
            // appends of unknown count: the positions so far stay valid
            if name == "append" || name == "extend" {
                an.heap.set_prefix(oid, st, n);
                continue;
            }
            st.drop_obj_ns(oid, Ns::Pos);
        } else if name == "append" || name == "extend" {
            // the length was unknown already: the known prefix stays; one of a few lengths: the
            // items at each one's end (as each path has them)
            if let Some(alts) = alts {
                match if name == "append" { Some(vec![elem.clone()]) } else { ext_items.clone() } {
                    Some(items) => {
                        for n in alts {
                            for (i, item) in items.iter().enumerate() {
                                st.set_slot(oid, Ns::Pos, intern::key_int((n + i) as i64), item.clone(), false);
                            }
                        }
                    }
                    None => an.heap.set_prefix(oid, st, alts[0]),
                }
            }
            continue;
        }
        an.heap.set_llen(oid, st, None);
    }
    // the last element after an append (whatever the length)
    if name == "append" {
        if let (Some(oid), Some(x)) = (recv.single(), c.args.last()) {
            if an.heap.kind(oid) == Kind::List && an.heap.obj(oid).single {
                st.set_last(oid, x.clone());
            }
        }
    }
    an.none_v.clone()
}

/// `slice(stop)` / `slice(start, stop[, step])`: the label a `a:b:c` subscript makes (known
/// int / None bounds), else an unknown slice.
fn m_slice(_an: &mut Analyzer, _st: &mut State, c: &Call) -> Value {
    let none = intern::key_none();
    // a parameter bound: known at the call site
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
                let e = an.elem_value(&it.elem, &c.node, "el");
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
            if matches!(name.as_str(), "add" | "discard" | "remove") {
                if let Some(a) = c.args.first() {
                    an.check_hashable(a, st);
                }
            }
            // (a tuple of known values: its key)
            let item = c.args.first().and_then(|a| match a.single() {
                Some(o) if a.kv.is_bot() && an.heap.kind(o) == Kind::Tuple => an.value_key(o, st, 0),
                _ => a.kv.as_lit().filter(|_| a.pts.is_empty()),
            });
            for oid in recv.pts.iter().filter(|o| an.heap.kind(*o) == Kind::Set).collect::<Vec<_>>() {
                let strong = recv.pts.len() == 1 && an.heap.obj(oid).single;
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
                                let e = if name == "add" { a.clone() } else { { let it_ = an.iterate(a, &c.node, st); an.elem_value(&it_.elem, &c.node, "el") } };
                                kv = kv_join(&kv, &e.kv);
                                let cur = st.slot(oid, Ns::Elem, STAR);
                                st.set_slot(oid, Ns::Elem, STAR, cur.join(&Value::with_kv(e.pts.clone(), crate::models::torch::elem_kv(&e.kv))), true);
                            }
                        }
                        st.elems.insert(oid, kv);
                        an.heap.set_llen(oid, st, None);
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
    let maps = an.m.chainmaps.get(&oid).cloned().unwrap_or_default();
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
        _ => match qual.strip_prefix("urllib.parse.") {
            Some(n) if URLLIB_NAMES.contains(&n) => ("<urllib.parse>", n, URLLIB_SRC),
            _ => return None,
        },
    };
    let moid = match an.module(&modname) {
        Some(m) => m,
        None => {
            let (idx, tree) = crate::frontend::parse(modname, None, src.to_string()).ok()?;
            let m = an.make_module(modname, None, idx, tree);
            an.run_module(m, st);
            m
        }
    };
    if st.slot_opt(moid, Ns::Attr, key_str_lit(cname)).is_none() {
        an.run_module(moid, st);
    }
    let v = st.slot(moid, Ns::Attr, key_str_lit(cname));
    if cname == "UserDict" {
        if let Some(c) = v.single() {
            an.m.userdict_classes.insert(c);
        }
    }
    Some(v)
}

/// `urllib.parse` functions analysed as Python (CPython's, for str inputs; anything else is an
/// unknown call). Quoting is `models::pytext` (through `_up`).
const URLLIB_NAMES: &[&str] = &["urlparse", "urlsplit", "urlunparse", "urlunsplit", "urljoin", "urldefrag", "parse_qs", "parse_qsl", "urlencode", "ParseResult", "SplitResult", "DefragResult"];

const URLLIB_SRC: &str = r#"
import urllib.parse as _up
import _rowan_opaque
from typing import NamedTuple

uses_relative = ['', 'ftp', 'http', 'gopher', 'nntp', 'imap', 'wais', 'file', 'https', 'shttp', 'mms', 'prospero', 'rtsp', 'rtsps', 'rtspu', 'sftp', 'svn', 'svn+ssh', 'ws', 'wss']
uses_netloc = ['', 'ftp', 'http', 'gopher', 'nntp', 'telnet', 'imap', 'wais', 'file', 'mms', 'https', 'shttp', 'snews', 'prospero', 'rtsp', 'rtsps', 'rtspu', 'rsync', 'svn', 'svn+ssh', 'sftp', 'nfs', 'git', 'git+ssh', 'ws', 'wss', 'itms-services']
uses_params = ['', 'ftp', 'hdl', 'prospero', 'http', 'imap', 'https', 'shttp', 'rtsp', 'rtsps', 'rtspu', 'sip', 'sips', 'mms', 'sftp', 'tel']
scheme_chars = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789+-.'
_WS = '\x00\x01\x02\x03\x04\x05\x06\x07\x08\t\n\x0b\x0c\r\x0e\x0f\x10\x11\x12\x13\x14\x15\x16\x17\x18\x19\x1a\x1b\x1c\x1d\x1e\x1f '


def _userinfo(netloc):
    userinfo, have_info, hostinfo = netloc.rpartition('@')
    if have_info:
        username, have_password, password = userinfo.partition(':')
        if not have_password:
            password = None
    else:
        username = password = None
    return username, password


def _hostinfo(netloc):
    _, _, hostinfo = netloc.rpartition('@')
    _, have_open_br, bracketed = hostinfo.partition('[')
    if have_open_br:
        hostname, _, port = bracketed.partition(']')
        _, _, port = port.partition(':')
    else:
        hostname, _, port = hostinfo.partition(':')
    if not port:
        port = None
    return hostname, port


def _hostname(netloc):
    hostname = _hostinfo(netloc)[0]
    if not hostname:
        return None
    hostname, percent, zone = hostname.partition('%')
    return hostname.lower() + percent + zone


def _port(netloc):
    port = _hostinfo(netloc)[1]
    if port is not None:
        if port.isdigit() and port.isascii():
            port = int(port)
        else:
            raise ValueError("Port could not be cast to integer value")
        if not (0 <= port <= 65535):
            raise ValueError("Port out of range 0-65535")
    return port


class SplitResult(NamedTuple):
    scheme: str
    netloc: str
    path: str
    query: str
    fragment: str

    @property
    def username(self):
        return _userinfo(self.netloc)[0]

    @property
    def password(self):
        return _userinfo(self.netloc)[1]

    @property
    def hostname(self):
        return _hostname(self.netloc)

    @property
    def port(self):
        return _port(self.netloc)

    def geturl(self):
        return urlunsplit(self)


class ParseResult(NamedTuple):
    scheme: str
    netloc: str
    path: str
    params: str
    query: str
    fragment: str

    @property
    def username(self):
        return _userinfo(self.netloc)[0]

    @property
    def password(self):
        return _userinfo(self.netloc)[1]

    @property
    def hostname(self):
        return _hostname(self.netloc)

    @property
    def port(self):
        return _port(self.netloc)

    def geturl(self):
        return urlunparse(self)


class DefragResult(NamedTuple):
    url: str
    fragment: str

    def geturl(self):
        if self.fragment:
            return self.url + '#' + self.fragment
        return self.url


def _splitnetloc(url, start=0):
    delim = len(url)
    for c in '/?#':
        wdelim = url.find(c, start)
        if wdelim >= 0:
            delim = min(delim, wdelim)
    return url[start:delim], url[delim:]


def _urlsplit(url, scheme=None, allow_fragments=True):
    url = url.lstrip(_WS)
    url = url.replace('\t', '').replace('\r', '').replace('\n', '')
    if scheme is not None:
        scheme = scheme.strip(_WS)
        scheme = scheme.replace('\t', '').replace('\r', '').replace('\n', '')
    netloc = query = fragment = None
    i = url.find(':')
    if i > 0 and url[0].isascii() and url[0].isalpha():
        ok = True
        for c in url[:i]:
            if c not in scheme_chars:
                ok = False
                break
        if ok:
            scheme, url = url[:i].lower(), url[i+1:]
    if url[:2] == '//':
        netloc, url = _splitnetloc(url, 2)
        if '[' in netloc or ']' in netloc:
            return _rowan_opaque.urlsplit(url)
        if not netloc.isascii():
            return _rowan_opaque.urlsplit(url)
    if allow_fragments and '#' in url:
        url, fragment = url.split('#', 1)
    if '?' in url:
        url, query = url.split('?', 1)
    return (scheme, netloc, url, query, fragment)


def _splitparams(url):
    if '/' in url:
        i = url.find(';', url.rfind('/'))
        if i < 0:
            return url, None
    else:
        i = url.find(';')
    return url[:i], url[i+1:]


def urlsplit(url, scheme='', allow_fragments=True):
    if not isinstance(url, str) or not isinstance(scheme, str):
        return _rowan_opaque.urlsplit(url, scheme)
    scheme, netloc, url, query, fragment = _urlsplit(url, scheme, allow_fragments)
    return SplitResult(scheme or '', netloc or '', url, query or '', fragment or '')


def urlparse(url, scheme='', allow_fragments=True):
    if not isinstance(url, str) or not isinstance(scheme, str):
        return _rowan_opaque.urlparse(url, scheme)
    scheme, netloc, url, query, fragment = _urlsplit(url, scheme, allow_fragments)
    if (scheme or '') in uses_params and ';' in url:
        url, params = _splitparams(url)
    else:
        params = None
    return ParseResult(scheme or '', netloc or '', url, params or '', query or '', fragment or '')


def _urlunsplit(scheme, netloc, url, query, fragment):
    if netloc is not None:
        if url and url[:1] != '/':
            url = '/' + url
        url = '//' + netloc + url
    elif url[:2] == '//':
        url = '//' + url
    if scheme:
        url = scheme + ':' + url
    if query is not None:
        url = url + '?' + query
    if fragment is not None:
        url = url + '#' + fragment
    return url


def urlunsplit(components):
    scheme, netloc, url, query, fragment = components
    if not netloc:
        if scheme and scheme in uses_netloc and (not url or url[:1] == '/'):
            netloc = ''
        else:
            netloc = None
    return _urlunsplit(scheme or None, netloc, url, query or None, fragment or None)


def urlunparse(components):
    scheme, netloc, url, params, query, fragment = components
    if not netloc:
        if scheme and scheme in uses_netloc and (not url or url[:1] == '/'):
            netloc = ''
        else:
            netloc = None
    if params:
        url = "%s;%s" % (url, params)
    return _urlunsplit(scheme or None, netloc, url, query or None, fragment or None)


def urljoin(base, url, allow_fragments=True):
    if not base:
        return url
    if not url:
        return base
    if not isinstance(base, str) or not isinstance(url, str):
        return _rowan_opaque.urljoin(base, url)
    bscheme, bnetloc, bpath, bquery, bfragment = _urlsplit(base, None, allow_fragments)
    scheme, netloc, path, query, fragment = _urlsplit(url, None, allow_fragments)
    if scheme is None:
        scheme = bscheme
    if scheme != bscheme or (scheme and scheme not in uses_relative):
        return url
    if not scheme or scheme in uses_netloc:
        if netloc:
            return _urlunsplit(scheme, netloc, path, query, fragment)
        netloc = bnetloc
    if not path:
        path = bpath
        if query is None:
            query = bquery
            if fragment is None:
                fragment = bfragment
        return _urlunsplit(scheme, netloc, path, query, fragment)
    base_parts = bpath.split('/')
    if base_parts[-1] != '':
        del base_parts[-1]
    if path[:1] == '/':
        segments = path.split('/')
    else:
        segments = base_parts + path.split('/')
        segments = segments[:1] + [s for s in segments[1:-1] if s] + segments[-1:]
    resolved_path = []
    for seg in segments:
        if seg == '..':
            if resolved_path:
                resolved_path.pop()
        elif seg == '.':
            continue
        else:
            resolved_path.append(seg)
    if segments[-1] in ('.', '..'):
        resolved_path.append('')
    return _urlunsplit(scheme, netloc, '/'.join(resolved_path) or '/', query, fragment)


def urldefrag(url):
    if not isinstance(url, str):
        return _rowan_opaque.urldefrag(url)
    if '#' in url:
        s, n, p, q, frag = _urlsplit(url)
        defrag = _urlunsplit(s, n, p, q, None)
    else:
        frag = ''
        defrag = url
    return DefragResult(defrag, frag or '')


def parse_qsl(qs, keep_blank_values=False, strict_parsing=False, encoding='utf-8', errors='replace', max_num_fields=None, separator='&'):
    if not isinstance(qs, str) or not isinstance(separator, str) or not separator or strict_parsing or max_num_fields is not None:
        return _rowan_opaque.parse_qsl(qs)
    r = []
    if not qs:
        return r
    for name_value in qs.split(separator):
        if name_value:
            name, has_eq, value = name_value.partition('=')
            if value or keep_blank_values:
                name = _up.unquote_plus(name, encoding=encoding, errors=errors)
                value = _up.unquote_plus(value, encoding=encoding, errors=errors)
                r.append((name, value))
    return r


def parse_qs(qs, keep_blank_values=False, strict_parsing=False, encoding='utf-8', errors='replace', max_num_fields=None, separator='&'):
    parsed_result = {}
    pairs = parse_qsl(qs, keep_blank_values, strict_parsing, encoding, errors, max_num_fields, separator)
    for name, value in pairs:
        if name in parsed_result:
            parsed_result[name].append(value)
        else:
            parsed_result[name] = [value]
    return parsed_result


def urlencode(query, doseq=False, safe='', encoding=None, errors=None):
    if hasattr(query, "items"):
        query = query.items()
    l = []
    for k, v in query:
        k = _up.quote_plus(k if isinstance(k, str) else str(k), safe)
        if not doseq or isinstance(v, (str, int, float, bool)) or v is None:
            l.append(k + '=' + _up.quote_plus(v if isinstance(v, str) else str(v), safe))
        else:
            for elt in v:
                l.append(k + '=' + _up.quote_plus(elt if isinstance(elt, str) else str(elt), safe))
    return '&'.join(l)
"#;

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

/// `xs.pop()` / `xs.pop(i)` / `s.pop()`: the item; an empty list or an index outside it is an
/// IndexError (an empty set a KeyError), on every path when no receiver can have the item.
fn m_lpop(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let recv = c.recv.clone().unwrap_or_default();
    let idx = c.args.first().and_then(|a| a.kv.as_lit().filter(|_| a.pts.is_empty())).and_then(intern::key_as_int);
    let (mut outs, mut may_fail, mut all_fail) = (Vec::new(), recv.pts.is_empty(), !recv.pts.is_empty());
    for oid in recv.pts.iter() {
        let n = an.heap.llen(oid, st);
        let (ok, fails) = match (n, idx, c.args.is_empty()) {
            (Some(n), _, true) => (n > 0, n == 0),
            (Some(n), Some(i), _) => {
                let inside = (i >= 0 && (i as usize) < n) || (i < 0 && ((-i) as usize) <= n);
                (inside, !inside)
            }
            _ => (false, false),
        };
        may_fail |= !ok;
        all_fail &= fails;
        if fails {
            continue;
        }
        // (a set of several members gives any of them: its order is not promised)
        match n.filter(|n| *n > 1 && an.heap.kind(oid) == Kind::Set) {
            Some(n) => {
                outs.push(join_values((0..n).map(|i| st.slot(oid, Ns::Pos, intern::key_int(i as i64))).collect::<Vec<_>>()));
                an.heap.set_llen(oid, st, None);
                st.drop_obj_ns(oid, Ns::Pos);
            }
            None => outs.push(list_pop_one(an, st, oid, c)),
        }
    }
    let exc = if c.qual.starts_with("set.") { "KeyError" } else { "IndexError" };
    if all_fail {
        an.raise_here(exc, st);
        return Value::new();
    }
    if may_fail {
        an.may_raise_here(exc, st);
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
        // (a generator's `__next__` / `send`: the next item, as `next(g)`)
        "__next__" | "send" if recv.pts.iter().all(|o| an.heap.obj(o).iterator) => m_next(an, st, &Call { args: vec![recv], ..c.clone() }),
        "copy" => join_values(recv.pts.iter().map(|o| Value::obj(an.copy_object(o, &c.node, st, false, &mut IndexMap::new()))).collect::<Vec<_>>()),
        "remove" | "discard" | "clear" | "sort" | "reverse" | "insert" => {
            // `clear` on one list, whatever its length: empty
            if name == "clear" {
                if let Some(oid) = recv.single().filter(|o| an.heap.kind(*o) == Kind::List && an.heap.obj(*o).single && !crate::state::is_merged(*o)) {
                    st.drop_obj_ns(oid, Ns::Pos);
                    an.set_positions(oid, Vec::new(), st);
                    st.clear_last(oid);
                    return an.none_v.clone();
                }
            }
            // one list with known positions: updated in place
            if let Some(oid) = recv.single() {
                if let Some(mut ps) = an.known_positions(oid, st) {
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
                an.heap.set_llen(oid, st, None);
                st.drop_obj_ns(oid, Ns::Pos);
                if name == "insert" {
                    if let Some(x) = c.args.get(1) {
                        an.add_elem(oid, x, st);
                    }
                }
            }
            an.none_v.clone()
        }
        // (the statements' dunders called by name: as the statements)
        "__setitem__" if c.args.len() == 2 => {
            an.store_item(&recv, Ns::Item, &c.args[0], &c.args[1], &c.node, st);
            an.none_v.clone()
        }
        "__delitem__" if c.args.len() == 1 => {
            an.delete_subscript(&recv, &c.args[0], &c.node, st);
            an.none_v.clone()
        }
        "__iadd__" | "__ior__" | "__iand__" | "__isub__" | "__ixor__" if c.args.len() == 1 && recv.kv.is_bot() => {
            let kind = |k: Kind| !recv.pts.is_empty() && recv.pts.iter().all(|o| an.heap.kind(o) == k);
            let m = match name {
                "__iadd__" if kind(Kind::List) => "list.extend",
                "__ior__" if kind(Kind::Set) => "set.update",
                "__iand__" if kind(Kind::Set) => "set.intersection_update",
                "__isub__" if kind(Kind::Set) => "set.difference_update",
                "__ixor__" if kind(Kind::Set) => "set.symmetric_difference_update",
                _ => return an.unknown_call(&c.qual, c.recv.as_ref(), &c.args, &c.kwargs, &c.star_args, &c.star_kwargs, st, &c.node),
            };
            let mv = an.method_value(m, &recv);
            an.call_value(&mv, &c.node, None, st, Some(c.args.clone()));
            recv
        }
        // (any other method of a list or set, called by name, may change it: unknown code,
        // unless it only reads)
        _ if !LIST_READERS.contains(&name) && recv.pts.iter().any(|o| matches!(an.heap.kind(o), Kind::List | Kind::Set)) => an.unknown_call(&c.qual, c.recv.as_ref(), &c.args, &c.kwargs, &c.star_args, &c.star_kwargs, st, &c.node),
        _ => Value::data(&format!("L{}", c.line)),
    }
}

/// The methods of a list or set that only read it (those `m_list_other` does not model).
const LIST_READERS: &[&str] = &[
    "index", "count", "__len__", "__contains__", "__getitem__", "__iter__", "__reversed__", "__eq__", "__ne__", "__lt__", "__le__", "__gt__", "__ge__", "__hash__", "__repr__", "__str__", "__format__",
    "__sizeof__", "__bool__", "__add__", "__mul__", "__rmul__", "__or__", "__and__", "__sub__", "__xor__", "__ror__", "__rand__", "__rsub__", "__rxor__",
];

fn m_view(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = last(&c.qual);
    if name == "map" {
        // `df.columns.map(f)`: the column names renamed by f
        if let Some(v) = pandas::columns_map(an, st, c) {
            return v;
        }
    }
    // `obj.__dict__.update(...)`: stores on the instance's attributes
    let attrs_of: Vec<u32> = c.recv.iter().flat_map(|r| r.pts.iter()).filter_map(|v| match &an.heap.obj(v).payload {
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
        let strong = attrs_of.len() == 1 && an.heap.obj(attrs_of[0]).single;
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
            an.heap.set_llen(oid, st, None);
            let it = an.iterate(recv, &c.node, st);
            let elem = an.elem_value(&it.elem, &c.node, "el");
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
                an.store_key(oid, Ns::Item, &KeyVal::Lit(key_str_lit(k)), &v, strong, &c.node, st);
            }
            match hook {
                Some(h) => {
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
/// The JSON text httpx sends for a request's `json=` body (compact separators, no ASCII
/// escaping), when the value is known.
pub fn json_body_text(an: &Analyzer, st: &State, v: &Value) -> Option<String> {
    let o = JsonOpts { sort_keys: false, indent: None, item_sep: ",".into(), key_sep: ":".into(), ensure_ascii: false };
    json_text(an, st, v, &o, 0)
}

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
                            an.store_key(o, Ns::Attr, &KeyVal::Lit(intern::key_str_lit(name)), &v, strong, &c.node, st);
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

fn m_http(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let tag = format!("{}@L{}", json_tag(&c.qual), c.line);
    let oid = an.alloc_at(Kind::Response, &c.node, st, &tag, Payload::Str(tag.clone()), None, Ns::Item, None, None);
    an.heap.obj_mut(oid).payload = Payload::Str(tag);
    Value::obj(oid)
}

fn m_response_method(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
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

fn m_copy(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(a) = c.args.first() else { return Value::data("copy") };
    let deep = c.qual.ends_with("deepcopy");
    let mut outs = Vec::new();
    for o in a.pts.iter() {
        // (an object from input is a new object too: stores into the copy do not reach it)
        if matches!(an.heap.kind(o), Kind::Dict | Kind::List | Kind::Tuple | Kind::Set | Kind::Instance | Kind::Frame) || (an.heap.kind(o) == Kind::Unknown) {
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
    if let Some(moid) = an.module(&modname) {
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
    let moid = an.make_module(&modname, None, idx, tree);
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
    let moid = match an.module(&modname) {
        Some(m) => m,
        None => {
            let src = format!("def getter(obj):\n    return {}\n", body);
            let Ok((idx, tree)) = crate::frontend::parse(&modname, None, src) else { return Value::data("getter") };
            let m = an.make_module(&modname, None, idx, tree);
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
    let vid = an.heap.alloc_raw(Kind::View, &format!("view:attrs:{}", m), "", true, "globals()", Payload::View(ViewKind::Attrs, Value::obj(m)));
    an.heap.obj_mut(vid).payload = Payload::View(ViewKind::Attrs, Value::obj(m));
    Value::obj(vid)
}

/// What library base classes make of a class the program defines (`class C(Base)`): of one
/// (`class_base`), or of its direct library bases together (`ClassInfo::lib`).
#[derive(Default, Clone, Debug)]
pub struct ClassBase {
    pub is_enum: bool,
    pub enum_kind: crate::interp::EnumKind,
    pub str_mixin: bool,
    pub dict_base: bool,
    pub nn_module: bool,
    pub pydantic: bool,
    pub typed_dict: bool,
    pub named_tuple: bool,
    /// its instances may hold what the program does not show (a base rowan does not see)
    pub open: bool,
    /// the bases' qualified names
    pub quals: Vec<String>,
}

impl ClassBase {
    pub fn join(&mut self, o: ClassBase) {
        self.enum_kind.join(o.enum_kind);
        self.is_enum |= o.is_enum;
        self.str_mixin |= o.str_mixin;
        self.dict_base |= o.dict_base;
        self.nn_module |= o.nn_module;
        self.pydantic |= o.pydantic;
        self.typed_dict |= o.typed_dict;
        self.named_tuple |= o.named_tuple;
        self.open |= o.open;
        self.quals.extend(o.quals);
    }
}

pub fn class_base(q: &str) -> ClassBase {
    let mut f = ClassBase { is_enum: (q.ends_with("Enum") || q.ends_with("Flag")) && q.starts_with("enum."), quals: vec![q.to_string()], ..Default::default() };
    match q {
        "enum.StrEnum" | "enum.IntEnum" | "enum.IntFlag" | "enum.ReprEnum" => {
            f.enum_kind.mixed = true;
            f.enum_kind.repr_value = true;
        }
        "builtins.str" | "builtins.int" => f.enum_kind.mixed = true,
        _ => {}
    }
    f.enum_kind.is_str = q == "enum.StrEnum";
    f.str_mixin = q == "builtins.str";
    f.enum_kind.flag = matches!(q, "enum.Flag" | "enum.IntFlag");
    f.dict_base = q == "builtins.dict";
    f.nn_module = q.starts_with("torch.nn.");
    if pydantic::is_pydantic_base(q) {
        f.pydantic = true;
    } else if q.ends_with("TypedDict") {
        f.typed_dict = true;
    } else if q.ends_with("NamedTuple") {
        f.named_tuple = true;
    } else {
        // (a builtin exception base adds nothing to the instance's `vars()`)
        let plain = matches!(q, "builtins.object" | "object" | "abc.ABC" | "typing.Protocol" | "typing.Generic") || q.ends_with("Generic") || q.strip_prefix("builtins.").is_some_and(crate::interp::exprs::is_exception_name) || q.starts_with("enum.");
        f.open = !plain;
    }
    f
}

/// `locals()`: at module level the module's names (as `globals()`); in a function a snapshot
/// whose changes go nowhere.
fn m_locals(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    if an.frame().kind == crate::interp::FrameKind::Module {
        return m_globals(an, st, c);
    }
    Value::data(&format!("L{}", c.line))
}

/// `__import__("os")` / `importlib.import_module("os")`: the module (by a known name), as an
/// import statement gives it (`__import__("a.b")` gives `a`).
fn m_import(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(name) = c.args.first().and_then(|a| a.kv.as_lit().filter(|_| a.pts.is_empty())).and_then(intern::key_as_str) else {
        return an.unknown_call(&c.qual, None, &c.args, &c.kwargs, &c.star_args, &c.star_kwargs, st, &c.node);
    };
    let m = an.import_module(&name, &c.node, st);
    match name.split_once('.') {
        Some((top, _)) if c.qual == "builtins.__import__" => an.import_module(top, &c.node, st),
        _ => m,
    }
}

/// `exit()` / `quit()` / `sys.exit()`: SystemExit.
fn m_exit(an: &mut Analyzer, st: &mut State, _c: &Call) -> Value {
    an.raise_here("SystemExit", st);
    Value::new()
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

pub fn re_match_generated(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
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
    let moid = match an.module(&modname) {
        Some(m) => m,
        None => {
            // (an unknown text's groups: unknown strings, shown as `?`, not the helper's name)
            let items = names.iter().map(|n| format!("{:?}: str(_GROUP)", n)).collect::<Vec<_>>().join(", ");
            let groups = (0..ngroups).map(|_| "str(_GROUP)").collect::<Vec<_>>().join(", ");
            let src = format!(
                "class Match:\n    def groupdict(self, default=None):\n        return {{{}}}\n\n    def group(self, *names):\n        return str(_GROUP)\n\n    def groups(self, default=None):\n        return ({}{})\n\n    def start(self, g=0):\n        return int(_POS)\n\n    def end(self, g=0):\n        return int(_POS)\n\n    def span(self, g=0):\n        return (int(_POS), int(_POS))\n\n\ndef matched():\n    return Match()\n",
                items,
                groups,
                if ngroups == 1 { "," } else { "" }
            );
            let Ok((idx, tree)) = crate::frontend::parse(&modname, None, src) else { return Value::data("re") };
            let m = an.make_module(&modname, None, idx, tree);
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
        let cls = an.heap.instance_class(o);
        let Some(cls) = cls.filter(|cl| crate::models::dataclasses::is_named_tuple_class(an, *cl)) else { continue };
        let fields: Vec<String> = crate::models::dataclasses::dataclass_fields_mro(an, cls).into_iter().map(|(f, _)| f).collect();
        if c.qual.ends_with("_asdict") {
            let d = an.alloc_simple(Kind::Dict, &c.node, st, &format!("dict@L{}", c.line));
            for f in &fields {
                let v = st.slot(o, Ns::Attr, key_str_lit(f));
                an.store_key(d, Ns::Item, &KeyVal::Lit(key_str_lit(f)), &v, true, &c.node, st);
            }
            outs.push(Value::obj(d));
        } else {
            if c.kwargs.keys().any(|k| !fields.iter().any(|f| *f == *intern::str_get(*k))) {
                // `_replace` with an unknown field name: TypeError (ValueError before 3.14)
                an.raise_here("TypeError", st);
                continue;
            }
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
            let cls = match an.heap.kind(o) {
                Kind::Instance => an.heap.instance_class(o),
                _ => None,
            };
            let Some(cls) = cls.filter(|cl| !crate::models::dataclasses::dataclass_fields_mro(an, *cl).is_empty()).filter(|_| depth < 4) else {
                out = out.join(&Value::obj(o));
                continue;
            };
            let fields: Vec<String> = crate::models::dataclasses::dataclass_fields_mro(an, cls).into_iter().map(|(f, _)| f).collect();
            let mut vals = Vec::new();
            for f in fields {
                let x = st.slot(o, Ns::Attr, key_str_lit(&f));
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
    if let Some(moid) = an.module(&modname) {
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
    let moid = an.make_module(&modname, None, idx, tree);
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
        let cls = match an.heap.kind(o) {
            Kind::Instance => an.heap.instance_class(o),
            _ => None,
        };
        if let Some(cls) = cls.filter(|cl| an.heap.class_info_opt(*cl).map(|ci| ci.has_fields() && !ci.lib.named_tuple).unwrap_or(false)) {
            let specs = crate::models::dataclasses::dc_specs(an, cls);
            let mut kw: IndexMap<intern::StrId, Value> = IndexMap::new();
            for sp in specs.iter().filter(|sp| sp.init && !sp.initvar) {
                let v = st.slot(o, Ns::Attr, key_str_lit(&sp.name));
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
        if an.heap.kind(o) == Kind::Instance {
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
    if name == "Counter" {
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
                let it = if an.record_objects(&c.args[0]).iter().all(|o| an.heap.kind(*o) != Kind::Dict) { an.iterate(&c.args[0], &c.node, st).items } else { None };
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
            if !c.args[0].pts.is_empty() {
                // the factory runs per inserted key (see `load_item`); no shared default
                an.m.dd_factories.insert(oid, c.args[0].clone());
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
        an.m.chainmaps.insert(oid, maps.clone());
        // (built from parameters: the maps are known per call site)
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
        if update_from_instances(an, st, oid, &sa, true, c) {
        } else if !an.record_objects(&sa).is_empty() {
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
    an.heap.obj_mut(rid).iter_elem = true;
    let ctx = an.frame().ctx.clone();
    let vid = an.heap.alloc_raw(Kind::View, &format!("view:elems:{}", c.site), &ctx, true, "DictReader", Payload::View(ViewKind::Elems, Value::obj(rid)));
    an.heap.obj_mut(vid).payload = Payload::View(ViewKind::Elems, Value::obj(rid));
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
    let parsers: Vec<u32> = recv.pts.iter().filter(|o| an.heap.kind(*o) == Kind::Parser).collect();
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
                        let single = an.heap.obj(p).single;
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
                an.heap.set_llen(tid, st, Some(2));
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
    let memo = an.heap.alloc_raw(Kind::Dict, &format!("memo:{}", f), "", true, "lru_cache", Payload::None);
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
    if let Some(inner) = f.single().filter(|o| an.heap.kind(*o) == Kind::Partial) {
        if let Payload::Partial(f0, a0, k0) = an.heap.obj(inner).payload.clone() {
            func = f0;
            let mut a = a0;
            a.extend(args);
            args = a;
            let mut k = k0;
            for (kk, v) in kw.iter() {
                k.insert(*kk, v.clone());
            }
            kw = k;
            let mut s0 = an.m.partial_star.get(&inner).cloned().unwrap_or_default();
            s0.extend(star);
            star = s0;
        }
    }
    // `**opts`: its literal keys become bound keywords; an open mapping is passed on as it is
    let mut rest_star: Vec<Value> = Vec::new();
    for sk in &star {
        let recs = an.record_objects(sk);
        let closed = recs.len() == 1 && { let r = st.row(recs[0], Ns::Item); r.closed() && r.present == r.definite && r.present.iter().all(intern::key_is_str) };
        if closed {
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
    an.heap.obj_mut(oid).payload = payload;
    if !rest_star.is_empty() {
        an.m.partial_star.insert(oid, rest_star);
    } else {
        an.m.partial_star.remove(&oid);
    }
    Value::obj(oid)
}

fn m_chain(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let oid = an.alloc_simple(Kind::List, &c.node, st, &format!("chain@L{}", c.line));
    an.heap.set_llen(oid, st, None);
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
        let Payload::Partial(f, bound_args, bound_kw) = an.heap.obj(x).payload.clone() else { continue };
        let mut kw = bound_kw.clone();
        for (k, v) in c.kwargs.iter() {
            kw.insert(*k, v.clone());
        }
        let mut fake_args = bound_args.clone();
        fake_args.extend(c.args.iter().cloned());
        // (bound `**opts` of unknown keys: passed on before the call's own)
        let mut star = an.m.partial_star.get(&x).cloned().unwrap_or_default();
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
    Value::obj(an.heap.alloc_raw(Kind::External, "ext:logging.Logger", "", true, "logging.Logger", Payload::Str("logging.Logger".into())))
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
    match an.heap.kind(o) {
        Kind::Class => {
            let mut out: Vec<String> = Vec::new();
            let classes: Vec<crate::domain::ObjId> = if mro { an.heap.mro(o).iter().rev().copied().collect() } else { vec![o] };
            let mut any = false;
            for c in classes {
                let Some(ci) = an.heap.class_info_opt(c) else { continue };
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
            let fi = an.heap.func_info(o);
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
    if d.pts.is_empty() || !d.pts.iter().all(|o| matches!(an.heap.kind(o), Kind::Dict | Kind::Unknown)) {
        return an.unknown_call(&c.qual, None, &c.args, &c.kwargs, &c.star_args, &c.star_kwargs, st, &c.node);
    }
    let _ = st;
    let vid = an.heap.alloc_raw(Kind::View, &format!("view:proxy:{}:{}", c.site, d.pts.iter().map(|o| o.to_string()).collect::<Vec<_>>().join(",")), "", true, "mappingproxy", Payload::View(ViewKind::Proxy, d.clone()));
    an.heap.obj_mut(vid).payload = Payload::View(ViewKind::Proxy, d);
    Value::obj(vid)
}

/// `Decimal(x)` / `Fraction(a, b)` of known values: a literal (models::pynum).
fn m_number_new(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let lit = |v: &Value| v.kv.as_lit().filter(|_| v.pts.is_empty());
    let dec = c.qual.starts_with("decimal.");
    let a = c.arg(0, Some(if dec { "value" } else { "numerator" }));
    let b = if dec { None } else { c.arg(1, Some("denominator")) };
    let known = a.as_ref().map(|v| lit(v).is_some()).unwrap_or(true) && b.as_ref().map(|v| lit(v).is_some()).unwrap_or(true) && (!dec || c.args.len() <= 1);
    if known {
        let (ka, kb) = (a.as_ref().and_then(lit), b.as_ref().and_then(lit));
        let r = if dec { pynum::decimal_new(ka) } else { pynum::fraction_new(ka, kb) };
        if let Some(k) = r {
            return Value::lit(k);
        }
        // `Decimal("x")` / `Fraction(1, 0)`: raises, or a value not simulated
        let bad_text = ka.and_then(intern::key_as_str).is_some_and(|s| !s.trim().to_lowercase().contains("inf") && !s.trim().to_lowercase().contains("nan"));
        if bad_text || (!dec && kb.and_then(intern::key_as_int) == Some(0)) {
            an.raise_here(if dec { "decimal.InvalidOperation" } else { "ZeroDivisionError" }, st);
            return Value::new();
        }
    }
    Value::data(if dec { "Decimal" } else { "Fraction" })
}

/// `date(…)` / `datetime(…)` / `time(…)` / `timedelta(…)` / `timezone(…)` of known values.
fn m_chrono_new(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let class = last(&c.qual).to_string();
    let lit = |v: &Value| v.kv.as_lit().filter(|_| v.pts.is_empty());
    let args: Option<Vec<KeyId>> = c.args.iter().map(lit).collect();
    let kwargs: Option<Vec<(String, KeyId)>> = c.kwargs.iter().map(|(k, v)| lit(v).map(|x| (intern::str_get(*k).to_string(), x))).collect();
    if let (Some(args), Some(kwargs), true) = (args, kwargs, c.star_args.is_empty() && c.star_kwargs.is_empty()) {
        let kw: Vec<(&str, KeyId)> = kwargs.iter().map(|(k, v)| (k.as_str(), *v)).collect();
        match chrono::construct(&class, &args, &kw) {
            Some(Ok(k)) => return Value::lit(k),
            Some(Err(())) => {
                an.raise_here(if class == "timedelta" { "OverflowError" } else { "ValueError" }, st);
                return Value::new();
            }
            None => {}
        }
    }
    Value::data(&class)
}

/// `datetime.fromisoformat(…)`, `datetime.strptime(…)`, `date.fromordinal(…)`, `datetime.combine(…)`.
fn m_chrono_class(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let mut parts = c.qual.rsplit('.');
    let name = parts.next().unwrap_or("").to_string();
    let class = parts.next().unwrap_or("").to_string();
    let lit = |v: &Value| v.kv.as_lit().filter(|_| v.pts.is_empty());
    if let Some(args) = c.args.iter().map(lit).collect::<Option<Vec<KeyId>>>().filter(|_| c.kwargs.is_empty()) {
        match chrono::class_method(&class, &name, &args) {
            Some(Ok(k)) => return Value::lit(k),
            Some(Err(())) => {
                an.raise_here("ValueError", st);
                return Value::new();
            }
            None => {}
        }
    }
    Value::data(if class == "date" { "date" } else { "datetime" })
}

/// The clock: unknown.
fn m_chrono_clock(_an: &mut Analyzer, _st: &mut State, c: &Call) -> Value {
    Value::data(if c.qual.starts_with("datetime.date.") { "date" } else { "datetime" })
}

/// A changed decimal context: Decimal arithmetic is no longer simulated.
fn m_decimal_context(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    pynum::set_opaque(true);
    an.unknown_call(&c.qual, None, &c.args, &c.kwargs, &c.star_args, &c.star_kwargs, st, &c.node)
}

/// A Decimal / Fraction method (`d.quantize(…)`, `f.limit_denominator(…)`).
fn m_num_method(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = last(&c.qual).to_string();
    let lit = |v: &Value| v.kv.as_lit().filter(|_| v.pts.is_empty());
    let recv = c.recv.clone().unwrap_or_default();
    let args: Option<Vec<KeyId>> = c.args.iter().map(lit).collect();
    let kwargs: Option<Vec<(String, KeyId)>> = c.kwargs.iter().map(|(k, v)| lit(v).map(|x| (intern::str_get(*k).to_string(), x))).collect();
    let (Some(args), Some(kwargs)) = (args, kwargs) else {
        return Value::data("number");
    };
    let kw: Vec<(&str, KeyId)> = kwargs.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    let mut outs = Vec::new();
    for r in crate::domain::kv_literals(&recv.kv).iter() {
        let res = if chrono::is_chrono_key(r) { chrono::method(r, &name, &args, &kw) } else { pynum::method(r, &name, &args, &kw) };
        match res {
            Some(k) => outs.push(pure_result(an, st, c, k)),
            None => return Value::data("number"),
        }
    }
    if outs.is_empty() {
        return Value::data("number");
    }
    join_values(outs)
}

/// A literal result, a literal tuple as a tuple object.
fn pure_result(an: &mut Analyzer, st: &mut State, c: &Call, k: KeyId) -> Value {
    let items = intern::with_key(k, |kk| match kk {
        intern::Key::Tuple(xs) => Some(xs.clone()),
        _ => None,
    });
    match items {
        Some(xs) => {
            let vals: Vec<Value> = xs.iter().map(|x| Value::lit(*x)).collect();
            an.alloc_positional(Kind::Tuple, &c.node, Some(vals), Value::new(), DATA, st)
        }
        None => Value::lit(k),
    }
}

fn m_range(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let ints: Option<Vec<i64>> = c.args.iter().map(|a| if a.pts.is_empty() { a.kv.as_lit().and_then(intern::key_as_int) } else { None }).collect();
    let bounds = match ints.as_deref() {
        Some([stop]) => Some((0, *stop, 1)),
        Some([start, stop]) => Some((*start, *stop, 1)),
        Some([start, stop, step]) if *step != 0 => Some((*start, *stop, *step)),
        _ => None,
    };
    let Some((lo, hi, step)) = bounds.filter(|_| c.kwargs.is_empty()) else {
        let oid = an.alloc_simple(Kind::List, &c.node, st, &format!("range@L{}", c.line));
        an.heap.set_llen(oid, st, None);
        // (for the tensor model: an int of unknown value)
        st.elems.insert(oid, if an.m.torch_on { KeyVal::data(&crate::models::shape::dim_label(crate::models::shape::unknown())) } else { DATA });
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
        an.heap.obj_mut(oid).payload = Payload::Range(lo, hi, step);
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
    if an.m.torch_on {
        if let Some(r) = crate::models::torch::t_len(an, v, &c.node, st) {
            return r;
        }
    }
    // the length of a placeholder: decided per call site where it is a known container
    if v.pts.is_empty() {
        if let Some(k) = v.kv.as_lit() {
            if let Some(s) = intern::key_as_str(k) {
                return Value::lit(intern::key_int(s.chars().count() as i64));
            }
        }
        return Value::data("len");
    }
    // lists / tuples whose possible lengths are known (on this path, or as the alternatives of
    // the paths joined here): one of those
    if v.kv.is_bot() && v.pts.iter().all(|o| matches!(an.heap.kind(o), Kind::List | Kind::Tuple)) {
        let mut all: Vec<usize> = Vec::new();
        let mut known = true;
        for o in v.pts.iter() {
            match an.heap.len_alts(o, st) {
                Some(ls) => all.extend(ls),
                None => {
                    known = false;
                    break;
                }
            }
        }
        if known && !all.is_empty() {
            let ks = intern::KeySet::from_iter(all.into_iter().map(|n| intern::key_int(n as i64)));
            return Value::kv_only(crate::domain::KeyVal::lits(ks));
        }
    }
    if let Some(o) = v.single() {
        // a view of one dict: the dict's length
        if let Payload::View(ViewKind::Keys | ViewKind::Values | ViewKind::Items | ViewKind::Proxy, inner) = an.heap.obj(o).payload.clone() {
            if let Some(d) = inner.single().filter(|d| matches!(an.heap.kind(*d), Kind::Dict | Kind::Frame)) {
                let row = st.row(d, Ns::Item);
                if row.closed() && row.present == row.definite {
                    return Value::lit(intern::key_int(row.definite.len() as i64));
                }
                // a closed frame: between its definite and its possible columns
                if an.heap.kind(d) == Kind::Frame && row.closed() && row.present.len() - row.definite.len() <= 8 {
                    let ks = intern::KeySet::from_iter((row.definite.len()..=row.present.len()).map(|n| intern::key_int(n as i64)));
                    return Value::kv_only(crate::domain::KeyVal::lits(ks));
                }
            }
            return Value::data("len");
        }
        if let Some(n) = an.heap.llen(o, st) {
            return Value::lit(intern::key_int(n as i64));
        }
        // (a loop of a tensor dim's rounds appended to it once each)
        if let Some(l) = st.sym_len(o) {
            return Value::data(&intern::str_get(l));
        }
        if matches!(an.heap.kind(o), Kind::Dict) {
            let row = st.row(o, Ns::Item);
            if row.closed() && row.present == row.definite {
                return Value::lit(intern::key_int(row.definite.len() as i64));
            }
            // keys set on some paths: between the sure and the possible count
            if row.closed() && row.present.len() - row.definite.len() <= 8 {
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
    if an.m.torch_on {
        let e = it.elem.flat();
        if crate::models::torch::is_tensor(&e) {
            if let Some(s) = crate::models::torch::t_shape(an, &e, st) {
                let start = c.args.get(1).and_then(|x| crate::models::torch::t_shape(an, x, st)).unwrap_or_else(|| crate::models::shape::Shape::known(vec![]));
                return crate::models::torch::t_broadcast(an, &s, &start, &c.node, st, "sum");
            }
        }
    }
    if let Some(items) = it.items {
        let lit = |v: &Value| v.kv.as_lit().filter(|_| v.pts.is_empty());
        let start = match c.args.get(1).or(c.kwargs.get(&s("start"))) {
            Some(v) => match lit(v) {
                Some(k) => k,
                None => return Value::data("sum"),
            },
            None => intern::key_int(0),
        };
        let mut ks: Vec<KeyId> = Vec::with_capacity(items.len());
        for item in &items {
            match lit(item) {
                Some(k) => ks.push(k),
                None => return Value::data("sum"),
            }
        }
        return match py_sum(start, &ks) {
            Some(k) => Value::lit(k),
            None => Value::data("sum"),
        };
    }
    Value::data("sum")
}

/// CPython's `sum(items, start)` on literals: the exact int path, then the compensated
/// (Neumaier) float path, then plain `+`.
pub fn py_sum(start: KeyId, items: &[KeyId]) -> Option<KeyId> {
    let get = |k: KeyId| intern::key_get(k);
    let mut acc = start;
    let mut i = 0;
    // ints (an exact int start)
    if let intern::Key::Int(mut a) = get(acc) {
        while i < items.len() {
            match get(items[i]) {
                intern::Key::Int(x) => a = a.checked_add(x)?,
                intern::Key::Bool(b) => a = a.checked_add(b as i64)?,
                _ => break,
            }
            i += 1;
        }
        acc = intern::key_int(a);
        if i < items.len() {
            acc = pyvalues::operator_call("add", &[acc, items[i]])?;
            i += 1;
        }
    }
    // floats
    if let intern::Key::Float(bits) = get(acc) {
        let mut f = f64::from_bits(bits);
        let mut c = 0.0f64;
        while i < items.len() {
            match get(items[i]) {
                intern::Key::Float(xb) => {
                    let x = f64::from_bits(xb);
                    let t = f + x;
                    if f.abs() >= x.abs() {
                        c += (f - t) + x;
                    } else {
                        c += (x - t) + f;
                    }
                    f = t;
                }
                intern::Key::Int(x) => f += x as f64,
                intern::Key::Bool(b) => f += b as i64 as f64,
                _ => break,
            }
            i += 1;
        }
        if c != 0.0 && c.is_finite() {
            f += c;
        }
        acc = intern::key_float(f);
    }
    while i < items.len() {
        acc = pyvalues::operator_call("add", &[acc, items[i]])?;
        i += 1;
    }
    Some(acc)
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
    if an.heap.kind(o) == Kind::Pair {
        // a (key, value) pair compares like a 2-tuple
        let Payload::Pair(a, b) = an.heap.obj(o).payload.clone() else { return None };
        let lit = |x: &Value| x.kv.as_lit().filter(|_| x.pts.is_empty());
        let mut ks = vec![lit(&a)?];
        if let Some(y) = lit(&b) {
            ks.push(y);
        }
        return Some(ks);
    }
    if an.heap.kind(o) != Kind::Tuple {
        return None;
    }
    let n = an.heap.llen(o, st)?;
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
/// The items of `sorted(d.items())` when `d` is a closed dict whose keys are known,
/// pairwise comparable literals and only some of them maybe there: (key, value, sure) in
/// sorted order (the keys present at run time come in that order).
fn sorted_dict_items(an: &Analyzer, st: &State, a: &Value, c: &Call) -> Option<Vec<(KeyId, Value, bool)>> {
    let v = a.single().filter(|_| a.kv.is_bot())?;
    let Payload::View(ViewKind::Items, recv) = &an.heap.obj(v).payload else { return None };
    let d = recv.single().filter(|r| recv.kv.is_bot() && an.heap.kind(*r) == Kind::Dict)?;
    let row = st.row(d, Ns::Item);
    if !row.closed() || row.present == row.definite || row.present.len() > 64 || st.has_slot(d, Ns::Item, STAR) || an.m.counters.contains(&d) {
        return None;
    }
    let mut keys: Vec<KeyId> = row.present.iter().collect();
    for i in 0..keys.len() {
        for j in (i + 1)..keys.len() {
            crate::interp::compare::literal_order(keys[i], keys[j])?;
        }
    }
    keys.sort_by(|x, y| crate::interp::compare::literal_order(*x, *y).unwrap());
    let reverse = match c.kw("reverse") {
        None => false,
        Some(r) => intern::with_key(r.kv.as_lit()?, |kk| match kk { intern::Key::Bool(b) => Some(*b), _ => None })?,
    };
    if reverse {
        keys.reverse();
    }
    Some(keys.into_iter().map(|k| (k, st.slot(d, Ns::Item, k), row.definite.contains(k))).collect())
}

fn literal_sort_order(an: &mut Analyzer, st: &mut State, c: &Call, items: &[Value]) -> Option<Vec<usize>> {
    // instances ordering themselves (no key=): a stable sort on their `__lt__`
    if c.kw("key").is_none() && !items.is_empty() && items.iter().all(|x| x.single().map(|o| an.heap.kind(o) == Kind::Instance).unwrap_or(false)) {
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
            match crate::interp::compare::literal_order(*x, *y)? {
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


use crate::domain::ObjId;
use rustc_hash::FxHashSet;
use crate::intern::StrId;
use std::rc::Rc;

/// What the library models keep beside the state, per object (side tables), and the flags
/// of a running model. Keyed by objects, these are edges the collector follows
/// (`Analyzer::obj_edges`).
#[derive(Default)]
pub struct Tables {
    /// validating a FastAPI test request's body (errors are a 422)
    pub fa_body_validation: bool,
    /// unknown parameters annotated `int` / `bool` (true) or `float` (false) in torch modules
    pub t_numbers: FxHashMap<ObjId, bool>,
    /// unknown parameters annotated `str` in torch modules: their dim 0 prints as `len(s)`
    pub t_strs: FxHashSet<crate::models::shape::Sym>,
    /// module-level names bound to an unknown number, now dims named by them (`t_name_number`)
    pub t_named_numbers: std::collections::BTreeSet<crate::models::shape::Sym>,
    /// unknown call results named by the variable they were bound to (tensor dims)
    pub t_var_names: FxHashMap<ObjId, crate::models::shape::Sym>,
    /// function objects by qualname (a closure's printing sees its factory's symbols)
    pub t_funcs_by_qual: FxHashMap<String, Vec<ObjId>>,
    /// running a call that stands for "some module": no shape errors / requirements / facts
    pub t_quiet: bool,
    /// reading an attribute without the tensor model / without the user-class check
    pub t_getattr_plain: bool,
    pub t_skip_user: bool,
    /// the node of the function expression of the call being evaluated
    pub t_called_attr: usize,
    /// tensor-shape requirements / definite mismatches (models/torch.rs)
    pub shape_reqs: Vec<crate::models::torch::ShapeNote>,
    pub shape_errors: Vec<crate::models::torch::ShapeNote>,
    /// dicts with case-insensitive str keys (request headers): keys are lower-cased
    pub ci_dicts: FxHashSet<ObjId>,
    /// csv writers (their file and dialect), csv readers (their fieldnames)
    pub csv_writers: FxHashMap<ObjId, crate::models::csvio::CsvWriter>,
    pub csv_readers: FxHashMap<ObjId, Value>,
    /// argparse parsers, their subparsers and groups
    pub argp: FxHashMap<ObjId, crate::models::argparse::ArgObj>,
    /// futures: the exception classes their call raised (empty: any)
    pub future_excs: FxHashMap<ObjId, Vec<String>>,
    /// the StringIO objects `redirect_stdout` sends prints to (innermost last)
    pub stdout_redirect: Vec<Value>,
    /// ExitStack callbacks: (callable, positional, keyword arguments), in registration order
    pub exit_callbacks: FxHashMap<ObjId, Vec<(Value, Vec<Value>, IndexMap<StrId, Value>)>>,
    /// TestClient cookie jars (dicts with `set`)
    pub cookie_jars: FxHashSet<ObjId>,
    /// configparser parsers and their sections (models::configparser)
    pub cfg_parsers: FxHashMap<ObjId, crate::models::configparser::CfgSpec>,
    pub cfg_sections: FxHashMap<ObjId, (crate::models::configparser::CfgSpec, String)>,
    /// compiled patterns (text, flags) and match objects (models::remodel)
    pub re_patterns: FxHashMap<ObjId, (String, u32)>,
    pub re_matches: FxHashMap<ObjId, crate::models::remodel::ReMatch>,
    /// pathlib path objects' classes (models::pathtext)
    pub path_classes: FxHashMap<ObjId, String>,
    /// sqlite3 connections (their databases), cursors (their connection), Row dicts (columns)
    pub sql_dbs: FxHashMap<ObjId, crate::models::sqlite::Db>,
    pub sql_cursors: FxHashMap<ObjId, ObjId>,
    pub sql_rows: FxHashMap<ObjId, Vec<String>>,
    /// itertools.count / cycle / repeat(x) objects
    pub infinites: FxHashMap<ObjId, crate::models::itertools::Infinite>,
    /// partials' bound `**mapping`s of unknown keys
    pub partial_star: FxHashMap<ObjId, Vec<Value>>,
    /// pydantic model classes described (`pyd_class`), `TypeAdapter` types by index
    pub pyd_classes: FxHashMap<ObjId, Rc<crate::models::pydantic::PClass>>,
    pub pyd_adapters: Vec<crate::models::pydantic::Ann>,
    pub pyd_depth: u32,
    pub pyd_ann_depth: u32,
    /// validation problems found per running validation; raised ValidationErrors' problems;
    /// the field being validated
    pub pyd_errs: Vec<Vec<crate::models::pydantic::PErr>>,
    pub pyd_verrs: Vec<Vec<crate::models::pydantic::PErr>>,
    pub pyd_field: Option<KeyId>,
    /// a nested model being validated for this field of the enclosing one (its errors' loc
    /// starts with it)
    pub pyd_nest_prefix: Option<KeyId>,
    /// the instance whose field is being validated (containers made for it are its own)
    pub pyd_inst: Option<ObjId>,
    /// numpy structured arrays (fields are Item keys); the exception a missing item raises
    /// when every target is one (`ValueError`, not KeyError)
    pub np_struct: FxHashSet<ObjId>,
    pub pyd_last_failed: bool,
    /// pydantic instances: the fields their input surely / maybe gave (`model_fields_set`)
    pub pyd_set: FxHashMap<ObjId, (KeySet, KeySet)>,
    /// specialisations of generic models: their type variables' bindings
    pub pyd_generic: FxHashMap<ObjId, Vec<(String, crate::models::pydantic::Ann)>>,
    /// FastAPI: routes, `include_router` edges (app, router, prefix), router prefixes, handlers
    pub fa_routes: Vec<crate::models::fastapi::FaRoute>,
    pub fa_includes: Vec<(ObjId, ObjId, String)>,
    pub fa_prefix: FxHashMap<ObjId, String>,
    pub fa_handlers: FxHashSet<ObjId>,
    /// FastAPI hooks: (app, "exc" / "mw", exception class, function); requests a middleware's
    /// call_next dispatches (route, request)
    pub fa_hooks: Vec<(ObjId, String, Value, Option<ObjId>)>,
    pub fa_pending: Vec<(usize, crate::models::fastapi::Req)>,
    /// the status code of the last dispatched request; the injected `Response` objects of the
    /// requests being dispatched
    pub fa_status: Value,
    pub fa_out_resp: Vec<Vec<ObjId>>,
    /// per request being dispatched: the dependencies' values (each runs once)
    /// the handler's return value from inside yield dependencies
    pub fa_gen_ret: Option<Value>,
    pub fa_dep_cache: Vec<FxHashMap<ObjId, Value>>,
    /// `dependencies=[…]` of apps / routers (also given at include_router)
    pub fa_app_deps: FxHashMap<ObjId, Vec<Value>>,
    /// zip objects made with `strict=True`
    pub zip_strict: FxHashSet<ObjId>,
    /// dicts that are `collections.Counter`s
    pub counters: FxHashSet<ObjId>,
    /// `sorted(d.items())` of a closed dict whose keys are known literals (some maybe there):
    /// the items in sorted order with their sureness, for `dict(...)` of the list
    pub sorted_items: FxHashMap<ObjId, Rc<Vec<(KeyId, Value, bool)>>>,
    /// ChainMaps: the maps they look through (the first takes the writes)
    pub chainmaps: FxHashMap<ObjId, Vec<Value>>,
    /// deques and their maxlen
    pub deques: FxHashMap<ObjId, Option<i64>>,
    /// the generated `UserDict` classes (their instances' keys are `self.data`'s)
    pub userdict_classes: FxHashSet<ObjId>,
    /// `defaultdict(factory)`: the factory, called once per inserted key
    pub dd_factories: FxHashMap<ObjId, Value>,
    // ---- library state the interpreter also reads
    /// Enum classes: their members in definition order
    pub enum_members: FxHashMap<ObjId, Vec<(String, ObjId)>>,
    /// every name of an Enum in definition order, aliases (a name whose value an earlier
    /// member has) included: name → member (`Color[name]`, `__members__`)
    pub enum_aliases: FxHashMap<ObjId, Vec<(String, ObjId)>>,
    /// Enum classes: (mixes in str / int: equal to its value, `str()` is the value (ReprEnum:
    /// StrEnum, IntEnum, IntFlag), a Flag)
    pub enum_kind: FxHashMap<ObjId, crate::interp::EnumKind>,
    /// singledispatch functions: (type, implementation) in registration order; the methods
    pub sdispatch: FxHashMap<ObjId, Vec<(Value, ObjId)>>,
    pub sdispatch_method: FxHashSet<ObjId>,
    pub sd_bypass: Option<ObjId>,
    /// builtin exception objects: their class names
    pub exc_objs: FxHashMap<ObjId, String>,
    /// entry parameters annotated with a TypedDict: the declared row's reads are still the
    /// input's schema (the parameter's source)
    pub td_param_src: FxHashMap<ObjId, crate::domain::SourceId>,
    /// `@functools.wraps(fn)` wrappers: fn
    pub wrapped: FxHashMap<ObjId, Value>,
    /// classes decorated with functools.total_ordering
    pub total_ordering: FxHashSet<ObjId>,
    /// `vars(obj)` dicts: stores into them are stores of obj's attributes
    pub attr_mirrors: FxHashMap<ObjId, ObjId>,
    /// lazy iterators (zip / islice / map …) over one-shot iterators: what consuming them
    /// takes from each source (Some(n): n items, None: all)
    pub lazy_src: FxHashMap<ObjId, Vec<(ObjId, Option<usize>)>>,
    /// some module imported torch: tensor semantics for `@`, tensor methods on unknowns, …
    pub torch_on: bool,
    /// torch itself is imported (numpy alone turns the tensor model on too, but keeps exact
    /// literal arithmetic)
    pub torch_real: bool,
    pub series: ObjId,
}

/// A partial's `func` / `args` / `keywords`.
pub fn partial_attr(an: &mut Analyzer, oid: ObjId, name: &str, node: &crate::frontend::Node, state: &mut State) -> Value {
    let Payload::Partial(f, args, kw) = an.heap.obj(oid).payload.clone() else { unreachable!() };
match name {
    "func" => f,
    "args" => {
        let tid = an.alloc_at(Kind::Tuple, node, state, "partial.args", Payload::None, None, Ns::Item, None, Some(&format!("pargs{}", oid)));
        an.set_positions(tid, args, state);
        Value::obj(tid)
    }
    _ => {
        let did = an.alloc_at(Kind::Dict, node, state, "partial.keywords", Payload::None, Some(Row::new()), Ns::Item, None, Some(&format!("pkw{}", oid)));
        let mut row = Row::new();
        for (k, v) in kw.iter() {
            let kk = key_str_lit(&intern::str_get(*k));
            row = row.store(kk, true);
            state.set_slot(did, Ns::Item, kk, v.clone(), true);
        }
        if let Some(st2) = an.m.partial_star.get(&oid).cloned() {
            for sk in st2 {
                an.copy_into(did, Ns::Item, &sk, state, false, &KeySet::new());
                row = state.row(did, Ns::Item).join(&row);
            }
        }
        state.set_row(did, Ns::Item, row);
        Value::obj(did)
    }
}
}

/// An attribute of a model object (pydantic FieldInfo / errors / TypeAdapter, FastAPI objects,
/// paths, stdlib objects, csv readers): None for any other object.
pub fn model_object_attr(an: &mut Analyzer, oid: ObjId, name: &str, node: &crate::frontend::Node, state: &mut State) -> Option<Value> {
    let nk = key_str_lit(name);
    let q = match &an.heap.obj(oid).payload {
        Payload::Method(q, _) => q.clone(),
        _ => String::new(),
    };
    let k = an.heap.kind(oid);
    Some(if k == Kind::Namespace && q.starts_with("fieldinfo.") {
        pydantic::fieldinfo_attr(an, state, &q, name, node)
    } else if k == Kind::Namespace && pathtext::is_path(an, oid) && pathtext::PATH_PROPS.contains(&name) {
        pathtext::path_attr(an, state, oid, name, node).unwrap_or_else(|| Value::data(name))
    } else if k == Kind::Namespace && q.starts_with("stdobj.") {
        match state.slot_opt(oid, Ns::Attr, nk) {
            Some(v) => v,
            None => an.method_value(&format!("{}.{}", q, name), &Value::obj(oid)),
        }
    } else if matches!(k, Kind::List | Kind::View) && an.m.csv_readers.contains_key(&oid) && matches!(name, "fieldnames" | "line_num" | "dialect" | "restkey" | "restval") {
        // a csv reader's fieldnames / line_num
        if name == "fieldnames" { an.m.csv_readers[&oid].clone() } else { Value::data(name) }
    } else if k == Kind::Namespace && q.starts_with("pydverr.") {
        an.method_value(&format!("{}.{}", q, name), &Value::obj(oid))
    } else if k == Kind::Namespace && (q.starts_with("typeadapter.") || q.starts_with("fastapi.")) {
        match state.slot_opt(oid, Ns::Attr, nk) {
            Some(v) => v,
            None => match fastapi::object_attr(an, oid, name, node, state) {
                Some(v) => v,
                None => an.method_value(&format!("{}.{}", q, name), &Value::obj(oid)),
            },
        }
    } else {
        return None;
    })
}

/// A NamedTuple class's `_fields` / `_field_defaults`, a TypedDict class's
/// `__required_keys__` / `__optional_keys__` / `__total__`: None for other names or classes.
pub fn record_class_attr(an: &mut Analyzer, oid: ObjId, name: &str, node: &crate::frontend::Node, state: &mut State) -> Option<Value> {
    if an.class_lookup(oid, name).is_some() {
        return None;
    }
    if matches!(name, "_fields" | "_field_defaults") && crate::models::dataclasses::is_named_tuple_class(an, oid) {
        return Some({
            let fs = crate::models::dataclasses::dataclass_fields_mro(an, oid);
        if name == "_fields" {
            let vs: Vec<Value> = fs.iter().map(|(f, _)| Value::lit(key_str_lit(f))).collect();
            let kv = KeyVal::lits(vs.iter().filter_map(|v| v.kv.as_lit()).collect());
            an.alloc_positional(Kind::Tuple, node, Some(vs), Value::new(), kv, state)
        } else {
            let mut names = Vec::new();
            let mut vals = Vec::new();
            for (f, d) in fs.iter() {
                if let Some(d) = d {
                    names.push(f.clone());
                    vals.push(an.eval(d, state));
                }
            }
            crate::models::names_dict(an, state, node, &names, Some(vals), "default")
        }
        });
    }
    if matches!(name, "__required_keys__" | "__optional_keys__" | "__total__") && an.heap.class_info_opt(oid).is_some_and(|ci| ci.lib.typed_dict) {
        return Some(match crate::models::dataclasses::typed_dict_keys(an, oid) {
            Some((req, opt)) if name != "__total__" => {
                let ks: Vec<Value> = intern::sorted_by_str(if name == "__required_keys__" { req } else { opt }.iter()).into_iter().map(Value::lit).collect();
                let o = an.alloc_simple(Kind::Set, node, state, &format!("{}@L{}", name, node.line));
                an.set_members(o, KeySet::from_iter(ks.iter().filter_map(|v| v.kv.as_lit())), state);
                Value::obj(o)
            }
            Some(_) => {
                let total = an.heap.class_info_opt(oid).and_then(|ci| ci.node.arguments.as_ref().and_then(|a| a.keywords.iter().find(|k| k.arg.as_ref().map(|x| x.as_str()) == Some("total")).map(|k| !matches!(&k.value, ast::Expr::BooleanLiteral(b) if !b.value)))).unwrap_or(true);
                Value::lit(intern::key_bool(total))
            }
            None => Value::data("typeddict"),
        });
    }
    None
}
