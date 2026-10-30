//! The library models: what calls of the standard library, pandas, pydantic, FastAPI, PyTorch
//! and numpy do to the values they are given. A model is a function `fn(&mut Analyzer, &mut
//! State, &Call) -> Value` (`Model`), found by the callee's qualified name in the `Registry`
//! (exact names, then prefix fallbacks; library functions known to have no effect are
//! `noeffect`). Models see the `Heap`, their own side tables (`Tables`) and the interpreter's
//! `pub` methods (`interp/api.rs`); a call without a model is an unknown call.
//!
//! This file holds the registry, the module constants (`sys.maxsize`, `string.digits`, …),
//! `os.environ`, the side tables and the attributes of model objects; `README.md` in this
//! folder maps the other files.

pub mod argparse;
pub mod builtins;
pub mod chrono;
pub mod concurrency;
pub mod configparser;
pub mod containers;
pub mod counter;
pub mod csvio;
pub mod dataclasses;
pub mod enums;
pub mod exceptions;
pub mod fastapi;
pub mod itertools;
pub mod json;
pub mod minire;
pub mod pandas;
pub mod pathtext;
pub mod pure;
pub mod pydantic;
pub mod pynum;
pub mod pyre;
pub mod pytext;
pub mod pyvalues;
pub mod remodel;
pub mod shape;
pub mod singledispatch;
pub mod sqlite;
pub mod stdfuncs;
pub mod stdlib;
pub mod tables;
pub mod torch;

use std::rc::Rc;

use indexmap::IndexMap;
use ruff_python_ast as ast;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::domain::{join_values, kv_join, KeyVal, Ns, ObjId, Row, RowData, Value, DATA};
use crate::intern::{self, key_str_lit, s, KeyId, KeySet, StrId, STAR};
use crate::interp::iter::Elem;
use crate::interp::{Analyzer, Args, Call, Payload, ViewKind};
use crate::source::{FnKind, RenameFn, Source, SourceKind};
use crate::state::{Kind, State};

use builtins::*;
use containers::*;
use json::*;
use stdlib::*;

/// A library function's model: what a call of it gives, and what it does to the state.
pub type Model = fn(&mut Analyzer, &mut State, &Call) -> Value;

/// What made a live iterator (`Tables::live_iters`): the call, and the model to run it again
/// on its inputs as they are now (None: `builtins::refill_live`'s own way).
pub struct LiveIter {
    pub call: Call,
    pub rerun: Option<Model>,
}

/// Library functions known to have no effect on what they are given (`noeffect`), by prefix.
pub const ALLOWLIST_PREFIXES: &[&str] = &[
    "logging.", "print", "warnings.", "time.", "datetime.", "re.", "math.", "random.", "os.", "shutil.", "glob.", "subprocess.", "io.",
    "tempfile.", "csv.writer", "csv.reader", "sys.", "traceback.", "inspect.", "importlib.", "platform.", "socket.", "ssl.", "signal.",
    "threading.", "multiprocessing.", "concurrent.", "asyncio.", "queue.", "heapq.", "bisect.", "array.", "zlib.", "gzip.", "bz2.", "lzma.",
    "zipfile.", "tarfile.", "hmac.", "secrets.", "binascii.", "codecs.", "unicodedata.", "locale.", "gettext.", "calendar.", "difflib.",
    "pprint.pp", "textwrap.", "typing.", "pathlib.", "uuid.", "hashlib.", "base64.", "string.", "itertools.count", "numpy.", "np.", "pd.api",
    "pytest.", "unittest.", "abc.", "enum.", "decimal.", "fractions.", "statistics.", "operator.", "struct.", "builtins.NotImplementedError",
];

/// Prefixes whose models are registered as `PREFIX*` (a method family: `str.*`, `DataFrame.*`).
const PREFIXES: &[&str] = &[
    "Series.str.", "Series.dt.", "Series.cat.", "Series.sparse.", "Series.list.", "Series.struct.", "Series.", "str.", "DataFrame.", "list.", "set.", "tuple.", "dict.", "view.", "groupby.", "zip.", "enumerate.", "unknown.", "class.", "pair.",
    "parser.", "num.", "dcinit.", "statistics.", "cfgparser.", "cfgsection.", "partial.", "response.", "pydantic.cls.", "pydantic.", "typeadapter.", "fieldinfo.", "pydverr.", "counter.", "chainmap.", "deque.", "stdobj.", "threadsub.", "slice.", "memo.", "fastapi.", "starlette.", "torch.", "tensor.", "tensorq.", "nnmodule.", "math.", "numpy.", "np.",
];

