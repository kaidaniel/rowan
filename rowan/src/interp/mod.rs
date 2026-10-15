//! Abstract interpreter over the Python AST.  Port of
//! `rowan/interp.py`; the statement/expression/narrowing/access/call parts live in the
//! sibling files of this module.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use indexmap::IndexMap;
use ruff_python_ast as ast;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::domain::{
    Chain, Classification, KeyVal, Ns, ObjId, Row, Source, SourceKind, Tier, Value,
};
use crate::frontend::{self, Node};
use crate::intern::{self, key_str_lit, s, KeyId, KeySet, StrId};
use crate::models::Registry;
use crate::scopes::{self as scopemod, Scopes};
use crate::state::{Kind, State};

pub mod access;
pub mod calls;
pub mod effects;
pub mod alloc;
pub mod exprs;
pub mod funcs;
pub mod gc;
pub mod heap;
pub use heap::*;
pub mod narrow;
pub use crate::models::tables::*;
pub mod stmts;

pub const UNROLL_MAX: usize = 16;
pub const LOOP_MAX_ITER: usize = 40;
pub const REC_MAX_ITER: usize = 8;
pub const PATH_DEPTH: u32 = 3;
pub const MAX_ACTIVE: usize = 4;
/// records an unknown call's result joins at most (past that: an open row)
pub const MAX_ARG_JOIN: usize = 64;
/// a generator's suspension point: the yield node, the `yield from` item, the unrolled loop
/// positions
pub type YieldKey = (usize, usize, Vec<usize>);

/// The states of one generator activation at its suspension points, in order, keyed by the
/// yield site and its unroll path (a key may repeat), with the first index of each key.
#[derive(Default)]
pub struct YieldStates {
    pub list: Vec<(YieldKey, State)>,
    first: FxHashMap<YieldKey, usize>,
}

impl YieldStates {
    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    pub fn push(&mut self, key: YieldKey, st: State) {
        self.first.entry(key.clone()).or_insert(self.list.len());
        self.list.push((key, st));
    }

    /// The first state recorded under `key`.
    pub fn get_mut(&mut self, key: &YieldKey) -> Option<&mut State> {
        let i = *self.first.get(key)?;
        Some(&mut self.list[i].1)
    }

    pub fn states(&self) -> impl Iterator<Item = &State> {
        self.list.iter().map(|(_, s)| s)
    }
}
/// havoc walks remembered (`havoc_with`); past this many the table starts over
pub const HAVOC_MEMOS: usize = 1024;
/// what the cache of code closures (`code_closure`) may hold; past it the cache starts over
pub const CLOSURE_CACHE: usize = 1 << 18;

/// What a havoc walk met and the heap it left (`havoc_with`).
pub struct HavocMemo {
    pub roots: Vec<ObjId>,
    pub rows: crate::layered::LayerMap<(ObjId, Ns), Row>,
    pub slots: crate::layered::TwoLevel<ObjId, crate::state::SlotKey, Value>,
    pub elems: crate::layered::LayerMap<ObjId, KeyVal>,
    pub lens: crate::layered::LayerMap<ObjId, (Option<usize>, usize, u16)>,
    pub visited: FxHashSet<ObjId>,
    pub code: Vec<ObjId>,
}
/// Call sites kept in an allocation context (deeper chains: a hash of the prefix).
pub const CHAIN: usize = 2;
/// Statements a function code may run in all its calls (`ROWAN_LIMITS=budget=N`): past them
/// its calls are unknown code.
pub const FN_BUDGET: usize = 4096;
/// A safety net, not the bound on work (the budget is): past this many milliseconds
/// (`ROWAN_LIMITS=ms=N`) calls of user code are unknown calls and running bodies stop.
pub const DEADLINE_MS: usize = 10000;

/// `ROWAN_LIMITS=budget=N,ms=N`: the budget and the deadline (the only two knobs).

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
    debug_level() > 0
}

pub const VALUE_GUARDS: &[&str] = &["ValueError", "Exception", "BaseException", "*"];
pub const VALIDATION_GUARDS: &[&str] = &["ValidationError", "ValueError", "Exception", "BaseException", "*"];
pub const ITEM_GUARDS: &[&str] = &["KeyError", "LookupError", "Exception", "BaseException", "IndexError", "*"];
pub const ATTR_GUARDS: &[&str] = &["AttributeError", "Exception", "BaseException", "*"];

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
    pub providers: Vec<crate::domain::SourceId>,
    /// unknown code that may have added or removed the key (its call sites)
    pub mutators: Vec<String>,
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
}

impl FuncInfo {
    /// The qualified name as Python spells it (no `class ` markers): `Conn.__init__`.
    pub fn py_qualname(&self) -> String {
        self.qualname.replace("class ", "")
    }
}

impl FuncInfo {
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
    /// the body did not finish normally (it raised or returned) — on every run
    pub body_died: bool,
    /// the caller's names before the body (each run at another yield starts from them)
    pub env_before: Option<IndexMap<StrId, Value>>,
    /// the body may run again at another reachable yield (not a `then` callback: it runs once)
    pub multi: bool,
    /// the caller's exits from the body (its `return`s), joined over the runs: the caller's
    /// once the with statement ends (the generator's own are restored after each run)
    pub body_exits: Option<State>,
    /// the values the body returned (with `body_exits`)
    pub body_ret: Option<Value>,
}

/// An exception class's name and its bases' names; `true`: that is the whole MRO.
pub type ExcNames = (std::rc::Rc<Vec<String>>, bool);
pub type RaiseRec = (Option<ExcNames>, State, Value);

/// The states of a try's recorded raises, joined (None: no raise recorded).
pub fn join_raise_recs(recs: &[RaiseRec]) -> Option<State> {
    State::join_all(recs.iter().map(|r| r.1.copy()).collect())
}

/// The class of an exception that code rowan cannot see (or a model that does not say which)
/// may raise: any class, which no handler surely catches or surely misses.
pub fn any_exc() -> ExcNames {
    (std::rc::Rc::new(Vec::new()), false)
}

/// Is this the class `any_exc` stands for?
pub fn is_any_exc(c: &Option<ExcNames>) -> bool {
    c.as_ref().is_some_and(|(n, full)| n.is_empty() && !full)
}

/// A running try context. Every point of its body (and of the calls it makes) that may raise
/// records the state there: a handler starts from the join of the states whose exceptions it
/// may catch, a finally on an exceptional way out from those it does not.
pub struct TryCtx {
    /// the exception class names its handlers catch (`*`: a bare except; `!reraise`: every
    /// handler ends by re-raising)
    pub caught: FxHashSet<String>,
    /// the frame depth and caller-env depth it runs in: a raise inside a callee hands the
    /// handler the try frame's locals
    pub frames: usize,
    pub envs: usize,
    /// the state before it and the allocation serial then (what was made since, and nothing
    /// reaches after the unwind, is garbage in a raise state)
    pub pre: State,
    pub mark: u64,
    /// the raises recorded, per class: (class, the states, the exceptions), joined when it ends
    pub recs: Vec<(Option<ExcNames>, crate::state::Joins, Value)>,
    /// the last raise recorded, before collection (a loop of calls raising at one point adds
    /// nothing)
    pub last: Option<(State, Option<ExcNames>, Value)>,
}

/// What a try context recorded while its body ran (`Analyzer::leave_try`).
pub struct TryExit {
    /// the states and exceptions at the recorded raises, joined per class
    pub recs: Vec<RaiseRec>,
}

impl TryExit {
    /// The join of the states at the recorded raises (None: none recorded).
    pub fn raised(&self) -> Option<State> {
        join_raise_recs(&self.recs)
    }

    /// The exceptions raised at the recorded points.
    pub fn value(&self) -> Value {
        crate::domain::join_values(self.recs.iter().map(|r| r.2.clone()).collect::<Vec<_>>())
    }

