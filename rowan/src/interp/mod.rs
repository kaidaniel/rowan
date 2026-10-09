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

pub const SAFE_METHODS: &[&str] = &[
    "dict.get", "dict.keys", "dict.values", "dict.items", "dict.setdefault", "dict.copy", "list.append", "str.upper", "str.lower",
    "str.strip", "str.title", "str.format", "str.join",
];
pub const STR_METHOD_NAMES: &[&str] = &[
    "strip", "lower", "upper", "title", "split", "replace", "startswith", "endswith", "format", "join", "lstrip", "rstrip", "encode",
    "decode", "isdigit", "isalpha", "zfill", "casefold", "capitalize", "partition", "rsplit", "splitlines", "find", "count", "ljust",
    "rjust", "center",
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
pub const ITEM_GUARDS: &[&str] = &["KeyError", "LookupError", "Exception", "BaseException", "IndexError", "*"];
pub const ATTR_GUARDS: &[&str] = &["AttributeError", "Exception", "BaseException", "*"];
pub const KNOWN_IDENTITY_DECORATORS: &[&str] = &[
    "functools.lru_cache", "functools.cache", "functools.wraps", "contextlib.contextmanager", "abc.abstractmethod", "typing.overload",
    "typing.final", "builtins.staticmethod", "builtins.classmethod", "pytest.fixture", "asyncio.coroutine", "dataclasses.dataclass",
    "typing.no_type_check", "functools.total_ordering", "typing.runtime_checkable",
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
    /// subclasses pydantic's `BaseModel`: annotated fields like a dataclass (other members
    /// come from pydantic: the base stays open)
    pub pydantic: bool,
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

/// Everything a model needs about a call.
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
}

pub struct RowCheck {
    pub line: u32,
    pub name: String,
    pub row: Option<Row>,
    pub ns: Ns,
    /// the name's value: its known literals, whether it may be anything else, its objects
    pub value: ValueInfo,
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
    /// per enclosing try (parallel to `raise_states`): the exceptions raised at those points
    /// (what the handler's `as` name holds)
    pub raise_values: Vec<Option<Value>>,
    /// the exception the next `record_raise_state` is about
    pub cur_exc: Option<Value>,
    /// exception names handled by the running `except` clauses (innermost last): what a bare
    /// `raise` re-raises
    pub handling: Vec<FxHashSet<String>>,
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
    pub cm_frames: Vec<(usize, IndexMap<StrId, Value>)>,
    /// the next user call is analysed concretely (no summary)
    pub force_tab_once: bool,
    /// `vars(obj)` dicts: stores into them are stores of obj's attributes
    pub attr_mirrors: FxHashMap<ObjId, ObjId>,
    /// results of unknown calls: (the call as written, its line, what it was given — which
    /// unknown code may return)
    pub origins: FxHashMap<ObjId, (String, u32, Vec<String>)>,
    /// Enum classes: their members in definition order
    pub enum_members: FxHashMap<ObjId, Vec<(String, ObjId)>>,
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
    /// `#@row NAME:` checks requested by the golden harness: (line, name)
    pub row_check_requests: Vec<(u32, String)>,
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
            raise_values: Vec::new(),
            cur_exc: None,
            handling: Vec::new(),
            cmp_right: None,
            comp_cells: Vec::new(),
            key_alias: None,
            cm_hook: None,
            cm_frames: Vec::new(),
            force_tab_once: false,
            attr_mirrors: FxHashMap::default(),
            origins: FxHashMap::default(),
            enum_members: FxHashMap::default(),
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
            fresh_in_progress: FxHashSet::default(),
            main_module: 0,
            main_tree: tree,
            series: 0,
            none_v: Value::lit(intern::key_none()),
            row_check_requests: Vec::new(),
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
            if existed == Some(oid) && self.is_live(oid, state) {
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
                    // one summary object for both, weak
                    self.obj_mut(oid).single = false;
                }
            }
            oid
        };
        let obj_single = self.obj(oid).single;
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
    pub fn alloc_at(&mut self, kind: Kind, node: &Node, state: &mut State, label: &str, payload: Payload, row: Option<Row>, ns: Ns, single: Option<bool>, ctx: Option<&str>) -> ObjId {
        let site = site_string(node);
        self.alloc(kind, &site, state, label, payload, row, ns, single, ctx)
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
        oid
    }

    // ------------------------------------------------------------ reporting
    pub fn file_of_frame(&self) -> String {
        let m = self.frame().module;
        if m == self.main_module {
            return String::new();
        }
        let mi = self.module_info(m);
        let p = mi.path.clone().unwrap_or_else(|| mi.name.clone());
        std::path::Path::new(&p).file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or(p)
    }

    pub fn verdict(&mut self, node: &Node, key: KeyId, cls: &Classification, oid: ObjId, ns: Ns, file_override: Option<&str>, text_override: Option<&str>) {
        if matches!(cls.tier, Tier::Map | Tier::Sym) {
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
        });
    }

    pub fn node_text(&self, node: &Node) -> String {
        if node.addr == 0 {
            return String::new();
        }
        frontend::text_at(node)
    }

    pub fn note(&mut self, msg: String) {
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
            self.analyse_uncalled_range(start, n, state);
            start = n;
        }
    }

    fn analyse_uncalled_range(&mut self, from: ObjId, n: ObjId, state: &mut State) {
        for oid in from..n {
            if self.kind(oid) != Kind::Function || self.analysed_funcs.contains(&oid) {
                continue;
            }
            let fi = self.func_info(oid);
            // closures made inside a summary run stand for every call's closure (their cells
            // are relative): they are reached through the real calls
            if !fi.closure.is_empty() && (self.obj(oid).ctx.contains("sum") || disabled("entryclosures")) {
                continue;
            }
            if matches!(fi.node, FuncNode::Lambda(_)) && disabled("entryclosures") {
                continue;
            }
            if fi.module != self.main_module {
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
            let mut st = state.copy();
            st.ret = None;
            let mut args: Vec<Value> = Vec::new();
            let mi = self.module_info(fi.module);
            self.frames.push(Frame::new(FrameKind::Module, mi.scopes.module(), fi.module, &format!("entry:{}", fi.qualname), true, "<entry>"));
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
            for a in params {
                let is_vararg = fparams.vararg.as_deref().map(|v| std::ptr::eq(v, a)).unwrap_or(false);
                let is_kwarg = fparams.kwarg.as_deref().map(|v| std::ptr::eq(v, a)).unwrap_or(false);
                if is_vararg || is_kwarg {
                    // a caller may pass any arguments here
                    if !disabled("entrystar") {
                        let poid = self.unknown_source(&format!("param {}.{}", fi.qualname, a.name.id), SourceKind::Param, &mut st, &format!("param {}", a.name.id), "", Ns::Item, vec![]);
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
                    let poid = self.unknown_source(&format!("param {}.{}", fi.qualname, a.name.id), SourceKind::Param, &mut st, &format!("param {}", a.name.id), "", Ns::Item, vec![]);
                    args.push(d.join(&Value::obj(poid)));
                    continue;
                }
                let mut v = match &a.annotation {
                    Some(ann) => self.param_from_annotation(ann, &fi, &mut st),
                    None => None,
                };
                if v.is_none() {
                    let poid = self.unknown_source(&format!("param {}.{}", fi.qualname, a.name.id), SourceKind::Param, &mut st, &format!("param {}", a.name.id), "", Ns::Item, vec![]);
                    v = Some(Value::obj(poid));
                }
                args.push(v.unwrap());
            }
            let fake = Node::synthetic(mi.idx, fi.node.node().line);
            self.call_user(oid, self_val, args, IndexMap::new(), star_args, star_kwargs, &mut st, &fake, None, true);
            self.frames.pop();
            // keep schema requirements discovered in entry points
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
    pub fn param_from_annotation(&mut self, ann: &'static ast::Expr, fi: &FuncInfo, state: &mut State) -> Option<Value> {
        let node = Self::strip_annotation(ann)?;
        let v = self.eval(node, state);
        for oid in v.pts.iter() {
            let o = self.kind(oid);
            if o == Kind::Class {
                let ci = self.class_info(oid);
                if ci.has_fields() {
                    let fields: KeySet = KeySet::from_iter(ci.fields().iter().map(|(f, _)| key_str_lit(f)));
                    let ns = if ci.typed_dict { Ns::Item } else { Ns::Attr };
                    let inst = self.alloc(
                        Kind::Instance,
                        &format!("param:{}:{}", fi.qualname, ci.name),
                        state,
                        &format!("{} param", ci.name),
                        Payload::Cls(oid),
                        Some(Row::closed_with(fields.clone())),
                        ns,
                        Some(true),
                        Some(&format!("entry:{}", fi.qualname)),
                    );
                    for f in fields.iter() {
                        let v = self.typed_field_value(oid, &intern::key_py_str(f), state, &format!("entry:{}", fi.qualname));
                        state.set_slot(inst, ns, f, v, true);
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
                        args.push(d.clone());
                        continue;
                    }
                    let v = match &a.annotation {
                        Some(ann) => self.param_from_annotation(ann, &fi, state),
                        None => None,
                    };
                    args.push(match v {
                        Some(v) => v,
                        None => Value::obj(self.unknown_source(&format!("param {}.{}", fi.qualname, a.name.id), SourceKind::Param, state, &format!("param {}", a.name.id), "", Ns::Item, vec![])),
                    });
                }
                let fake = Node::synthetic(fi.node.node().module, fi.node.node().line);
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
    pub fn record_resolved_fact(&mut self, line: u32, name: StrId, v: &Value, rows: Vec<(ObjId, Option<Row>, Option<Row>)>, state: &State) {
        let mut scratch = state.copy_ref();
        for (o, item, attr) in &rows {
            scratch.drop_obj(*o);
            if let Some(r) = item {
                scratch.set_row(*o, Ns::Item, r.clone());
            }
            if let Some(r) = attr {
                scratch.set_row(*o, Ns::Attr, r.clone());
            }
        }
        let text = crate::report::render_value(self, &scratch, v, 0, &mut Vec::new());
        let e = self.facts.entry((line, name, false)).or_insert_with(|| Fact { symbolic: false, value: Value::new(), rows: IndexMap::new(), texts: Vec::new() });
        e.value = e.value.join(v);
        if !e.texts.contains(&text) && e.texts.len() < 4 {
            e.texts.push(text);
        }
        let join_opt = |a: &Option<Row>, b: &Option<Row>| -> Option<Row> {
            match (a, b) {
                (Some(x), Some(y)) => Some(x.join(y)),
                (Some(x), None) | (None, Some(x)) => Some(x.clone()),
                _ => None,
            }
        };
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

    pub fn record_fact(&mut self, node: &Node, name: StrId, v: &Value, state: &State) {
        if !self.facts_on || node.addr == 0 {
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
            if let Some(top) = self.sum_fact_obs.last_mut() {
                if top.len() < 4096 {
                    top.push(crate::summary::FactObs { line: node.line, name, value: v.clone(), rows: rows.clone() });
                }
            }
        }
        let text = crate::report::render_value(self, state, v, 0, &mut Vec::new());
        let e = self.facts.entry((node.line, name, symbolic)).or_insert_with(|| Fact { symbolic, value: Value::new(), rows: IndexMap::new(), texts: Vec::new() });
        e.value = e.value.join(v);
        if !e.texts.contains(&text) && e.texts.len() < 4 {
            e.texts.push(text);
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
    pub fn value_info(&self, v: &Value) -> ValueInfo {
        let mut info = ValueInfo::default();
        if v.kv.is_literal() {
            info.lits = crate::domain::kv_literals(&v.kv).iter().collect();
        } else if !v.kv.is_bot() {
            info.unknown = true;
        }
        for o in v.pts.iter() {
            match self.kind(o) {
                Kind::Unknown | Kind::Ph | Kind::External => info.unknown = true,
                _ => info.objects.push(self.obj(o).label.clone()),
            }
        }
        if v.pts.is_empty() && v.kv.is_bot() {
            info.unknown = true;
        }
        info
    }

    /// Row-check support for the golden harness: after a statement at a requested line.
    pub fn after_stmt(&mut self, stmt: &ast::Stmt, state: &mut State) {
        if self.row_check_requests.is_empty() {
            return;
        }
        let n = frontend::node(stmt);
        if n.module != self.module_info(self.main_module).idx {
            return;
        }
        let reqs: Vec<(u32, String)> = self.row_check_requests.iter().filter(|(ln, _)| *ln == n.line).cloned().collect();
        for (ln, name) in reqs {
            let v = self.load_name(s(&name), &n, state);
            let mut rows: Vec<Row> = Vec::new();
            let mut ns_used = Ns::Item;
            let mut obs_rows: Vec<(ObjId, Ns, Row)> = Vec::new();
            for oid in v.pts.iter() {
                let typed = self.kind(oid) == Kind::Ph && self.ph_info(oid).cls.get().map(|c| self.class_info_opt(c).is_some()).unwrap_or(false);
                let ns = if matches!(self.kind(oid), Kind::Instance | Kind::Namespace) || typed { Ns::Attr } else { Ns::Item };
                ns_used = ns;
                rows.push(state.row(oid, ns));
                obs_rows.push((oid, ns, state.row(oid, ns)));
            }
            let must: Vec<KeyId> = state.name_keys.iter().filter(|(n, _)| *intern::str_get(*n) == *name).map(|(_, k)| *k).collect();
            // inside a summary the rows are relative to its entry: kept with the summary and
            // resolved per call
            if !self.sum_stack.is_empty() && !disabled("obs") {
                if let Some(top) = self.sum_obs.last_mut() {
                    top.push(crate::summary::Obs { line: ln, name, rows: obs_rows, must, value: v.clone() });
                }
                continue;
            }
            let row = if rows.is_empty() { None } else { Some(must.iter().fold(crate::domain::join_rows(rows), |r, k| r.store(*k, true))) };
            let value = self.value_info(&v);
            self.row_checks.push(RowCheck { line: ln, name, row, ns: ns_used, value });
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
