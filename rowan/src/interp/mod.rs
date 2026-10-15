//! Abstract interpreter over the Python AST.  Port of
//! `rowan/interp.py`; the statement/expression/narrowing/access/call parts live in the
//! sibling files of this module.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use indexmap::IndexMap;
use ruff_python_ast as ast;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::domain::{
    Ns, ObjId, Row, Source, SourceKind, Value,
};
use crate::frontend::{self, Node};
use crate::intern::{self, key_str_lit, s, KeyId, KeySet, StrId};
use crate::models::Registry;
use crate::scopes::{self as scopemod, Scopes};
use crate::state::{Kind, State};

pub mod access;
pub mod budget;
pub use budget::{Budget, Clock};
pub mod calls;
pub mod effects;
pub mod alloc;
pub mod exprs;
pub mod findings;
pub use findings::*;
pub mod funcs;
pub mod gc;
pub mod gens;
pub use gens::*;
pub mod heap;
pub use heap::*;
pub mod names;
pub use names::Names;
pub mod narrow;
pub use crate::models::tables::*;
pub mod recursion;
pub use recursion::{ActKey, Recursion};
pub mod stmts;
pub mod tries;
pub use tries::*;
pub mod unknown;
pub use unknown::{any_object, Havoc, HavocMemo};

pub const UNROLL_MAX: usize = 16;
pub const LOOP_MAX_ITER: usize = 40;
pub const REC_MAX_ITER: usize = 8;
pub const PATH_DEPTH: u32 = 3;
pub const MAX_ACTIVE: usize = 4;
/// records an unknown call's result joins at most (past that: an open row)
pub const MAX_ARG_JOIN: usize = 64;
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

#[derive(Default)]
pub struct Analyzer {
    /// the library models' own tables (`models::Tables`)
    pub m: crate::models::Tables,
    pub filename: String,
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
    /// what the analysis found: verdicts, notes, summaries, golden row checks, hover facts
    /// (`interp/findings.rs`)
    pub findings: Findings,
    /// the report's names for unknown objects and calls (`interp/names.rs`)
    pub names: Names,
    /// unknown code's havoc walks (`interp/unknown.rs`)
    pub havoc: Havoc,
    /// what code may change when it does not run (`interp/effects.rs`)
    pub effects: effects::EffectsCache,
    /// budgets, the deadline and the work counters (`interp/budget.rs`)
    pub budget: Budget,
    /// the running activations, recursion's fixpoints (`interp/recursion.rs`)
    pub recursion: Recursion,
    /// generators and `@contextmanager` bodies run at their consumers (`interp/gens.rs`)
    pub gens: Gens,
    /// the ways out of running code: raises, try contexts, handlers, break / continue, finally
    /// (`interp/tries.rs`)
    pub unwind: Unwind,
    pub models: Registry,
    pub frames: Vec<Frame>,
    /// lazy iterators (zip / islice / map …) over one-shot iterators: what consuming them
    /// takes from each source (Some(n): n items, None: all)
    pub lazy_src: FxHashMap<ObjId, Vec<(ObjId, Option<usize>)>>,
    /// a context part every allocation adds (a model run once per receiver)
    pub alloc_extra: Option<String>,
    /// per running comprehension: the cells of its variables (closed over by functions made
    /// inside it, which see the variable's latest value)
    pub comp_cells: Vec<FxHashMap<StrId, ObjId>>,
    /// (canonical key, key as written) while an access with a numerically-equal key runs
    pub key_alias: Option<(KeyId, KeyId)>,
    /// `vars(obj)` dicts: stores into them are stores of obj's attributes
    pub attr_mirrors: FxHashMap<ObjId, ObjId>,
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
    /// closures made inside concrete calls in torch modules (no entry run of their own)
    pub t_concrete_closures: FxHashSet<ObjId>,
    /// the state a factory's entry run ended in, for the closures it made
    pub t_closure_states: FxHashMap<ObjId, std::rc::Rc<State>>,
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
    pub user_classes: Vec<ObjId>,
    /// class attributes (flow-insensitive)
    pub cls_slots: FxHashMap<(ObjId, Ns, KeyId), Value>,

    pub entry_instances: FxHashMap<ObjId, ObjId>,
    pub fresh_in_progress: FxHashSet<ObjId>,
    pub main_module: ObjId,
    pub series: ObjId,
    pub none_v: Value,
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
    /// a copy's original (through copies made at the same node: `out = deepcopy(out)` in a loop
    /// makes one copy site, not a new one per round)
    pub copy_root: FxHashMap<ObjId, ObjId>,
    /// locals of the callers of the running call (innermost last), for liveness
    pub env_stack: Vec<IndexMap<StrId, Value>>,
    /// `@name.setter` functions by (defining class, attribute)
    pub prop_setters: FxHashMap<(ObjId, KeyId), ObjId>,
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

    pub fn frame(&self) -> &Frame {
        self.frames.last().expect("no frame")
    }
    pub fn frame_mut(&mut self) -> &mut Frame {
        self.frames.last_mut().expect("no frame")
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

    /// Past the deadline a statement in a function body does not run: its path ends, and
    /// the call it belongs to is replaced by an unknown call (`call_user`). Module code goes on
    /// (its calls are unknown calls). Checked every 64 statements.
    pub fn cut_body(&mut self, state: &mut State) -> bool {
        if !self.budget.tick() {
            return false;
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
        self.findings.final_state = Some(state);
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
            if self.budget.over_hard_deadline() {
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
            let refused = self.budget.out.contains(&oid);
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
                eprintln!("[rowan] entry {} L{} t={:.1}s objs={} verdicts={} quiet={}", fi.qualname, fi.node.node().line, self.budget.now(), self.heap.len(), self.findings.mark(), quiet);
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
                    let subs: Vec<ObjId> = self.user_classes.iter().copied().filter(|c| *c == cls || (!torch && self.heap.mro(*c).contains(&cls))).collect();
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
            let saved_quiet = std::mem::replace(&mut self.findings.quiet, quiet);
            let called_torch = self.analysed_funcs.contains(&oid) && self.torch_modules.contains(&fi.module);
            let saved_qr = std::mem::replace(&mut self.findings.quiet_records, called_torch);
            self.call_user(oid, self_val, args, kwargs, star_args, star_kwargs, &mut st, &fake, None, true);
            self.findings.quiet_records = saved_qr;
            self.findings.quiet = saved_quiet;
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
        let has_fields = self.heap.mro(cls_oid).iter().any(|c| self.heap.class_info_opt(*c).map(|ci| ci.has_fields()).unwrap_or(false));
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
            let any = Value::obj(crate::interp::any_object(&mut self.heap));
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

