//! The interpreter's interface to the library models (`models/`), the report and the driver:
//! `Call` (everything a model is told about a call), `Args` (a call's arguments, as calls pass
//! them on), and these methods.
//! The components' state (`program`, `findings`, `names`, `budget`, `recursion`, `entry`,
//! `gens`, `unwind`, `cx`, `mode`, …) is the interpreter's own (`pub(in crate::interp)`), and
//! so are its methods unless marked `pub`. A model sees:
//!
//! * the `Heap` (objects, payloads, lengths, class attributes, keys standing for objects) and
//!   its own `models::Tables`;
//! * the methods in this file: modules and models, frames (a local's assignments), branches,
//!   entry runs, notes, guarded reads, raises with an exception object, catching frames, plain
//!   setattr, symbols;
//! * the interpreter's `pub` methods, which are these:
//!   - evaluation: `eval`, `eval_key`, `exec_block`, `run_module`, `stmt_try_parts`, `load_name`,
//!     `frame`, `frame_mut`;
//!   - modules: `import_module`, `make_module`, `is_local_module`, `type_only_name`,
//!     `file_of_frame`;
//!   - reads and writes: `load_attr`, `load_item`, `store_attr`, `store_item`, `store_key`,
//!     `delete_item`, `delete_subscript`, `copy_into`, `copy_object`, `method_value`,
//!     `class_lookup`, `class_lookup_on_path`, the literal views (`lit_keys`, `sure_lit_keys`,
//!     `literal_mapping`, `literal_dict_items`, `maybe_dict_items`, `known_members`,
//!     `known_positions`, `all_values`, `record_objects`) and their writers (`set_members`,
//!     `set_positions`, `add_elem`, `normalize_set`, `literal_set`), keys (`unkey`, `opaque_key`,
//!     `value_key`, `rename_kv`, `hashes_keys`, `check_hashable`, `check_hashable_if`),
//!     `path_placeholder_value`;
//!   - operations: `binop`, `truthiness`, `truth_of`, `instance_less`, `isinstance_classes`,
//!     `instance_of`, `hierarchy_closed`, `value_type`, `as_type`;
//!   - iteration: `iterate`, `iterable_part`, `unpack`, `elem_of`, `elem_value`, `consume_lazy`,
//!     `cursor_of`, `set_cursor`, `exhaust`, `gen_took_item`;
//!   - calls: `call_value`, `call_one`, `call_user`, `call_external`, `unknown_call`,
//!     `construct`, `call_method_if_defined`, `call_method_with`, `run_dunder`,
//!     `decorator_qualname`;
//!   - allocation: `alloc`, `alloc_at`, `alloc_simple`, `alloc_instance`, `alloc_positional`,
//!     `alloc_positional_sfx`, `unknown_source`, `as_annotated` (an unknown as annotated);
//!   - raises: `raise_here`, `may_raise_here`, `flag_may_raise`, `enter_try`, `leave_try`,
//!     `reraise_rec`, `guards`;
//!   - verdicts and requirements: `verdict`, `verdict_guarded`, `is_guarded`,
//!     `require_providers`, `require_providers_k`;
//!   - also `obj_edges` (what an object refers to), `strip_annotation`, `slice_label_bounds`;
//!   - the driver's: `new`, `run`, `was_cut`, `set_cancel`, and for runs on demand
//!     `main_block_unrun`, `on_demand`, `general_run_at`, `general_run_of`,
//!     `outermost_functions`.

use indexmap::IndexMap;
use ruff_python_ast as ast;

use super::{Analyzer, CmHook, ExcNames, Frame, FrameKind, FuncInfo, Names};
use crate::domain::{row_joins, ObjId, Value};
use crate::frontend::Node;
use crate::intern::{s, KeyId, StrId};
use crate::state::{fp_visits, State};

/// A call's arguments as given: positional ones, keywords, and the values of `*xs` and `**kw`.
#[derive(Clone, Copy)]
pub struct Args<'a> {
    pub pos: &'a [Value],
    pub kw: &'a IndexMap<StrId, Value>,
    pub star: &'a [Value],
    pub star_kw: &'a [Value],
}