    /// Code that may raise anything ran (a raise of `any_exc`).
    pub fn any(&self) -> bool {
        self.recs.iter().any(|r| is_any_exc(&r.0))
    }
}

pub use crate::models::exceptions::{builtin_exc_names, EXC_BASES};

#[derive(Debug)]
pub struct ClassInfo {
    pub name: String,
    pub node: &'static ast::StmtClassDef,
    pub bases: Vec<ObjId>,
    pub module: ObjId,
    /// inherits from something we do not see
    pub open_base: bool,
    pub dataclass_fields: RefCell<Option<Vec<(String, Option<&'static ast::Expr>)>>>,
    /// what its direct library bases make of it (`dict`, `TypedDict`, `NamedTuple`, pydantic's
    /// `BaseModel`, `nn.Module`, …)
    pub lib: crate::models::ClassBase,
    /// decorated with `pydantic.dataclasses.dataclass`: validated like a model, no model API
    pub pyd_dataclass: std::cell::Cell<bool>,
    /// `@dataclass(frozen=True)` / `@dataclass(kw_only=True)`
    pub dc_frozen: std::cell::Cell<bool>,
    pub dc_kw_only: std::cell::Cell<bool>,
    /// `@dataclass(order=True)`: instances of the class compare their fields as tuples
    pub dc_order: std::cell::Cell<bool>,
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
    /// the function's code (`FuncNode::addr`), whose statements count against its budget
    pub code: usize,
    pub cells: IndexMap<StrId, ObjId>,
    pub class_oid: Option<ObjId>,
    /// the variables of the running comprehensions (innermost last): their values are in the
    /// state's names, the enclosing ones' put back when the comprehension ends
    pub comp_names: Vec<Vec<StrId>>,
    pub loop_depth: u32,
    pub label: String,
    /// indices of the enclosing unrolled loop iterations (part of the call context)
    pub unroll: Vec<usize>,
    /// a call: the objects allocated from here on are its own, and the state before
    /// it (a `return` drops its own objects nothing can reach any more)
    pub pre_call: Option<PreCall>,
}

/// What a generator's body does to objects older than its call, by how far it has run: the
/// facts of those objects before it ran (`pre`), at its k-th yield (`at[k]`), at its end,
/// and joined over all of them.
pub struct GenEffects {
    pub rows: Vec<(ObjId, Ns)>,
    pub slots: Vec<(ObjId, crate::state::SlotKey)>,
    pub elems: Vec<ObjId>,
    pub lens: Vec<ObjId>,
    pub pre: State,
    pub at: Vec<Option<State>>,
    pub end: Option<State>,
    pub join: State,
}

/// What code changed of an object: one stored key, or anything (None).
pub type Change = (ObjId, Option<crate::state::SlotKey>);

/// A generator run whole for a consumer that runs code between its items: the state after
/// the run, the objects the run changed, what it read after a yield, the budgets before it.
pub struct GenWatch {
    pub gid: ObjId,
    pub after: State,
    pub own: rustc_hash::FxHashSet<ObjId>,
    pub reads: rustc_hash::FxHashSet<Change>,
    /// the budgets spent before the run (a run again of the code gets them back)
    pub budget: std::rc::Rc<FxHashMap<usize, usize>>,
    /// the birth mark after the run: objects born since are new
    pub born: u64,
}

/// A generator call whose body has not run: it runs at the first consumption, from the state
/// there (`force_gen`), in the call's own site and context, so the generator object is the same.
pub struct LazyGen {
    pub foid: ObjId,
    pub self_v: Option<Value>,
    pub args: Vec<Value>,
    pub kwargs: IndexMap<StrId, Value>,
    pub node: Node,
    pub site: String,
    pub ctx: (String, bool),
    pub chain: Vec<u32>,
}

/// (see `Frame::pre_call`)
#[derive(Clone)]
pub struct PreCall(pub u64, pub std::rc::Rc<State>);

impl std::fmt::Debug for PreCall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PreCall({})", self.0)
    }
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
            code: 0,
            cells: IndexMap::new(),
            class_oid: None,
            comp_names: Vec::new(),
            loop_depth: 0,
            label: label.to_string(),
            unroll: Vec::new(),
            pre_call: None,
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

impl EnumKind {
    pub fn join(&mut self, o: EnumKind) {
        (self.mixed, self.repr_value, self.flag, self.is_str) = (self.mixed | o.mixed, self.repr_value | o.repr_value, self.flag | o.flag, self.is_str | o.is_str);
    }
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
}

/// Where a narrowed value is written back (`_narrow_isinstance`'s `store` closure).
#[derive(Clone)]
pub enum StoreTarget {
    Name(StrId),
    Narrowed(StrId),
    Slot(ObjId, Ns, KeyId),
}