/// The models by qualified name.
pub struct Registry {
    table: FxHashMap<&'static str, Model>,
}

impl Registry {
    /// Every model, by the names it answers to.
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
        r(&["random.choice", "secrets.choice", "random.sample", "random.choices"], m_random_pick);
        r(&["reprlib.repr"], m_reprlib_repr);
        r(&["operator.setitem"], m_op_setitem);
        r(&["builtins.object.__setattr__"], m_object_setattr);
        r(&["builtins.object.__delattr__"], m_object_delattr);
        r(&["builtins.object.__new__"], m_object_new);
        r(&["builtins.object.__init__", "builtins.type.__init__"], m_object_init);
        r(&["builtins.type.__new__", "abc.ABCMeta.__new__"], m_type_new);
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
                "builtins.chr", "builtins.ord", "builtins.divmod", "builtins.pow",
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
        r(&["annotated.append", "annotated.add", "annotated.insert", "annotated.extend", "annotated.update", "annotated.pop"], m_annotated);
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
        r(&["namedtuple._asdict", "namedtuple._replace", "namedtuple.index", "namedtuple.count"], m_nt_method);
        r(&["operator.itemgetter", "operator.attrgetter"], m_getter);
        r(&["builtins.property"], m_property);
        // a coroutine of the program runs where it is awaited (`Analyzer::await_value`): by
        // `asyncio.run`, `wait_for`, `shield`, `gather`; a task at any later `await`
        r(&["asyncio.run", "asyncio.wait_for", "asyncio.shield", "asyncio.create_task", "asyncio.ensure_future"], m_await_arg);
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
        r(&["functools.update_wrapper"], singledispatch::m_update_wrapper);
        r(&["functools.lru_cache", "functools.cache", "functools.lru_cache.decorating"], singledispatch::m_lru_cache);
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

    /// The model of `qual`: an exact one, a pandas method's, a prefix family's, `noeffect` for
    /// the allowlisted libraries and harmless builtins; None (an unknown call) for the rest and
    /// for the library functions that change what they are given.
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
        allowlisted(qual).then_some(noeffect as Model)
    }

    /// Does the model of `qual` read a `*xs` of items not known itself (the others read only
    /// the arguments written: `int(*xs)` would be `int()`)? `noeffect`'s by their names, and
    /// those that look at `Call::star_args`.
    pub fn reads_stars(&self, qual: &str) -> bool {
        const READS_STARS: &[&str] = &[
            "builtins.print", "builtins.object.__init__", "builtins.exec", "builtins.eval", "operator.", "asyncio.gather", "functools.update_wrapper", "os.path.", "posixpath.", "pathlib.",
            "datetime.", "partial.", "dcinit.", "torch.", "tensor.", "tensorq.", "nnmodule.", "numpy.", "np.",
        ];
        READS_STARS.iter().any(|p| qual.starts_with(p)) || (!self.table.contains_key(qual) && allowlisted(qual))
    }
}

/// A library function `noeffect` stands for (no model of its own): by its module
/// (`ALLOWLIST_PREFIXES`), or a builtin that changes nothing it is given and runs none of the
/// program's code; not one that changes its arguments (an unknown call).
fn allowlisted(qual: &str) -> bool {
    const MUTATING: &[&str] = &["random.shuffle", "operator.setitem", "operator.delitem", "struct.pack_into", "heapq.heapreplace", "heapq.heappushpop", "bisect.insort_right"];
    if MUTATING.contains(&qual) || operator_in_place(qual) || qual.starts_with("array.") {
        return false;
    }
    ALLOWLIST_PREFIXES.iter().any(|p| qual.starts_with(p) || qual.strip_prefix("builtins.").is_some_and(|q| q.starts_with(p)))
        || matches!(qual.strip_prefix("builtins."), Some("help" | "copyright" | "credits" | "license" | "breakpoint" | "compile" | "ascii" | "bin" | "oct" | "complex" | "dir" | "classmethod" | "staticmethod" | "bytearray" | "memoryview"))
}

/// `operator.iadd` & co.: `a op= b`, which changes a list, dict, set or instance in place
/// (not `operator.index`, `is_`, `itemgetter`, `inv`).
pub fn operator_in_place(qual: &str) -> bool {
    qual.strip_prefix("operator.i").is_some_and(|r| !matches!(r, "ndex" | "s_" | "s_not" | "temgetter" | "nv" | "nvert"))
}