impl<'a> Args<'a> {
    pub fn new(pos: &'a [Value], kw: &'a IndexMap<StrId, Value>, star: &'a [Value], star_kw: &'a [Value]) -> Args<'a> {
        Args { pos, kw, star, star_kw }
    }

    /// Keyword arguments only.
    pub fn kw(kw: &'a IndexMap<StrId, Value>) -> Args<'a> {
        Args { pos: &[], kw, star: &[], star_kw: &[] }
    }

    /// Positional arguments only.
    pub fn pos(pos: &'a [Value]) -> Args<'a> {
        static NO_KEYWORDS: std::sync::LazyLock<IndexMap<StrId, Value>> = std::sync::LazyLock::new(IndexMap::new);
        Args { pos, kw: &NO_KEYWORDS, star: &[], star_kw: &[] }
    }
}

/// Everything a model needs about a call.
#[derive(Clone)]
pub struct Call {
    /// the qualified name of what is called (`json.loads`, `dict.get`)
    pub qual: String,
    /// a method's receiver
    pub recv: Option<Value>,
    pub args: Vec<Value>,
    pub kwargs: IndexMap<StrId, Value>,
    /// the values of `*xs` and `**kw` arguments
    pub star_args: Vec<Value>,
    pub star_kwargs: Vec<Value>,
    pub node: Node,
    /// the call as written (None: a call the interpreter makes, a dunder, a callback)
    pub call_ast: Option<&'static ast::ExprCall>,
    /// `L{line}`: where the call's objects are made
    pub site: String,
    pub line: u32,
}

impl Call {
    /// The call's arguments.
    pub fn arguments(&self) -> Args<'_> {
        Args::new(&self.args, &self.kwargs, &self.star_args, &self.star_kwargs)
    }

    /// Argument `i`, or the keyword `name`.
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
    /// The keyword argument `name`.
    pub fn kw(&self, name: &str) -> Option<Value> {
        self.kwargs.get(&s(name)).cloned()
    }
}

impl Analyzer {
    // ------------------------------------------------------------ the program
    /// The module imported under `name`, if it is.
    pub fn module(&self, name: &str) -> Option<ObjId> {
        self.program.modules.get(name).copied()
    }

    /// The module object of frontend module `idx`, if it was imported.
    pub fn module_of_idx(&self, idx: u32) -> Option<ObjId> {
        self.program.modules.values().copied().find(|m| self.heap.module_info(*m).idx == idx)
    }

    /// The modules imported so far, by name.
    pub fn modules(&self) -> &IndexMap<String, ObjId> {
        &self.program.modules
    }

    /// The model of a library function by qualified name.
    pub fn model(&self, qual: &str) -> Option<crate::models::Model> {
        self.program.models.lookup(qual)
    }

    /// The classes the program defines, in definition order.
    pub fn user_classes(&self) -> &[ObjId] {
        &self.program.user_classes
    }

    /// The main module's object.
    pub fn main_module(&self) -> ObjId {
        self.program.main_module
    }
    /// The main module's frontend index.
    pub fn main_idx(&self) -> u32 {
        self.program.main_idx
    }

    /// Module `idx`'s path relative to the main file's directory ("" for the main module).
    pub fn rel_file(&self, idx: u32) -> String {
        self.program.rel_file(idx)
    }

    /// Does module `m` import torch (its functions all get entry runs)?
    pub fn is_torch_module(&self, m: ObjId) -> bool {
        self.entry.torch_modules.contains(&m)
    }

    // ------------------------------------------------------------ the running code
    /// The running frames (innermost last).
    pub fn frames(&self) -> &[Frame] {
        &self.cx.frames
    }

    /// `f` runs in frame `fr` (code evaluated in another module, an annotation, a default).
    pub fn in_frame<R>(&mut self, fr: Frame, f: impl FnOnce(&mut Analyzer) -> R) -> R {
        self.cx.frames.push(fr);
        let r = f(self);
        self.cx.frames.pop();
        r
    }

    /// `f` runs in a module frame of `module` (context `ctx`, strong allocations).
    pub fn in_module<R>(&mut self, module: ObjId, ctx: &str, label: &str, f: impl FnOnce(&mut Analyzer) -> R) -> R {
        let mi = self.heap.module_info(module);
        self.in_frame(Frame::new(FrameKind::Module, mi.scopes.module(), module, ctx, true, label), f)
    }

    /// The names of the caller `back` calls up (1: the running call's caller).
    pub fn caller_env(&self, back: usize) -> Option<&IndexMap<StrId, Value>> {
        let n = self.cx.env_stack.len();
        if back > n {
            return None;
        }
        self.cx.env_stack.get(n - back)
    }

    /// The call sites (lines) of the running calls, innermost last.
    pub fn call_chain(&self) -> &[u32] {
        &self.cx.call_chain
    }

    /// How many branches and loops the running code is inside of.
    pub fn cond_depth(&self) -> i32 {
        self.cx.cond_depth
    }