/// Facts about one name at one line of the main module, joined over every execution of
/// the statement (hover support): the value it held and the rows of the objects it pointed to.
#[derive(Clone)]
pub struct Fact {
    /// object -> (item row, attr row) as they were when the name was read / written
    pub rows: IndexMap<ObjId, (Option<Row>, Option<Row>)>,
    /// the value rendered (`report::render_value`) at each recording, distinct texts
    pub texts: Vec<String>,
    /// the symbols the texts mention, with what is known about each (`report::value_legend`)
    pub legend: Vec<(String, String)>,
    /// display names of the tensor dims the texts mention (`B` for x.shape[0])
    pub t_names: FxHashMap<crate::models::shape::AtomId, String>,
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

/// When the analysis started (the deadline counts from it).
#[derive(Clone, Copy)]
pub struct Clock(std::time::Instant);

impl Default for Clock {
    fn default() -> Self {
        Clock(std::time::Instant::now())
    }
}

impl Clock {
    pub fn elapsed(&self) -> std::time::Duration {
        self.0.elapsed()
    }
}

#[derive(Default)]
pub struct Analyzer {
    /// the library models' own tables (`models::Tables`)
    pub m: crate::models::Tables,
    pub filename: String,
    /// statements run per function code (`FuncNode::addr`: every closure object made from one
    /// `def` shares the budget)
    pub tab_count: FxHashMap<usize, usize>,
    /// every body run of a call (the golden `#@tabulations <= N` bound)
    pub tab_runs: usize,
    /// `__next__` functions whose calls were not decided once (`unroll_next`)
    pub no_unroll_next: FxHashSet<ObjId>,
    pub fresh_init: FxHashSet<ObjId>,
    /// per comprehension generator level: whether its items are known (unrolled)
    pub comp_known: Vec<bool>,
    /// recency abstraction: per loop iteration being analysed (innermost last), the recent
    /// objects allocated in it: (recent oid, kind, site, ctx)
    pub recent_stack: Vec<Vec<(ObjId, Kind, String, String)>>,
    pub search_paths: Vec<String>,
    /// installed packages to follow into site-packages (top-level names; `*` for all)
    pub follow: Vec<String>,
    /// site-packages directories searched for followed packages
    pub site_paths: Vec<String>,
    pub analyse_entry_points: bool,
    /// the objects (`interp/heap.rs`)
    pub heap: Heap,
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
    /// per running activation: the heap it started from, that heap grown by the heaps its
    /// recursive calls were made in (on older objects they reach), and its birth mark
    pub rec_heap: FxHashMap<(ObjId, String), (std::rc::Rc<State>, Option<State>, u64)>,
    pub loop_stack: Vec<LoopCtx>,
    /// the finally blocks of the running `try` statements: (`loop_stack` depth at the try,
    /// the block, not running yet): a `break` / `continue` runs those of its loop first
    pub finally_stack: Vec<(usize, std::rc::Rc<crate::interp::stmts::Finally>, bool)>,
    /// the running try contexts (innermost last): `try` statements, swallowing `with`s and the
    /// models' own catching frames
    pub tries: Vec<TryCtx>,
    /// the exception the next `record_raise_state` is about
    pub cur_exc: Option<Value>,
    /// exception names handled by the running `except` clauses (innermost last): what a bare
    /// `raise` re-raises
    pub handling: Vec<FxHashSet<String>>,
    /// the exception values the running handlers hold (a bare `raise` re-raises them)
    pub handling_values: Vec<Value>,
    /// inside a call whose exceptions a thread / future keeps: reads still require their keys
    pub catching_depth: u32,
    /// generators made but not run yet: the call that made each (`force_gen`)
    pub lazy_gens: FxHashMap<ObjId, std::rc::Rc<LazyGen>>,
    /// generators run whole for a consumer that runs code between their items: (generator,
    /// the state after its run, what it was given and yields) — see `with_gen_restart`
    pub gen_watch: Vec<GenWatch>,
    /// what the running `for` statements iterate (`gen_used_up`)
    pub for_iters: Vec<crate::domain::ObjSet>,
    /// running generator activations: whether they yielded, and what they read since (an
    /// object's key, or the whole object) — what code between their items may change for them
    pub gen_reads: Vec<(ObjId, bool, rustc_hash::FxHashSet<Change>)>,
    /// the reads of the last generator run (`force_gen` takes them)
    pub gen_last_reads: Option<rustc_hash::FxHashSet<Change>>,
    /// generators whose consumer changes objects they reach: those objects, changed for the
    /// generator at each of its yields (`consumer_between`)
    pub gen_havoc: FxHashMap<ObjId, Vec<Change>>,
    /// the call site and context of the generator `force_gen` is running
    pub force_at: Option<(String, String, bool)>,
    /// a generator's effects on older objects, per item: see `set_cursor`
    pub gen_eff: FxHashMap<ObjId, std::rc::Rc<GenEffects>>,
    /// the state at each positioned yield of the running generator activations
    pub gen_pos_states: FxHashMap<ObjId, Vec<Option<State>>>,
    /// positions of a generator's activation that two paths yield (no single state)
    pub gen_pos_conflict: rustc_hash::FxHashSet<(ObjId, usize)>,
    /// lazy iterators (zip / islice / map …) over one-shot iterators: what consuming them
    /// takes from each source (Some(n): n items, None: all)
    pub lazy_src: FxHashMap<ObjId, Vec<(ObjId, Option<usize>)>>,
    /// a context part every allocation adds (a model run once per receiver)
    pub alloc_extra: Option<String>,
    /// a load guarded by a `try` handler is running (its key is still required)
    pub caught_load: bool,
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
    /// `vars(obj)` dicts: stores into them are stores of obj's attributes
    pub attr_mirrors: FxHashMap<ObjId, ObjId>,
    /// results of unknown calls: (the call as written, its line, what it was given — which
    /// unknown code may return)
    pub origins: FxHashMap<ObjId, (String, u32, Vec<String>)>,
    /// unknown call sites (sources and mutators are named by them) as written: (the call's
    /// text, its module, its line), for reports
    pub site_names: FxHashMap<StrId, (String, u32, u32)>,
    /// the sites of calls of the program's code that did not run (`skipped_call`)
    pub skipped_sites: FxHashSet<StrId>,
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
    pub model_epoch: u64,
    pub model_born: FxHashMap<ObjId, u64>,
    /// modules that import torch (their functions all get entry runs)
    pub torch_modules: FxHashSet<ObjId>,
    /// the function an entry run (open parameters) is analysing
    pub entry_run: Option<ObjId>,
    /// functions that had their entry run
    pub entry_done: FxHashSet<ObjId>,
    /// functions some call of which their budget refused: their own lines then need a reporting
    /// run with open parameters (it covers the calls that did not run)
    pub budget_out: FxHashSet<ObjId>,
    /// closures made inside concrete calls in torch modules (no entry run of their own)
    pub t_concrete_closures: FxHashSet<ObjId>,
    /// the state a factory's entry run ended in, for the closures it made
    pub t_closure_states: FxHashMap<ObjId, std::rc::Rc<State>>,
    /// golden rows: the joined value of (line, name, call chain, entry run) so far
    pub t_row_values: FxHashMap<(u32, String, Vec<u32>, bool), Value>,
    /// the next hover fact is an assignment's
    pub t_fact_write: bool,
    /// the verdict being recorded is on a guarded access
    pub verdict_guarded: bool,
    /// expressions in call position (`x.m` of `x.m(…)`): a method read of an input is not a
    /// key of it
    pub callee_nodes: FxHashSet<usize>,
    /// the access being made is on a value that may also be a scalar (a str, a number): no
    /// object's lack of the key makes it fail for sure
    pub recv_scalar: bool,
    /// Enum classes: their members in definition order
    pub enum_members: FxHashMap<ObjId, Vec<(String, ObjId)>>,
    /// every name of an Enum in definition order, aliases (a name whose value an earlier
    /// member has) included: name → member (`Color[name]`, `__members__`)
    pub enum_aliases: FxHashMap<ObjId, Vec<(String, ObjId)>>,
    /// Enum classes: (mixes in str / int: equal to its value, `str()` is the value (ReprEnum:
    /// StrEnum, IntEnum, IntFlag), a Flag)
    pub enum_kind: FxHashMap<ObjId, EnumKind>,
    /// members and Flag combinations ↔ their keys (`Key::Member`)
    pub member_key: FxHashMap<ObjId, intern::KeyId>,
    pub key_member: FxHashMap<intern::KeyId, ObjId>,
    /// singledispatch functions: (type, implementation) in registration order; the methods
    pub sdispatch: FxHashMap<ObjId, Vec<(Value, ObjId)>>,
    pub sdispatch_method: FxHashSet<ObjId>,
    pub sd_bypass: Option<ObjId>,
    /// identity-hashed objects used as dict keys ↔ their keys
    pub identity_keys: RefCell<FxHashMap<ObjId, KeyId>>,
    /// builtin exception objects: their class names
    pub exc_objs: FxHashMap<ObjId, String>,
    /// entry parameters annotated with a TypedDict: the declared row's reads are still the
    /// input's schema (the parameter's source)
    pub td_param_src: FxHashMap<ObjId, crate::domain::SourceId>,
    /// `@functools.wraps(fn)` wrappers: fn
    pub wrapped: FxHashMap<ObjId, Value>,
    /// classes decorated with functools.total_ordering
    pub total_ordering: FxHashSet<ObjId>,
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
    /// names imported under `if TYPE_CHECKING:` (module, name) -> (module imported, name in it)
    pub type_only: FxHashMap<(ObjId, StrId), (String, Option<String>)>,
    pub final_state: Option<State>,
    pub user_classes: Vec<ObjId>,
    /// class attributes (flow-insensitive)
    pub cls_slots: FxHashMap<(ObjId, Ns, KeyId), Value>,
    /// statements executed plus objects instantiated (debug output)
    pub work: usize,
    /// objects visited by reachability walks (liveness, GC): part of the work measure
    pub walked: std::cell::Cell<usize>,
    /// objects visited by unknown code's havoc walks (`havoc_with`)
    pub havoc_walked: usize,
    /// when the analysis started (see `DEADLINE_MS`)
    pub started: Clock,
    /// the hard deadline passed: function bodies stop, the calls they are in become unknown calls
    pub hard_cut: bool,
    cut_tick: u32,
    /// per function code (`FuncNode::addr`): the names it reads beyond its locals
    pub code_uses: FxHashMap<usize, Rc<effects::CodeUses>>,
    /// per code object: what it may change when a call of it does not run (`code_closure`)
    pub closures: FxHashMap<Vec<ObjId>, Rc<effects::Closure>>,
    /// what `closures` holds (roots, code, slots, values), bounded by `CLOSURE_CACHE`
    pub closures_size: usize,
    /// the code objects the last `havoc_reachable` met (functions, classes, bound methods)
    pub havoc_code: Vec<ObjId>,
    /// the receiver of a skipped method call (`skipped_call`) and how often a havoc met it
    pub havoc_guard: Option<(ObjId, u32, std::rc::Rc<[KeyId]>)>,
    /// per object, the last havoc walk that met it (`havoc_walk`)
    pub havoc_marks: Vec<u32>,
    pub havoc_walks: u32,
    /// per (unknown code, values given): what its last havoc walk met, and the heap it left
    pub havoc_memo: FxHashMap<(StrId, u64), HavocMemo>,

