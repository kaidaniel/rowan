//! Abstract interpreter over the Python AST (SPECIFICATION.md §5–§10).  Port of
//! `rowan/interp.py`; the statement/expression/narrowing/access/call parts live in the
//! sibling files of this module.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use indexmap::IndexMap;
use ruff_python_ast as ast;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::domain::{
    Chain, Classification, KeyVal, KeysOf, Ns, ObjId, Row, Source, SourceKind, Tier, Value,
};
use crate::frontend::{self, Node};
use crate::intern::{self, key_str_lit, s, KeyId, KeySet, StrId};
use crate::models::Registry;
use crate::scopes::{self as scopemod, Scopes};
use crate::state::{Kind, State};
use crate::summary::{Summary, SymReq, Event, PH};

pub mod access;
pub mod calls;
pub mod exprs;
pub mod funcs;
pub mod narrow;
pub mod stmts;

pub const UNROLL_MAX: usize = 16;
pub const LOOP_MAX_ITER: usize = 40;
pub const REC_MAX_ITER: usize = 8;
pub const PATH_DEPTH: u32 = 3;
pub const MAX_ACTIVE: usize = 4;
pub const MAX_MUTATED: usize = 24;
pub const MAX_CLONES: usize = 256;
/// Milliseconds after which the analysis degrades gracefully: no new tabulations (summaries
/// are used instead) and no new clones (allocation sites collapse onto their context-free
/// object). The per-function caps above stay as hard limits. `ROWAN_LIMITS=ms=N`.
pub const MAX_MS: usize = 2000;

/// `ROWAN_ON=name,...` switches experimental refinements on (`sigsum`: summaries specialised
/// by the classes of their arguments).
pub fn enabled(name: &str) -> bool {
    thread_local! {
        static ON: std::collections::HashSet<String> = std::env::var("ROWAN_ON").map(|v| v.split(',').map(|s| s.trim().to_string()).collect()).unwrap_or_default();
    }
    ON.with(|s| s.contains(name))
}

/// `ROWAN_NO=name,...` also switches named engine refinements off (bisecting).
pub fn disabled(name: &str) -> bool {
    thread_local! {
        static OFF: std::collections::HashSet<String> = std::env::var("ROWAN_NO").map(|v| v.split(',').map(|s| s.trim().to_string()).collect()).unwrap_or_default();
    }
    OFF.with(|s| s.contains(name))
}

/// Tunable limits: `ROWAN_LIMITS=tab=24,call=32,clones=8,chain=2,path=3,active=4` overrides
/// the defaults (for bisecting precision / time trade-offs).
pub fn limit(name: &str, default: usize) -> usize {
    thread_local! {
        static LIMITS: std::collections::HashMap<String, usize> = {
            let mut m = std::collections::HashMap::new();
            if let Ok(v) = std::env::var("ROWAN_LIMITS") {
                for part in v.split(',') {
                    if let Some((k, n)) = part.split_once('=') {
                        if let Ok(n) = n.trim().parse::<usize>() {
                            m.insert(k.trim().to_string(), n);
                        }
                    }
                }
            }
            m
        };
    }
    LIMITS.with(|m| m.get(name).copied().unwrap_or(default))
}

pub fn debug_enabled() -> bool {
    match Ok::<u8, ()>(debug_level()) {
        Ok(v) => v > 0,
        Err(_) => false,
    }
}

pub const SAFE_METHODS: &[&str] = &["dict.pop", "list.pop", 
    "dict.get", "dict.keys", "dict.values", "dict.items", "dict.setdefault", "dict.copy", "list.append", "str.upper", "str.lower",
    "str.strip", "str.title", "str.format", "str.join",
];

/// Builtin container / str methods that raise only on arguments of the wrong type (not on
/// the values): their calls are not "may raise anything".
pub fn safe_method(q: &str) -> bool {
    if SAFE_METHODS.contains(&q) {
        return true;
    }
    let Some((recv, name)) = q.split_once('.') else { return false };
    match recv {
        "str" => !matches!(name, "index" | "rindex" | "format" | "format_map" | "encode" | "decode"),
        "list" => matches!(name, "append" | "extend" | "insert" | "sort" | "reverse" | "copy" | "count" | "clear"),
        "dict" => matches!(name, "get" | "keys" | "values" | "items" | "setdefault" | "copy" | "update" | "clear" | "fromkeys"),
        "set" | "frozenset" => matches!(name, "add" | "update" | "union" | "intersection" | "difference" | "symmetric_difference" | "copy" | "clear" | "discard" | "issubset" | "issuperset" | "isdisjoint" | "intersection_update" | "difference_update" | "symmetric_difference_update"),
        "tuple" => name == "count",
        _ => false,
    }
}
pub const STR_METHOD_NAMES: &[&str] = &[
    "strip", "lower", "upper", "title", "split", "replace", "startswith", "endswith", "format", "join", "lstrip", "rstrip", "encode",
    "decode", "isdigit", "isalpha", "zfill", "casefold", "capitalize", "partition", "rsplit", "splitlines", "find", "count", "ljust",
    "rjust", "center", "rpartition", "rfind", "rindex", "isascii", "isalnum", "isspace", "islower", "isupper", "istitle", "isnumeric",
    "isdecimal", "isidentifier", "isprintable", "removeprefix", "removesuffix", "swapcase", "expandtabs", "format_map", "translate",
];
pub const SCALAR_EXTRA_NAMES: &[&str] = &[
    "append", "extend", "sort", "index", "insert", "remove", "isoformat", "strftime", "timestamp", "date", "time",
    "total_seconds", "to_bytes", "bit_length", "as_integer_ratio", "is_integer", "hex", "conjugate", "real", "imag", "tolist",
    "item", "copy", "clear", "reverse",
];
pub fn is_scalar_method(name: &str) -> bool {
    STR_METHOD_NAMES.contains(&name) || SCALAR_EXTRA_NAMES.contains(&name)
}
pub const NOEFFECT_METHOD_NAMES: &[&str] = &[
    "info", "debug", "warning", "warn", "error", "critical", "exception", "log", "write", "flush", "close", "print", "assert_called",
    "assert_called_with",
];
pub const DICT_METHOD_NAMES: &[&str] = &["get", "items", "keys", "values", "update", "pop", "setdefault", "copy", "popitem", "clear"];
/// Methods that make a class-less placeholder a list (modelled by `list.*`).
pub const LIST_METHOD_NAMES: &[&str] = &["append", "extend", "insert", "remove", "sort", "reverse"];
pub const VALUE_GUARDS: &[&str] = &["ValueError", "Exception", "BaseException", "*"];
pub const ITEM_GUARDS: &[&str] = &["KeyError", "LookupError", "Exception", "BaseException", "IndexError", "*"];
pub const ATTR_GUARDS: &[&str] = &["AttributeError", "Exception", "BaseException", "*"];
pub const KNOWN_IDENTITY_DECORATORS: &[&str] = &[
    "functools.lru_cache", "functools.cache", "functools.wraps", "contextlib.contextmanager", "abc.abstractmethod", "typing.overload",
    "typing.final", "builtins.staticmethod", "builtins.classmethod", "pytest.fixture", "asyncio.coroutine", "dataclasses.dataclass",
    "typing.no_type_check", "functools.total_ordering", "typing.runtime_checkable",
    // pydantic registers these (models::pydantic reads them from the class body)
    "pydantic.field_validator", "pydantic.model_validator", "pydantic.validator", "pydantic.root_validator", "pydantic.field_serializer",
    "pydantic.model_serializer", "pydantic.computed_field", "pydantic.functional_validators.field_validator", "pydantic.dataclasses.dataclass",
];

// ---------------------------------------------------------------------------
// records produced for the reporter
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Verdict {
    pub line: u32,
    pub col: u32,
    pub key: KeyId,
    pub tier: Tier,
    pub obj: String,
    /// call sites (lines) active when the access ran
    pub chain: Chain,
    pub note: String,
    pub providers: Vec<String>,
    pub ns: Ns,
    pub text: String,
    pub oid: ObjId,
    /// module the access is in (empty = main file)
    pub file: String,
    /// inside a try whose handler catches the failure: the program handles it (no diagnostic)
    pub guarded: bool,
}

impl Verdict {
    pub fn dedupe_key(&self) -> (String, u32, u32, String, Tier, ObjId, Chain) {
        (self.file.clone(), self.line, self.col, intern::key_repr(self.key), self.tier, self.oid, self.chain.clone())
    }
}

#[derive(Clone, Copy, Debug)]
pub enum FuncNode {
    Def(&'static ast::StmtFunctionDef),
    Lambda(&'static ast::ExprLambda),
}

impl FuncNode {
    pub fn params(&self) -> Option<&'static ast::Parameters> {
        match self {
            FuncNode::Def(f) => Some(&f.parameters),
            FuncNode::Lambda(l) => l.parameters.as_deref(),
        }
    }
    pub fn node(&self) -> Node {
        match self {
            FuncNode::Def(f) => frontend::node(*f),
            FuncNode::Lambda(l) => frontend::node(*l),
        }
    }
    pub fn is_lambda(&self) -> bool {
        matches!(self, FuncNode::Lambda(_))
    }
    pub fn addr(&self) -> usize {
        match self {
            FuncNode::Def(f) => *f as *const _ as usize,
            FuncNode::Lambda(l) => *l as *const _ as usize,
        }
    }
    pub fn body(&self) -> &'static [ast::Stmt] {
        match self {
            FuncNode::Def(f) => &f.body,
            FuncNode::Lambda(_) => &[],
        }
    }
}

#[derive(Debug)]
pub struct FuncInfo {
    pub node: FuncNode,
    pub name: String,
    pub qualname: String,
    pub module: ObjId,
    pub scope: usize,
    /// free name -> cell oid
    pub closure: IndexMap<StrId, ObjId>,
    pub defaults: IndexMap<StrId, Value>,
    /// defining class oid
    pub cls: Option<ObjId>,
    pub is_property: Cell<bool>,
    pub is_static: Cell<bool>,
    pub is_classmethod: Cell<bool>,
    /// decorated with `contextlib.contextmanager`
    pub is_contextmanager: Cell<bool>,
    /// `functools.cached_property`: the first access stores the value on the instance
    pub is_cached: Cell<bool>,
    /// `functools.lru_cache` / `cache`: repeated arguments give the cached result
    pub is_memo: Cell<bool>,
    pub generator: Cell<Option<ObjId>>,
}

impl FuncInfo {
    /// The qualified name as Python spells it (no `class ` markers): `Conn.__init__`.
    pub fn py_qualname(&self) -> String {
        self.qualname.replace("class ", "")
    }
}

impl FuncInfo {
    pub fn clone_with_closure(&self, closure: IndexMap<StrId, ObjId>) -> FuncInfo {
        FuncInfo {
            node: self.node,
            name: self.name.clone(),
            qualname: self.qualname.clone(),
            module: self.module,
            scope: self.scope,
            closure,
            defaults: self.defaults.clone(),
            cls: self.cls,
            is_property: Cell::new(self.is_property.get()),
            is_static: Cell::new(self.is_static.get()),
            is_classmethod: Cell::new(self.is_classmethod.get()),
            is_contextmanager: Cell::new(self.is_contextmanager.get()),
            is_cached: Cell::new(self.is_cached.get()),
            is_memo: Cell::new(self.is_memo.get()),
            generator: Cell::new(self.generator.get()),
        }
    }
}

/// The body of a `with` whose manager is a `@contextmanager` generator: run where the
/// generator (one frame above `depth`) yields.
pub struct CmHook {
    pub body: &'static [ast::Stmt],
    pub target: Option<&'static ast::Expr>,
    pub depth: usize,
    pub env_after: Option<IndexMap<StrId, Value>>,
    pub ran: bool,
    /// instead of a body: code run with the yielded value (a FastAPI yield dependency: the rest
    /// of the request)
    pub then: Option<Box<dyn FnOnce(&mut Analyzer, Value, &mut State)>>,
    /// the body did not finish normally (it raised or returned)
    pub body_died: bool,
}

/// An exception class's name and its bases' names; `true`: that is the whole MRO.
pub type ExcNames = (Vec<String>, bool);
pub type RaiseRec = (Option<ExcNames>, State, Value);

/// Builtin (and modelled library) exception classes and their bases.
pub const EXC_BASES: &[(&str, &str)] = &[
    ("Exception", "BaseException"), ("SystemExit", "BaseException"), ("KeyboardInterrupt", "BaseException"), ("GeneratorExit", "BaseException"),
    ("BaseExceptionGroup", "BaseException"),
    ("ArithmeticError", "Exception"), ("ZeroDivisionError", "ArithmeticError"), ("OverflowError", "ArithmeticError"), ("FloatingPointError", "ArithmeticError"),
    ("LookupError", "Exception"), ("KeyError", "LookupError"), ("IndexError", "LookupError"),
    ("ValueError", "Exception"), ("UnicodeError", "ValueError"), ("UnicodeDecodeError", "UnicodeError"), ("UnicodeEncodeError", "UnicodeError"),
    ("UnicodeTranslateError", "UnicodeError"),
    ("TypeError", "Exception"), ("AttributeError", "Exception"), ("NameError", "Exception"), ("UnboundLocalError", "NameError"),
    ("RuntimeError", "Exception"), ("NotImplementedError", "RuntimeError"), ("RecursionError", "RuntimeError"), ("AssertionError", "Exception"),
    ("ImportError", "Exception"), ("ModuleNotFoundError", "ImportError"), ("EOFError", "Exception"), ("MemoryError", "Exception"),
    ("StopIteration", "Exception"), ("StopAsyncIteration", "Exception"), ("SyntaxError", "Exception"), ("IndentationError", "SyntaxError"),
    ("SystemError", "Exception"), ("BufferError", "Exception"), ("ReferenceError", "Exception"), ("Warning", "Exception"),
    ("OSError", "Exception"), ("IOError", "Exception"), ("EnvironmentError", "Exception"),
    ("FileNotFoundError", "OSError"), ("PermissionError", "OSError"), ("TimeoutError", "OSError"), ("ConnectionError", "OSError"),
    ("BlockingIOError", "OSError"), ("ChildProcessError", "OSError"), ("FileExistsError", "OSError"), ("InterruptedError", "OSError"),
    ("IsADirectoryError", "OSError"), ("NotADirectoryError", "OSError"), ("ProcessLookupError", "OSError"),
    ("BrokenPipeError", "ConnectionError"), ("ConnectionAbortedError", "ConnectionError"), ("ConnectionRefusedError", "ConnectionError"),
    ("ConnectionResetError", "ConnectionError"),
    // (library exceptions the models raise)
    ("JSONDecodeError", "ValueError"), ("ValidationError", "ValueError"), ("FrozenInstanceError", "AttributeError"),
];

/// The names of a builtin exception class and its bases (`IOError` is `OSError`).
pub fn builtin_exc_names(name: &str) -> ExcNames {
    let mut n = name.rsplit('.').next().unwrap_or(name).to_string();
    let mut out: Vec<String> = Vec::new();
    loop {
        if n == "IOError" || n == "EnvironmentError" || n == "OSError" {
            out.extend(["OSError", "IOError", "EnvironmentError"].iter().map(|s| s.to_string()));
            n = "Exception".into();
            continue;
        }
        out.push(n.clone());
        if n == "BaseException" {
            return (out, true);
        }
        match EXC_BASES.iter().find(|(c, _)| *c == n) {
            Some((_, b)) => n = b.to_string(),
            None => return (out, false),
        }
    }
}