    /// `f` runs under a branch: the requirements it records are "sometimes".
    pub fn in_branch<R>(&mut self, f: impl FnOnce(&mut Analyzer) -> R) -> R {
        self.cx.branch_depth += 1;
        let r = f(self);
        self.cx.branch_depth -= 1;
        r
    }

    /// The function an entry run (open parameters) is analysing.
    pub fn entry_function(&self) -> Option<ObjId> {
        self.entry.run
    }

    /// `f` runs as the entry run of `oid` (a framework calls it with any input), in a module
    /// frame of its module, given the node its calls are made at.
    pub fn as_entry_run<R>(&mut self, oid: ObjId, fi: &FuncInfo, general: bool, f: impl FnOnce(&mut Analyzer, &Node) -> R) -> R {
        let mi = self.heap.module_info(fi.module);
        self.cx.frames.push(Frame::new(FrameKind::Module, mi.scopes.module(), fi.module, &format!("entry:{}", fi.qualname), true, "<entry>"));
        self.entry.scope = Some(oid);
        let fake = Node::synthetic(mi.idx, fi.node.node().line);
        let saved_entry = self.entry.run.replace(oid);
        let saved_general = std::mem::replace(&mut self.entry.general, general.then_some(oid));
        let r = f(self, &fake);
        self.entry.scope = None;
        self.entry.run = saved_entry;
        self.entry.general = saved_general;
        self.cx.frames.pop();
        r
    }

    /// Function `f` needs no entry run of its own (a framework's handler run already).
    pub fn mark_entry_done(&mut self, f: ObjId) {
        self.entry.done.insert(f);
    }

    /// `f` runs with `then` as the body of the `@contextmanager`-style generator it calls next
    /// (run with the yielded value at the yield); the hook as it was left.
    pub fn with_cm_then(&mut self, then: Box<dyn FnOnce(&mut Analyzer, Value, &mut State)>, f: impl FnOnce(&mut Analyzer)) -> Option<CmHook> {
        let hook = CmHook { body: &[], target: None, depth: self.cx.frames.len(), env_after: None, ran: false, then: Some(then), body_died: false, env_before: None, multi: false, body_exits: None, body_ret: None };
        let prev = self.gens.cm_hook.replace(hook);
        f(self);
        std::mem::replace(&mut self.gens.cm_hook, prev)
    }

    // ------------------------------------------------------------ findings
    /// A note for the report.
    pub fn note(&mut self, msg: String) {
        self.findings.note(msg);
    }

    /// `f` runs as a requirement-only run: no verdicts, row checks, facts or notes.
    pub fn quietly<R>(&mut self, f: impl FnOnce(&mut Analyzer) -> R) -> R {
        let saved = std::mem::replace(&mut self.findings.quiet, true);
        let r = f(self);
        self.findings.quiet = saved;
        r
    }

    /// `f`'s accesses are guarded (a handler catches their failure) when `guarded`.
    pub fn guarded_if<R>(&mut self, guarded: bool, f: impl FnOnce(&mut Analyzer) -> R) -> R {
        let saved = self.mode.verdict_guarded;
        self.mode.verdict_guarded |= guarded;
        let r = f(self);
        self.mode.verdict_guarded = saved;
        r
    }

    /// The report's names for unknown objects and calls.
    pub fn names(&self) -> &Names {
        &self.names
    }

    /// Object `oid` is printed as `sym` (in function `scope`'s names; 0: module level).
    pub fn set_symbol(&mut self, oid: ObjId, sym: String, scope: u32) {
        self.names.set(oid, sym, scope);
    }