/// Is there a model for anything in module `name` (so following its source is not wanted)?
pub fn has_module_models(name: &str) -> bool {
    thread_local! {
        static KNOWN: std::cell::RefCell<FxHashMap<String, bool>> = std::cell::RefCell::new(FxHashMap::default());
    }
    if let Some(b) = KNOWN.with(|k| k.borrow().get(name).copied()) {
        return b;
    }
    let b = module_models_uncached(name);
    KNOWN.with(|k| k.borrow_mut().insert(name.to_string(), b));
    b
}

fn module_models_uncached(name: &str) -> bool {
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

/// The model of a library function known to change nothing it is given: an unknown result
/// (a call given the program's functions is an unknown call all the same: it may call them).
/// Library functions that look at the functions they are given (their names, text, docs,
/// signatures, types) without calling them.
const LOOKS_AT_CODE: &[&str] = &[
    "inspect.is", "inspect.getdoc", "inspect.getsourcefile", "inspect.getfile", "inspect.getmodule", "inspect.getsource", "inspect.getsourcelines",
    "inspect.getcomments", "inspect.signature", "inspect.getfullargspec",
];

/// The library functions taken to change nothing they are given that may call what they are
/// given (so also a library function or bound method that changes its receiver or arguments).
const CALLS_GIVEN: &[&str] = &["operator.", "threading.", "concurrent.", "asyncio.", "multiprocessing.", "os.walk", "os.fwalk", "shutil.", "signal.", "sys.set"];

pub fn noeffect(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    // (a library function given what it may call — the program's functions, classes, callable
    // instances; a library function or bound method that may change its receiver or arguments,
    // where it calls what it is given, `CALLS_GIVEN` — may call it: `inspect.getmembers(x, pred)`, `asyncio.to_thread(f, …)`, `os.walk(top,
    // onerror=errs.append)`, `operator.call(d.setdefault, k, v)`; typing's, abc's and enum's
    // look at classes only)
    let types_only = ["typing.", "abc.", "enum."].iter().any(|p| c.qual.starts_with(p));
    // (library code given library callables: those that call what they are given, a thread's
    // target, `operator.call`; not `sys.modules.get(m.__name__)`)
    let calls_lib = CALLS_GIVEN.iter().any(|p| c.qual.starts_with(p));
    let code = |v: &Value| {
        v.pts.iter().any(|o| match an.heap.kind(o) {
            Kind::Function | Kind::Bound | Kind::Partial => true,
            Kind::Method | Kind::External => calls_lib,
            Kind::Class => !types_only,
            Kind::Instance => an.heap.instance_class(o).is_some_and(|cl| an.class_lookup(cl, "__call__").is_some()),
            _ => false,
        })
    };
    if !c.qual.starts_with("builtins.") && !LOOKS_AT_CODE.iter().any(|p| c.qual.starts_with(p)) && c.args.iter().chain(c.kwargs.values()).chain(c.star_args.iter()).any(code) {
        return an.unknown_call(&c.qual, c.recv.as_ref(), c.arguments(), st, &c.node);
    }
    // (it may run the dunders of the program's instances it is given: `os.fspath(p)`, `logging`'s
    // `%s`; a builtin's are run by `builtin_dunder`; `inspect.isclass` & co. look at types only)
    if !["builtins.", "inspect.is", "typing.", "abc."].iter().any(|p| c.qual.starts_with(p)) {
        let vals: Vec<Value> = c.args.iter().chain(c.kwargs.values()).chain(c.star_args.iter()).chain(c.star_kwargs.iter()).cloned().collect();
        an.library_dunders(&vals, st, &c.node);
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
pub fn may_type_error(an: &mut Analyzer, qual: &str, recv: Option<&Value>, args: &[Value], kwargs: &IndexMap<StrId, Value>, r: &Value, state: &mut State) {
    if !state.reachable {
        return;
    }
    if let ("builtins.hash", [a]) = (qual, args) {
        an.check_hashable(a, state);
        return;
    }
    // `format(v, spec)` with a spec v's type rejects: None takes none (TypeError), a str no
    // number's (ValueError); a result not known may be either
    if let ("builtins.format", [v, spec]) = (qual, args) {
        let spec = spec.kv.as_lit().filter(|_| spec.pts.is_empty()).and_then(intern::key_as_str);
        if spec.as_ref().is_some_and(|t| t.is_empty()) {
            return;
        }
        let number_code = spec.as_ref().is_some_and(|t| t.ends_with(|c: char| "bcdoxXneEfFgG%".contains(c)));
        match v.kv.as_lit().filter(|_| v.pts.is_empty()).map(intern::key_get) {
            Some(intern::Key::None) => return an.raise_here("TypeError", state),
            Some(intern::Key::Str(_)) if number_code => return an.raise_here("ValueError", state),
            _ => {}
        }
        if !(r.pts.is_empty() && r.kv.is_literal()) {
            an.may_raise_here("TypeError", state);
            an.may_raise_here("ValueError", state);
        }
        return;
    }
    if sure_type_error(an, qual, args, kwargs, state) {
        an.raise_here("TypeError", state);
        return;
    }
    // (literals beside objects, `None | [1]`: the literals alone may be rejected)
    for (i, a) in args.iter().enumerate().filter(|(_, a)| !a.pts.is_empty() && a.kv.is_literal()) {
        let mut alone = args.to_vec();
        alone[i] = Value::kv_only(a.kv.clone());
        if sure_type_error(an, qual, &alone, kwargs, state) {
            an.may_raise_here("TypeError", state);
            return;
        }
    }
    match takes_types(an, qual, recv, args, kwargs, state) {
        Some(true) => return,
        Some(false) => {
            an.raise_here("TypeError", state);
            return;
        }
        None => {}
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

/// What a parameter of a str method takes.
#[derive(Clone, Copy)]
enum P {
    Str,
    Int,
    OptStr,
    OptInt,
    /// a str or a tuple of strs (`startswith`)
    Strs,
    /// a str of one character (`center`'s fill)
    Char,
}

/// A str method's positional parameters, required and optional, and its keyword ones.
fn str_params(name: &str) -> Option<(&'static [P], &'static [P], &'static [(&'static str, P)])> {
    use P::*;
    Some(match name {
        "lower" | "upper" | "title" | "casefold" | "capitalize" | "swapcase" | "isdigit" | "isalpha" | "isspace" | "isascii" | "isalnum" | "islower" | "isupper" | "istitle" | "isnumeric" | "isdecimal" | "isidentifier" | "isprintable" => (&[], &[], &[]),
        "strip" | "lstrip" | "rstrip" => (&[], &[OptStr], &[]),
        "startswith" | "endswith" => (&[Strs], &[OptInt, OptInt], &[]),
        "replace" => (&[Str, Str], &[Int], &[]),
        "split" | "rsplit" => (&[], &[OptStr, Int], &[("sep", OptStr), ("maxsplit", Int)]),
        "find" | "rfind" | "count" => (&[Str], &[OptInt, OptInt], &[]),
        "partition" | "rpartition" | "removeprefix" | "removesuffix" => (&[Str], &[], &[]),
        "splitlines" => (&[], &[Int], &[("keepends", Int)]),
        "expandtabs" => (&[], &[Int], &[("tabsize", Int)]),
        "zfill" => (&[Int], &[], &[]),
        "ljust" | "rjust" | "center" => (&[Int], &[Char], &[]),
        _ => return None,
    })
}

/// Is `v` of one of the types `tys` (or None, with `none`)? Some(true) surely, Some(false)
/// surely not (literals of other types only), None not known (objects, untyped unknowns).
fn of_types(v: &Value, tys: &[&str], none: bool) -> Option<bool> {
    if !v.pts.is_empty() {
        return None;
    }
    match &v.kv {
        KeyVal::Lit(_) | KeyVal::Lits(_) => {
            let ok = |k: KeyId| (none && intern::with_key(k, |k| matches!(k, intern::Key::None))) || tys.iter().any(|t| crate::domain::key_type(k, t));
            let ks = crate::domain::kv_literals(&v.kv);
            if ks.iter().all(ok) {
                Some(true)
            } else if !ks.iter().any(ok) {
                Some(false)
            } else {
                None
            }
        }
        KeyVal::Data(l, _) => crate::domain::label_type(&intern::str_get(*l)).filter(|t| tys.contains(t)).map(|_| true),
        _ => None,
    }
}

/// Does `v` take parameter `p`'s type (as `of_types`), the method's receiver of type `text`
/// (str or bytes)?
fn takes(an: &Analyzer, v: &Value, p: P, text: &str, st: &State) -> Option<bool> {
    const INT: &[&str] = &["int", "bool"];
    match p {
        P::Str => of_types(v, &[text], false),
        P::Int => of_types(v, INT, false),
        P::OptStr => of_types(v, &[text], true),
        P::OptInt => of_types(v, INT, true),
        P::Strs => of_types(v, &[text], false).or_else(|| (v.kv.is_bot() && !v.pts.is_empty() && v.pts.iter().all(|o| an.heap.kind(o) == Kind::Tuple && items_of_type(an, o, text, st))).then_some(true)),
        P::Char => match of_types(v, &[text], false) {
            Some(true) if v.kv.is_literal() => Some(crate::domain::kv_literals(&v.kv).iter().all(|k| intern::key_as_str(k).is_some_and(|t| t.chars().count() == 1) || intern::with_key(k, |k| matches!(k, intern::Key::Bytes(b) if b.len() == 1)))),
            Some(false) => Some(false),
            _ => None,
        },
    }
}

/// Are the items of container `o` (no iterator) all of type `ty`?
fn items_of_type(an: &Analyzer, o: ObjId, ty: &str, st: &State) -> bool {
    !an.heap.obj(o).iterator && st.slot(o, Ns::Elem, STAR).pts.is_empty() && st.elems.get(&o).is_some_and(|kv| of_types(&Value::kv_only(kv.clone()), &[ty], false) == Some(true))
}

/// Can Python iterate `v` without running code (or failing): a list, tuple, dict, set or view
/// (an iterator too with `iterators`: iterating one gives back itself), a str?
fn plain_iterable(an: &Analyzer, v: &Value, iterators: bool) -> bool {
    let objs = v.pts.iter().all(|o| matches!(an.heap.kind(o), Kind::List | Kind::Tuple | Kind::Dict | Kind::Set | Kind::View) && (iterators || !an.heap.obj(o).iterator));
    objs && (!v.pts.is_empty() || !v.kv.is_bot()) && (v.kv.is_bot() || of_types(&Value::kv_only(v.kv.clone()), &["str"], false) == Some(true))
}

/// A builtin or container / str method given arguments of the types it takes, whatever they
/// hold: Some(true) where it raises no TypeError (`len` of a list, `map` of the program's
/// function over one, `list.append` of anything, `s.endswith(".egg")`), Some(false) where it
/// surely does (`s.endswith(1)`), None where that is not known here.
fn takes_types(an: &Analyzer, qual: &str, recv: Option<&Value>, args: &[Value], kwargs: &IndexMap<StrId, Value>, st: &State) -> Option<bool> {
    let fits = |b: bool| b.then_some(true);
    // (the program's code, which records its own raises where it runs, as where it is called)
    let code = |v: &Value| v.kv.is_bot() && !v.pts.is_empty() && v.pts.iter().all(|o| matches!(an.heap.kind(o), Kind::Function | Kind::Bound | Kind::Class));
    let none = |v: &Value| v.pts.is_empty() && v.kv.as_lit().is_some_and(|k| intern::with_key(k, |k| matches!(k, intern::Key::None)));
    if let Some(name) = qual.strip_prefix("str.") {
        // (bytes' methods are str's: the receiver's type is the text's)
        let text = recv.and_then(|r| an.value_type(r)).filter(|t| matches!(*t, "str" | "bytes"))?;
        if name == "join" {
            let [a] = args else { return None };
            let strs = of_types(a, &[text], false) == Some(true) || (a.kv.is_bot() && !a.pts.is_empty() && a.pts.iter().all(|o| matches!(an.heap.kind(o), Kind::List | Kind::Tuple | Kind::Set) && items_of_type(an, o, text, st)));
            return fits(kwargs.is_empty() && strs);
        }
        let (req, opt, kw) = str_params(name)?;
        if args.len() < req.len() || args.len() > req.len() + opt.len() {
            return None;
        }
        let mut all = true;
        let pos = args.iter().zip(req.iter().chain(opt.iter()));
        let named = kwargs.iter().map(|(k, v)| kw.iter().find(|(n, _)| s(n) == *k).map(|(_, p)| (v, *p)));
        for t in pos.map(|(v, p)| Some((v, *p))).chain(named) {
            match t.and_then(|(v, p)| takes(an, v, p, text, st)) {
                Some(true) => {}
                Some(false) => return Some(false),
                None => all = false,
            }
        }
        return fits(all);
    }
    if !kwargs.is_empty() && !matches!(qual, "builtins.enumerate") {
        return None;
    }
    let start_ok = || kwargs.iter().all(|(k, v)| *k == s("start") && takes(an, v, P::Int, "", st) == Some(true));
    match (qual, args) {
        ("builtins.len", [a]) => fits(plain_iterable(an, a, false)),
        ("builtins.iter", [a]) => fits(plain_iterable(an, a, true)),
        ("builtins.list" | "builtins.tuple", [a]) => fits(plain_iterable(an, a, false)),
        ("builtins.list" | "builtins.tuple", []) => Some(true),
        ("builtins.enumerate", [a]) => fits(plain_iterable(an, a, true) && start_ok()),
        ("builtins.enumerate", [a, n]) => fits(plain_iterable(an, a, true) && kwargs.is_empty() && takes(an, n, P::Int, "", st) == Some(true)),
        ("builtins.zip", xs) => fits(xs.iter().all(|x| plain_iterable(an, x, true))),
        ("builtins.map", [f, xs @ ..]) if !xs.is_empty() => fits(code(f) && xs.iter().all(|x| plain_iterable(an, x, true))),
        ("builtins.filter", [f, x]) => fits((code(f) || none(f)) && plain_iterable(an, x, true)),
        ("list.append", [_]) => Some(true),
        ("list.extend", [a]) => fits(plain_iterable(an, a, false)),
        ("list.insert", [i, _]) => fits(takes(an, i, P::Int, "", st) == Some(true)),
        _ => None,
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
    // (a builtin iterating an argument that is a number or None: `enumerate(3)`, `dict(None)`)
    let iterated: &[Value] = match qual.strip_prefix("builtins.") {
        Some("enumerate" | "reversed" | "dict" | "set" | "frozenset" | "list" | "tuple" | "sorted" | "sum" | "any" | "all") => &args[..args.len().min(1)],
        Some("min" | "max") if args.len() == 1 => args,
        Some("zip") => args,
        Some("map" | "filter") => args.get(1..).unwrap_or_default(),
        _ => &[],
    };
    if iterated.iter().any(|a| matches!(lit_kind(a), Some(0 | 3))) {
        return true;
    }
    // (`min(a, b)`: arguments of kinds that do not compare, or None)
    if matches!(qual, "builtins.min" | "builtins.max") && args.len() >= 2 && kwargs.is_empty() {
        let ks: Vec<Option<u8>> = args.iter().map(lit_kind).collect();
        return ks.contains(&Some(3)) || ks.iter().flatten().any(|k| ks.iter().flatten().any(|j| j != k));
    }
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
/// The builtins and container methods that run dunders of what they are given or hold that no
/// model of theirs runs as CPython does (how many times, which: `sorted` compares items, `sum`
/// adds them, `str([x])` runs `x.__repr__`, `d.get(x)` hashes x, `xs.count(x)` compares x,
/// `list(x)` iterates x): the dunders of their arguments themselves (`own`) and of the instances
/// their containers hold, at any depth through containers (`items`).
const COMPARE: &[&str] = &["__eq__", "__ne__", "__lt__", "__le__", "__gt__", "__ge__"];
const PRINT: &[&str] = &["__repr__", "__str__", "__format__"];
const HASH: &[&str] = &["__hash__", "__eq__"];
const ADD: &[&str] = &["__add__", "__radd__", "__iadd__"];
const COPY: &[&str] = &["__copy__", "__deepcopy__", "__reduce__", "__reduce_ex__", "__getstate__", "__setstate__", "__getattr__", "__getattribute__", "__setattr__"];

/// (the dunders of the arguments themselves, those of the instances their containers hold)
fn driven(qual: &str) -> Option<(&'static [&'static str], &'static [&'static str])> {
    // (iterating an instance runs its `__iter__` / `__getitem__` where the models iterate it,
    // `Analyzer::iterate`: here only what they do not, a size hint's `__len__`)
    const HINT: &[&str] = &["__len__", "__length_hint__"];
    Some(match qual.strip_prefix("builtins.").unwrap_or(qual) {
        "sorted" | "list.sort" => (HINT, COMPARE),
        "max" | "min" => (&[], COMPARE),
        "sum" => (&[], ADD),
        "list" | "tuple" => (HINT, &[]),
        "reversed" => (&["__reversed__", "__len__", "__getitem__"], &[]),
        "any" | "all" => (&[], &["__bool__", "__len__"]),
        "set" | "frozenset" | "dict" => (&[], HASH),
        "str" | "repr" | "print" | "format" => (&[], PRINT),
        "hash" => (&[], HASH),
        "copy.copy" | "copy.deepcopy" => (&[], COPY),
        "list.count" | "list.index" | "list.remove" | "list.__contains__" | "tuple.count" | "tuple.index" => (&[], COMPARE),
        "dict.get" | "dict.setdefault" | "dict.pop" | "dict.__contains__" | "dict.fromkeys" | "dict.update" | "set.add" | "set.discard" | "set.remove" | "set.update" | "set.__contains__" => (&[], HASH),
        _ => return None,
    })
}

/// A call of a builtin or container method that runs dunders no model runs (`driven`): those of
/// the program's instances it is given (and its receiver), and those their containers hold.
pub fn driven_dunders(an: &mut Analyzer, qual: &str, recv: Option<&Value>, args: &[Value], state: &mut State, node: &crate::frontend::Node) {
    let Some((own, items)) = driven(qual) else { return };
    let vals: Vec<Value> = recv.into_iter().chain(args.iter()).cloned().collect();
    let top: Vec<ObjId> = vals.iter().flat_map(|v| v.pts.iter()).filter(|o| an.heap.kind(*o) == Kind::Instance).collect();
    if !own.is_empty() && !top.is_empty() {
        an.instance_dunders(top, own, state, node);
    }
    if !items.is_empty() {
        let held = an.container_instances(&vals, state);
        if !held.is_empty() {
            an.instance_dunders(held, items, state, node);
        }
    }
}

pub fn builtin_dunder(an: &mut Analyzer, qual: &str, a: Args, state: &mut State, node: &crate::frontend::Node) -> Option<Value> {
    let Args { pos: args, kw: kwargs, star: star_args, star_kw: star_kwargs } = a;
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
        an.call_external(qual, Args::new(&a, kwargs, &[], &[]), state, node, None)
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
            Some(an.call_external(qual, Args::new(&texts, kwargs, &[], &[]), state, node, None))
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
                "builtins.bin" | "builtins.oct" | "builtins.hex" | "builtins.complex" => an.call_external(qual, Args::new(&[r], kwargs, &[], &[]), state, node, None),
                "builtins.bytes" | "builtins.bytearray" => {
                    an.iterate(&r, node, state);
                    Value::data(&format!("L{}", node.line))
                }
                // (of the builtin's type whatever the dunder gave: CPython raises otherwise)
                _ => match qual {
                    "builtins.len" | "builtins.hash" | "builtins.int" => of_type(an, r, "int"),
                    "builtins.str" | "builtins.repr" | "builtins.ascii" => of_type(an, r, "str"),
                    "builtins.float" => of_type(an, r, "float"),
                    _ => r,
                },
            };
            let tail = rest_call(an, rest, state);
            Some(r.join(&tail))
        }
        _ => None,
    }
}