    pub entry_instances: FxHashMap<ObjId, ObjId>,
    pub fresh_in_progress: FxHashSet<ObjId>,
    pub main_module: ObjId,
    pub series: ObjId,
    pub none_v: Value,
    /// `#@row NAME:` checks requested by the golden harness: (file relative to the main
    /// file's directory, "" for the main file; line; name)
    pub row_check_requests: Vec<(String, u32, String)>,
    /// keys a check names as absent (`absent={k}`): an open row lists only the absent keys it
    /// has seen; the check also asks whether reading k would surely fail there
    pub row_absent_probes: Vec<(String, u32, String, KeyId)>,
    /// the comprehension written as the first argument of `torch.cat` / `torch.stack` being
    /// called (its address): those raise on no tensors, so it ran at least once
    pub nonempty_comp: usize,
    /// set by a comprehension above for its generator: at least one iteration ran
    pub nonempty_gen: bool,
    /// whether the last `loop_fixpoint` converged (its last round ran from the fixpoint)
    pub loop_converged: bool,
    /// module index -> its path relative to the main file's directory (`rel_file`)
    pub rel_files: std::cell::RefCell<FxHashMap<u32, String>>,
    /// while a test is evaluated (`split`): its frame depth and the value each node of it had
    pub trace: Option<(usize, FxHashMap<usize, Value>)>,
    /// the values the test being narrowed had (`split`, read by `traced`)
    pub traced: FxHashMap<usize, Value>,
    /// the states at the yields of a generator's running activation (copies, read per key
    /// where the caller needs them: joining whole heaps per yield was the cost)
    pub gen_yield_states: FxHashMap<ObjId, YieldStates>,
    /// a copy's original (through copies made at the same node: `out = deepcopy(out)` in a loop
    /// makes one copy site, not a new one per round)
    pub copy_root: FxHashMap<ObjId, ObjId>,
    /// the class of the exception being recorded (set beside `cur_exc`)
    pub cur_exc_cls: Option<ExcNames>,
    /// the call being evaluated: where an exception a model raises is allocated
    pub exc_node: Option<Node>,
    pub item_exc: Option<&'static str>,
    /// what a missing key raises in the reads a model makes (pydantic's field reads: a
    /// ValidationError)
    pub item_exc_force: Option<&'static str>,
    /// a requirement-only entry run (`param F.x` schemas of a called function): no verdicts,
    /// row checks, facts or notes
    pub quiet: bool,
    /// no record-access verdicts (shapes still reported): the own run of a called function
    /// of a torch / numpy module
    pub quiet_records: bool,
    /// locals of the callers of the running call (innermost last), for liveness
    pub env_stack: Vec<IndexMap<StrId, Value>>,
    /// `@name.setter` functions by (defining class, attribute)
    pub prop_setters: FxHashMap<(ObjId, KeyId), ObjId>,
    pub row_checks: Vec<RowCheck>,
    /// collect `facts` (the `--facts` mode)
    pub facts_on: bool,
    pub facts: IndexMap<(u32, StrId), Fact>,
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
    pub fn new(source: String, filename: &str, search_paths: Vec<String>, analyse_entry_points: bool, follow_imports: bool) -> Result<Analyzer, String> {
        crate::state::reset_thread_state();
        crate::models::pynum::set_opaque(false);
        crate::models::pynum::reset_used();
        crate::models::chrono::reset_used();
        let (idx, tree) = frontend::parse("__main__", Some(filename), source)?;
        frontend::set_main(idx);
        let mut an = Analyzer {
            filename: filename.to_string(),
            search_paths: if follow_imports { search_paths } else { Vec::new() },
            analyse_entry_points,
            none_v: Value::lit(intern::key_none()),
            ..Default::default()
        };
        an.main_module = an.make_module("__main__", Some(filename), idx, tree);
        an.series = an.alloc_global(Kind::Series, "builtin:Series", "Series", Payload::None);
        Ok(an)
    }

    // ------------------------------------------------------------------ setup
    /// A module object (registered under `name`) for a parsed tree; `path` is its file, a
    /// namespace package's directory, or None (a module made by a model).
    pub fn make_module(&mut self, name: &str, path: Option<&str>, idx: u32, tree: &'static ast::ModModule) -> ObjId {
        let pkg = match path {
            Some(p) if name == "__main__" => package_of_file(p),
            Some(p) if p.ends_with("__init__.py") || std::path::Path::new(p).is_dir() => name.to_string(),
            _ => name.rsplit_once('.').map(|(a, _)| a.to_string()).unwrap_or_default(),
        };
        let info = ModuleInfo { name: name.to_string(), path: path.map(|p| p.to_string()), pkg, idx, tree, scopes: scopemod::analyse(tree) };
        let oid = self.alloc_global(Kind::Module, &format!("module:{}", name), &format!("module {}", name), Payload::Module(Rc::new(info)));
        self.modules.insert(name.to_string(), oid);
        oid
    }

    /// A try context begins: raises inside it record the state there for its handlers and
    /// its finally; `pre` is the state before the body.
    pub fn enter_try(&mut self, caught: FxHashSet<String>, pre: &mut State) {
        let (frames, envs, mark) = (self.frames.len(), self.env_stack.len(), self.heap.serial());
        self.tries.push(TryCtx { caught, frames, envs, pre: pre.copy(), mark, recs: Vec::new(), last: None });
    }

    /// The try context ends: what it recorded. What callees made since the try began and
    /// nothing reaches after the unwind (the try's names, the frames below it, the exception)
    /// is garbage in a raise state: a try around a large recursion would otherwise join every
    /// callee's objects into its handlers.
    pub fn leave_try(&mut self) -> TryExit {
        let Some(t) = self.tries.pop() else { return TryExit { recs: Vec::new() } };
        let mut recs: Vec<RaiseRec> = t.recs.into_iter().filter_map(|(c, sts, e)| Some((c, sts.finish()?, e))).collect();
        for (_, st, exc) in recs.iter_mut() {
            let mut roots: Vec<ObjId> = Vec::new();
            self.state_roots(st, t.envs, t.frames, &mut roots);
            for (o, v) in st.lasts.iter() {
                roots.push(*o);
                v.push_objs(&mut roots);
            }
            exc.push_objs(&mut roots);
            self.collect_since(t.mark, &t.pre, roots, st, Some(t.envs));
        }
        TryExit { recs }
    }

    /// The exception classes the running try contexts catch (innermost last).
    pub fn guards(&self) -> impl DoubleEndedIterator<Item = &FxHashSet<String>> {
        self.tries.iter().map(|t| &t.caught)
    }

    pub fn frame(&self) -> &Frame {
        self.frames.last().expect("no frame")
    }
    pub fn frame_mut(&mut self) -> &mut Frame {
        self.frames.last_mut().expect("no frame")
    }

    pub fn now(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
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
        if self.heap.kind(parent) == Kind::Cell {
            return self.heap.obj(parent).label.strip_prefix("cell ").map(|x| x.to_string());
        }
        let base = self.symbols.get(&parent)?;
        Some(match ns {
            Ns::Attr => format!("{}.{}", base, intern::key_py_str(key)),
            Ns::Cell => base.clone(),
            _ if key == intern::STAR => format!("{}[*]", base),
            _ => format!("{}[{}]", base, crate::domain::key_display(key)),
        })
    }