#[derive(Debug)]
pub struct ClassInfo {
    pub name: String,
    pub node: &'static ast::StmtClassDef,
    pub bases: Vec<ObjId>,
    pub module: ObjId,
    /// inherits from something we do not see
    pub open_base: bool,
    pub dataclass_fields: RefCell<Option<Vec<(String, Option<&'static ast::Expr>)>>>,
    pub typed_dict: bool,
    pub named_tuple: bool,
    /// subclasses `dict` directly
    pub dict_base: bool,
    /// subclasses pydantic's `BaseModel`: annotated fields like a dataclass, validated
    /// (models::pydantic)
    pub pydantic: bool,
    /// decorated with `pydantic.dataclasses.dataclass`: validated like a model, no model API
    pub pyd_dataclass: std::cell::Cell<bool>,
    /// `@dataclass(frozen=True)` / `@dataclass(kw_only=True)`
    pub dc_frozen: std::cell::Cell<bool>,
    pub dc_kw_only: std::cell::Cell<bool>,
    /// `@dataclass(order=True)`: instances of the class compare their fields as tuples
    pub dc_order: std::cell::Cell<bool>,
    /// subclasses something in `torch.nn` (`nn.Module`): calling an instance runs `forward`
    pub nn_module: bool,
    /// attr → classes assigned to self.attr (lazy)
    pub field_classes: RefCell<Option<FxHashMap<String, Vec<ObjId>>>>,
    pub any_fields: RefCell<Option<FxHashSet<String>>>,
}

impl ClassInfo {
    pub fn has_fields(&self) -> bool {
        self.dataclass_fields.borrow().is_some()
    }
    pub fn fields(&self) -> Vec<(String, Option<&'static ast::Expr>)> {
        self.dataclass_fields.borrow().clone().unwrap_or_default()
    }
}

#[derive(Debug)]
pub struct ModuleInfo {
    pub name: String,
    pub path: Option<String>,
    /// the dotted package the module belongs to (its own name for an `__init__.py`), the
    /// base of its relative imports
    pub pkg: String,
    pub idx: u32,
    pub tree: &'static ast::ModModule,
    pub scopes: Scopes,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FrameKind {
    Module,
    Function,
    Class,
}

#[derive(Debug)]
pub struct Frame {
    pub kind: FrameKind,
    pub scope: usize,
    pub module: ObjId,
    pub ctx: String,
    pub single: bool,
    /// `single` ignoring loops: with the recency abstraction an allocation inside a loop is
    /// a strong "recent" object when this holds
    pub single_nl: bool,
    pub func: Option<ObjId>,
    pub cells: IndexMap<StrId, ObjId>,
    pub class_oid: Option<ObjId>,
    pub overlays: Vec<IndexMap<StrId, Value>>,
    pub loop_depth: u32,
    pub label: String,
    pub symbolic: bool,
    /// indices of the enclosing unrolled loop iterations (part of the call context)
    pub unroll: Vec<usize>,
}

impl Frame {
    pub fn new(kind: FrameKind, scope: usize, module: ObjId, ctx: &str, single: bool, label: &str) -> Frame {
        Frame {
            kind,
            scope,
            module,
            ctx: ctx.to_string(),
            single,
            single_nl: single,
            func: None,
            cells: IndexMap::new(),
            class_oid: None,
            overlays: Vec::new(),
            loop_depth: 0,
            label: label.to_string(),
            symbolic: false,
            unroll: Vec::new(),
        }
    }
}

#[derive(Default)]
pub struct LoopCtx {
    pub breaks: Option<State>,
    pub continues: Option<State>,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct EnumKind {
    pub mixed: bool,
    pub repr_value: bool,
    pub flag: bool,
    /// str-based (`auto()` is the lower-cased name in a StrEnum)
    pub is_str: bool,
}

/// Everything a model needs about a call.
#[derive(Clone)]
pub struct Call {
    pub qual: String,
    pub recv: Option<Value>,
    pub args: Vec<Value>,
    pub kwargs: IndexMap<StrId, Value>,
    pub star_args: Vec<Value>,
    pub star_kwargs: Vec<Value>,
    pub node: Node,
    pub call_ast: Option<&'static ast::ExprCall>,
    pub site: String,
    pub line: u32,
}

impl Call {
    pub fn arg(&self, i: usize, name: Option<&str>) -> Option<Value> {
        if i < self.args.len() {
            return Some(self.args[i].clone());
        }
        if let Some(n) = name {
            if let Some(v) = self.kwargs.get(&s(n)) {
                return Some(v.clone());
            }
        }
        None
    }
    pub fn kw(&self, name: &str) -> Option<Value> {
        self.kwargs.get(&s(name)).cloned()
    }
    pub fn arg_node(&self, i: usize, name: Option<&str>) -> Option<&'static ast::Expr> {
        let c = self.call_ast?;
        if i < c.arguments.args.len() && !matches!(c.arguments.args[i], ast::Expr::Starred(_)) {
            return Some(&c.arguments.args[i]);
        }
        if let Some(n) = name {
            for kw in c.arguments.keywords.iter() {
                if kw.arg.as_ref().map(|a| a.id.as_str()) == Some(n) {
                    return Some(&kw.value);
                }
            }
        }
        None
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ViewKind {
    Keys,
    Values,
    Items,
    Rows,
    Elems,
    /// `obj.__dict__`: the instance's attributes as a dict
    Attrs,
    /// `types.MappingProxyType(d)`: a read-only live view of the dict
    Proxy,
}

impl ViewKind {
    pub fn name(self) -> &'static str {
        match self {
            ViewKind::Keys => "keys",
            ViewKind::Values => "values",
            ViewKind::Items => "items",
            ViewKind::Rows => "rows",
            ViewKind::Elems => "elems",
            ViewKind::Attrs => "attrs",
            ViewKind::Proxy => "proxy",
        }
    }
    pub fn from_name(n: &str) -> ViewKind {
        match n {
            "keys" => ViewKind::Keys,
            "values" => ViewKind::Values,
            "items" => ViewKind::Items,
            "rows" => ViewKind::Rows,
            _ => ViewKind::Elems,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Payload {
    None,
    Module(Rc<ModuleInfo>),
    Func(Rc<FuncInfo>),
    Class(Rc<ClassInfo>),
    Ph(Rc<PH>),
    /// instance: its class
    Cls(ObjId),
    /// external qualname / response tag
    Str(String),
    Method(String, Value),
    Bound(ObjId, Value),
    Super(ObjId, Value),
    View(ViewKind, Value),
    Zip(Value, Value),
    Enumerate(Value, Option<i64>),
    Pair(Value, Value),
    Partial(Value, Vec<Value>, IndexMap<StrId, Value>),
    Groupby(Vec<ObjId>, Option<KeySet>, bool, Option<(KeySet, bool)>),
    Locidx(String, ObjId),
    /// `range(start, stop, step)` with known bounds
    Range(i64, i64, i64),
}

impl Payload {
    pub fn is_none(&self) -> bool {
        matches!(self, Payload::None)
    }
}

#[derive(Debug)]
pub struct Obj {
    pub id: ObjId,
    pub kind: Kind,
    pub site: String,
    pub ctx: String,
    pub single: bool,
    pub label: String,
    pub payload: Payload,
    /// containers built from a literal: positional slots are valid
    pub literal_len: Option<usize>,
    /// after appends of unknown count: positions below this are still valid
    pub known_prefix: Option<usize>,
    /// summary object standing for "the current element" of a loop
    pub iter_elem: bool,
    /// record this object is a view of (virtual row -> frame)
    pub derived_from: Option<ObjId>,
    /// for a `dict(x)` / `copy` of an entry object inside a summary: that object (its
    /// unknown content stands for the copy's as long as the copy did not overwrite it)
    pub copied_from: Option<ObjId>,
    /// access-path depth under an open source (placeholders, elements)
    pub depth: u32,
    /// a one-shot iterator (generator, generator expression, `iter(...)`): a full iteration
    /// exhausts it
    pub iterator: bool,
}

/// Where a narrowed value is written back (`_narrow_isinstance`'s `store` closure).
#[derive(Clone)]
pub enum StoreTarget {
    Name(StrId),
    Narrowed(StrId),
    Slot(ObjId, Ns, KeyId),
}

/// Engine options, all on by default; `ROWAN_NO=recency,scc,site,narrowed,positional`
/// switches one off (for bisecting a change in results).
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// resolve a placeholder to allocation sites with one representative row per distinct
    /// entry row, instead of per object
    pub site_resolution: bool,
    /// `State::same` compares the path-sensitive `narrowed` facts too
    pub narrowed_same: bool,
    /// re-iterate summaries computed against a partial summary of a function still being
    /// summarised (proper SCC iteration for mutual recursion)
    pub scc: bool,
    /// recency abstraction for objects allocated in loops
    pub recency: bool,
    /// positional unpacking of literal tuple keys
    pub positional: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { site_resolution: true, narrowed_same: true, scc: true, recency: true, positional: true }
    }
}

impl Options {
    pub fn from_env() -> Self {
        let mut o = Options::default();
        if let Ok(v) = std::env::var("ROWAN_NO") {
            for name in v.split(',') {
                match name.trim() {
                    "site" | "site_resolution" => o.site_resolution = false,
                    "narrowed" | "narrowed_same" => o.narrowed_same = false,
                    "scc" => o.scc = false,
                    "recency" => o.recency = false,
                    "positional" => o.positional = false,
                    _ => {}
                }
            }
        }
        o
    }
}

/// Facts about one name at one line of the main module, joined over every execution of
/// the statement (hover support): the value it held and the rows of the objects it pointed to.
#[derive(Clone)]
pub struct Fact {
    /// recorded inside a summary body (rows are relative to the function's entry)
    pub symbolic: bool,
    pub value: Value,
    /// object -> (item row, attr row) as they were when the name was read / written
    pub rows: IndexMap<ObjId, (Option<Row>, Option<Row>)>,
    /// the value rendered (`report::render_value`) at each recording, distinct texts
    pub texts: Vec<String>,
    /// the symbols the texts mention, with what is known about each (`report::value_legend`)
    pub legend: Vec<(String, String)>,
    /// display names of the tensor dims the texts mention (`B` for x.shape[0])
    pub t_names: FxHashMap<crate::shape::AtomId, String>,
    /// the text from the function's own analysis (open parameters), when there is one: the
    /// hover leads with it; `texts` then holds what calls gave
    pub primary: Option<String>,
    /// recorded by an assignment (whose value the hover shows, not the reads on the line)
    pub written: bool,
}

pub struct RowCheck {
    /// the module's path relative to the main file's directory ("" for the main file)
    pub file: String,
    pub line: u32,
    pub name: String,
    pub row: Option<Row>,
    pub ns: Ns,
    /// the name's value: its known literals, whether it may be anything else, its objects
    pub value: ValueInfo,
    /// tensor facts for the golden harness: {"shape", "dim", "chain", "entry"}
    pub tensor: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default)]
pub struct ValueInfo {
    pub lits: Vec<KeyId>,
    pub unknown: bool,
    pub objects: Vec<String>,
}

pub struct Analyzer {
    pub filename: String,
    pub use_summaries: bool,
    pub summary_table: FxHashMap<crate::summary::SummaryKey, Rc<Summary>>,
    /// argument-class signatures seen per function (capped, see `MAX_SIGNATURES`)
    pub sig_count: FxHashMap<ObjId, Vec<Vec<(String, ObjId)>>>,
    /// summary key variants (code cells, typed cells, signature) seen per function: each gets
    /// its own allocation context
    pub key_variants: FxHashMap<ObjId, Vec<(Vec<(ObjId, Vec<ObjId>)>, Vec<(ObjId, ObjId)>, Vec<(String, ObjId)>)>>,
    pub tab_count: FxHashMap<(ObjId, &'static str), usize>,
    pub module_done: bool,
    pub fresh_init: FxHashSet<ObjId>,
    pub cell_classes: FxHashMap<ObjId, ObjId>,
    pub sum_stack: Vec<ObjId>,
    /// allocation context of each summary on `sum_stack` (per argument signature)
    pub sum_ctxs: Vec<String>,
    /// per comprehension generator level: whether its items are known (unrolled)
    pub comp_known: Vec<bool>,
    pub sum_reqs: Vec<IndexMap<crate::summary::SymKey, SymReq>>,
    pub sum_events: Vec<IndexMap<(&'static str, ObjId), Event>>,
    pub sum_hit: FxHashSet<ObjId>,
    pub sum_partial: FxHashMap<ObjId, Rc<Summary>>,
    pub sum_obj_mark: Vec<ObjId>,
    /// per summary on the stack: functions further down the stack whose *partial* summary
    /// this computation used (the result is then provisional, re-done per round of theirs)
    pub sum_deps: Vec<FxHashSet<ObjId>>,
    /// recency abstraction: per loop iteration being analysed (innermost last), the recent
    /// objects allocated in it: (recent oid, kind, site, ctx)
    pub recent_stack: Vec<Vec<(ObjId, Kind, String, String)>>,
    pub summary_provisional: FxHashMap<crate::summary::SummaryKey, (Rc<Summary>, FxHashSet<ObjId>)>,
    pub model_call: u32,
    pub search_paths: Vec<String>,
    /// installed packages to follow into site-packages (top-level names; `*` for all)
    pub follow: Vec<String>,
    /// site-packages directories searched for followed packages
    pub site_paths: Vec<String>,
    pub analyse_entry_points: bool,
    pub objects: Vec<Obj>,
    pub obj_index: FxHashMap<(Kind, String, String), ObjId>,
    pub verdicts: Vec<Verdict>,
    pub verdict_keys: FxHashSet<(String, u32, u32, String, Tier, ObjId, Chain)>,
    pub notes: Vec<String>,
    pub notes_set: FxHashSet<String>,
    pub summaries: IndexMap<(String, String), (Value, Vec<(ObjId, Row)>)>,
    pub models: Registry,
    pub frames: Vec<Frame>,
    pub active: FxHashMap<(ObjId, String), bool>,
    pub active_by_func: FxHashMap<ObjId, Vec<(ObjId, String)>>,
    pub rec_table: FxHashMap<(ObjId, String), (Value, State, crate::state::Footprint)>,
    pub rec_hit: FxHashSet<(ObjId, String)>,
    pub rec_entry: FxHashMap<(ObjId, String), (Option<Value>, Vec<Value>, IndexMap<StrId, Value>)>,
    pub rec_widened: FxHashSet<(ObjId, String)>,
    pub loop_stack: Vec<LoopCtx>,
    pub guards: Vec<FxHashSet<String>>,
    pub may_raise: Vec<bool>,
    /// per enclosing try (or swallowing with): the joined state at the guarded accesses
    /// that may raise inside it (the handler's entry, precisely)
    pub raise_states: Vec<Option<State>>,
    /// per enclosing try (parallel to `raise_states`): the frame depth and caller-env depth
    /// it runs in and the state before it, so a raise inside a callee hands the handler the
    /// try frame's locals
    pub try_marks: Vec<(usize, usize, State)>,
    /// summary depth at each try (a raise inside a callee's summary is not this try's)
    pub try_sum_depth: Vec<usize>,
    /// per enclosing try (parallel to `raise_states`): the exceptions raised at those points
    /// (what the handler's `as` name holds)
    pub raise_values: Vec<Option<Value>>,
    /// the exception the next `record_raise_state` is about
    pub cur_exc: Option<Value>,
    /// exception names handled by the running `except` clauses (innermost last): what a bare
    /// `raise` re-raises
    pub handling: Vec<FxHashSet<String>>,
    /// the exception values the running handlers hold (a bare `raise` re-raises them)
    pub handling_values: Vec<Value>,
    /// inside a call whose exceptions a thread / future keeps: reads still require their keys
    pub catching_depth: u32,
    /// the right operand of a comparison link already evaluated (chained comparisons)
    pub cmp_right: Option<Value>,
    /// per running comprehension: the cells of its variables (closed over by functions made
    /// inside it, which see the variable's latest value)
    pub comp_cells: Vec<FxHashMap<StrId, ObjId>>,
    /// (canonical key, key as written) while an access with a numerically-equal key runs
    pub key_alias: Option<(KeyId, KeyId)>,
    /// a `with` over a `@contextmanager` call: its body runs at the generator's yield
    pub cm_hook: Option<CmHook>,
    /// while a `@contextmanager` body runs: (generator frame depth, its locals), so a try in
    /// the generator around the yield sees its own names when the body raises
    /// `@contextmanager` generators whose with body is running: (their frame depth, their env
    /// at the yield, the env stack's length then)
    pub cm_frames: Vec<(usize, IndexMap<StrId, Value>, usize)>,
    /// the next user call is analysed concretely (no summary)
    pub force_tab_once: bool,
    /// `vars(obj)` dicts: stores into them are stores of obj's attributes
    pub attr_mirrors: FxHashMap<ObjId, ObjId>,
    /// results of unknown calls: (the call as written, its line, what it was given — which
    /// unknown code may return)
    pub origins: FxHashMap<ObjId, (String, u32, Vec<String>)>,
    /// unknown objects named in the program's own terms (`a`, `a(b)`, `cfg["db"]`,
    /// `rows[*]`, `json.loads(text)`): printed instead of `?`
    pub symbols: FxHashMap<ObjId, String>,
    /// the function whose names a symbol uses (0: module level); it is printed only there
    pub symbol_scope: FxHashMap<ObjId, u32>,
    /// while an entry run's parameters are made: the function they belong to
    pub entry_scope: Option<u32>,
    /// some module imported torch: tensor semantics for `@`, tensor methods on unknowns, …
    pub torch_on: bool,
    /// torch itself is imported (numpy alone turns the tensor model on too, but keeps exact
    /// literal arithmetic)
    pub torch_real: bool,
    /// model calls running (`run_model`), the outermost one's number, and the objects
    /// allocated in it
    pub model_depth: u32,
    /// validating a FastAPI test request's body (errors are a 422)
    pub fa_body_validation: bool,
    pub model_epoch: u64,
    pub model_born: FxHashMap<ObjId, u64>,
    /// pandas is imported: frame-style accesses on placeholders are decided per call site
    pub pandas_on: bool,
    /// modules that import torch (their functions all get entry runs and are tabulated)
    pub torch_modules: FxHashSet<ObjId>,
    /// the function an entry run (open parameters) is analysing
    pub entry_run: Option<ObjId>,
    /// functions that had their entry run
    pub entry_done: FxHashSet<ObjId>,
    /// unknown call results named by the variable they were bound to (tensor dims)
    pub t_var_names: FxHashMap<ObjId, crate::shape::Sym>,
    /// closures made inside concrete calls in torch modules (no entry run of their own)
    pub t_concrete_closures: FxHashSet<ObjId>,
    /// the state a factory's entry run ended in, for the closures it made
    pub t_closure_states: FxHashMap<ObjId, std::rc::Rc<State>>,
    /// function objects by qualname (a closure's printing sees its factory's symbols)
    pub t_funcs_by_qual: FxHashMap<String, Vec<ObjId>>,
    /// golden rows: the joined value of (line, name, call chain, entry run) so far
    pub t_row_values: FxHashMap<(u32, String, Vec<u32>, bool), Value>,
    /// the next hover fact is an assignment's
    pub t_fact_write: bool,
    /// running a call that stands for "some module": no shape errors / requirements / facts
    pub t_quiet: bool,
    /// reading an attribute without the tensor model / without the user-class check
    pub t_getattr_plain: bool,
    pub t_skip_user: bool,
    /// the node of the function expression of the call being evaluated
    pub t_called_attr: usize,
    /// the verdict being recorded is on a guarded access
    pub verdict_guarded: bool,
    /// tensor-shape requirements / definite mismatches (models/torch.rs)
    pub shape_reqs: Vec<crate::models::torch::ShapeNote>,
    pub shape_errors: Vec<crate::models::torch::ShapeNote>,
    /// (class, symbol of an `__init__` value) → `self.attr` holding it
    pub t_sym_alias: FxHashMap<(ObjId, crate::shape::Sym), String>,
    pub t_dim_alias: FxHashMap<(ObjId, crate::shape::DimId), String>,
    /// Enum classes: their members in definition order
    pub enum_members: FxHashMap<ObjId, Vec<(String, ObjId)>>,
    /// Enum classes: (mixes in str / int: equal to its value, `str()` is the value (ReprEnum:
    /// StrEnum, IntEnum, IntFlag), a Flag)
    pub enum_kind: FxHashMap<ObjId, EnumKind>,
    /// members and Flag combinations ↔ their keys (`Key::Member`)
    pub member_key: FxHashMap<ObjId, intern::KeyId>,
    pub key_member: FxHashMap<intern::KeyId, ObjId>,
    /// dicts with case-insensitive str keys (request headers): keys are lower-cased
    pub ci_dicts: FxHashSet<ObjId>,
    /// csv writers (their file and dialect), csv readers (their fieldnames)
    pub csv_writers: FxHashMap<ObjId, crate::models::csvio::CsvWriter>,
    pub csv_readers: FxHashMap<ObjId, Value>,
    /// argparse parsers, their subparsers and groups
    pub argp: FxHashMap<ObjId, crate::models::argparse::ArgObj>,
    /// singledispatch functions: (type, implementation) in registration order; the methods
    pub sdispatch: FxHashMap<ObjId, Vec<(Value, ObjId)>>,
    pub sdispatch_method: FxHashSet<ObjId>,
    pub sd_bypass: Option<ObjId>,
    /// futures: the exception classes their call raised (empty: any)
    pub future_excs: FxHashMap<ObjId, Vec<String>>,
    /// identity-hashed objects used as dict keys ↔ their keys
    pub identity_keys: RefCell<FxHashMap<ObjId, KeyId>>,
    /// builtin exception objects: their class names
    pub exc_objs: FxHashMap<ObjId, String>,
    /// the StringIO objects `redirect_stdout` sends prints to (innermost last)
    pub stdout_redirect: Vec<Value>,
    /// ExitStack callbacks: (callable, positional, keyword arguments), in registration order
    pub exit_callbacks: FxHashMap<ObjId, Vec<(Value, Vec<Value>, IndexMap<StrId, Value>)>>,
    /// TestClient cookie jars (dicts with `set`)
    pub cookie_jars: FxHashSet<ObjId>,
    /// entry parameters annotated with a TypedDict: the declared row's reads are still the
    /// input's schema (the parameter's source)
    pub td_param_src: FxHashMap<ObjId, crate::domain::SourceId>,
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
    /// functions whose summary met a connection from outside (left alone there): a summary
    /// used without tabulating leaves those databases' rows unknown
    pub sql_touched: FxHashMap<ObjId, FxHashSet<ObjId>>,
    pub sql_cursors: FxHashMap<ObjId, ObjId>,
    pub sql_rows: FxHashMap<ObjId, Vec<String>>,
    /// itertools.count / cycle / repeat(x) objects
    pub infinites: FxHashMap<ObjId, crate::models::itertools::Infinite>,
    /// partials' bound `**mapping`s of unknown keys
    pub partial_star: FxHashMap<ObjId, Vec<Value>>,
    /// `@functools.wraps(fn)` wrappers: fn
    pub wrapped: FxHashMap<ObjId, Value>,
    /// classes decorated with functools.total_ordering
    pub total_ordering: FxHashSet<ObjId>,
    /// the requirements known when a try context was first flagged as raising anywhere
    /// (`may_raise` depth, requirements): its handler needs no less
    pub flag_reqs: Vec<(usize, crate::layered::TwoLevel<crate::intern::SourceId, KeyId, crate::domain::ReqInfo>)>,
    pub identity_objs: RefCell<FxHashMap<KeyId, ObjId>>,
    /// inside `object.__setattr__`: a plain store even when the class defines `__setattr__`
    pub bypass_setattr: bool,
    pub cond_depth: i32,
    /// like `cond_depth` without loops: a requirement recorded under an `if`/`try`/boolean
    /// operator is "sometimes"; one inside a loop body is per element (always)
    pub branch_depth: i32,
    pub call_chain: Vec<u32>,
    pub analysed_funcs: FxHashSet<ObjId>,
    pub modules: IndexMap<String, ObjId>,
    pub exh_log: Option<Vec<(ObjId, Ns, Row, KeySet, intern::LensId)>>,
    pub final_state: Option<State>,
    pub user_classes: Vec<ObjId>,
    /// class attributes (flow-insensitive)
    pub cls_slots: FxHashMap<(ObjId, Ns, KeyId), Value>,
    pub site_clones: FxHashMap<String, usize>,
    /// statements executed plus objects instantiated (debug output)
    pub work: usize,
    /// return / break / continue statements executed (a try body that ends with one did not raise)
    pub exit_count: usize,
    /// when the analysis started (see `MAX_MS`)
    pub started: std::time::Instant,
    pub call_cache: FxHashMap<calls::Sig, Rc<calls::CacheHit>>,
    pub entry_instances: FxHashMap<ObjId, ObjId>,
    pub fresh_in_progress: FxHashSet<ObjId>,
    pub main_module: ObjId,
    pub main_tree: &'static ast::ModModule,
    pub series: ObjId,
    pub none_v: Value,
    /// `#@row NAME:` checks requested by the golden harness: (file relative to the main
    /// file's directory, "" for the main file; line; name)
    pub row_check_requests: Vec<(String, u32, String)>,
    /// module index -> its path relative to the main file's directory (`rel_file`)
    pub rel_files: std::cell::RefCell<FxHashMap<u32, String>>,
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
    /// the instance whose field is being validated (containers made for it are its own)
    pub pyd_inst: Option<ObjId>,
    /// the last value of each call node, and whether a test is being narrowed (`eval`)
    pub call_memo: FxHashMap<usize, Value>,
    /// numpy structured arrays (fields are Item keys); the exception a missing item raises
    /// when every target is one (`ValueError`, not KeyError)
    pub np_struct: FxHashSet<ObjId>,
    /// per generator object, the join of the states at its yields (a caller resumes from one
    /// of them when the body never ends)
    pub gen_yield_states: FxHashMap<ObjId, State>,
    /// a copy's original (through copies made at the same node: `out = deepcopy(out)` in a loop
    /// makes one copy site, not a new one per round)
    pub copy_root: FxHashMap<ObjId, ObjId>,
    /// raises recorded by an enclosing try (`record_raise_state`), counted
    pub raise_records: u64,
    /// the raises recorded for each running try (the depth of `raise_states` when it began):
    /// the exception's class names (None: not known), the state, the exception value
    pub raise_cls: Vec<(usize, Vec<RaiseRec>)>,
    /// the class of the exception being recorded (set beside `cur_exc`)
    pub cur_exc_cls: Option<ExcNames>,
    /// the call being evaluated: where an exception a model raises is allocated
    pub exc_node: Option<Node>,
    pub item_exc: Option<&'static str>,
    pub narrow_depth: u32,
    pub pyd_last_failed: bool,
    /// pydantic instances: the fields their input surely / maybe gave (`model_fields_set`)
    pub pyd_set: FxHashMap<ObjId, (KeySet, KeySet)>,
    /// pydantic instances validated from placeholders: those inputs
    pub pyd_inputs: FxHashMap<ObjId, Vec<ObjId>>,
    /// specialisations of generic models: their type variables' bindings
    pub pyd_generic: FxHashMap<ObjId, Vec<(String, crate::models::pydantic::Ann)>>,
    /// a requirement-only entry run (`param F.x` schemas of a called function): no verdicts,
    /// row checks, facts or notes
    pub quiet: bool,
    /// no record-access verdicts (shapes still reported): the own run of a called function
    /// of a torch / numpy module
    pub quiet_records: bool,
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
    /// ChainMaps: the maps they look through (the first takes the writes)
    pub chainmaps: FxHashMap<ObjId, Vec<Value>>,
    /// deques and their maxlen
    pub deques: FxHashMap<ObjId, Option<i64>>,
    /// the generated `UserDict` classes (their instances' keys are `self.data`'s)
    pub userdict_classes: FxHashSet<ObjId>,
    /// per module: global names a function body mutates (`mutated_in_functions`)
    pub mutated_globals: std::cell::RefCell<FxHashMap<u32, FxHashSet<String>>>,
    /// per summary being built: its row-check observations
    pub sum_obs: Vec<Vec<crate::summary::Obs>>,
    /// per summary being built: the hover facts recorded in its body (resolved per call)
    pub sum_fact_obs: Vec<Vec<crate::summary::FactObs>>,
    /// locals of the callers of the running call (innermost last), for liveness
    pub env_stack: Vec<IndexMap<StrId, Value>>,
    /// `@name.setter` functions by (defining class, attribute)
    pub prop_setters: FxHashMap<(ObjId, KeyId), ObjId>,
    /// `defaultdict(factory)`: the factory, called once per inserted key
    pub dd_factories: FxHashMap<ObjId, Value>,
    /// `self.f = p` in `__init__` with `p` a parameter: the classes of the arguments seen for
    /// `p` at construction sites, by (class, field) — the field's class when there is one
    pub ctor_field_classes: FxHashMap<(ObjId, String), FxHashSet<ObjId>>,
    /// per running call (tabulated body or summary): did it raise an uncaught exception /
    /// call unknown code?
    pub raise_stack: Vec<bool>,
    pub row_checks: Vec<RowCheck>,
    /// collect `facts` (the `--facts` mode)
    pub facts_on: bool,
    pub facts: IndexMap<(u32, StrId, bool), Fact>,
    pub main_stmt_lines: FxHashSet<usize>,
    pub t0: std::time::Instant,
    pub opts: Options,
}

/// `ROWAN_DEBUG` as a number (0 when unset), read once.
pub fn debug_level() -> u8 {
    static LEVEL: std::sync::OnceLock<u8> = std::sync::OnceLock::new();
    *LEVEL.get_or_init(|| match std::env::var("ROWAN_DEBUG") {
        Ok(v) if v == "2" => 2,
        Ok(v) if !v.is_empty() && v != "0" => 1,
        _ => 0,
    })
}

pub fn site_string(n: &Node) -> String {
    format!("{}L{}:{}-{}", frontend::site_prefix(n.module), n.line, n.col, n.end_col)
}

impl Analyzer {
    pub fn new(source: String, filename: &str, search_paths: Vec<String>, analyse_entry_points: bool, follow_imports: bool, use_summaries: Option<bool>) -> Result<Analyzer, String> {
        crate::models::pynum::set_opaque(false);
        crate::models::pynum::reset_used();
        crate::models::chrono::reset_used();
        let use_summaries = use_summaries.unwrap_or_else(|| std::env::var("ROWAN_SUMMARIES").map(|v| v != "0").unwrap_or(true));
        let (idx, tree) = frontend::parse("__main__", Some(filename), source)?;
        frontend::set_main(idx);
        let mut an = Analyzer {
            filename: filename.to_string(),
            use_summaries,
            summary_table: FxHashMap::default(),
            sig_count: FxHashMap::default(),
            key_variants: FxHashMap::default(),
            tab_count: FxHashMap::default(),
            module_done: false,
            fresh_init: FxHashSet::default(),
            cell_classes: FxHashMap::default(),
            sum_stack: Vec::new(),
            sum_ctxs: Vec::new(),
            comp_known: Vec::new(),
            sum_reqs: Vec::new(),
            sum_events: Vec::new(),
            sum_hit: FxHashSet::default(),
            sum_partial: FxHashMap::default(),
            sum_obj_mark: Vec::new(),
            sum_deps: Vec::new(),
            recent_stack: Vec::new(),
            summary_provisional: FxHashMap::default(),
            model_call: 0,
            search_paths: if follow_imports { search_paths } else { Vec::new() },
            follow: Vec::new(),
            site_paths: Vec::new(),
            analyse_entry_points,
            objects: vec![Obj {
                id: 0,
                kind: Kind::Unknown,
                site: String::new(),
                ctx: String::new(),
                single: true,
                label: "<none>".into(),
                payload: Payload::None,
                literal_len: None,
            known_prefix: None,
                iter_elem: false,
                derived_from: None,
                copied_from: None,
                depth: 0,
                iterator: false,
            }],
            obj_index: FxHashMap::default(),
            verdicts: Vec::new(),
            verdict_keys: FxHashSet::default(),
            notes: Vec::new(),
            notes_set: FxHashSet::default(),
            summaries: IndexMap::new(),
            models: Registry::new(),
            frames: Vec::new(),
            active: FxHashMap::default(),
            active_by_func: FxHashMap::default(),
            rec_table: FxHashMap::default(),
            rec_hit: FxHashSet::default(),
            rec_entry: FxHashMap::default(),
            rec_widened: FxHashSet::default(),
            loop_stack: Vec::new(),
            guards: Vec::new(),
            may_raise: Vec::new(),
            raise_states: Vec::new(),
            try_marks: Vec::new(),
            try_sum_depth: Vec::new(),
            raise_values: Vec::new(),
            cur_exc: None,
            handling: Vec::new(),
            handling_values: Vec::new(),
            catching_depth: 0,
            cmp_right: None,
            comp_cells: Vec::new(),
            key_alias: None,
            cm_hook: None,
            cm_frames: Vec::new(),
            force_tab_once: false,
            attr_mirrors: FxHashMap::default(),
            origins: FxHashMap::default(),
            symbols: FxHashMap::default(),
            symbol_scope: FxHashMap::default(),
            entry_scope: None,
            enum_members: FxHashMap::default(),
            enum_kind: FxHashMap::default(),
            member_key: FxHashMap::default(),
            key_member: FxHashMap::default(),
            ci_dicts: FxHashSet::default(),
            csv_writers: FxHashMap::default(),
            csv_readers: FxHashMap::default(),
            argp: FxHashMap::default(),
            sdispatch: FxHashMap::default(),
            sdispatch_method: FxHashSet::default(),
            sd_bypass: None,
            future_excs: FxHashMap::default(),
            identity_keys: RefCell::new(FxHashMap::default()),
            exc_objs: FxHashMap::default(),
            stdout_redirect: Vec::new(),
            exit_callbacks: FxHashMap::default(),
            cookie_jars: FxHashSet::default(),
            td_param_src: FxHashMap::default(),
            cfg_parsers: FxHashMap::default(),
            cfg_sections: FxHashMap::default(),
            re_patterns: FxHashMap::default(),
            re_matches: FxHashMap::default(),
            path_classes: FxHashMap::default(),
            sql_dbs: FxHashMap::default(),
            sql_touched: FxHashMap::default(),
            sql_cursors: FxHashMap::default(),
            sql_rows: FxHashMap::default(),
            infinites: FxHashMap::default(),
            partial_star: FxHashMap::default(),
            wrapped: FxHashMap::default(),
            total_ordering: FxHashSet::default(),
            flag_reqs: Vec::new(),
            identity_objs: RefCell::new(FxHashMap::default()),
            bypass_setattr: false,
            cond_depth: 0,
            branch_depth: 0,
            call_chain: Vec::new(),
            analysed_funcs: FxHashSet::default(),
            modules: IndexMap::new(),
            exh_log: None,
            final_state: None,
            user_classes: Vec::new(),
            cls_slots: FxHashMap::default(),
            site_clones: FxHashMap::default(),
            work: 0,
            exit_count: 0,
            started: std::time::Instant::now(),
            call_cache: FxHashMap::default(),
            entry_instances: FxHashMap::default(),
            torch_on: false,
            torch_real: false,
            model_depth: 0,
            fa_body_validation: false,
            model_epoch: 0,
            model_born: FxHashMap::default(),
            pandas_on: false,
            torch_modules: FxHashSet::default(),
            entry_run: None,
            entry_done: FxHashSet::default(),
            t_var_names: FxHashMap::default(),
            t_concrete_closures: FxHashSet::default(),
            t_closure_states: FxHashMap::default(),
            t_funcs_by_qual: FxHashMap::default(),
            t_row_values: FxHashMap::default(),
            t_fact_write: false,
            t_quiet: false,
            t_getattr_plain: false,
            t_skip_user: false,
            t_called_attr: 0,
            verdict_guarded: false,
            shape_reqs: Vec::new(),
            shape_errors: Vec::new(),
            t_sym_alias: FxHashMap::default(),
            t_dim_alias: FxHashMap::default(),
            fresh_in_progress: FxHashSet::default(),
            main_module: 0,
            main_tree: tree,
            series: 0,
            none_v: Value::lit(intern::key_none()),
            row_check_requests: Vec::new(),
            rel_files: Default::default(),
            pyd_classes: FxHashMap::default(),
            pyd_adapters: Vec::new(),
            pyd_depth: 0,
            pyd_ann_depth: 0,
            pyd_errs: Vec::new(),
            pyd_verrs: Vec::new(),
            pyd_field: None,
            pyd_inst: None,
            call_memo: FxHashMap::default(),
            np_struct: FxHashSet::default(),
            gen_yield_states: FxHashMap::default(),
            copy_root: FxHashMap::default(),
            raise_records: 0,
            raise_cls: Vec::new(),
            cur_exc_cls: None,
            exc_node: None,
            item_exc: None,
            narrow_depth: 0,
            pyd_last_failed: false,
            pyd_set: FxHashMap::default(),
            pyd_inputs: FxHashMap::default(),
            pyd_generic: FxHashMap::default(),
            quiet: false,
            quiet_records: false,
            fa_routes: Vec::new(),
            fa_includes: Vec::new(),
            fa_prefix: FxHashMap::default(),
            fa_handlers: FxHashSet::default(),
            fa_hooks: Vec::new(),
            fa_pending: Vec::new(),
            fa_status: Value::new(),
            fa_out_resp: Vec::new(),
            fa_gen_ret: None,
            fa_dep_cache: Vec::new(),
            fa_app_deps: FxHashMap::default(),
            zip_strict: FxHashSet::default(),
            counters: FxHashSet::default(),
            chainmaps: FxHashMap::default(),
            deques: FxHashMap::default(),
            userdict_classes: FxHashSet::default(),
            mutated_globals: Default::default(),
            sum_obs: Vec::new(),
            sum_fact_obs: Vec::new(),
            env_stack: Vec::new(),
            prop_setters: FxHashMap::default(),
            dd_factories: FxHashMap::default(),
            ctor_field_classes: FxHashMap::default(),
            raise_stack: Vec::new(),
            row_checks: Vec::new(),
            facts_on: false,
            facts: IndexMap::new(),
            main_stmt_lines: FxHashSet::default(),
            t0: std::time::Instant::now(),
            opts: Options::from_env(),
        };
        an.main_module = an.make_module("__main__", Some(filename), idx, tree);
        an.series = an.alloc_global(Kind::Series, "builtin:Series", "Series", Payload::None);
        Ok(an)
    }

    // ------------------------------------------------------------------ setup
    fn make_module(&mut self, name: &str, path: Option<&str>, idx: u32, tree: &'static ast::ModModule) -> ObjId {
        let pkg = match path {
            Some(p) if name == "__main__" => package_of_file(p),
            Some(p) if p.ends_with("__init__.py") => name.to_string(),
            _ => name.rsplit_once('.').map(|(a, _)| a.to_string()).unwrap_or_default(),
        };
        let info = ModuleInfo { name: name.to_string(), path: path.map(|p| p.to_string()), pkg, idx, tree, scopes: scopemod::analyse(tree) };
        let oid = self.alloc_global(Kind::Module, &format!("module:{}", name), &format!("module {}", name), Payload::Module(Rc::new(info)));
        self.modules.insert(name.to_string(), oid);
        oid
    }

    pub fn frame(&self) -> &Frame {
        self.frames.last().expect("no frame")
    }
    pub fn frame_mut(&mut self) -> &mut Frame {
        self.frames.last_mut().expect("no frame")
    }

    pub fn obj(&self, oid: ObjId) -> &Obj {
        &self.objects[oid as usize]
    }
    pub fn obj_mut(&mut self, oid: ObjId) -> &mut Obj {
        &mut self.objects[oid as usize]
    }
    pub fn kind(&self, oid: ObjId) -> Kind {
        self.objects[oid as usize].kind
    }
    pub fn label(&self, oid: ObjId) -> String {
        self.objects[oid as usize].label.clone()
    }
    pub fn has_obj(&self, oid: ObjId) -> bool {
        (oid as usize) < self.objects.len() && oid > 0
    }

    pub fn module_info(&self, moid: ObjId) -> Rc<ModuleInfo> {
        match &self.obj(moid).payload {
            Payload::Module(m) => m.clone(),
            _ => panic!("not a module: {}", moid),
        }
    }
    pub fn func_info(&self, foid: ObjId) -> Rc<FuncInfo> {
        match &self.obj(foid).payload {
            Payload::Func(f) => f.clone(),
            _ => panic!("not a function: {}", foid),
        }
    }
    pub fn func_info_opt(&self, foid: ObjId) -> Option<Rc<FuncInfo>> {
        match &self.obj(foid).payload {
            Payload::Func(f) => Some(f.clone()),
            _ => None,
        }
    }
    pub fn class_info(&self, coid: ObjId) -> Rc<ClassInfo> {
        match &self.obj(coid).payload {
            Payload::Class(c) => c.clone(),
            _ => panic!("not a class: {} ({})", coid, self.obj(coid).label),
        }
    }
    pub fn class_info_opt(&self, coid: ObjId) -> Option<Rc<ClassInfo>> {
        if !self.has_obj(coid) {
            return None;
        }
        match &self.obj(coid).payload {
            Payload::Class(c) => Some(c.clone()),
            _ => None,
        }
    }
    /// The class of an instance object (payload), if it is a user class.
    pub fn instance_class(&self, oid: ObjId) -> Option<ObjId> {
        match &self.obj(oid).payload {
            Payload::Cls(c) => Some(*c),
            _ => None,
        }
    }
    pub fn ph_info(&self, oid: ObjId) -> Rc<PH> {
        match &self.obj(oid).payload {
            Payload::Ph(p) => p.clone(),
            _ => panic!("not a placeholder: {}", oid),
        }
    }
    pub fn ph_info_opt(&self, oid: ObjId) -> Option<Rc<PH>> {
        match &self.obj(oid).payload {
            Payload::Ph(p) => Some(p.clone()),
            _ => None,
        }
    }
    pub fn external_qual(&self, oid: ObjId) -> String {
        match &self.obj(oid).payload {
            Payload::Str(q) => q.clone(),
            _ => String::new(),
        }
    }
    pub fn is_record_like(&self, oid: ObjId) -> bool {
        self.kind(oid).is_record_like()
    }
    pub fn now(&self) -> f64 {
        self.t0.elapsed().as_secs_f64()
    }

    // ------------------------------------------------------------ allocation
    pub fn alloc_global(&mut self, kind: Kind, site: &str, label: &str, payload: Payload) -> ObjId {
        let label = if label.is_empty() { site } else { label };
        self.alloc_raw(kind, site, "", true, label, payload)
    }

    pub fn alloc_raw(&mut self, kind: Kind, site: &str, ctx: &str, single: bool, label: &str, payload: Payload) -> ObjId {
        let key = (kind, site.to_string(), ctx.to_string());
        if let Some(&oid) = self.obj_index.get(&key) {
            let o = &mut self.objects[oid as usize];
            o.single = o.single && single;
            if !payload.is_none() {
                o.payload = payload;
            }
            return oid;
        }
        let n = self.site_clones.get(site).copied().unwrap_or(0);
        if (n >= limit("clones", MAX_CLONES) || self.over_deadline()) && !ctx.is_empty() {
            // too many clones of one allocation site: collapse onto the context-free summary object
            return self.alloc_raw(kind, site, "", false, label, payload);
        }
        self.site_clones.insert(site.to_string(), n + 1);
        let oid = self.objects.len() as ObjId;
        self.objects.push(Obj {
            id: oid,
            kind,
            site: site.to_string(),
            ctx: ctx.to_string(),
            single,
            label: label.to_string(),
            payload,
            literal_len: None,
            known_prefix: None,
            iter_elem: false,
            derived_from: None,
            copied_from: None,
            depth: 0,
            iterator: site.starts_with("gen:"),
        });
        self.obj_index.insert(key, oid);
        oid
    }

    /// What models keep about an object, for its clone (a summary's template instantiated).
    pub fn copy_side_tables(&mut self, t: ObjId, nid: ObjId) {
        // (an exception object, a numpy structured array)
        if let Some(n) = self.exc_objs.get(&t).cloned() {
            self.exc_objs.insert(nid, n);
        }
        if self.np_struct.contains(&t) {
            self.np_struct.insert(nid);
        }
        if let Some(a) = self.argp.get(&t).cloned() {
            self.argp.insert(nid, a);
        }
        if let Some(w) = self.csv_writers.get(&t).cloned() {
            self.csv_writers.insert(nid, w);
        }
        if let Some(r) = self.csv_readers.get(&t).cloned() {
            self.csv_readers.insert(nid, r);
        }
        if self.ci_dicts.contains(&t) {
            self.ci_dicts.insert(nid);
        }
        if let Some(d) = self.deques.get(&t).cloned() {
            self.deques.insert(nid, d);
        }
        if self.counters.contains(&t) {
            self.counters.insert(nid);
        }
        if let Some(m) = self.chainmaps.get(&t).cloned() {
            self.chainmaps.insert(nid, m);
        }
        if let Some(p) = self.partial_star.get(&t).cloned() {
            self.partial_star.insert(nid, p);
        }
        if let Some(w) = self.wrapped.get(&t).cloned() {
            self.wrapped.insert(nid, w);
        }
        if let Some(c) = self.cfg_parsers.get(&t).cloned() {
            self.cfg_parsers.insert(nid, c);
        }
        if let Some(c) = self.cfg_sections.get(&t).cloned() {
            self.cfg_sections.insert(nid, c);
        }
        if let Some(c) = self.re_patterns.get(&t).cloned() {
            self.re_patterns.insert(nid, c);
        }
        if let Some(c) = self.re_matches.get(&t).cloned() {
            self.re_matches.insert(nid, c);
        }
        if let Some(c) = self.path_classes.get(&t).cloned() {
            self.path_classes.insert(nid, c);
        }
        if let Some(c) = self.sql_dbs.get(&t).cloned() {
            self.sql_dbs.insert(nid, c);
        }
        if let Some(c) = self.sql_cursors.get(&t).cloned() {
            self.sql_cursors.insert(nid, c);
        }
        if let Some(c) = self.sql_rows.get(&t).cloned() {
            self.sql_rows.insert(nid, c);
        }
    }

    /// The length of a list / tuple on this path (positions 0..n are its items), else None.
    pub fn llen(&self, o: ObjId, st: &State) -> Option<usize> {
        match st.lens.get(&o) {
            Some(v) => v.0,
            None => self.objects[o as usize].literal_len,
        }
    }

    /// The number of leading positions known (the length when known; after appends of unknown
    /// count, the length before them).
    pub fn prefix(&self, o: ObjId, st: &State) -> Option<usize> {
        let p = match st.lens.get(&o) {
            Some((Some(n), _)) => n,
            Some((None, p)) => p,
            None => self.objects[o as usize].literal_len.or(self.objects[o as usize].known_prefix).unwrap_or(0),
        };
        Some(p).filter(|p| *p > 0)
    }

    /// (also the object's own field: what a state not descended from the one it was made in,
    /// a summary's, reads)
    pub fn set_llen(&mut self, o: ObjId, st: &mut State, v: Option<usize>) {
        st.lens.insert(o, (v, v.unwrap_or(0)));
        self.objects[o as usize].literal_len = v;
        self.objects[o as usize].known_prefix = None;
    }

    /// The length is unknown now, its first `n` positions are still known.
    pub fn set_prefix(&mut self, o: ObjId, st: &mut State, n: usize) {
        st.lens.insert(o, (None, n));
        self.objects[o as usize].literal_len = None;
        self.objects[o as usize].known_prefix = Some(n);
    }

    pub fn site_of(&self, node: &Node) -> String {
        site_string(node)
    }

    /// Allocate an object at `site` (string) in the current frame's context.  `single`:
    /// explicit strength; `None` = the frame decides (strong outside loops; inside a loop a
    /// strong *recent* object with the recency abstraction, else the weak per-site object).
    pub fn alloc(&mut self, kind: Kind, site: &str, state: &mut State, label: &str, payload: Payload, row: Option<Row>, ns: Ns, single: Option<bool>, ctx: Option<&str>) -> ObjId {
        self.alloc_full(kind, site, state, label, payload, row, ns, single, ctx, true)
    }

    /// `alloc` for an object cloned from a template: `tsingle` is the template's strength.
    #[allow(clippy::too_many_arguments)]
    pub fn alloc_in_frame(&mut self, kind: Kind, site: &str, state: &mut State, label: &str, payload: Payload, row: Option<Row>, ns: Ns, tsingle: bool, ctx: &str, ctx_single: bool) -> ObjId {
        if !tsingle || !ctx_single {
            return self.alloc_full(kind, site, state, label, payload, row, ns, Some(false), Some(ctx), true);
        }
        self.alloc_full(kind, site, state, label, payload, row, ns, None, Some(ctx), true)
    }

    #[allow(clippy::too_many_arguments)]
    fn alloc_full(&mut self, kind: Kind, site: &str, state: &mut State, label: &str, payload: Payload, row: Option<Row>, ns: Ns, single: Option<bool>, ctx: Option<&str>, _reserved: bool) -> ObjId {
        let fr = self.frame();
        let ctx = ctx.map(|c| c.to_string()).unwrap_or_else(|| fr.ctx.clone());
        let label = if label.is_empty() { format!("{}@{}", kind.name(), site) } else { label.to_string() };
        // inside unrolled loop iterations each iteration is straight-line code: its own site
        let site_buf;
        let site: &str = if !fr.unroll.is_empty() && !fr.symbolic {
            site_buf = format!("{}#{}", site, fr.unroll.iter().map(|i| i.to_string()).collect::<Vec<_>>().join("."));
            &site_buf
        } else {
            site
        };
        let (single, recent) = match single {
            Some(x) => (x, false),
            None => {
                let strong = fr.single && fr.loop_depth == 0;
                if !strong && self.opts.recency && fr.single_nl && !self.recent_stack.is_empty() {
                    (true, true)
                } else {
                    (strong, false)
                }
            }
        };
        let oid = if recent {
            let rsite = format!("{}#recent", site);
            let oid = self.alloc_raw(kind, &rsite, &ctx, true, &label, payload);
            let top = self.recent_stack.last_mut().unwrap();
            if !top.iter().any(|(o, _, _, _)| *o == oid) {
                top.push((oid, kind, site.to_string(), ctx.clone()));
            }
            oid
        } else {
            let existed = self.obj_index.get(&(kind, site.to_string(), ctx.clone())).copied();
            let payload2 = payload.clone();
            let mut oid = self.alloc_raw(kind, site, &ctx, single, &label, payload);
            // (made earlier in the same model call: held by the model's own code)
            let model_held = self.model_depth > 0 && self.model_born.get(&oid) == Some(&self.model_epoch) && !disabled("modelborn");
            if self.model_depth > 0 {
                self.model_born.insert(oid, self.model_epoch);
            }
            if existed == Some(oid) && (self.is_live(oid, state) || model_held) {
                // allocated again while the earlier object is still reachable (a method run
                // twice on one receiver, a call repeated in a loop)
                if self.opts.recency && single && !self.recent_stack.is_empty() {
                    // inside a loop: a strong recent object, folded into the summary object
                    // at the loop's back-edge (the earlier object becomes that summary)
                    let rsite = format!("{}#recent", site);
                    let r = self.alloc_raw(kind, &rsite, &ctx, true, &label, payload2);
                    let top = self.recent_stack.last_mut().unwrap();
                    if !top.iter().any(|(o, _, _, _)| *o == r) {
                        top.push((r, kind, site.to_string(), ctx.clone()));
                    }
                    oid = r;
                } else {
                    // one summary object for both, weak (no longer one key)
                    self.obj_mut(oid).single = false;
                    if let Some(k) = self.identity_keys.borrow_mut().remove(&oid) {
                        self.identity_objs.borrow_mut().remove(&k);
                    }
                }
            }
            oid
        };
        let obj_single = self.obj(oid).single;
        // born here: its length is unknown until the code making it says
        state.lens.insert(oid, (None, 0));
        if kind.is_record() || kind == Kind::Frame {
            let fresh = row.unwrap_or_default();
            if state.has_row(oid, ns) && !obj_single {
                let j = state.row(oid, ns).join(&fresh);
                state.set_row(oid, ns, j);
            } else {
                state.set_row(oid, ns, fresh);
                if obj_single {
                    // strong re-allocation: drop stale slots of this object
                    state.drop_obj(oid);
                }
            }
        } else if kind.is_container() && obj_single {
            state.drop_obj(oid);
            state.elems.remove(&oid);
        }
        oid
    }

    /// `alloc` at an AST node.
    /// A model's `ctx` is a suffix of the frame's context: the same code run from two call
    /// contexts makes two objects.
    pub fn alloc_at(&mut self, kind: Kind, node: &Node, state: &mut State, label: &str, payload: Payload, row: Option<Row>, ns: Ns, single: Option<bool>, ctx: Option<&str>) -> ObjId {
        let site = site_string(node);
        let full = match ctx {
            Some(c) if !disabled("allocctx") => Some(format!("{}/{}", self.frame().ctx, c)),
            Some(c) => Some(c.to_string()),
            None => None,
        };
        self.alloc(kind, &site, state, label, payload, row, ns, single, full.as_deref())
    }

    /// Short form used by most models: a record-ish object at a node with a row.
    pub fn alloc_simple(&mut self, kind: Kind, node: &Node, state: &mut State, label: &str) -> ObjId {
        self.alloc_at(kind, node, state, label, Payload::None, None, Ns::Item, None, None)
    }

    pub fn unknown_source(&mut self, site: &str, kind: SourceKind, state: &mut State, label: &str, ctx: &str, _ns: Ns, path: Vec<KeyId>) -> ObjId {
        let src = intern::source(Source { site: site.into(), kind, path });
        let slabel = crate::domain::source_label(src);
        let kname = match kind {
            SourceKind::External => "external",
            SourceKind::Call => "call",
            SourceKind::Param => "param",
            SourceKind::Name => "name",
            SourceKind::Data => "data",
            SourceKind::Class => "class",
        };
        let label = if label.is_empty() { slabel.clone() } else { label.to_string() };
        let oid = self.alloc_raw(Kind::Unknown, &format!("{}:{}", kname, slabel), ctx, true, &label, Payload::None);
        for n in [Ns::Item, Ns::Attr] {
            if !state.has_row(oid, n) {
                state.set_row(oid, n, Row::with_source(src));
            }
        }
        if !self.symbols.contains_key(&oid) {
            let sym = match kind {
                SourceKind::Param => Some(label.trim_start_matches("param ").to_string()),
                SourceKind::Name => Some(label.trim_start_matches('?').to_string()),
                _ => None,
            };
            if let Some(sym) = sym.filter(|x| !x.is_empty()) {
                let scope = if kind == SourceKind::Param { self.entry_scope.unwrap_or_else(|| self.frame().func.unwrap_or(0)) } else { 0 };
                self.set_symbol(oid, sym, scope);
            }
        }
        oid
    }

    pub fn set_symbol(&mut self, oid: ObjId, sym: String, scope: u32) {
        self.symbols.insert(oid, sym);
        self.symbol_scope.insert(oid, scope);
    }

    /// The symbol an object is printed by where the printer is now (`report::in_scope`): a
    /// symbol uses the names of its function, module-level ones are visible everywhere.
    pub fn symbol_for(&self, oid: ObjId) -> Option<&String> {
        let s = self.symbol_scope.get(&oid).copied().unwrap_or(0);
        if s == 0 || s == crate::report::render_scope() {
            self.symbols.get(&oid)
        } else {
            None
        }
    }

    /// The symbol of `parent`'s content under (ns, key): `d["k"]`, `o.x`, `rows[*]`, a cell's
    /// variable; None when the parent has no symbol.
    pub fn sym_access(&self, parent: ObjId, ns: Ns, key: KeyId) -> Option<String> {
        if self.kind(parent) == Kind::Cell {
            return self.obj(parent).label.strip_prefix("cell ").map(|x| x.to_string());
        }
        let base = self.symbols.get(&parent)?;
        Some(match ns {
            Ns::Attr => format!("{}.{}", base, intern::key_py_str(key)),
            Ns::Cell => base.clone(),
            _ if key == intern::STAR => format!("{}[*]", base),
            _ if crate::interp::access::param_slot_of(key).is_some() => format!("{}[?]", base),
            _ => format!("{}[{}]", base, crate::domain::key_display(key)),
        })
    }

    // ------------------------------------------------------------ reporting
    pub fn file_of_frame(&self) -> String {
        let m = self.frame().module;
        if m == self.main_module {
            return String::new();
        }
        let idx = self.module_info(m).idx;
        self.rel_file(idx)
    }

    /// The path of module `idx` relative to the main file's directory ("" for the main
    /// module; the file name when it lies elsewhere, the module name when it has no file).
    pub fn rel_file(&self, idx: u32) -> String {
        if idx == self.module_info(self.main_module).idx {
            return String::new();
        }
        if let Some(f) = self.rel_files.borrow().get(&idx) {
            return f.clone();
        }
        let canon = |p: &str| std::fs::canonicalize(p).map(|x| x.to_string_lossy().to_string()).unwrap_or_else(|_| p.to_string());
        let f = match frontend::module_path(idx) {
            Some(p) => {
                let main_dir = std::path::Path::new(&canon(&self.filename)).parent().map(|d| d.to_path_buf());
                let cp = canon(&p);
                match main_dir.as_ref().and_then(|d| std::path::Path::new(&cp).strip_prefix(d).ok()) {
                    Some(r) => r.to_string_lossy().to_string(),
                    None => std::path::Path::new(&p).file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or(p),
                }
            }
            None => frontend::module_name(idx),
        };
        self.rel_files.borrow_mut().insert(idx, f.clone());
        f
    }

    pub fn verdict(&mut self, node: &Node, key: KeyId, cls: &Classification, oid: ObjId, ns: Ns, file_override: Option<&str>, text_override: Option<&str>) {
        if matches!(cls.tier, Tier::Map | Tier::Sym) || self.quiet || self.quiet_records {
            return;
        }
        // a key written as `True` that reached the dict's `1`: reported as written
        let key = match self.key_alias {
            Some((c, o)) if c == key => o,
            _ => key,
        };
        let text = match text_override {
            Some(t) if !t.is_empty() => t.to_string(),
            _ => self.node_text(node),
        };
        let fname = match file_override {
            Some(f) => f.to_string(),
            None => self.file_of_frame(),
        };
        let chain = self.call_chain.clone();
        let dk = (fname.clone(), node.line, node.col, intern::key_repr(key), cls.tier, oid, chain.clone());
        if self.verdict_keys.contains(&dk) {
            return;
        }
        self.verdict_keys.insert(dk);
        self.verdicts.push(Verdict {
            line: node.line,
            col: node.col,
            key,
            tier: cls.tier,
            obj: self.obj(oid).label.clone(),
            chain,
            note: cls.note.clone(),
            providers: cls.providers.iter().map(|(s, _)| crate::domain::source_label(*s)).collect(),
            ns,
            text,
            oid,
            file: fname,
            guarded: self.verdict_guarded,
        });
    }

    /// A verdict on an access the program guards (a try handling the failure): recorded for
    /// the golden harness and the hover, never a diagnostic.
    pub fn verdict_guarded(&mut self, node: &Node, key: KeyId, cls: &Classification, oid: ObjId, ns: Ns) {
        let saved = std::mem::replace(&mut self.verdict_guarded, true);
        self.verdict(node, key, cls, oid, ns, None, None);
        self.verdict_guarded = saved;
    }

    pub fn node_text(&self, node: &Node) -> String {
        if node.addr == 0 {
            return String::new();
        }
        frontend::text_at(node)
    }

    pub fn note(&mut self, msg: String) {
        if self.quiet {
            return;
        }
        if !self.notes_set.contains(&msg) {
            self.notes_set.insert(msg.clone());
            self.notes.push(msg);
        }
    }

    pub fn access_site(&self, node: &Node) -> String {
        format!("L{}", node.line)
    }

    pub fn access_site_id(&self, node: &Node) -> StrId {
        s(&format!("L{}", node.line))
    }

    // ------------------------------------------------------------- entry
    /// Past the time budget: the analysis keeps going but stops refining (see `MAX_MS`).
    /// Whether a function body of module `m` calls a mutating method on the global `name`
    /// (`LOG.append(…)`): a textual scan of the indented lines, cached per module.
    pub fn mutated_in_functions(&self, m: ObjId, name: &str) -> bool {
        let idx = self.module_info(m).idx;
        if !self.mutated_globals.borrow().contains_key(&idx) {
            let mut set: FxHashSet<String> = FxHashSet::default();
            const MUT: &[&str] = &["append", "extend", "insert", "add", "update", "pop", "remove", "discard", "clear", "appendleft", "popleft", "extendleft", "sort", "reverse", "setdefault", "popitem", "move_to_end"];
            for line in frontend::module_source_lines(idx) {
                if !line.starts_with(' ') && !line.starts_with('\t') {
                    continue;
                }
                // `NAME[k] = v` / `NAME[k] += v` / `del NAME[k]` in a function body
                let t = line.trim_start();
                let t2 = t.strip_prefix("del ").unwrap_or(t);
                let name: String = t2.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
                if !name.is_empty() && t2[name.len()..].starts_with('[') {
                    let rest = &t2[name.len()..];
                    let mut depth = 0i32;
                    let mut end = None;
                    for (j, ch) in rest.char_indices() {
                        match ch {
                            '[' | '(' | '{' => depth += 1,
                            ']' | ')' | '}' => {
                                depth -= 1;
                                if depth == 0 {
                                    end = Some(j);
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                    if let Some(e) = end {
                        let after = rest[e + 1..].trim_start();
                        let assign = after.starts_with('=') && !after.starts_with("==") || ["+=", "-=", "*=", "/=", "|=", "&=", "//=", "%="].iter().any(|op| after.starts_with(op));
                        if assign || t.starts_with("del ") {
                            set.insert(name.clone());
                        }
                    }
                }
                let b = line.as_bytes();
                let mut i = 0;
                while let Some(p) = line[i..].find('.') {
                    let dot = i + p;
                    // the name before the dot
                    let mut st = dot;
                    while st > 0 && (b[st - 1].is_ascii_alphanumeric() || b[st - 1] == b'_') {
                        st -= 1;
                    }
                    let after: String = line[dot + 1..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
                    if st < dot && (st == 0 || b[st - 1] != b'.') && MUT.contains(&after.as_str()) && line[dot + 1 + after.len()..].starts_with('(') {
                        set.insert(line[st..dot].to_string());
                    }
                    i = dot + 1;
                }
            }
            self.mutated_globals.borrow_mut().insert(idx, set);
        }
        self.mutated_globals.borrow().get(&idx).map(|s| s.contains(name)).unwrap_or(false)
    }

    /// A module of the program itself: the main file's directory or below.
    pub fn is_local_module(&self, m: ObjId) -> bool {
        if m == self.main_module {
            return true;
        }
        let idx = self.module_info(m).idx;
        let Some(p) = frontend::module_path(idx) else { return false };
        let canon = |p: &str| std::fs::canonicalize(p).map(|x| x.to_string_lossy().to_string()).unwrap_or_else(|_| p.to_string());
        let main_dir = std::path::Path::new(&canon(&self.filename)).parent().map(|d| d.to_path_buf());
        match main_dir {
            Some(d) => std::path::Path::new(&canon(&p)).starts_with(&d) && !p.contains("site-packages"),
            None => false,
        }
    }

    pub fn over_deadline(&self) -> bool {
        self.started.elapsed().as_millis() as usize > limit("ms", MAX_MS)
    }

    pub fn run(&mut self) {
        crate::state::set_narrowed_same(self.opts.narrowed_same);
        let mut state = State::new();
        let main = self.main_module;
        self.run_module(main, &mut state);
        self.module_done = true;
        if self.analyse_entry_points {
            self.analyse_uncalled(&mut state);
        }
        self.final_state = Some(state);
    }

    pub fn run_module(&mut self, moid: ObjId, state: &mut State) {
        let info = self.module_info(moid);
        let fr = Frame::new(FrameKind::Module, info.scopes.module(), moid, "", true, &format!("<module {}>", info.name));
        self.frames.push(fr);
        self.exec_block(&info.tree.body, state);
        self.frames.pop();
        state.reachable = true;
        state.exits = None;
    }

    /// Entry-point rule (§6.4): functions never called get analysed with open parameters.
    fn analyse_uncalled(&mut self, state: &mut State) {
        // entry runs can make functions (closures, lambdas, inner defs) nobody calls either:
        // a few rounds, each over the functions made so far
        let mut start: ObjId = 1;
        for _round in 0..4 {
            let n = self.objects.len() as ObjId;
            if start >= n {
                break;
            }
            self.analyse_uncalled_range(start, n, state, false);
            start = n;
        }
        // then the requirement-only runs of the functions the program calls (last: summaries
        // made while quiet hold no verdicts and must not be reused by a reporting run)
        let n = self.objects.len() as ObjId;
        self.analyse_uncalled_range(1, n, state, true);
    }

    fn analyse_uncalled_range(&mut self, from: ObjId, n: ObjId, state: &mut State, quiet_pass: bool) {
        for oid in from..n {
            if self.kind(oid) != Kind::Function || self.entry_done.contains(&oid) {
                continue;
            }
            let fi = self.func_info(oid);
            // functions of torch modules get their own run even when called (the shapes
            // inside, in terms of their own parameters)
            // a FastAPI handler: called by the framework with any request (also when the
            // program's own TestClient calls it), in any module
            let handler = self.fa_handlers.contains(&oid);
            // a function the program calls: a requirement-only run with open parameters gives
            // its own input contract (`param F.x` schemas)
            let quiet = self.analysed_funcs.contains(&oid) && !self.torch_modules.contains(&fi.module) && !handler;
            if quiet != quiet_pass {
                continue;
            }
            if quiet && (disabled("paramschemas") || !fi.closure.is_empty() || !matches!(fi.node, FuncNode::Def(_)) || !self.is_local_module(fi.module) || self.over_deadline()) || self.t_concrete_closures.contains(&oid) {
                continue;
            }
            self.entry_done.insert(oid);
            // closures made inside a summary run stand for every call's closure (their cells
            // are relative): they are reached through the real calls
            if !fi.closure.is_empty() && (self.obj(oid).ctx.contains("sum") || disabled("entryclosures")) {
                continue;
            }
            if matches!(fi.node, FuncNode::Lambda(_)) && disabled("entryclosures") {
                continue;
            }
            if fi.module != self.main_module && !handler && !(quiet && self.is_local_module(fi.module)) {
                continue;
            }
            if handler && !disabled("fastapi") {
                let mut st = state.copy();
                st.ret = None;
                let mi = self.module_info(fi.module);
                self.frames.push(Frame::new(FrameKind::Module, mi.scopes.module(), fi.module, &format!("entry:{}", fi.qualname), true, "<entry>"));
                self.entry_scope = Some(oid);
                let fake = Node::synthetic(mi.idx, fi.node.node().line);
                let saved_entry = self.entry_run.replace(oid);
                if self.fa_entry_through_middleware(oid, &mut st, &fake).is_none() {
                    self.fa_dep_cache.push(FxHashMap::default());
                    if let Some(idx) = self.fa_routes.iter().position(|r| r.func == Some(oid)) {
                        self.fa_route_deps(idx, None, &mut st, &fake);
                    }
                    let gens = self.fa_yield_deps(oid, &mut st);
                    self.fa_run_handler(oid, gens, None, true, &mut st, &fake);
                    self.fa_dep_cache.pop();
                }
                self.entry_scope = None;
                self.entry_run = saved_entry;
                self.frames.pop();
                for (src, d) in st.reqs.items() {
                    let tgt = state.reqs.writable(&src);
                    for (k, info) in d.iter() {
                        let nv = match tgt.get(k) {
                            Some(cur) => cur.join(info),
                            None => info.clone(),
                        };
                        tgt.insert(*k, nv);
                    }
                }
                continue;
            }
            let Some(fparams) = fi.node.params().or_else(|| None) else {
                // a lambda without parameters
                if let FuncNode::Lambda(_) = fi.node {
                    let mut st = state.copy();
                    st.ret = None;
                    let mi = self.module_info(fi.module);
                    self.frames.push(Frame::new(FrameKind::Module, mi.scopes.module(), fi.module, &format!("entry:{}", fi.qualname), true, "<entry>"));
                    let fake = Node::synthetic(mi.idx, fi.node.node().line);
                    self.call_user(oid, None, vec![], IndexMap::new(), vec![], vec![], &mut st, &fake, None, true);
                    self.frames.pop();
                }
                continue;
            };
            if debug_enabled() {
                eprintln!(
                    "[rowan] entry {} L{} t={:.1}s objs={} summaries={} verdicts={}",
                    fi.qualname,
                    fi.node.node().line,
                    self.now(),
                    self.objects.len(),
                    self.summary_table.len(),
                    self.verdicts.len()
                );
            }
            // a closure made by a factory's entry run starts from that run's state (its cells)
            let mut st = match self.t_closure_states.get(&oid) {
                Some(cs) => {
                    let mut c = cs.copy_ref();
                    c.reachable = true;
                    c.exits = None;
                    c
                }
                None => state.copy(),
            };
            st.ret = None;
            let mut args: Vec<Value> = Vec::new();
            let mi = self.module_info(fi.module);
            self.frames.push(Frame::new(FrameKind::Module, mi.scopes.module(), fi.module, &format!("entry:{}", fi.qualname), true, "<entry>"));
            self.entry_scope = Some(oid);
            let mut params = scopemod::all_params(fparams);
            let mut self_val: Option<Value> = None;
            if let Some(cls) = fi.cls {
                if !fi.is_static.get() {
                    let inst = match self.entry_instances.get(&cls) {
                        Some(&i) => i,
                        None => {
                            let cname = self.class_info(cls).name.clone();
                            let i = self.fresh_instance(cls, state, &format!("entry:{}", cname));
                            self.entry_instances.insert(cls, i);
                            st = state.copy();
                            st.ret = None;
                            i
                        }
                    };
                    self_val = Some(Value::obj(inst));
                    if !params.is_empty() {
                        params.remove(0);
                    }
                }
            }
            let mut star_args: Vec<Value> = Vec::new();
            let mut star_kwargs: Vec<Value> = Vec::new();
            if self.torch_modules.contains(&fi.module) {
                for (i, a) in params.iter().enumerate() {
                    crate::shape::note_param(crate::shape::sym(oid, a.name.id.as_str()), i as u32);
                }
            }
            let mut kwargs: IndexMap<StrId, Value> = IndexMap::new();
            for a in params {
                // (keyword-only parameters are passed by keyword)
                let kwonly = fparams.kwonlyargs.iter().any(|k| std::ptr::eq(&k.parameter, a)) && !disabled("entrykwonly");
                let n_before = args.len();
                let is_vararg = fparams.vararg.as_deref().map(|v| std::ptr::eq(v, a)).unwrap_or(false);
                let is_kwarg = fparams.kwarg.as_deref().map(|v| std::ptr::eq(v, a)).unwrap_or(false);
                if is_vararg || is_kwarg {
                    // a caller may pass any arguments here
                    if !disabled("entrystar") {
                        let poid = self.unknown_source(&format!("param {}.{}", fi.py_qualname(), a.name.id), SourceKind::Param, &mut st, &format!("param {}", a.name.id), "", Ns::Item, vec![]);
                        if is_vararg {
                            star_args.push(Value::obj(poid));
                        } else {
                            star_kwargs.push(Value::obj(poid));
                        }
                    }
                    continue;
                }
                let pname = s(a.name.id.as_str());
                if let Some(d) = fi.defaults.get(&pname) {
                    // the default, or whatever a caller passes
                    if disabled("entrydefaults") {
                        args.push(d.clone());
                        continue;
                    }
                    let poid = self.unknown_source(&format!("param {}.{}", fi.py_qualname(), a.name.id), SourceKind::Param, &mut st, &format!("param {}", a.name.id), "", Ns::Item, vec![]);
                    // a number default in a torch module: the parameter (whatever a caller
                    // passes, the default included) stays a name dims can be written in
                    if self.torch_modules.contains(&fi.module) && d.pts.is_empty() && d.kv.as_lit().map(intern::key_is_number).unwrap_or(false) {
                        args.push(Value::obj(poid));
                        continue;
                    }
                    args.push(d.join(&Value::obj(poid)));
                    if kwonly && args.len() > n_before {
                        let v = args.pop().unwrap();
                        kwargs.insert(pname, v);
                    }
                    continue;
                }
                let mut v = match &a.annotation {
                    Some(ann) => self.param_from_annotation(ann, &fi, a.name.id.as_str(), &mut st),
                    None => None,
                };
                if v.is_none() {
                    let poid = self.unknown_source(&format!("param {}.{}", fi.py_qualname(), a.name.id), SourceKind::Param, &mut st, &format!("param {}", a.name.id), "", Ns::Item, vec![]);
                    v = Some(Value::obj(poid));
                }
                if kwonly {
                    kwargs.insert(pname, v.unwrap());
                } else {
                    args.push(v.unwrap());
                }
            }
            self.entry_scope = None;
            let fake = Node::synthetic(mi.idx, fi.node.node().line);
            let saved_entry = self.entry_run.replace(oid);
            let before = self.objects.len() as ObjId;
            let saved_quiet = std::mem::replace(&mut self.quiet, quiet);
            let called_torch = self.analysed_funcs.contains(&oid) && self.torch_modules.contains(&fi.module) && !disabled("torchquietrec");
            let saved_qr = std::mem::replace(&mut self.quiet_records, called_torch);
            self.call_user(oid, self_val, args, kwargs, star_args, star_kwargs, &mut st, &fake, None, true);
            self.quiet_records = saved_qr;
            self.quiet = saved_quiet;
            self.entry_run = saved_entry;
            if quiet {
                // functions made by a requirement-only run are not the program's
                let after = self.objects.len() as ObjId;
                for o in before..after {
                    if self.kind(o) == Kind::Function {
                        self.entry_done.insert(o);
                    }
                }
            }
            if self.torch_modules.contains(&fi.module) {
                let after = self.objects.len() as ObjId;
                let made: Vec<ObjId> = (before..after).filter(|o| self.kind(*o) == Kind::Function && !self.func_info(*o).closure.is_empty()).collect();
                if !made.is_empty() {
                    let snap = std::rc::Rc::new(st.copy());
                    for o in made {
                        self.t_closure_states.insert(o, snap.clone());
                    }
                }
            }
            self.frames.pop();
            // keep schema requirements discovered in entry points (a requirement-only run: those
            // of the function's own parameters)
            let own = format!("param {}.", fi.py_qualname());
            let span = (fi.node.node().line, fi.node.node().end_line);
            let in_main = fi.module == self.main_module;
            for (src, d) in st.reqs.items() {
                if quiet && !own_source(&crate::domain::source_label(src), &own, span, in_main) {
                    continue;
                }
                let tgt = state.reqs.writable(&src);
                for (k, info) in d.iter() {
                    let nv = match tgt.get(k) {
                        Some(cur) => cur.join(info),
                        None => info.clone(),
                    };
                    tgt.insert(*k, nv);
                }
            }
        }
    }

    /// Strip `Optional[X]`, `list[X]`, `"X"`, `X | None` down to the inner expression.
    pub fn strip_annotation(ann: &'static ast::Expr) -> Option<&'static ast::Expr> {
        let mut node = ann;
        while let ast::Expr::Subscript(sub) = node {
            node = &sub.slice;
            if let ast::Expr::Tuple(t) = node {
                if let Some(first) = t.elts.first() {
                    node = first;
                }
            }
        }
        if let ast::Expr::BinOp(b) = node {
            if b.op == ast::Operator::BitOr {
                node = &b.left;
            }
        }
        if let ast::Expr::StringLiteral(sl) = node {
            let txt = sl.value.to_str().to_string();
            return frontend::parse_expression(&txt);
        }
        Some(node)
    }

    /// A declared row for an annotated entry-point parameter (§8.5).
    pub fn param_from_annotation(&mut self, ann: &'static ast::Expr, fi: &FuncInfo, pname: &str, state: &mut State) -> Option<Value> {
        let node = Self::strip_annotation(ann)?;
        let v = self.eval(node, state);
        for oid in v.pts.iter() {
            let o = self.kind(oid);
            if o == Kind::Class {
                let ci = self.class_info(oid);
                if ci.has_fields() {
                    let mut fields: KeySet = KeySet::from_iter(ci.fields().iter().map(|(f, _)| key_str_lit(f)));
                    let ns = if ci.typed_dict { Ns::Item } else { Ns::Attr };
                    // a TypedDict's `total=False` / `NotRequired` keys may be absent
                    let mut declared = Row::closed_with(fields.clone());
                    if ci.typed_dict && !disabled("tdoptional") {
                        if let Some((req, opt)) = self.typed_dict_keys(oid) {
                            fields = req.union(&opt);
                            declared = Row::from_data(crate::domain::RowData { present: fields.clone(), definite: req.clone(), likely: req, ..Default::default() });
                        }
                    }
                    let inst = self.alloc(
                        Kind::Instance,
                        &format!("param:{}:{}", fi.qualname, ci.name),
                        state,
                        &format!("{} param", ci.name),
                        Payload::Cls(oid),
                        Some(declared),
                        ns,
                        Some(true),
                        Some(&format!("entry:{}", fi.qualname)),
                    );
                    for f in fields.iter() {
                        let v = self.typed_field_value(oid, &intern::key_py_str(f), state, &format!("entry:{}", fi.qualname));
                        state.set_slot(inst, ns, f, v, true);
                    }
                    if ci.typed_dict && !disabled("tdparam") {
                        let src = Source::new(&format!("param {}.{}", fi.py_qualname(), pname), SourceKind::Param);
                        self.td_param_src.insert(inst, src);
                    }
                    return Some(Value::obj(inst));
                }
                let inst = self.fresh_instance(oid, state, &format!("entry:{}", fi.qualname));
                return Some(Value::obj(inst));
            }
            if o == Kind::External && self.external_qual(oid).ends_with("DataFrame") {
                let src = Source::new(&format!("param {}", fi.qualname), SourceKind::Param);
                let fid = self.alloc(
                    Kind::Frame,
                    &format!("param:{}", fi.qualname),
                    state,
                    "frame param",
                    Payload::None,
                    Some(Row::with_source(src)),
                    Ns::Item,
                    Some(true),
                    Some(&format!("entry:{}", fi.qualname)),
                );
                return Some(Value::obj(fid));
            }
        }
        None
    }

    /// The value of a declared field: an instance of its annotated user class with `fixes`
    /// (nested TypedDicts / dataclasses), otherwise opaque data.
    pub fn typed_field_value(&mut self, cls: ObjId, field: &str, state: &mut State, ctx: &str) -> Value {
        if let Some(fc) = self.field_class(cls, field, state) {
            if self.class_info_opt(fc).is_some() && !self.fresh_in_progress.contains(&fc) {
                let inst = self.fresh_instance(fc, state, &format!("{}.{}", ctx, field));
                return Value::obj(inst);
            }
        }
        Value::data(&format!("field {}", field))
    }

    pub fn fresh_instance(&mut self, cls_oid: ObjId, state: &mut State, ctx: &str) -> ObjId {
        let ci = self.class_info(cls_oid);
        if self.fresh_in_progress.contains(&cls_oid) {
            // self-referential annotation: open instance
            return self.unknown_source(&format!("instance {}", ci.name), SourceKind::Class, state, &format!("{} (recursive)", ci.name), "", Ns::Attr, vec![]);
        }
        self.fresh_in_progress.insert(cls_oid);
        let r = self.fresh_instance_inner(cls_oid, &ci, state, ctx);
        self.fresh_in_progress.remove(&cls_oid);
        r
    }

    fn fresh_instance_inner(&mut self, cls_oid: ObjId, ci: &ClassInfo, state: &mut State, ctx: &str) -> ObjId {
        let has_init = self.class_lookup(cls_oid, "__init__").is_some();
        let has_fields = self.mro(cls_oid).iter().any(|c| self.class_info_opt(*c).map(|ci| ci.has_fields()).unwrap_or(false));
        // a class that never initialises its own attributes is only used through subclasses
        let open_inst = ci.open_base || !(has_init || has_fields);
        let row = if open_inst { Row::with_source(Source::new(&format!("instance {}", ci.name), SourceKind::Class)) } else { Row::new() };
        let inst = self.alloc(
            Kind::Instance,
            &format!("instance:{}:{}", ci.name, ctx),
            state,
            &format!("{}()", ci.name),
            Payload::Cls(cls_oid),
            Some(row),
            Ns::Attr,
            Some(true),
            Some(ctx),
        );
        // dataclass / TypedDict / NamedTuple fields (own and inherited) are present on a fresh instance
        let mut seen: FxHashSet<ObjId> = FxHashSet::default();
        let mut order = vec![cls_oid];
        while !order.is_empty() {
            let c = order.remove(0);
            if !seen.insert(c) {
                continue;
            }
            let Some(cinfo) = self.class_info_opt(c) else { continue };
            if cinfo.has_fields() {
                let ns = if cinfo.typed_dict { Ns::Item } else { Ns::Attr };
                for (f, _) in cinfo.fields() {
                    let k = key_str_lit(&f);
                    let r = state.row(inst, ns).store(k, true);
                    state.set_row(inst, ns, r);
                    let v = self.typed_field_value(c, &f, state, ctx);
                    state.set_slot(inst, ns, k, v, true);
                }
            }
            order.extend(cinfo.bases.iter().copied());
        }
        if let Some(init_cls) = self.class_lookup(cls_oid, "__init__") {
            let inits = self.cls_slots.get(&(init_cls, Ns::Attr, key_str_lit("__init__"))).cloned().unwrap_or_default();
            for init in inits.pts.iter() {
                if self.kind(init) != Kind::Function {
                    continue;
                }
                let fi = self.func_info(init);
                let Some(params_all) = fi.node.params() else { continue };
                let saved_scope = self.entry_scope;
                if self.torch_modules.contains(&fi.module) {
                    self.entry_scope = Some(init);
                }
                let params: Vec<&ast::Parameter> = scopemod::all_params(params_all).into_iter().skip(1).collect();
                let mut args = Vec::new();
                for a in params {
                    let is_var = params_all.vararg.as_deref().map(|v| std::ptr::eq(v, a)).unwrap_or(false)
                        || params_all.kwarg.as_deref().map(|v| std::ptr::eq(v, a)).unwrap_or(false);
                    if is_var {
                        continue;
                    }
                    let pname = s(a.name.id.as_str());
                    if let Some(d) = fi.defaults.get(&pname) {
                        let number = d.pts.is_empty() && d.kv.as_lit().map(intern::key_is_number).unwrap_or(false);
                        if !self.torch_modules.contains(&fi.module) {
                            args.push(d.clone());
                            continue;
                        }
                        if !number {
                            // the default, or whatever a caller passes
                            let poid = self.unknown_source(&format!("param {}.{}", fi.py_qualname(), a.name.id), SourceKind::Param, state, &format!("param {}", a.name.id), "", Ns::Item, vec![]);
                            args.push(d.join(&Value::obj(poid)));
                            continue;
                        }
                    }
                    let v = match &a.annotation {
                        Some(ann) => self.param_from_annotation(ann, &fi, a.name.id.as_str(), state),
                        None => None,
                    };
                    args.push(match v {
                        Some(v) => v,
                        None => Value::obj(self.unknown_source(&format!("param {}.{}", fi.py_qualname(), a.name.id), SourceKind::Param, state, &format!("param {}", a.name.id), "", Ns::Item, vec![])),
                    });
                }
                self.entry_scope = saved_scope;
                let fake = if self.torch_modules.contains(&fi.module) { Node::synthetic(fi.node.node().module, frontend::node(ci.node).line) } else { Node::synthetic(fi.node.node().module, fi.node.node().line) };
                self.call_user(init, Some(Value::obj(inst)), args, IndexMap::new(), vec![], vec![], state, &fake, None, true);
            }
        }
        inst
    }

    /// Hover facts: a name was read or written at `node` holding `v` (concrete analyses only;
    /// the symbolic rows of a summary body are not meaningful to a reader).
    /// A concrete hover fact for a line inside a function whose summary was instantiated:
    /// `rows` are the rows resolved for this call; the text shows them with values unknown
    /// (the caller's state after the call does not tell what the slots held at that line).
    pub fn record_resolved_fact(&mut self, line: u32, name: StrId, v: &Value, rows: Vec<(ObjId, Option<Row>, Option<Row>)>, slots: Vec<(ObjId, Ns, KeyId, Value)>, scope: u32, state: &State) {
        let mut scratch = state.copy_ref();
        // contents as they were at that line (captured, resolved for this call); objects whose
        // contents were not captured show `?` rather than what they hold after the call
        let mut cleared: Vec<ObjId> = Vec::new();
        for (o, _, _, _) in &slots {
            if !cleared.contains(o) {
                scratch.drop_obj(*o);
                cleared.push(*o);
            }
        }
        for (o, ns, k, sv) in &slots {
            scratch.set_slot(*o, *ns, *k, sv.clone(), false);
        }
        for (o, item, attr) in &rows {
            if !cleared.contains(o) {
                scratch.drop_obj(*o);
            }
            if let Some(r) = item {
                scratch.set_row(*o, Ns::Item, r.clone());
            }
            if let Some(r) = attr {
                scratch.set_row(*o, Ns::Attr, r.clone());
            }
        }
        let (text, legend) = crate::report::in_scope(scope, || (crate::report::render_value(self, &scratch, v, 0, &mut Vec::new()), crate::report::value_legend(self, &scratch, v)));
        let e = self.facts.entry((line, name, false)).or_insert_with(|| Fact { symbolic: false, value: Value::new(), rows: IndexMap::new(), texts: Vec::new(), legend: Vec::new(), t_names: FxHashMap::default(), primary: None, written: false });
        e.value = e.value.join(v);
        if !e.texts.contains(&text) && e.texts.len() < 4 {
            e.texts.push(text);
        }
        for (sym, desc) in legend {
            match e.legend.iter_mut().find(|(s0, _)| *s0 == sym) {
                Some((_, d)) if *d != desc && !d.split(" | ").any(|p| p == desc) => *d = format!("{} | {}", d, desc),
                Some(_) => {}
                None => e.legend.push((sym, desc)),
            }
        }
        let join_opt = |a: &Option<Row>, b: &Option<Row>| -> Option<Row> {
            match (a, b) {
                (Some(x), Some(y)) => Some(x.join(y)),
                (Some(x), None) | (None, Some(x)) => Some(x.clone()),
                _ => None,
            }
        };
        for (oid, item, attr) in rows {
            // the objects the name holds (nested ones only shaped the text)
            if !v.pts.contains(oid) {
                continue;
            }
            match e.rows.get_mut(&oid) {
                Some((ci, ca)) => {
                    *ci = join_opt(ci, &item);
                    *ca = join_opt(ca, &attr);
                }
                None => {
                    e.rows.insert(oid, (item, attr));
                }
            }
        }
    }

    pub fn record_fact(&mut self, node: &Node, name: StrId, v: &Value, state: &State) {
        if !self.facts_on || node.addr == 0 || self.quiet {
            return;
        }
        let symbolic = !self.sum_stack.is_empty();
        if node.module != self.module_info(self.main_module).idx {
            return;
        }
        let mut rows: Vec<(ObjId, Option<Row>, Option<Row>)> = Vec::new();
        for oid in v.pts.iter() {
            let k = self.kind(oid);
            let item = if matches!(k, Kind::Dict | Kind::Frame | Kind::Unknown | Kind::Namespace | Kind::Ph | Kind::Module | Kind::Class) || state.has_row(oid, Ns::Item) { Some(state.row(oid, Ns::Item)) } else { None };
            let attr = if matches!(k, Kind::Instance | Kind::Namespace | Kind::Unknown | Kind::Ph | Kind::Module | Kind::Class) || state.has_row(oid, Ns::Attr) { Some(state.row(oid, Ns::Attr)) } else { None };
            rows.push((oid, item, attr));
        }
        let join_opt = |a: &Option<Row>, b: &Option<Row>| -> Option<Row> {
            match (a, b) {
                (Some(x), Some(y)) => Some(x.join(y)),
                (Some(x), None) | (None, Some(x)) => Some(x.clone()),
                _ => None,
            }
        };
        if symbolic && !disabled("factobs") {
            let full = self.sum_fact_obs.last().map(|t| t.len() >= 4096).unwrap_or(true);
            let slots = if full { Vec::new() } else { crate::report::reachable_slots(self, state, v) };
            let rows = if full { Vec::new() } else { crate::report::reachable_rows(self, state, v) };
            let fscope = self.frame().func.unwrap_or(0);
            if let Some(top) = self.sum_fact_obs.last_mut() {
                if !full {
                    top.push(crate::summary::FactObs { line: node.line, name, value: v.clone(), rows: rows.clone(), slots, scope: fscope });
                }
            }
        }
        let scope = self.frame().func.unwrap_or(0);
        // tensors: dims named by locals / letters, printed in the function's own terms
        let (pctx, tnames, tlegend) = if self.torch_on {
            let mut ctx = self.t_print_ctx(state);
            let (names, legend) = crate::shape::with_ctx(ctx.clone(), || self.t_hover_names(v, state));
            ctx.atom_names = names.clone();
            (Some(ctx), names, legend)
        } else {
            (None, FxHashMap::default(), Vec::new())
        };
        let render = |an: &Analyzer| crate::report::in_scope(scope, || (crate::report::render_value(an, state, v, 0, &mut Vec::new()), crate::report::value_legend(an, state, v)));
        let (text, mut legend) = match pctx {
            Some(ctx) => crate::shape::with_ctx(ctx, || render(self)),
            None => render(self),
        };
        legend.extend(tlegend);
        let writing = std::mem::take(&mut self.t_fact_write);
        let e = self.facts.entry((node.line, name, symbolic)).or_insert_with(|| Fact { symbolic, value: Value::new(), rows: IndexMap::new(), texts: Vec::new(), legend: Vec::new(), t_names: FxHashMap::default(), primary: None, written: false });
        // in torch programs an assignment's value is what the hover shows (`y = f(y)`)
        if self.torch_on {
            if writing && !e.written {
                e.written = true;
                e.texts.clear();
                e.legend.clear();
                e.primary = None;
                e.value = Value::new();
            } else if !writing && e.written {
                return;
            }
        }
        // the function analysed on its own (not a call from elsewhere): the general answer
        let own = self.entry_run.is_some() && self.call_chain.len() <= 1 && self.torch_on;
        if own {
            e.t_names = tnames;
            e.primary = Some(match &e.primary {
                Some(p) if *p != text && !p.split(" | ").any(|x| x == text) => format!("{} | {}", p, text),
                Some(p) => p.clone(),
                None => text.clone(),
            });
        } else {
            for (a, n) in tnames {
                e.t_names.entry(a).or_insert(n);
            }
        }
        e.value = e.value.join(v);
        if !e.texts.contains(&text) && e.texts.len() < 4 {
            e.texts.push(text);
        }
        for (sym, desc) in legend {
            match e.legend.iter_mut().find(|(s0, _)| *s0 == sym) {
                Some((_, d)) if *d != desc && !d.split(" | ").any(|p| p == desc) => *d = format!("{} | {}", d, desc),
                Some(_) => {}
                None => e.legend.push((sym, desc)),
            }
        }
        for (oid, item, attr) in rows {
            match e.rows.get_mut(&oid) {
                Some((ci, ca)) => {
                    *ci = join_opt(ci, &item);
                    *ca = join_opt(ca, &attr);
                }
                None => {
                    e.rows.insert(oid, (item, attr));
                }
            }
        }
    }

    /// Is `oid` reachable from a binding the program can still use: the locals of every
    /// active frame, loop overlays, cells, module objects, narrowed values, the return value?
    pub fn is_live(&self, oid: ObjId, state: &State) -> bool {
        fn push_value(v: &Value, work: &mut Vec<ObjId>) {
            work.extend(v.pts.iter());
            match &v.kv {
                KeyVal::KeysOf(ko) => work.extend(ko.objs.iter()),
                KeyVal::Param(p) => work.push(*p),
                _ => {}
            }
        }
        let mut work: Vec<ObjId> = Vec::new();
        for v in state.env.values() {
            push_value(v, &mut work);
        }
        for env in &self.env_stack {
            for v in env.values() {
                push_value(v, &mut work);
            }
        }
        for v in state.narrowed.values() {
            push_value(v, &mut work);
        }
        if let Some(r) = &state.ret {
            push_value(r, &mut work);
        }
        for fr in &self.frames {
            work.push(fr.module);
            work.extend(fr.cells.values().copied());
            for ov in &fr.overlays {
                for v in ov.values() {
                    push_value(v, &mut work);
                }
            }
        }
        let mut seen: FxHashSet<ObjId> = FxHashSet::default();
        while let Some(o) = work.pop() {
            if o == oid {
                return true;
            }
            if !seen.insert(o) {
                continue;
            }
            for (_, v) in state.slots.for_key(&o) {
                push_value(&v, &mut work);
            }
            if let Some(kv) = state.elems.get(&o) {
                push_value(&Value::kv_only(kv), &mut work);
            }
            if !self.has_obj(o) {
                continue;
            }
            match &self.obj(o).payload {
                Payload::Func(fi) => work.extend(fi.closure.values().copied()),
                Payload::Ph(p) => {
                    if let Some(par) = p.parent {
                        work.push(par);
                    }
                }
                Payload::Method(_, v) | Payload::View(_, v) | Payload::Enumerate(v, _) => push_value(v, &mut work),
                Payload::Bound(f, v) | Payload::Super(f, v) => {
                    work.push(*f);
                    push_value(v, &mut work);
                }
                Payload::Zip(a, b) | Payload::Pair(a, b) => {
                    push_value(a, &mut work);
                    push_value(b, &mut work);
                }
                Payload::Partial(f, args, kw) => {
                    push_value(f, &mut work);
                    for a in args {
                        push_value(a, &mut work);
                    }
                    for v in kw.values() {
                        push_value(v, &mut work);
                    }
                }
                _ => {}
            }
        }
        false
    }

    /// What a value is, for the golden harness's `#@value` checks: literals it may be, whether
    /// it may also be something unknown, and the (known) objects it may be.
    pub fn value_info(&self, v: &Value, state: &State) -> ValueInfo {
        let mut info = ValueInfo::default();
        if v.kv.is_literal() {
            info.lits = crate::domain::kv_literals(&v.kv).iter().collect();
        } else if !v.kv.is_bot() {
            info.unknown = true;
        }
        for o in v.pts.iter() {
            match self.kind(o) {
                Kind::Unknown | Kind::Ph | Kind::External => info.unknown = true,
                // a tuple of known literals is that literal (as Python prints it)
                Kind::Tuple | Kind::Pair if self.literal_tuple(o, state, 0).is_some() => info.lits.push(self.literal_tuple(o, state, 0).unwrap()),
                _ => info.objects.push(self.obj(o).label.clone()),
            }
        }
        if v.pts.is_empty() && v.kv.is_bot() {
            info.unknown = true;
        }
        info
    }

    /// The literal a tuple object stands for: every position one known literal (nested
    /// tuples too).
    pub fn literal_tuple(&self, o: ObjId, state: &State, depth: u32) -> Option<KeyId> {
        if depth > 4 {
            return None;
        }
        // a (key, value) pair (zip / enumerate / items): a 2-tuple
        if self.kind(o) == Kind::Pair {
            let Payload::Pair(k, v) = &self.obj(o).payload else { return None };
            let one = |x: &Value| -> Option<KeyId> {
                match (x.pts.len(), &x.kv) {
                    (0, KeyVal::Lit(k)) => Some(*k),
                    (1, KeyVal::Bot) if matches!(self.kind(x.pts.first().unwrap()), Kind::Tuple | Kind::Pair) => self.literal_tuple(x.pts.first().unwrap(), state, depth + 1),
                    _ => None,
                }
            };
            return Some(intern::key_tuple(vec![one(k)?, one(v)?]));
        }
        let n = self.llen(o, state)?;
        let mut items = Vec::with_capacity(n);
        for i in 0..n {
            let v = state.slot(o, Ns::Pos, intern::key_int(i as i64));
            let k = match (v.pts.len(), &v.kv) {
                (0, KeyVal::Lit(k)) => *k,
                (1, KeyVal::Bot) => {
                    let x = v.pts.first().unwrap();
                    if !matches!(self.kind(x), Kind::Tuple | Kind::Pair) {
                        return None;
                    }
                    self.literal_tuple(x, state, depth + 1)?
                }
                _ => return None,
            };
            items.push(k);
        }
        Some(intern::key_tuple(items))
    }

    /// Row-check support for the golden harness: after a statement at a requested line.
    pub fn after_stmt(&mut self, stmt: &ast::Stmt, state: &mut State) {
        if self.row_check_requests.is_empty() || self.quiet {
            return;
        }
        let n = frontend::node(stmt);
        let file = self.rel_file(n.module);
        if !state.reachable && !matches!(stmt, ast::Stmt::Return(_) | ast::Stmt::Break(_) | ast::Stmt::Continue(_)) {
            // the statement raised on this path: nothing to observe after it
            return;
        }
        // (a simple statement spanning several lines: a check on any of its lines)
        let simple = matches!(stmt, ast::Stmt::Assign(_) | ast::Stmt::AnnAssign(_) | ast::Stmt::AugAssign(_) | ast::Stmt::Expr(_) | ast::Stmt::Return(_) | ast::Stmt::Delete(_));
        let last = if simple { n.end_line } else { n.line };
        let reqs: Vec<(u32, String)> = self.row_check_requests.iter().filter(|(f, ln, _)| *ln >= n.line && *ln <= last && *f == file).map(|(_, ln, name)| (*ln, name.clone())).collect();
        for (ln, name) in reqs {
            let v = self.load_name(s(&name), &n, state);
            let mut rows: Vec<Row> = Vec::new();
            let mut ns_used = Ns::Item;
            let mut obs_rows: Vec<(ObjId, Ns, Row)> = Vec::new();
            for oid in v.pts.iter() {
                // a UserDict's keys are those of its `data`
                if self.kind(oid) == Kind::Instance && self.instance_class(oid).map(|c| self.mro(c).iter().any(|m| self.userdict_classes.contains(m))).unwrap_or(false) {
                    let d = state.slot(oid, Ns::Attr, key_str_lit("data"));
                    if let Some(dd) = d.single().filter(|x| self.kind(*x) == Kind::Dict) {
                        ns_used = Ns::Item;
                        rows.push(state.row(dd, Ns::Item));
                        obs_rows.push((dd, Ns::Item, state.row(dd, Ns::Item)));
                        continue;
                    }
                }
                // a mapping proxy shows its dict
                if let Payload::View(ViewKind::Proxy, inner) = self.obj(oid).payload.clone() {
                    for dd in inner.pts.iter() {
                        rows.push(state.row(dd, Ns::Item));
                        obs_rows.push((dd, Ns::Item, state.row(dd, Ns::Item)));
                    }
                    continue;
                }
                let typed = self.kind(oid) == Kind::Ph && self.ph_info(oid).cls.get().map(|c| self.class_info_opt(c).is_some()).unwrap_or(false);
                // (a numpy structured array is keyed by its fields)
                let ns = if (matches!(self.kind(oid), Kind::Instance | Kind::Namespace) || typed) && !self.np_struct.contains(&oid) { Ns::Attr } else { Ns::Item };
                ns_used = ns;
                rows.push(state.row(oid, ns));
                obs_rows.push((oid, ns, state.row(oid, ns)));
            }
            let must: Vec<KeyId> = state.name_keys.iter().filter(|(n, _)| *intern::str_get(*n) == *name).map(|(_, k)| *k).collect();
            // inside a summary the rows are relative to its entry: kept with the summary and
            // resolved per call
            if !self.sum_stack.is_empty() && !disabled("obs") {
                // a tuple of literals is that literal (its positions are not kept with the summary)
                let mut val = v.clone();
                if val.kv.is_bot() {
                    if let Some(o) = val.single().filter(|o| self.kind(*o) == Kind::Tuple) {
                        if let Some(k) = self.literal_tuple(o, state, 0) {
                            val = Value::lit(k);
                        }
                    }
                }
                if let Some(top) = self.sum_obs.last_mut() {
                    top.push(crate::summary::Obs { file: file.clone(), line: ln, name, rows: obs_rows, must, value: val });
                }
                continue;
            }
            let row = if rows.is_empty() { None } else { Some(must.iter().fold(crate::domain::join_rows(rows), |r, k| r.store(*k, true))) };
            let value = self.value_info(&v, state);
            let tensor = if self.torch_on {
                // a line run several times in one analysis (a loop body): the join of its values
                let (chain, entry) = (self.call_chain.clone(), self.entry_run.is_some());
                let prev = self.t_row_values.get(&(ln, name.clone(), chain.clone(), entry)).cloned();
                let joined = match prev {
                    Some(p) => p.join(&v),
                    None => v.clone(),
                };
                self.t_row_values.insert((ln, name.clone(), chain, entry), joined.clone());
                Some(self.t_check_json(&joined, state))
            } else {
                None
            };
            self.row_checks.push(RowCheck { file: file.clone(), line: ln, name, row, ns: ns_used, value, tensor });
        }
    }
}

impl Analyzer {
    /// Recency abstraction, back-edge step: the recent objects allocated during a loop
    /// iteration are folded into their weak per-site summary objects, and every reference
    /// to them written during the iteration is re-pointed to the summary object.
    pub fn rename_recent(&mut self, back: &mut State, head: &State, recs: Vec<(ObjId, Kind, String, String)>, mark: ObjId) {
        if recs.is_empty() {
            return;
        }
        let mut map: FxHashMap<ObjId, ObjId> = FxHashMap::default();
        for (r, kind, site, ctx) in &recs {
            let (label, payload, iter_elem, derived, depth) = {
                let o = self.obj(*r);
                (o.label.clone(), o.payload.clone(), o.iter_elem, o.derived_from, o.depth)
            };
            let sid = self.alloc_raw(*kind, site, ctx, false, &label, payload);
            {
                let so = self.obj_mut(sid);
                so.single = false;
                so.literal_len = None;
                so.iter_elem = iter_elem;
                so.depth = depth;
                if so.derived_from.is_none() {
                    so.derived_from = derived;
                }
            }
            back.lens.insert(sid, (None, 0));
            map.insert(*r, sid);
        }
        let rename_value = |v: &Value, map: &FxHashMap<ObjId, ObjId>| -> Option<Value> {
            let hit_pts = v.pts.iter().any(|o| map.contains_key(&o));
            let hit_kv = matches!(&v.kv, KeyVal::KeysOf(ko) if ko.objs.iter().any(|o| map.contains_key(&o)));
            if !hit_pts && !hit_kv {
                return None;
            }
            let pts = v.pts.map(|o| *map.get(&o).unwrap_or(&o));
            let kv = match &v.kv {
                KeyVal::KeysOf(ko) if hit_kv => KeyVal::KeysOf(KeysOf { objs: ko.objs.map(|o| *map.get(&o).unwrap_or(&o)), ..ko.clone() }),
                other => other.clone(),
            };
            Some(Value::with_kv(pts, kv))
        };
        // 1. the recent objects' own facts move to the summary objects (weakly)
        for (&r, &sid) in &map {
            for ns in [Ns::Item, Ns::Attr] {
                if let Some(row) = back.rows.get(&(r, ns)) {
                    let nr = match back.rows.get(&(sid, ns)) {
                        Some(cur) => cur.join(&row),
                        None => row,
                    };
                    back.rows.insert((sid, ns), nr);
                    back.rows.remove(&(r, ns));
                }
            }
            for ((ns, k), v) in back.slots_of(r) {
                if ns == Ns::Pos {
                    continue;
                }
                let v = rename_value(&v, &map).unwrap_or(v);
                back.set_slot(sid, ns, k, v, false);
            }
            back.drop_obj(r);
            if let Some(kv) = back.elems.get(&r) {
                let kv = rename_value(&Value::kv_only(kv.clone()), &map).map(|v| v.kv).unwrap_or(kv);
                let nv = match back.elems.get(&sid) {
                    Some(cur) => crate::domain::kv_join(&cur, &kv),
                    None => kv,
                };
                back.elems.insert(sid, nv);
                back.elems.remove(&r);
            }
        }
        // 2. references written during the iteration
        let fp = back.footprint(head);
        for (oid, sk) in fp.slots {
            if map.contains_key(&oid) {
                continue;
            }
            if let Some(v) = back.slots.get(&oid, &sk) {
                if let Some(nv) = rename_value(&v, &map) {
                    back.slots.set(&oid, sk, nv);
                }
            }
        }
        for oid in fp.elems {
            if map.contains_key(&oid) {
                continue;
            }
            if let Some(kv) = back.elems.get(&oid) {
                if let Some(nv) = rename_value(&Value::kv_only(kv), &map) {
                    back.elems.insert(oid, nv.kv);
                }
            }
        }
        let env_updates: Vec<(StrId, Value)> = back.env.iter().filter_map(|(k, v)| rename_value(v, &map).map(|nv| (*k, nv))).collect();
        for (k, nv) in env_updates {
            back.env.insert(k, nv);
        }
        let nar_updates: Vec<(StrId, Value)> = back.narrowed.iter().filter_map(|(k, v)| rename_value(v, &map).map(|nv| (*k, nv))).collect();
        for (k, nv) in nar_updates {
            back.narrowed.insert(k, nv);
        }
        if let Some(r) = &back.ret {
            if let Some(nv) = rename_value(r, &map) {
                back.ret = Some(nv);
            }
        }
        for ov in self.frame_mut().overlays.iter_mut() {
            let ups: Vec<(StrId, Value)> = ov.iter().filter_map(|(k, v)| rename_value(v, &map).map(|nv| (*k, nv))).collect();
            for (k, nv) in ups {
                ov.insert(k, nv);
            }
        }
        // 3. objects allocated during the iteration that hold references in their payloads
        let n = self.objects.len() as ObjId;
        for oid in mark..n {
            let o = &mut self.objects[oid as usize];
            if let Some(d) = o.derived_from {
                if let Some(&sd) = map.get(&d) {
                    o.derived_from = Some(sd);
                }
            }
            let rv = |v: &Value| rename_value(v, &map).unwrap_or_else(|| v.clone());
            o.payload = match &o.payload {
                Payload::Method(q, v) => Payload::Method(q.clone(), rv(v)),
                Payload::Bound(f, v) => Payload::Bound(*map.get(f).unwrap_or(f), rv(v)),
                Payload::Super(c, v) => Payload::Super(*c, rv(v)),
                Payload::View(k, v) => Payload::View(*k, rv(v)),
                Payload::Zip(a, b) => Payload::Zip(rv(a), rv(b)),
                Payload::Enumerate(v, s0) => Payload::Enumerate(rv(v), *s0),
                Payload::Pair(a, b) => Payload::Pair(rv(a), rv(b)),
                Payload::Partial(f, args, kw) => Payload::Partial(rv(f), args.iter().map(&rv).collect(), kw.iter().map(|(k, v)| (*k, rv(v))).collect()),
                Payload::Groupby(fs, ks, ai, sel) => Payload::Groupby(fs.iter().map(|f| *map.get(f).unwrap_or(f)).collect(), ks.clone(), *ai, sel.clone()),
                Payload::Locidx(nm, x) => Payload::Locidx(nm.clone(), *map.get(x).unwrap_or(x)),
                other => other.clone(),
            };
        }
        // 4. flow-insensitive class attributes
        let ups: Vec<((ObjId, Ns, KeyId), Value)> = self.cls_slots.iter().filter_map(|(k, v)| rename_value(v, &map).map(|nv| (*k, nv))).collect();
        for (k, nv) in ups {
            self.cls_slots.insert(k, nv);
        }
    }
}

// ---------------------------------------------------------------------------
// AST helpers
// ---------------------------------------------------------------------------

pub fn chain_text(node: &ast::Expr) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    let mut n = node;
    while let ast::Expr::Attribute(a) = n {
        parts.push(a.attr.id.as_str());
        n = &a.value;
    }
    let ast::Expr::Name(nm) = n else { return None };
    parts.push(nm.id.as_str());
    parts.reverse();
    Some(parts.join("."))
}

pub fn is_main_guard(test: &ast::Expr) -> bool {
    let ast::Expr::Compare(c) = test else { return false };
    let ast::Expr::Name(l) = &*c.left else { return false };
    if l.id.as_str() != "__name__" || c.ops.len() != 1 || c.ops[0] != ast::CmpOp::Eq {
        return false;
    }
    matches!(&c.comparators[0], ast::Expr::StringLiteral(sl) if sl.value.to_str() == "__main__")
}

/// True if the loop body has a break/continue targeting this loop.
pub fn contains_jump(body: &[ast::Stmt]) -> bool {
    fn visit(n: &ast::Stmt) -> bool {
        match n {
            ast::Stmt::Break(_) | ast::Stmt::Continue(_) => true,
            ast::Stmt::For(f) => f.orelse.iter().any(visit),
            ast::Stmt::While(w) => w.orelse.iter().any(visit),
            ast::Stmt::FunctionDef(_) | ast::Stmt::ClassDef(_) => false,
            ast::Stmt::If(i) => i.body.iter().any(visit) || i.elif_else_clauses.iter().any(|c| c.body.iter().any(visit)),
            ast::Stmt::With(w) => w.body.iter().any(visit),
            ast::Stmt::Try(t) => {
                t.body.iter().any(visit)
                    || t.handlers.iter().any(|h| {
                        let ast::ExceptHandler::ExceptHandler(h) = h;
                        h.body.iter().any(visit)
                    })
                    || t.orelse.iter().any(visit)
                    || t.finalbody.iter().any(visit)
            }
            ast::Stmt::Match(m) => m.cases.iter().any(|c| c.body.iter().any(visit)),
            _ => false,
        }
    }
    body.iter().any(visit)
}

pub fn is_generator(body: &[ast::Stmt]) -> bool {
    struct V(bool);
    impl<'a> ast::visitor::source_order::SourceOrderVisitor<'a> for V {
        fn visit_expr(&mut self, e: &'a ast::Expr) {
            if matches!(e, ast::Expr::Yield(_) | ast::Expr::YieldFrom(_)) {
                self.0 = true;
            }
            if !self.0 {
                ast::visitor::source_order::walk_expr(self, e);
            }
        }
        fn visit_stmt(&mut self, st: &'a ast::Stmt) {
            // nested functions own their yields in Python too, but ast.walk descends into them:
            // the prototype marks the outer function as well; keep that behaviour
            if !self.0 {
                ast::visitor::source_order::walk_stmt(self, st);
            }
        }
    }
    let mut v = V(false);
    for st in body {
        use ast::visitor::source_order::SourceOrderVisitor;
        v.visit_stmt(st);
        if v.0 {
            return true;
        }
    }
    false
}

pub fn dotted_last(n: &ast::Expr) -> String {
    match n {
        ast::Expr::Name(x) => x.id.to_string(),
        ast::Expr::Attribute(a) => a.attr.id.to_string(),
        ast::Expr::Call(c) => dotted_last(&c.func),
        other => frontend::text(other),
    }
}

/// All names used in an expression (ast.walk + isinstance(ast.Name)).
pub fn names_in(e: &ast::Expr) -> Vec<String> {
    struct V(Vec<String>);
    impl<'a> ast::visitor::source_order::SourceOrderVisitor<'a> for V {
        fn visit_expr(&mut self, e: &'a ast::Expr) {
            if let ast::Expr::Name(n) = e {
                self.0.push(n.id.to_string());
            }
            ast::visitor::source_order::walk_expr(self, e);
        }
    }
    use ast::visitor::source_order::SourceOrderVisitor;
    let mut v = V(Vec::new());
    v.visit_expr(e);
    v.0
}

pub fn contains_call(e: &ast::Expr) -> bool {
    struct V(bool);
    impl<'a> ast::visitor::source_order::SourceOrderVisitor<'a> for V {
        fn visit_expr(&mut self, e: &'a ast::Expr) {
            if matches!(e, ast::Expr::Call(_)) {
                self.0 = true;
            }
            ast::visitor::source_order::walk_expr(self, e);
        }
    }
    use ast::visitor::source_order::SourceOrderVisitor;
    let mut v = V(false);
    v.visit_expr(e);
    v.0
}

pub fn literal_key_of(e: &ast::Expr) -> Option<KeyId> {
    Some(match e {
        ast::Expr::StringLiteral(sl) => key_str_lit(sl.value.to_str()),
        ast::Expr::NumberLiteral(n) => match &n.value {
            ast::Number::Int(i) => intern::key_int(i.as_i64()?),
            ast::Number::Float(f) => intern::key_float(*f),
            ast::Number::Complex { .. } => return None,
        },
        ast::Expr::BooleanLiteral(b) => intern::key_bool(b.value),
        ast::Expr::NoneLiteral(_) => intern::key_none(),
        ast::Expr::BytesLiteral(b) => intern::key(intern::Key::Bytes(b.value.bytes().collect())),
        ast::Expr::EllipsisLiteral(_) => intern::key(intern::Key::Ellipsis),
        _ => return None,
    })
}

pub fn is_constant(e: &ast::Expr) -> bool {
    matches!(
        e,
        ast::Expr::StringLiteral(_) | ast::Expr::NumberLiteral(_) | ast::Expr::BooleanLiteral(_) | ast::Expr::NoneLiteral(_) | ast::Expr::BytesLiteral(_) | ast::Expr::EllipsisLiteral(_)
    )
}

pub const BUILTIN_NAMES: &[&str] = &[
    "dict", "list", "set", "tuple", "len", "print", "str", "int", "float", "bool", "range", "enumerate", "zip", "sorted", "min", "max", "sum",
    "any", "all", "isinstance", "type", "open", "getattr", "setattr", "hasattr", "repr", "iter", "next", "abs", "round", "map", "filter",
    "reversed", "id", "hash", "object", "Exception", "KeyError", "ValueError", "TypeError", "AttributeError", "IndexError", "LookupError",
    "RuntimeError", "property", "staticmethod", "classmethod", "super", "vars", "format", "frozenset", "bytes", "input",
    "NotImplementedError", "StopIteration", "BaseException", "callable", "chr", "ord", "divmod", "pow", "slice", "issubclass", "memoryview",
    "bytearray", "complex", "globals", "locals", "exec", "eval", "compile", "delattr", "help", "hex", "oct", "bin", "ascii", "breakpoint",
    "FileNotFoundError", "OSError", "IOError", "ZeroDivisionError", "AssertionError", "NameError", "ImportError", "ModuleNotFoundError",
    "PermissionError", "TimeoutError", "ConnectionError", "StopAsyncIteration", "GeneratorExit", "KeyboardInterrupt", "SystemExit", "Warning",
    "UserWarning", "DeprecationWarning", "__name__", "__file__", "__doc__", "NotImplemented", "Ellipsis", "None", "True", "False", "anext",
    "aiter", "__import__", "__build_class__", "__debug__", "__loader__", "__package__", "__spec__", "ArithmeticError", "BlockingIOError",
    "BrokenPipeError", "BufferError", "BytesWarning", "ChildProcessError", "ConnectionAbortedError", "ConnectionRefusedError",
    "ConnectionResetError", "EOFError", "EnvironmentError", "FloatingPointError", "FutureWarning", "ImportWarning", "IndentationError",
    "InterruptedError", "IsADirectoryError", "MemoryError", "NotADirectoryError", "OverflowError", "PendingDeprecationWarning",
    "ProcessLookupError", "RecursionError", "ReferenceError", "ResourceWarning", "RuntimeWarning", "SyntaxError", "SyntaxWarning",
    "SystemError", "TabError", "UnboundLocalError", "UnicodeDecodeError", "UnicodeEncodeError", "UnicodeError", "UnicodeTranslateError",
    "UnicodeWarning", "copyright", "credits", "exit", "license", "quit",
];

pub fn is_builtin_name(n: &str) -> bool {
    BUILTIN_NAMES.contains(&n)
}

/// The dotted package a file belongs to: the chain of enclosing directories that have an
/// `__init__.py` (empty for a file outside any package).
pub fn package_of_file(path: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut dir = std::path::Path::new(path).parent().map(|p| p.to_path_buf());
    while let Some(d) = dir {
        if !d.join("__init__.py").exists() {
            break;
        }
        match d.file_name() {
            Some(n) => parts.push(n.to_string_lossy().to_string()),
            None => break,
        }
        dir = d.parent().map(|p| p.to_path_buf());
    }
    parts.reverse();
    parts.join(".")
}

/// The directory that holds the outermost package of `path` (the import root for `import
/// pkg.sub` from inside the package), when the file is inside a package.
pub fn package_root_of_file(path: &str) -> Option<String> {
    let mut dir = std::path::Path::new(path).parent().map(|p| p.to_path_buf())?;
    if !dir.join("__init__.py").exists() {
        return None;
    }
    while dir.join("__init__.py").exists() {
        dir = dir.parent()?.to_path_buf();
    }
    Some(dir.to_string_lossy().to_string())
}

/// A requirement-only run keeps what the function itself demands: of its parameters, and of
/// the inputs it reads on its own lines (`json.loads@L20` inside it; main module only, where
/// a source's line is unambiguous).
fn own_source(label: &str, own: &str, span: (u32, u32), in_main: bool) -> bool {
    if label.starts_with(own) {
        return true;
    }
    if !in_main || label.starts_with("param ") {
        return false;
    }
    let site = label.split('[').next().unwrap_or("");
    match site.rsplit_once("@L").and_then(|(_, n)| n.parse::<u32>().ok()) {
        Some(n) => span.0 <= n && n <= span.1,
        None => false,
    }
}