/// What a builtin gives of `r`, what a dunder returned, when CPython makes sure it is of type
/// `ty` (raising otherwise): its unknowns (scalar or objects not known) an unknown of that type.
fn of_type(an: &Analyzer, r: Value, ty: &'static str) -> Value {
    let unknown = |o: ObjId| an.heap.kind(o) == Kind::Unknown;
    let mut out = Value::with_kv(r.pts.filter(|o| !unknown(o)), crate::domain::kv_of_type(&r.kv, ty));
    if r.pts.iter().any(unknown) {
        out = out.join(&Value::data(&crate::domain::typed_label(ty, "")));
    }
    out
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
        // (what it holds is text: `os.environ["HOME"]` is a str, as if annotated)
        if !an.heap.ann.contains_key(&oid) {
            if let Some(a) = crate::frontend::parse_expression(if qual == "os.environ" { "dict[str, str]" } else { "dict[bytes, bytes]" }) {
                let m = an.main_module();
                an.heap.ann.insert(oid, (a, m));
            }
        }
        return Some(oid);
    }
    None
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
    /// partials being called (`m_partial_call`): one built over itself (`cb = partial(cb, i)`
    /// in a loop) called again from within is unknown code there
    pub partial_calling: Vec<ObjId>,
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
    /// a validation called with `from_attributes=True` (or `from_orm`) is running
    pub pyd_from_attrs: bool,
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
    pub td_param_src: FxHashMap<ObjId, crate::source::SourceId>,
    /// `@functools.wraps(fn)` wrappers: fn
    pub wrapped: FxHashMap<ObjId, Value>,
    /// classes decorated with functools.total_ordering
    pub total_ordering: FxHashSet<ObjId>,
    /// lazy iterators (zip / islice / map …) over one-shot iterators: what consuming them
    /// takes from each source (Some(n): n items, None: all)
    pub lazy_src: FxHashMap<ObjId, Vec<(ObjId, Option<usize>)>>,
    /// one-shot iterators over containers (lists, dicts, sets, deques), reading them as they are
    /// at each step (`iter(xs)`, `reversed`, `map`, `filter`, `islice`, `chain`, `zip` & co.):
    /// the call that made each (`Analyzer::sync_live`: where the containers' facts changed since,
    /// by the hashes in its `\u{1}srcfp` slot, it takes its items again, `refill_live` or the
    /// model again; a reversed list's from its first index)
    pub live_iters: FxHashMap<ObjId, Rc<LiveIter>>,
    /// some module imported torch: tensor semantics for `@`, tensor methods on unknowns, …
    pub torch_on: bool,
    /// torch itself is imported (numpy alone turns the tensor model on too, but keeps exact
    /// literal arithmetic)
    pub torch_real: bool,
    pub series: ObjId,
}