    /// What the running function's own code assigns to local `name` (`name = e`, `name &= e`:
    /// each `e`; not in nested functions); empty at module level.
    pub fn local_assignments(&self, name: &str) -> Vec<&'static ruff_python_ast::Expr> {
        let Some(fi) = self.frame().func.and_then(|f| self.heap.func_info_opt(f)) else { return vec![] };
        crate::syntax::assignments_to(fi.node.body(), name)
    }

    // ------------------------------------------------------------ exceptions
    /// An exception named `name` is raised here (`sure`) or may be: `exc` the exception object
    /// a model made (None: one of the class, if a handler needs it), `cls` its class names
    /// (None: the builtin class `name`).
    pub fn raise_exc(&mut self, name: &str, exc: Option<Value>, cls: Option<ExcNames>, sure: bool, state: &mut State) {
        let (had_exc, had_cls) = (exc.is_some(), cls.is_some());
        if let Some(e) = exc {
            self.unwind.cur_exc = Some(e);
        }
        if let Some(c) = cls {
            self.unwind.cur_exc_cls = Some(c);
        }
        if sure {
            self.raise_here(name, state);
        } else {
            self.may_raise_here(name, state);
        }
        if had_exc {
            self.unwind.cur_exc = None;
        }
        if had_cls {
            self.unwind.cur_exc_cls = None;
        }
    }

    /// Is a try statement (or a model's catching frame) running?
    pub fn in_try(&self) -> bool {
        !self.unwind.tries.is_empty()
    }

    /// `f` runs inside a call whose exceptions a thread / future keeps: its reads still
    /// require their keys.
    pub fn catching<R>(&mut self, f: impl FnOnce(&mut Analyzer) -> R) -> R {
        self.unwind.catching_depth += 1;
        let r = f(self);
        self.unwind.catching_depth -= 1;
        r
    }

    /// `f`'s reads of missing keys raise `exc` (pydantic's field reads: a ValidationError).
    pub fn with_item_exc<R>(&mut self, exc: &'static str, f: impl FnOnce(&mut Analyzer) -> R) -> R {
        let saved = std::mem::replace(&mut self.unwind.item_exc_force, Some(exc));
        let r = f(self);
        self.unwind.item_exc_force = saved;
        r
    }

    // ------------------------------------------------------------ stores
    /// Inside `object.__setattr__`: a plain store even when the class defines `__setattr__`.
    pub fn setattr_bypassed(&self) -> bool {
        self.mode.bypass_setattr
    }

    /// `f`'s attribute stores are plain (`object.__setattr__`).
    pub fn plain_setattr<R>(&mut self, f: impl FnOnce(&mut Analyzer) -> R) -> R {
        let saved = std::mem::replace(&mut self.mode.bypass_setattr, true);
        let r = f(self);
        self.mode.bypass_setattr = saved;
        r
    }

    // ------------------------------------------------------------ the driver
    /// Collect the hover facts (`--facts`).
    pub fn collect_facts(&mut self) {
        self.findings.facts_on = true;
    }

    /// Follow these installed packages into these site-packages directories.
    pub fn follow_packages(&mut self, follow: Vec<String>, site_paths: impl FnOnce() -> Vec<String>) {
        self.program.follow = follow;
        if !self.program.follow.is_empty() {
            self.program.site_paths = site_paths();
        }
    }

    /// A `#@row NAME:` check of the golden harness: (file relative to the main file's
    /// directory, line, name).
    pub fn request_row_check(&mut self, req: (String, u32, String)) {
        if !req.2.is_empty() && !self.findings.row_check_requests.contains(&req) {
            self.findings.row_check_requests.push(req);
        }
    }

    /// Does what is recorded here hold for any call of the running function (its run for any
    /// call, or module code)?
    pub fn for_any_call(&self) -> bool {
        self.in_general_run()
    }

    /// The golden harness checks values for any call (`#@general`).
    pub fn request_general_checks(&mut self) {
        self.findings.general_checks = true;
    }

    /// A key a row check names as absent.
    pub fn request_absent_probe(&mut self, probe: (String, u32, String, KeyId)) {
        self.findings.row_absent_probes.push(probe);
    }

    /// How the analysis went, for the editor's note on a cut one: whether the deadline (or a
    /// cancel) cut it, the time it took and its deadline, the statements it ran.
    pub fn analysis_json(&self) -> serde_json::Value {
        serde_json::json!({"cut": self.budget.hard_cut, "callees_cut": self.budget.hover_cut.get().then(super::budget::Budget::hover_work_limit), "ms": self.budget.elapsed_ms() as u64, "deadline_ms": super::limit("ms", super::DEADLINE_MS), "statements": self.budget.statements})
    }

    /// The work done so far: statements, body runs, objects, havoc visits (see `work_line`).
    pub fn work_counts(&self) -> [usize; 4] {
        [self.budget.statements, self.budget.runs, self.heap.len(), self.havoc.visited as usize]
    }

    /// The deterministic work counters (`ROWAN_DEBUG=1`'s `work:` line).
    pub fn work_line(&self) -> String {
        format!("work: {} (units {}, tabulations {}, objects {}, {} ms{}; budgeted tabulations {}, row joins {}, visits {} (walks {}, joins {}), havoc visits {})", self.budget.statements, self.budget.work_units(), self.budget.runs, self.heap.len(), self.budget.elapsed_ms(), if self.budget.hard_cut { ", cut at the deadline" } else { "" }, self.budget.spent_total(), row_joins(), self.budget.walked.get() + fp_visits(), self.budget.walked.get(), fp_visits(), self.havoc.visited)
    }
}
