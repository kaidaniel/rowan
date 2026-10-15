//! Abstract interpreter over the Python AST.  Port of
//! `rowan/interp.py`; the statement/expression/narrowing/access/call parts live in the
//! sibling files of this module.

use std::cell::{Cell, RefCell};

use indexmap::IndexMap;
use ruff_python_ast as ast;
use rustc_hash::FxHashSet;

use crate::domain::{Ns, ObjId, Value};
use crate::frontend::{self, Node};
use crate::intern::{self, s, KeyId, StrId};
use crate::scopes::Scopes;
use crate::state::{Kind, State};

pub mod access;
pub mod attrs;
pub mod items;
pub mod keys;
pub mod loops;
pub mod budget;
pub use budget::{Budget, Clock};
pub mod calls;
pub mod classes;
pub mod compare;
pub mod comps;
pub mod context;
pub use context::{Context, Modes};
pub mod effects;
pub mod alloc;
pub mod api;
pub mod entry;
pub use entry::Entries;
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
pub mod ops;
pub mod program;
pub use program::{package_of_file, package_root_of_file, Program};
pub use crate::models::tables::*;
pub mod recursion;
pub mod report;
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
    pub(in crate::interp) fn params(&self) -> Option<&'static ast::Parameters> {
        match self {
            FuncNode::Def(f) => Some(&f.parameters),
            FuncNode::Lambda(l) => l.parameters.as_deref(),
        }
    }
    pub(in crate::interp) fn node(&self) -> Node {
        match self {
            FuncNode::Def(f) => frontend::node(*f),
            FuncNode::Lambda(l) => frontend::node(*l),
        }
    }
    pub(in crate::interp) fn addr(&self) -> usize {
        match self {
            FuncNode::Def(f) => *f as *const _ as usize,
            FuncNode::Lambda(l) => *l as *const _ as usize,
        }
    }
    pub(in crate::interp) fn body(&self) -> &'static [ast::Stmt] {
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
    /// the main file, import paths, models, modules imported (`interp/program.rs`)
    pub(in crate::interp) program: Program,
    /// the objects (`interp/heap.rs`)
    pub heap: Heap,
    /// what the analysis found: verdicts, notes, summaries, golden row checks, hover facts
    /// (`interp/findings.rs`)
    pub(in crate::interp) findings: Findings,
    /// the report's names for unknown objects and calls (`interp/names.rs`)
    pub(in crate::interp) names: Names,
    /// unknown code's havoc walks (`interp/unknown.rs`)
    pub(in crate::interp) havoc: Havoc,
    /// what code may change when it does not run (`interp/effects.rs`)
    pub(in crate::interp) effects: effects::EffectsCache,
    /// budgets, the deadline and the work counters (`interp/budget.rs`)
    pub(in crate::interp) budget: Budget,
    /// the running activations, recursion's fixpoints (`interp/recursion.rs`)
    pub(in crate::interp) recursion: Recursion,
    /// entry runs: which functions had one, their receivers (`interp/entry.rs`)
    pub(in crate::interp) entry: Entries,
    /// generators and `@contextmanager` bodies run at their consumers (`interp/gens.rs`)
    pub(in crate::interp) gens: Gens,
    /// the ways out of running code: raises, try contexts, handlers, break / continue, finally
    /// (`interp/tries.rs`)
    pub(in crate::interp) unwind: Unwind,
    /// the context the running code is in: frames, callers' names, call chain, branch depth
    /// (`interp/context.rs`)
    pub(in crate::interp) cx: Context,
    /// modes an evaluation sets for the code it calls (`interp/context.rs`)
    pub(in crate::interp) mode: Modes,
    /// `None`
    pub none_v: Value,
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
        let program = Program::new(filename, idx, if follow_imports { search_paths } else { Vec::new() }, analyse_entry_points);
        let mut an = Analyzer {
            program,
            none_v: Value::lit(intern::key_none()),
            ..Default::default()
        };
        an.program.main_module = an.make_module("__main__", Some(filename), idx, tree);
        an.m.series = an.alloc_global(Kind::Series, "builtin:Series", "Series", Payload::None);
        Ok(an)
    }

    pub fn frame(&self) -> &Frame {
        self.cx.frames.last().expect("no frame")
    }
    pub fn frame_mut(&mut self) -> &mut Frame {
        self.cx.frames.last_mut().expect("no frame")
    }

    // ------------------------------------------------------------- entry

    /// Past the deadline a statement in a function body does not run: its path ends, and
    /// the call it belongs to is replaced by an unknown call (`call_user`). Module code goes on
    /// (its calls are unknown calls). Checked every 64 statements.
    pub(in crate::interp) fn cut_body(&mut self, state: &mut State) -> bool {
        if !self.budget.tick() {
            return false;
        }
        if self.cx.frames.last().is_some_and(|f| f.kind == FrameKind::Function) {
            state.reachable = false;
            return true;
        }
        false
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