    // ------------------------------------------------------------ reporting
    pub fn file_of_frame(&self) -> String {
        let m = self.frame().module;
        if m == self.main_module {
            return String::new();
        }
        let idx = self.heap.module_info(m).idx;
        self.rel_file(idx)
    }

    /// The main module's frontend index.
    pub fn main_idx(&self) -> u32 {
        self.heap.module_info(self.main_module).idx
    }

    /// The path of module `idx` relative to the main file's directory ("" for the main
    /// module; the file name when it lies elsewhere, the module name when it has no file).
    pub fn rel_file(&self, idx: u32) -> String {
        if idx == self.main_idx() {
            return String::new();
        }
        if let Some(f) = self.rel_files.borrow().get(&idx) {
            return f.clone();
        }
        let f = match frontend::module_path(idx) {
            Some(p) => self.rel_to_main(&p).unwrap_or_else(|| std::path::Path::new(&p).file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or(p)),
            None => frontend::module_name(idx),
        };
        self.rel_files.borrow_mut().insert(idx, f.clone());
        f
    }

    pub fn verdict(&mut self, node: &Node, key: KeyId, cls: &Classification, oid: ObjId, ns: Ns, file_override: Option<&str>, text_override: Option<&str>) {
        self.gen_read(oid, Some((ns, key)));
        if matches!(cls.tier, Tier::Map) || self.quiet || self.quiet_records {
            return;
        }
        let weakened;
        let cls = if self.recv_scalar && cls.tier == Tier::Missing {
            weakened = Classification::with_note(Tier::Possibly, format!("{}, unless the value is a plain one such as a str", cls.note));
            &weakened
        } else {
            cls
        };
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
            obj: self.heap.obj(oid).label.to_string(),
            chain,
            note: cls.note.clone(),
            providers: cls.providers.iter().map(|(s, _)| *s).collect(),
            mutators: { let mut m: Vec<String> = cls.mutators.iter().map(|m| intern::str_get(*m).to_string()).collect(); m.sort(); m },
            ns,
            text,
            oid,
            file: fname,
            guarded: self.verdict_guarded,
        });
    }

    /// A verdict on an access the program guards (a try handling the failure): recorded for
    /// the golden harness and the hover, never a diagnostic.
    /// Forget the verdicts recorded since `mark` (a run whose states a later run supersedes).
    /// A running generator that yielded reads `o` (one key, or all of it).
    pub fn gen_read(&mut self, o: ObjId, k: Option<crate::state::SlotKey>) {
        for (_, yielded, reads) in self.gen_reads.iter_mut() {
            if *yielded {
                reads.insert((o, k));
            }
        }
    }

    pub fn drop_verdicts_since(&mut self, mark: usize) {
        for v in self.verdicts.drain(mark..) {
            self.verdict_keys.remove(&v.dedupe_key());
        }
    }

    pub fn verdict_guarded(&mut self, node: &Node, key: KeyId, cls: &Classification, oid: ObjId, ns: Ns) {
        let saved = std::mem::replace(&mut self.verdict_guarded, true);
        self.verdict(node, key, cls, oid, ns, None, None);
        self.verdict_guarded = saved;
    }

    pub fn node_text(&self, node: &Node) -> String {
        if node.addr == 0 {
            return String::new();
        }
        // (an expression written over several lines, on one: the report's column and a hover
        // keep their layout)
        let t = frontend::text_at(node);
        if t.contains('\n') { t.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ") } else { t }
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


    // ------------------------------------------------------------- entry

    /// A module of the program itself: the main file's directory or below.
    pub fn is_local_module(&self, m: ObjId) -> bool {
        if m == self.main_module {
            return true;
        }
        let idx = self.heap.module_info(m).idx;
        frontend::module_path(idx).map(|p| !p.contains("site-packages") && self.rel_to_main(&p).is_some()).unwrap_or(false)
    }

    /// Path `p` relative to the main file's directory, when it lies there or below.
    fn rel_to_main(&self, p: &str) -> Option<String> {
        let canon = |p: &str| std::fs::canonicalize(p).map(|x| x.to_string_lossy().to_string()).unwrap_or_else(|_| p.to_string());
        let dir = std::path::Path::new(&canon(&self.filename)).parent()?.to_path_buf();
        std::path::Path::new(&canon(p)).strip_prefix(&dir).ok().map(|r| r.to_string_lossy().to_string())
    }

    /// The work done so far: statements run and objects visited by reachability walks (16 to a
    /// unit): a deterministic cost measure (`ROWAN_DEBUG=1`, the golden `#@work` bound).
    pub fn work_units(&self) -> usize {
        self.work + self.walked.get() / 16
    }

    /// Past the deadline (`DEADLINE_MS`): calls of user code are unknown code from there on.
    pub fn over_hard_deadline(&self) -> bool {
        self.started.elapsed().as_millis() as usize > limit("ms", DEADLINE_MS)
    }

    /// Past the deadline a statement in a function body does not run: its path ends, and
    /// the call it belongs to is replaced by an unknown call (`call_user`). Module code goes on
    /// (its calls are unknown calls). Checked every 64 statements.
    pub fn cut_body(&mut self, state: &mut State) -> bool {
        if !self.hard_cut {
            self.cut_tick = self.cut_tick.wrapping_add(1);
            if self.cut_tick % 64 != 0 || !self.over_hard_deadline() {
                return false;
            }
            self.hard_cut = true;
        }
        if self.frames.last().is_some_and(|f| f.kind == FrameKind::Function) {
            state.reachable = false;
            return true;
        }
        false
    }

    pub fn run(&mut self) {
        let mut state = State::new();
        let main = self.main_module;
        self.run_module(main, &mut state);
        if self.analyse_entry_points {
            self.analyse_uncalled(&mut state);
        }
        self.final_state = Some(state);
    }

    pub fn run_module(&mut self, moid: ObjId, state: &mut State) {
        let info = self.heap.module_info(moid);
        let fr = Frame::new(FrameKind::Module, info.scopes.module(), moid, "", true, &format!("<module {}>", info.name));
        self.frames.push(fr);
        // (imported from inside a function: the function's returns so far are its own)
        let (exits, ret) = (state.exits.take(), state.ret.take());
        self.with_gen_restart(state, true, &mut |an, s| an.exec_block(&info.tree.body, s));
        self.frames.pop();
        state.reachable = true;
        state.exits = exits;
        state.ret = ret;
    }

    /// Entry-point rule (§6.4): functions never called get analysed with open parameters.
    fn analyse_uncalled(&mut self, state: &mut State) {
        // entry runs can make functions (closures, lambdas, inner defs) nobody calls either:
        // a few rounds, each over the functions made so far
        let mut start: ObjId = 1;
        for _round in 0..4 {
            let n = self.heap.len() as ObjId;
            if start >= n {
                break;
            }
            self.analyse_uncalled_range(start, n, state, false);
            start = n;
        }
        // then the requirement-only runs of the functions the program calls (last: runs made
        // while quiet hold no verdicts)
        let n = self.heap.len() as ObjId;
        self.analyse_uncalled_range(1, n, state, true);
    }

    fn analyse_uncalled_range(&mut self, from: ObjId, n: ObjId, state: &mut State, quiet_pass: bool) {
        for oid in from..n {
            if self.over_hard_deadline() {
                break;
            }
            if self.heap.kind(oid) != Kind::Function || self.entry_done.contains(&oid) {
                continue;
            }
            let fi = self.heap.func_info(oid);
            // functions of torch modules get their own run even when called (the shapes
            // inside, in terms of their own parameters)
            // a FastAPI handler: called by the framework with any request (also when the
            // program's own TestClient calls it), in any module
            let handler = self.m.fa_handlers.contains(&oid);
            // a function the program calls: a requirement-only run with open parameters gives
            // its own input contract (`param F.x` schemas)
            let refused = self.budget_out.contains(&oid);
            let quiet = self.analysed_funcs.contains(&oid) && !self.torch_modules.contains(&fi.module) && !handler && !refused;
            if quiet != quiet_pass && !(quiet_pass && refused) {
                continue;
            }
            if quiet && (!fi.closure.is_empty() || !matches!(fi.node, FuncNode::Def(_)) || !self.is_local_module(fi.module)) || self.t_concrete_closures.contains(&oid) {
                continue;
            }
            self.entry_done.insert(oid);
            if fi.module != self.main_module && !handler && !((quiet || refused) && self.is_local_module(fi.module)) {
                continue;
            }
            if handler {
                self.fa_entry_run(oid, &fi, state);
                continue;
            }
            let Some(fparams) = fi.node.params() else {
                // a lambda without parameters
                if let FuncNode::Lambda(_) = fi.node {
                    let mut st = state.copy();
                    st.ret = None;
                    let mi = self.heap.module_info(fi.module);
                    self.frames.push(Frame::new(FrameKind::Module, mi.scopes.module(), fi.module, &format!("entry:{}", fi.qualname), true, "<entry>"));
                    let fake = Node::synthetic(mi.idx, fi.node.node().line);
                    self.call_user(oid, None, vec![], IndexMap::new(), vec![], vec![], &mut st, &fake, None, true);
                    self.frames.pop();
                }
                continue;
            };
            if debug_enabled() {
                eprintln!("[rowan] entry {} L{} t={:.1}s objs={} verdicts={} quiet={}", fi.qualname, fi.node.node().line, self.now(), self.heap.len(), self.verdicts.len(), quiet);
            }
            // a closure made by a factory's entry run starts from that run's state (its cells)
            let mut st = match self.t_closure_states.get(&oid) {
                Some(cs) => {
                    let mut c = cs.copy();
                    c.reachable = true;
                    c.exits = None;
                    c
                }
                None => state.copy(),
            };
            st.ret = None;
            let mut args: Vec<Value> = Vec::new();
            let mi = self.heap.module_info(fi.module);
            self.frames.push(Frame::new(FrameKind::Module, mi.scopes.module(), fi.module, &format!("entry:{}", fi.qualname), true, "<entry>"));
            self.entry_scope = Some(oid);
            let mut params = scopemod::all_params(fparams);
            let mut self_val: Option<Value> = None;
            if let Some(cls) = fi.cls {
                if !fi.is_static.get() {
                    // (an instance of the class or of one of its subclasses: what their
                    // `__init__`s set may be there; a torch module's method is analysed in
                    // its own terms: its attributes as symbols)
                    let torch = self.torch_modules.contains(&fi.module);
                    let subs: Vec<ObjId> = self.user_classes.iter().copied().filter(|c| *c == cls || (!torch && self.mro(*c).contains(&cls))).collect();
                    let mut insts: Vec<ObjId> = Vec::new();
                    for c in subs {
                        let inst = match self.entry_instances.get(&c) {
                            Some(&i) => i,
                            None => {
                                let cname = self.heap.class_info(c).name.clone();
                                let i = self.fresh_instance(c, state, &format!("entry:{}", cname));
                                self.entry_instances.insert(c, i);
                                st = state.copy();
                                st.ret = None;
                                i
                            }
                        };
                        insts.push(inst);
                    }
                    self_val = Some(Value::objs(crate::domain::ObjSet::from_vec(insts)));
                    if !params.is_empty() {
                        params.remove(0);
                    }
                }
            }
            let mut star_args: Vec<Value> = Vec::new();
            let mut star_kwargs: Vec<Value> = Vec::new();
            if self.torch_modules.contains(&fi.module) {
                for (i, a) in params.iter().enumerate() {
                    crate::models::shape::note_param(crate::models::shape::sym(oid, a.name.id.as_str()), i as u32);
                }
            }
            let mut kwargs: IndexMap<StrId, Value> = IndexMap::new();
            for a in params {
                if [&fparams.vararg, &fparams.kwarg].iter().any(|v| v.as_deref().map(|v| std::ptr::eq(v, a)).unwrap_or(false)) {
                    // a caller may pass any arguments here
                    let v = Value::obj(self.param_source(&fi, a, &mut st));
                    if fparams.vararg.as_deref().map(|v| std::ptr::eq(v, a)).unwrap_or(false) {
                        star_args.push(v);
                    } else {
                        star_kwargs.push(v);
                    }
                    continue;
                }
                let v = self.open_param(&fi, a, false, &mut st);
                // (keyword-only parameters are passed by keyword)
                if fparams.kwonlyargs.iter().any(|k| std::ptr::eq(&k.parameter, a)) {
                    kwargs.insert(s(a.name.id.as_str()), v);
                } else {
                    args.push(v);
                }
            }
            self.entry_scope = None;
            let fake = Node::synthetic(mi.idx, fi.node.node().line);
            let saved_entry = self.entry_run.replace(oid);
            let before = self.heap.len() as ObjId;
            let saved_quiet = std::mem::replace(&mut self.quiet, quiet);
            let called_torch = self.analysed_funcs.contains(&oid) && self.torch_modules.contains(&fi.module);
            let saved_qr = std::mem::replace(&mut self.quiet_records, called_torch);
            self.call_user(oid, self_val, args, kwargs, star_args, star_kwargs, &mut st, &fake, None, true);
            self.quiet_records = saved_qr;
            self.quiet = saved_quiet;
            self.entry_run = saved_entry;
            if quiet {
                // functions made by a requirement-only run are not the program's
                let after = self.heap.len() as ObjId;
                for o in before..after {
                    if self.heap.kind(o) == Kind::Function {
                        self.entry_done.insert(o);
                    }
                }
            }
            if self.torch_modules.contains(&fi.module) {
                let after = self.heap.len() as ObjId;
                let made: Vec<ObjId> = (before..after).filter(|o| self.heap.kind(*o) == Kind::Function && !self.heap.func_info(*o).closure.is_empty()).collect();
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
            for (src, k) in st.reqs.diff_keys(&state.reqs) {
                if quiet && !own_source(&crate::domain::source_label(src), &own, span, in_main) {
                    continue;
                }
                let Some(info) = st.reqs.get(&src, &k) else { continue };
                let nv = match state.reqs.get(&src, &k) {
                    Some(cur) => cur.join(&info),
                    None => info,
                };
                state.reqs.set(&src, k, nv);
            }
        }
    }

    /// Strip `Optional[X]`, `list[X]`, `"X"`, `X | None` down to the inner expression.
    /// The class-bearing part of an annotation: wrappers that keep the value's class are
    /// unwrapped (`Optional`, `Annotated`, `Final`, `ClassVar`, `Required`, a union with `None`
    /// and one other member); a container (`list[Foo]`, `dict[str, Foo]`) is the container
    /// itself, not its element class; a union of several classes has none.
    pub fn strip_annotation(ann: &'static ast::Expr) -> Option<&'static ast::Expr> {
        let is_none = |e: &ast::Expr| matches!(e, ast::Expr::NoneLiteral(_)) || matches!(e, ast::Expr::Name(n) if n.id.as_str() == "None");
        let mut node = ann;
        for _ in 0..16 {
            match node {
                ast::Expr::StringLiteral(sl) => {
                    let txt = sl.value.to_str().to_string();
                    node = frontend::parse_expression(&txt)?;
                }
                ast::Expr::Subscript(sub) => {
                    let elts: Vec<&'static ast::Expr> = match &*sub.slice {
                        ast::Expr::Tuple(t) => t.elts.iter().collect(),
                        e => vec![e],
                    };
                    match dotted_last(&sub.value).as_str() {
                        "Optional" | "Annotated" | "Final" | "ClassVar" | "Required" | "NotRequired" | "ReadOnly" => node = elts.first()?,
                        "Union" => {
                            let rest: Vec<&'static ast::Expr> = elts.into_iter().filter(|e| !is_none(e)).collect();
                            if rest.len() != 1 {
                                return None;
                            }
                            node = rest.first()?;
                        }
                        // (a container: its own class; the old rule took the element's)
                        _ => return Some(&sub.value),
                    }
                }
                ast::Expr::BinOp(b) if b.op == ast::Operator::BitOr => {
                    // (`A | B | None`: flattened)
                    let mut parts: Vec<&'static ast::Expr> = Vec::new();
                    let mut stack: Vec<&'static ast::Expr> = vec![node];
                    while let Some(e) = stack.pop() {
                        match e {
                            ast::Expr::BinOp(b2) if b2.op == ast::Operator::BitOr => {
                                stack.push(&b2.right);
                                stack.push(&b2.left);
                            }
                            e => parts.push(e),
                        }
                    }
                    let rest: Vec<&'static ast::Expr> = parts.into_iter().filter(|e| !is_none(e)).collect();
                    if rest.len() != 1 {
                        return None;
                    }
                    node = rest.first()?;
                }
                _ => return Some(node),
            }
        }
        None
    }

    /// A declared row for an annotated entry-point parameter (§6.6).
    /// The unknown a caller passes for parameter `a` of `fi` (a `param F.a` input).
    fn param_source(&mut self, fi: &FuncInfo, a: &ast::Parameter, st: &mut State) -> ObjId {
        self.unknown_source(&format!("param {}.{}", fi.py_qualname(), a.name.id), SourceKind::Param, st, &format!("param {}", a.name.id), "", Ns::Item, vec![])
    }

    /// Parameter `a` of an entry run: its default or whatever a caller passes (only the
    /// default with `default_only`), as annotated; a number default in a torch module is not
    /// joined in (the parameter stays a name dims can be written in).
    fn open_param(&mut self, fi: &FuncInfo, a: &'static ast::Parameter, default_only: bool, st: &mut State) -> Value {
        let d = fi.defaults.get(&s(a.name.id.as_str())).cloned();
        if let (Some(d), true) = (&d, default_only) {
            return d.clone();
        }
        // (with a default, only a torch annotation is taken: an instance for a defaulted
        // parameter of a user class made runs cost coverage, click 186 -> 151 sites)
        let ann = a.annotation.as_deref();
        let given = match ann.and_then(|ann| if d.is_some() { self.t_param(ann, fi, a.name.id.as_str(), st) } else { self.param_from_annotation(ann, fi, a.name.id.as_str(), st) }) {
            Some(v) => v,
            None => Value::obj(self.param_source(fi, a, st)),
        };
        match d {
            Some(d) if !(self.torch_modules.contains(&fi.module) && d.pts.is_empty() && d.kv.as_lit().map(intern::key_is_number).unwrap_or(false)) => d.join(&given),
            _ => given,
        }
    }

    pub fn param_from_annotation(&mut self, ann: &'static ast::Expr, fi: &FuncInfo, pname: &str, state: &mut State) -> Option<Value> {
        let node = Self::strip_annotation(ann)?;
        if let Some(v) = self.t_param(ann, fi, pname, state) {
            return Some(v);
        }
        let mut v = self.eval(node, state);
        // (a class imported only for type checkers)
        if let ast::Expr::Name(n) = node {
            if !v.pts.iter().any(|o| self.heap.kind(o) == Kind::Class) {
                v = self.type_only_name(n.id.as_str(), state).unwrap_or(v);
            }
        }
        for oid in v.pts.iter() {
            let o = self.heap.kind(oid);
            if o == Kind::Class {
                let ci = self.heap.class_info(oid);
                if ci.has_fields() {
                    let mut fields: KeySet = KeySet::from_iter(ci.fields().iter().map(|(f, _)| key_str_lit(f)));
                    let ns = if ci.lib.typed_dict { Ns::Item } else { Ns::Attr };
                    // a TypedDict's `total=False` / `NotRequired` keys may be absent
                    let mut declared = Row::closed_with(fields.clone());
                    if ci.lib.typed_dict {
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
                        state.set_slot(inst, ns, f, Value::data(&format!("field {}", intern::key_py_str(f))), true);
                    }
                    if ci.lib.typed_dict {
                        let src = Source::new(&format!("param {}.{}", fi.py_qualname(), pname), SourceKind::Param);
                        self.td_param_src.insert(inst, src);
                    }
                    return Some(Value::obj(inst));
                }
                let inst = self.fresh_instance(oid, state, &format!("entry:{}", fi.qualname));
                return Some(Value::obj(inst));
            }
            if o == Kind::External && self.heap.external_qual(oid).ends_with("DataFrame") {
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

    pub fn fresh_instance(&mut self, cls_oid: ObjId, state: &mut State, ctx: &str) -> ObjId {
        let ci = self.heap.class_info(cls_oid);
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
        let has_fields = self.mro(cls_oid).iter().any(|c| self.heap.class_info_opt(*c).map(|ci| ci.has_fields()).unwrap_or(false));
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
            let Some(cinfo) = self.heap.class_info_opt(c) else { continue };
            if cinfo.has_fields() {
                let ns = if cinfo.lib.typed_dict { Ns::Item } else { Ns::Attr };
                for (f, _) in cinfo.fields() {
                    let k = key_str_lit(&f);
                    let r = state.row(inst, ns).store(k, true);
                    state.set_row(inst, ns, r);
                    state.set_slot(inst, ns, k, Value::data(&format!("field {}", f)), true);
                }
            }
            order.extend(cinfo.bases.iter().copied());
        }
        if let Some(init_cls) = self.class_lookup(cls_oid, "__init__") {
            let inits = self.cls_slots.get(&(init_cls, Ns::Attr, key_str_lit("__init__"))).cloned().unwrap_or_default();
            for init in inits.pts.iter() {
                if self.heap.kind(init) != Kind::Function {
                    continue;
                }
                let fi = self.heap.func_info(init);
                let Some(params_all) = fi.node.params() else { continue };
                let saved_scope = self.entry_scope;
                if self.torch_modules.contains(&fi.module) {
                    self.entry_scope = Some(init);
                }
                let mut args = Vec::new();
                let mut kwargs: IndexMap<StrId, Value> = IndexMap::new();
                for a in scopemod::all_params(params_all).into_iter().skip(1) {
                    if [&params_all.vararg, &params_all.kwarg].iter().any(|v| v.as_deref().map(|v| std::ptr::eq(v, a)).unwrap_or(false)) {
                        continue;
                    }
                    // (outside torch modules: the defaults, as a caller that passes nothing)
                    let v = self.open_param(&fi, a, !self.torch_modules.contains(&fi.module), state);
                    if params_all.kwonlyargs.iter().any(|k| std::ptr::eq(&k.parameter, a)) {
                        kwargs.insert(s(a.name.id.as_str()), v);
                    } else {
                        args.push(v);
                    }
                }
                self.entry_scope = saved_scope;
                let fake = if self.torch_modules.contains(&fi.module) { Node::synthetic(fi.node.node().module, frontend::node(ci.node).line) } else { Node::synthetic(fi.node.node().module, fi.node.node().line) };
                self.call_user(init, Some(Value::obj(inst)), args, kwargs, vec![], vec![], state, &fake, None, true);
            }
        }
        // (an instance a caller made may have had any of its methods called since: what they
        // set and `__init__` did not may be there too, holding anything; what they delete may
        // be gone)
        let (set, deleted) = self.attrs_set_by_methods(cls_oid, state);
        if !(set.is_empty() && deleted.is_empty()) && state.has_row(inst, Ns::Attr) {
            let any = Value::obj(self.any_object(state));
            let mut row = state.row(inst, Ns::Attr);
            // (a name the class itself defines — a property, a class-level default — reads
            // through the class anyway)
            let had = row.present.clone();
            let by_class = |an: &Analyzer, k: KeyId| intern::key_as_str(k).is_some_and(|n| an.class_lookup(cls_oid, &n).is_some());
            for k in set.into_iter().filter(|k| !had.contains(*k) && !by_class(self, *k)) {
                row = row.store(k, false);
                state.set_slot(inst, Ns::Attr, k, any.clone(), false);
            }
            for k in deleted {
                row = row.delete(k, false);
            }
            state.set_row(inst, Ns::Attr, row);
        }
        inst
    }

    pub fn record_fact(&mut self, node: &Node, name: StrId, v: &Value, state: &State) {
        if !self.facts_on || node.addr == 0 || self.quiet {
            return;
        }
        if node.module != self.heap.module_info(self.main_module).idx {
            return;
        }
        let mut rows: Vec<(ObjId, Option<Row>, Option<Row>)> = Vec::new();
        for oid in v.pts.iter() {
            let k = self.heap.kind(oid);
            let item = if matches!(k, Kind::Dict | Kind::Frame | Kind::Unknown | Kind::Namespace | Kind::Module | Kind::Class) || state.has_row(oid, Ns::Item) { Some(state.row(oid, Ns::Item)) } else { None };
            let attr = if matches!(k, Kind::Instance | Kind::Namespace | Kind::Unknown | Kind::Module | Kind::Class) || state.has_row(oid, Ns::Attr) { Some(state.row(oid, Ns::Attr)) } else { None };
            rows.push((oid, item, attr));
        }
        let join_opt = |a: &Option<Row>, b: &Option<Row>| -> Option<Row> {
            match (a, b) {
                (Some(x), Some(y)) => Some(x.join(y)),
                (Some(x), None) | (None, Some(x)) => Some(x.clone()),
                _ => None,
            }
        };
        let scope = self.frame().func.unwrap_or(0);
        // (a fact that has all the texts it shows (4) only joins the value and rows: rendering a
        // large value at every run of the line is what costs — not in torch programs, whose
        // hover is the assignment's own value)
        if !self.torch_on {
            if let Some(e) = self.facts.get_mut(&(node.line, name)) {
                if e.texts.len() >= 4 {
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
                    return;
                }
            }
        }
        // tensors: dims named by locals / letters, printed in the function's own terms
        let (pctx, tnames, tlegend) = if self.torch_on {
            let mut ctx = self.t_print_ctx(state);
            let (names, legend) = crate::models::shape::with_ctx(ctx.clone(), || self.t_hover_names(v, state));
            ctx.atom_names = names.clone();
            (Some(ctx), names, legend)
        } else {
            (None, FxHashMap::default(), Vec::new())
        };
        let render = |an: &Analyzer| crate::report::in_scope(scope, || (crate::report::render_value(an, state, v, 0, &mut Vec::new()), crate::report::value_legend(an, state, v)));
        let (text, mut legend) = match pctx {
            Some(ctx) => crate::models::shape::with_ctx(ctx, || render(self)),
            None => render(self),
        };
        legend.extend(tlegend);
        let writing = std::mem::take(&mut self.t_fact_write);
        let e = self.facts.entry((node.line, name)).or_insert_with(|| Fact { rows: IndexMap::new(), texts: Vec::new(), legend: Vec::new(), t_names: FxHashMap::default(), primary: None, written: false });
        // in torch programs an assignment's value is what the hover shows (`y = f(y)`)
        if self.torch_on {
            if writing && !e.written {
                e.written = true;
                e.texts.clear();
                e.legend.clear();
                e.primary = None;
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
            match self.heap.kind(o) {
                Kind::Unknown | Kind::External => info.unknown = true,
                // a tuple of known literals is that literal (as Python prints it)
                Kind::Tuple | Kind::Pair if self.literal_tuple(o, state, 0).is_some() => info.lits.push(self.literal_tuple(o, state, 0).unwrap()),
                _ => info.objects.push(self.heap.obj(o).label.to_string()),
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
        if self.heap.kind(o) == Kind::Pair {
            let Payload::Pair(k, v) = &self.heap.obj(o).payload else { return None };
            let one = |x: &Value| -> Option<KeyId> {
                match (x.pts.len(), &x.kv) {
                    (0, KeyVal::Lit(k)) => Some(*k),
                    (1, KeyVal::Bot) if matches!(self.heap.kind(x.pts.first().unwrap()), Kind::Tuple | Kind::Pair) => self.literal_tuple(x.pts.first().unwrap(), state, depth + 1),
                    _ => None,
                }
            };
            return Some(intern::key_tuple(vec![one(k)?, one(v)?]));
        }
        let n = self.heap.llen(o, state)?;
        let mut items = Vec::with_capacity(n);
        for i in 0..n {
            let v = state.slot(o, Ns::Pos, intern::key_int(i as i64));
            let k = match (v.pts.len(), &v.kv) {
                (0, KeyVal::Lit(k)) => *k,
                (1, KeyVal::Bot) => {
                    let x = v.pts.first().unwrap();
                    if !matches!(self.heap.kind(x), Kind::Tuple | Kind::Pair) {
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
            // (an observation, not a read of the program: no position, so no unbound-name check)
            let v = self.load_name(s(&name), &Node::none(), state);
            let mut rows: Vec<Row> = Vec::new();
            let mut ns_used = Ns::Item;
            for oid in v.pts.iter() {
                // a UserDict's keys are those of its `data`
                if self.heap.kind(oid) == Kind::Instance && self.heap.instance_class(oid).map(|c| self.mro(c).iter().any(|m| self.m.userdict_classes.contains(m))).unwrap_or(false) {
                    let d = state.slot(oid, Ns::Attr, key_str_lit("data"));
                    if let Some(dd) = d.single().filter(|x| self.heap.kind(*x) == Kind::Dict) {
                        ns_used = Ns::Item;
                        rows.push(state.row(dd, Ns::Item));
                        continue;
                    }
                }
                // a mapping proxy shows its dict
                if let Payload::View(ViewKind::Proxy, inner) = self.heap.obj(oid).payload.clone() {
                    for dd in inner.pts.iter() {
                        rows.push(state.row(dd, Ns::Item));
                    }
                    continue;
                }
                // (a numpy structured array is keyed by its fields)
                // (an instance of a dict subclass: its items)
                let dict_sub = self.heap.kind(oid) == Kind::Instance && self.heap.instance_class(oid).is_some_and(|c| self.mro(c).iter().any(|x| self.heap.class_info_opt(*x).map(|ci| ci.lib.dict_base).unwrap_or(false)));
                let ns = if matches!(self.heap.kind(oid), Kind::Instance | Kind::Namespace) && !self.m.np_struct.contains(&oid) && !dict_sub { Ns::Attr } else { Ns::Item };
                ns_used = ns;
                rows.push(state.row(oid, ns));
            }
            let must: Vec<KeyId> = state.name_keys.iter().filter(|(n, _)| *intern::str_get(*n) == *name).map(|(_, k)| *k).collect();
            let mut row = if rows.is_empty() { None } else { Some(must.iter().fold(crate::domain::join_rows(rows), |r, k| r.store(*k, true))) };
            if let Some(r) = row.as_mut() {
                for (_, _, _, k) in self.row_absent_probes.iter().filter(|(f, l, nm, _)| *f == file && *l == ln && *nm == name) {
                    if crate::domain::classify(r, *k).tier == crate::domain::Tier::Missing {
                        *r = r.delete(*k, true);
                    }
                }
            }
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

pub fn is_constant(e: &ast::Expr) -> bool {
    matches!(
        e,
        ast::Expr::StringLiteral(_) | ast::Expr::NumberLiteral(_) | ast::Expr::BooleanLiteral(_) | ast::Expr::NoneLiteral(_) | ast::Expr::BytesLiteral(_) | ast::Expr::EllipsisLiteral(_)
    )
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