impl Tables {
    /// Recent objects fold into their summaries (`map`, recent → summary: `gc.rs`
    /// `rename_recent`): what the tables say of a recent object holds for its summary too (a
    /// `functools.wraps` wrapper made in a loop is one on every round, a Counter one, a
    /// ChainMap's maps, a `vars()` dict's instance), and what they hold refers to the summaries
    /// (the recent objects are other objects once made again). An entry the summary has already
    /// stays (one of the objects it stands for; values joined where they are values).
    pub fn rename_recent(&mut self, map: &FxHashMap<ObjId, ObjId>) {
        let rv = |v: &Value| crate::interp::gc::rename_value(v, map).unwrap_or_else(|| v.clone());
        let ro = |o: &ObjId| map.get(o).copied().unwrap_or(*o);
        let rvs = |vs: &mut Vec<Value>| vs.iter_mut().for_each(|v| *v = rv(v));
        for (r, s) in map {
            for t in [&mut self.wrapped, &mut self.dd_factories, &mut self.csv_readers] {
                if let Some(v) = t.get(r).cloned() {
                    let j = t.get(s).map_or(v.clone(), |w| w.join(&v));
                    t.insert(*s, j);
                }
            }
            fn copy<V: Clone>(t: &mut FxHashMap<ObjId, V>, r: &ObjId, s: &ObjId) {
                if let Some(v) = t.get(r).cloned() {
                    t.entry(*s).or_insert(v);
                }
            }
            copy(&mut self.live_iters, r, s);
            copy(&mut self.chainmaps, r, s);
            copy(&mut self.deques, r, s);
            copy(&mut self.partial_star, r, s);
            copy(&mut self.sorted_items, r, s);
            copy(&mut self.lazy_src, r, s);
            copy(&mut self.exit_callbacks, r, s);
            copy(&mut self.sdispatch, r, s);
            copy(&mut self.sql_cursors, r, s);
            copy(&mut self.sql_rows, r, s);
            copy(&mut self.re_matches, r, s);
            copy(&mut self.re_patterns, r, s);
            copy(&mut self.csv_writers, r, s);
            copy(&mut self.cfg_parsers, r, s);
            copy(&mut self.cfg_sections, r, s);
            copy(&mut self.infinites, r, s);
            copy(&mut self.exc_objs, r, s);
            copy(&mut self.fa_app_deps, r, s);
            for t in [&mut self.counters, &mut self.ci_dicts, &mut self.zip_strict, &mut self.cookie_jars, &mut self.np_struct] {
                if t.contains(r) {
                    t.insert(*s);
                }
            }
        }
        for t in [&mut self.wrapped, &mut self.dd_factories, &mut self.csv_readers] {
            t.values_mut().for_each(|v| *v = rv(v));
        }
        for t in [&mut self.chainmaps, &mut self.partial_star, &mut self.fa_app_deps] {
            t.values_mut().for_each(&rvs);
        }
        self.sql_cursors.values_mut().for_each(|o| *o = ro(o));
        self.lazy_src.values_mut().flatten().for_each(|(o, _)| *o = ro(o));
        for items in self.sorted_items.values_mut() {
            if items.iter().any(|(_, v, _)| crate::interp::gc::rename_value(v, map).is_some()) {
                *items = Rc::new(items.iter().map(|(k, v, b)| (*k, rv(v), *b)).collect());
            }
        }
        for (f, args, kw) in self.exit_callbacks.values_mut().flatten() {
            *f = rv(f);
            rvs(args);
            kw.values_mut().for_each(|v| *v = rv(v));
        }
        for (v, f) in self.sdispatch.values_mut().flatten() {
            *v = rv(v);
            *f = ro(f);
        }
    }
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
