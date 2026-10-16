//! Running the program: the main module, then entry runs. Functions the program's own run
//! does not call run with open parameters (`param F.x` inputs; a method on an instance of its
//! class or a subclass), also those only other entry runs call; functions the program calls
//! get a requirement-only run for their own input contract; a
//! function some call of which its budget refused gets a reporting run. Functions of torch
//! modules get their own runs (shapes in terms of their parameters).
//!
//! For a call memo: `Entries` is flow-insensitive state calls change too (`analysed`: the
//! functions a call ran; `closure_states`, `concrete_closures`: closures a run made).

use indexmap::IndexMap;
use ruff_python_ast as ast;
use rustc_hash::{FxHashMap, FxHashSet};

use super::{debug_enabled, dotted_last, Analyzer, ClassInfo, FuncInfo, FuncNode, Frame, FrameKind, Payload};
use crate::domain::{Ns, ObjId, Row, Source, SourceKind, Value};
use crate::frontend::{self, Node};
use crate::intern::{self, key_str_lit, s, KeyId, KeySet, StrId};
use crate::scopes as scopemod;
use crate::state::{Kind, State};

/// Requirements per input (`State::reqs`).
type Reqs = crate::layered::TwoLevel<crate::domain::SourceId, KeyId, crate::domain::ReqInfo>;

#[derive(Default)]
pub struct Entries {
    /// on demand: the state every function's run starts from (`on_demand`)
    pub demand_start: Option<State>,
    /// the function an entry run (open parameters) is analysing
    pub run: Option<ObjId>,
    /// the function whose general run is going on (an entry run, or the `__init__` run that
    /// makes an entry run's instance, with any arguments): what its own lines record holds for
    /// any call
    pub general: Option<ObjId>,
    /// while an entry run's parameters are made: the function they belong to
    pub scope: Option<u32>,
    /// functions that had their entry run
    pub done: FxHashSet<ObjId>,
    /// functions some call ran
    pub analysed: FxHashSet<ObjId>,
    /// the functions the program's own run called (their entry run is requirement-only), and
    /// the objects made by then; a function an entry run makes is called when some run called
    /// it (whichever entry runs come first, a function only they call gets its own run)
    called: (ObjId, FxHashSet<ObjId>),
    /// per class: the instance its methods' entry runs take as `self` (None: its `__init__`
    /// does not complete with its defaults)
    pub instances: FxHashMap<ObjId, Option<ObjId>>,
    /// per class: the instance general runs take, its `__init__` given any arguments (as
    /// annotated, or the defaults): any instance a construction may make
    pub general_instances: FxHashMap<ObjId, Option<ObjId>>,
    /// instances made for this run are general ones (`general_instances`)
    any_instance: bool,
    /// an instance made with `__init__`'s defaults was made for this run: its values are not
    /// those of any call
    used_defaults: bool,
    /// the function whose own lines record no hover facts (a run with an instance made with
    /// `__init__`'s defaults: neither any call's nor the program's)
    pub no_facts: Option<ObjId>,
    /// functions whose entry run took an instance made with defaults: when hover facts are
    /// collected, a general run of their own (facts only) follows
    facts_runs: Vec<ObjId>,
    /// classes whose entry instance is being made (`fresh_instance`)
    pub fresh_in_progress: FxHashSet<ObjId>,
    /// modules that import torch (their functions all get entry runs)
    pub torch_modules: FxHashSet<ObjId>,
    /// closures made inside concrete calls in torch modules (no entry run of their own)
    pub concrete_closures: FxHashSet<ObjId>,
    /// the state a factory's entry run ended in, for the closures it made
    pub closure_states: FxHashMap<ObjId, std::rc::Rc<State>>,
    /// where the main module's run ended (`on_demand` goes back to it)
    module_end: Option<ModuleEnd>,
    /// functions run one at a time when asked (`general_run_of`): no instances made with
    /// `__init__`'s defaults
    pub on_demand: bool,
    /// set up for runs on demand: the main module's `__main__` block's calls run no body
    pub main_block_unrun: bool,
    /// per class: the instance runs for any call take, made in their start state (`base`)
    pub any_instances: FxHashMap<ObjId, Option<ObjId>>,
    /// the module's end is where any call starts: no function of the program may change what
    /// it holds (`after_any_code` changed nothing)
    plain_is_any: bool,
    /// the instances made now are for a run for any call (`entry_start`): the program's
    /// constructions and functions may have made and changed them
    for_any: bool,
    /// while the main module runs: the program's calls are noted (`note_call`)
    recording: bool,
    /// the functions the program called while the main module ran
    noted: FxHashSet<ObjId>,
    /// when values for any call are asked for: the heap at the last call module code made, and
    /// joined over them (`note_call`)
    calls_heap: Option<(State, State)>,
}

/// The analysis as the main module's run left it, before the entry runs: its state, the hover
/// facts and verdicts recorded by then, and whether the deadline cut it.
struct ModuleEnd {
    state: State,
    facts: IndexMap<(u32, StrId), super::Fact>,
    verdicts: usize,
    cut: bool,
}


/// On demand, a method of a class with more subclasses than this runs for any call on an
/// instance of the class made by its `__init__`, or of a subclass with any attributes
/// (`entry_receiver`).
const MANY_SUBCLASSES: usize = 64;
impl Analyzer {
    pub fn run(&mut self) {
        let mut state = State::new();
        let main = self.program.main_module;
        self.entry.recording = true;
        self.run_module(main, &mut state);
        self.entry.recording = false;
        if let Some((last, _)) = self.entry.calls_heap.as_mut() {
            *last = State::new();
        }
        let facts = if self.findings.facts_on { self.findings.facts.clone() } else { IndexMap::new() };
        self.entry.module_end = Some(ModuleEnd { state: state.copy(), facts, verdicts: self.findings.mark(), cut: self.budget.hard_cut });
        if self.program.analyse_entry_points {
            // (a function's run for any call starts after any of the program's functions ran)
            let mut base = state.copy();
            self.after_any_code(&mut base);
            self.entry.plain_is_any = base.same(&state);
            self.analyse_uncalled(&mut base, &mut state);
        }
        self.findings.final_state = Some(state);
    }

    pub fn run_module(&mut self, moid: ObjId, state: &mut State) {
        let info = self.heap.module_info(moid);
        let fr = Frame::new(FrameKind::Module, info.scopes.module(), moid, "", true, &format!("<module {}>", info.name));
        self.cx.frames.push(fr);
        // (imported from inside a function: the function's returns so far are its own)
        let (exits, ret) = (state.exits.take(), state.ret.take());
        self.with_gen_restart(state, true, &mut |an, s| an.exec_block(&info.tree.body, s));
        self.cx.frames.pop();
        state.reachable = true;
        state.exits = exits;
        state.ret = ret;
    }

    /// The program's call of `foid` from `state`, while the main module runs. A run for any
    /// call starts from the heaps of module code's calls too, joined (`calls_heap`): a call from
    /// inside a function sees what module code's last call saw, changed by the program's
    /// functions (`after_any_code` has that).
    pub(in crate::interp) fn note_call(&mut self, foid: ObjId, self_v: Option<&Value>, state: &State) {
        if !self.entry.recording || !state.reachable {
            return;
        }
        self.entry.noted.insert(foid);
        if !self.findings.wants_general() || self.frame().kind == FrameKind::Function {
            return;
        }
        let mut snap = state.heap_only();
        // (not the instance an `__init__` is making: no other code has it yet)
        if let Some(v) = self_v.filter(|_| self.heap.func_info(foid).name == "__init__") {
            v.pts.iter().for_each(|o| snap.forget_obj(o));
        }
        // (only what changed since the last one adds to the join)
        match self.entry.calls_heap.as_mut() {
            Some((last, joined)) => {
                joined.join_from(last, &snap, false);
                *last = snap;
            }
            None => self.entry.calls_heap = Some((snap.copy(), snap)),
        }
    }

    /// `state` after any of the program's functions ran, given anything (unknown code's walk of
    /// what their code may change), and as module code's calls saw it.
    fn after_any_code(&mut self, state: &mut State) {
        let code: Vec<ObjId> = (1..self.heap.len() as ObjId).filter(|o| self.heap.kind(*o) == Kind::Function && self.is_local_module(self.heap.func_info(*o).module)).collect();
        let rid = crate::interp::any_object(&mut self.heap);
        // (all of the program's code runs: what it does with a function's result is known
        // where the program is all its callers)
        self.effects.whole = Some(std::rc::Rc::new(self.caller_returns(state)));
        self.follow_code(code, rid, s("program@"), state);
        self.effects.whole = None;
        // (and as module code's calls saw it: what it changed later, of the objects it still has)
        if let Some((_, joined)) = &self.entry.calls_heap {
            state.join_known(joined);
        }
    }

    /// Where a run of `oid` for any call starts: `base`; the requirements are `reqs`' (a run
    /// keeps what it adds to them).
    pub(in crate::interp) fn entry_start(&self, base: &State, reqs: &State) -> State {
        let mut st = base.copy();
        st.reqs = reqs.reqs.clone();
        (st.ret, st.exits, st.reachable) = (None, None, true);
        st
    }

    /// Did the deadline cut the analysis (`run`)?
    pub fn was_cut(&self) -> bool {
        self.budget.hard_cut
    }

    /// The flag that, once set (from another thread), cuts the analysis as its deadline does.
    pub fn set_cancel(&mut self, flag: std::sync::Arc<std::sync::atomic::AtomicBool>) {
        self.budget.cancel = Some(flag);
    }

    /// Before `run`, for runs on demand: the calls of the main module's `__main__` block run no
    /// body (they are skipped calls: what they may change, changed).
    pub fn main_block_unrun(&mut self) {
        self.entry.main_block_unrun = true;
    }

    /// Go over to running functions on demand (`general_run_of`), after `run`: back to the
    /// state the main module's run ended in, with the hover facts it recorded for any call (module
    /// code; not what the program's calls gave) and its verdicts (none if the deadline cut it).
    pub fn on_demand(&mut self) {
        let Some(end) = self.entry.module_end.take() else { return };
        self.entry.on_demand = true;
        // (the instances entry runs made live in their states, not in the module's)
        self.entry.instances.clear();
        self.entry.general_instances.clear();
        self.entry.any_instances.clear();
        // (a call may come after any of the program's functions ran: what they may change of
        // what the module made is changed)
        let mut state = end.state;
        self.after_any_code(&mut state);
        self.entry.demand_start = Some(state.copy());
        self.findings.final_state = Some(state);
        self.findings.drop_since(if end.cut { 0 } else { end.verdicts });
        self.findings.facts = end.facts;
        self.findings.facts.retain(|_, f| {
            f.rows.clear();
            f.calls.clear();
            !f.general.is_empty()
        });
    }

    /// On demand: the general run (hover facts and verdicts) of the function of the main module whose
    /// definition spans `line` (the outermost one: a nested function runs in its run), from the
    /// state the module ended in, with a fresh budget and deadline. Its first and last lines;
    /// None if no function defined there exists.
    pub fn general_run_at(&mut self, line: u32) -> Option<(u32, u32)> {
        let main = self.program.main_module;
        let (oid, first, last) = self.outermost_functions(|m| m == main).into_iter().find(|(_, a, b)| *a <= line && line <= *b)?;
        self.general_run_of(oid).then_some((first, last))
    }

    /// The functions of the modules `pick` picks that are no other function's own (module-level
    /// defs, methods of classes outside functions), with their first and last lines.
    pub fn outermost_functions(&self, pick: impl Fn(ObjId) -> bool) -> Vec<(ObjId, u32, u32)> {
        let mut all: Vec<(ObjId, u32, u32, u32)> = Vec::new();
        let mut seen: FxHashSet<usize> = FxHashSet::default();
        for oid in 1..self.heap.len() as ObjId {
            if self.heap.kind(oid) != Kind::Function {
                continue;
            }
            let fi = self.heap.func_info(oid);
            if !matches!(fi.node, FuncNode::Def(_)) || !pick(fi.module) || !seen.insert(fi.node.addr()) {
                continue;
            }
            let n = fi.node.node();
            all.push((oid, n.module, n.line, n.end_line));
        }
        // (by module, then by first line, the wider first: a function inside the last one kept is not)
        all.sort_by_key(|&(_, m, a, b)| (m, a, std::cmp::Reverse(b)));
        let mut out: Vec<(ObjId, u32, u32)> = Vec::new();
        let mut last: Option<(u32, u32)> = None;
        for (oid, m, a, b) in all {
            if last.is_some_and(|(lm, lb)| lm == m && a <= lb) {
                continue;
            }
            last = Some((m, b));
            out.push((oid, a, b));
        }
        out
    }

    /// The work a run on demand is measured by: statements, objects made, objects visited by
    /// unknown code's walks (16 to a unit).
    pub(in crate::interp) fn hover_work(&self) -> usize {
        self.budget.statements + self.heap.len() + self.havoc.visited as usize / 16
    }

    /// On demand: the general run of function `oid` (hover facts and verdicts), from the state
    /// the module ended in, with a fresh budget and deadline; false if nothing is set up.
    pub fn general_run_of(&mut self, oid: ObjId) -> bool {
        // (every run from the same state: a run's answer does not depend on the runs before it,
        // and what they made does not weigh on it)
        let Some(mut state) = self.entry.demand_start.as_ref().map(|s| s.copy()) else { return false };
        self.entry.general_instances.clear();
        self.entry.any_instances.clear();
        self.budget.fresh();
        // (the verdicts kept are this run's)
        self.findings.drop_since(0);
        let fi = self.heap.func_info(oid);
        // (its own calls run whole; theirs, past the work a run on demand may do, are unknown)
        self.budget.hover = Some((self.hover_work(), 2));
        // (the instances made live on in the state; the requirements found go to it too)
        let mut reqs = state.copy();
        self.entry_run(oid, &fi, Run::OnDemand, &mut state, &mut reqs);
        state.reqs = reqs.reqs;
        self.budget.hover = None;
        self.findings.final_state = Some(state);
        true
    }

    /// Entry-point rule (§6.4): functions never called get analysed with open parameters, from
    /// `base` (instances for them are made there); the requirements they find go to `state`.
    pub(super) fn analyse_uncalled(&mut self, base: &mut State, state: &mut State) {
        self.entry.called = (self.heap.len() as ObjId, self.entry.analysed.clone());
        // entry runs can make functions (closures, lambdas, inner defs) nobody calls either:
        // a few rounds, each over the functions made so far
        let mut start: ObjId = 1;
        for _round in 0..4 {
            let n = self.heap.len() as ObjId;
            if start >= n {
                break;
            }
            self.analyse_uncalled_range(start, n, base, state, false);
            start = n;
        }
        // then the requirement-only runs of the functions the program calls (last: runs made
        // while quiet hold no verdicts)
        let n = self.heap.len() as ObjId;
        self.analyse_uncalled_range(1, n, base, state, true);
        // the general runs (hover facts only) of functions whose entry run took an instance
        // made with `__init__`'s defaults
        if self.findings.wants_general() {
            for oid in std::mem::take(&mut self.entry.facts_runs) {
                if self.budget.over_hard_deadline() {
                    break;
                }
                let fi = self.heap.func_info(oid);
                if self.m.fa_handlers.contains(&oid) {
                    // (a handler's run for any request: its requirements are its run's with
                    // verdicts; what it reads of its parameters, its contract)
                    let mut st = self.entry_start(base, state);
                    st.reqs = Default::default();
                    let (saved, mut scratch) = (std::mem::replace(&mut self.findings.quiet, true), State::new());
                    crate::models::fastapi::fa_entry_run(self, oid, &fi, st, &mut scratch, true);
                    self.findings.quiet = saved;
                    self.note_contract(&fi, &scratch.reqs, &Default::default());
                    continue;
                }
                self.entry_run(oid, &fi, Run::FactsOnly, base, state);
            }
        }
    }

    pub(super) fn analyse_uncalled_range(&mut self, from: ObjId, n: ObjId, base: &mut State, state: &mut State, quiet_pass: bool) {
        for oid in from..n {
            if self.budget.over_hard_deadline() {
                break;
            }
            if self.heap.kind(oid) != Kind::Function || self.entry.done.contains(&oid) {
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
            let called = if oid < self.entry.called.0 { self.entry.called.1.contains(&oid) } else { self.entry.analysed.contains(&oid) };
            let quiet = called && !self.entry.torch_modules.contains(&fi.module) && !handler && !refused;
            if quiet != quiet_pass && !(quiet_pass && refused) {
                continue;
            }
            if quiet && (!fi.closure.is_empty() || !matches!(fi.node, FuncNode::Def(_)) || !self.is_local_module(fi.module)) || self.entry.concrete_closures.contains(&oid) {
                continue;
            }
            self.entry.done.insert(oid);
            if fi.module != self.program.main_module && !handler && !((quiet || refused) && self.is_local_module(fi.module)) {
                continue;
            }
            if handler {
                // (with verdicts, from the module's end: its values hold for any request only if
                // that is where any call starts)
                let general = self.entry.plain_is_any && !self.entry.noted.contains(&oid);
                let mut st = state.copy();
                st.ret = None;
                let before = state.reqs.clone();
                crate::models::fastapi::fa_entry_run(self, oid, &fi, st, state, general);
                if general {
                    self.note_contract(&fi, &state.reqs.clone(), &before);
                }
                if !general && (self.findings.wants_general()) {
                    self.entry.facts_runs.push(oid);
                }
                continue;
            }
            self.entry_run(oid, &fi, if quiet { Run::Quiet } else { Run::Verdicts }, base, state);
        }
    }

    /// The entry run of `oid` with open parameters, from `base` (`entry_start`); the requirements
    /// it finds on the inputs are kept in `state` (not those of a facts-only run).
    fn entry_run(&mut self, oid: ObjId, fi: &FuncInfo, run: Run, base: &mut State, state: &mut State) {
        // (on demand: verdicts, for any call)
        let quiet = !matches!(run, Run::Verdicts | Run::OnDemand);
        let Some(fparams) = fi.node.params() else {
            // a lambda without parameters
            if let FuncNode::Lambda(_) = fi.node {
                let mut st = self.entry_start(base, state);
                let mi = self.heap.module_info(fi.module);
                self.cx.frames.push(Frame::new(FrameKind::Module, mi.scopes.module(), fi.module, &format!("entry:{}", fi.qualname), true, "<entry>"));
                let fake = Node::synthetic(mi.idx, fi.node.node().line);
                self.call_user(oid, None, vec![], IndexMap::new(), vec![], vec![], &mut st, &fake, true);
                self.cx.frames.pop();
            }
            return;
        };
        if debug_enabled() {
            eprintln!("[rowan] entry {} L{} t={:.1}s objs={} verdicts={} run={:?} stmts={}", fi.qualname, fi.node.node().line, self.budget.now(), self.heap.len(), self.findings.mark(), run, self.budget.statements);
        }
        let mi = self.heap.module_info(fi.module);
        self.cx.frames.push(Frame::new(FrameKind::Module, mi.scopes.module(), fi.module, &format!("entry:{}", fi.qualname), true, "<entry>"));
        self.entry.scope = Some(oid);
        // (a run with verdicts takes instances made with `__init__`'s defaults, as a caller
        // that passes nothing — its verdicts are those; the others, and torch modules', any
        // instance)
        let saved_any = std::mem::replace(&mut self.entry.any_instance, run != Run::Verdicts || self.entry.torch_modules.contains(&fi.module));
        let saved_used = std::mem::replace(&mut self.entry.used_defaults, false);
        let saved_q = std::mem::replace(&mut self.findings.quiet, quiet);
        let mut params = scopemod::all_params(fparams);
        // (a run with verdicts, or with its own requirements, is one call right after the module
        // ran, from `state`; the others hold for any call, from `base` and the program's calls:
        // `entry_start`)
        let plain = matches!(run, Run::Verdicts | Run::Quiet);
        let saved_for_any = std::mem::replace(&mut self.entry.for_any, !plain);
        base.reqs = state.reqs.clone();
        let self_val = self.entry_receiver(fi, &mut params, state, base, plain);
        if !plain {
            state.reqs = base.reqs.clone();
        }
        let mut st = self.entry_run_start(oid, run, base, state);
        let (args, kwargs, star_args, star_kwargs) = self.entry_arguments(oid, fi, fparams, params, false, &mut st);
        self.findings.quiet = saved_q;
        let defaults = std::mem::replace(&mut self.entry.used_defaults, saved_used);
        self.entry.any_instance = saved_any;
        self.entry.for_any = saved_for_any;
        self.entry.scope = None;
        // (its values hold for any call: not one with an instance made with the defaults, nor
        // one from the module's end unless that is where any call starts)
        let general = !defaults && (!plain || (self.entry.plain_is_any && !self.entry.noted.contains(&oid)));
        if !general && plain && self.findings.wants_general() {
            self.entry.facts_runs.push(oid);
        }
        let fake = Node::synthetic(mi.idx, fi.node.node().line);
        let saved_entry = self.entry.run.replace(oid);
        let saved_general = std::mem::replace(&mut self.entry.general, general.then_some(oid));
        let saved_nofacts = std::mem::replace(&mut self.entry.no_facts, (!general).then_some(oid));
        let before = self.heap.len() as ObjId;
        let saved_quiet = std::mem::replace(&mut self.findings.quiet, quiet);
        let called_torch = self.entry.analysed.contains(&oid) && self.entry.torch_modules.contains(&fi.module);
        let saved_qr = std::mem::replace(&mut self.findings.quiet_records, called_torch);
        let start_reqs = st.reqs.clone();
        self.call_user(oid, self_val, args, kwargs, star_args, star_kwargs, &mut st, &fake, true);
        if general {
            self.note_contract(fi, &st.reqs, &start_reqs);
        }
        self.findings.quiet_records = saved_qr;
        self.findings.quiet = saved_quiet;
        self.entry.run = saved_entry;
        self.entry.general = saved_general;
        self.entry.no_facts = saved_nofacts;
        self.functions_made(fi, quiet, before, &st);
        self.cx.frames.pop();
        if run != Run::FactsOnly {
            self.keep_entry_requirements(fi, quiet || run == Run::OnDemand, &st, state);
        }
    }

    /// Where an entry run of `oid` starts: a closure made by a factory's entry run, from that
    /// run's state (its cells); a run with verdicts or its own requirements, from `state`; one
    /// for any call, from `entry_start` (a facts-only one with only its own requirements: its
    /// function's contract).
    fn entry_run_start(&mut self, oid: ObjId, run: Run, base: &State, state: &State) -> State {
        let mut st = match self.entry.closure_states.get(&oid) {
            Some(cs) => {
                let mut c = cs.copy();
                (c.reachable, c.exits) = (true, None);
                c
            }
            None if matches!(run, Run::Verdicts | Run::Quiet) => state.copy(),
            None => self.entry_start(base, state),
        };
        st.ret = None;
        if run == Run::FactsOnly {
            st.reqs = Default::default();
        }
        st
    }

    /// After an entry run of `fi` (`st` its end, `before` the objects made by its start): the
    /// functions a requirement-only run made are not the program's; the closures a torch
    /// module's function made start from its end.
    fn functions_made(&mut self, fi: &FuncInfo, quiet: bool, before: ObjId, st: &State) {
        let after = self.heap.len() as ObjId;
        if quiet {
            for o in before..after {
                if self.heap.kind(o) == Kind::Function {
                    self.entry.done.insert(o);
                }
            }
        }
        if self.entry.torch_modules.contains(&fi.module) {
            let made: Vec<ObjId> = (before..after).filter(|o| self.heap.kind(*o) == Kind::Function && !self.heap.func_info(*o).closure.is_empty()).collect();
            if !made.is_empty() {
                let snap = std::rc::Rc::new(st.copy());
                for o in made {
                    self.entry.closure_states.insert(o, snap.clone());
                }
            }
        }
    }

    /// The receiver of a method's entry run: an instance of its class or of one of its
    /// subclasses (made once per class, in `state`).
    fn entry_receiver(&mut self, fi: &FuncInfo, params: &mut Vec<&'static ast::Parameter>, state: &mut State, base: &mut State, plain: bool) -> Option<Value> {
        let mut self_val: Option<Value> = None;
        if let Some(cls) = fi.cls {
            if !fi.is_static.get() {
                // (an instance of the class or of one of its subclasses: what their
                // `__init__`s set may be there; a torch module's method is analysed in
                // its own terms: its attributes as symbols)
                let torch = self.entry.torch_modules.contains(&fi.module);
                let subs: Vec<ObjId> = self.program.user_classes.iter().copied().filter(|c| *c == cls || (!torch && self.heap.mro(*c).contains(&cls))).collect();
                // (a classmethod's is the class or one of its subclasses)
                if fi.is_classmethod.get() {
                    if !params.is_empty() {
                        params.remove(0);
                    }
                    return Some(Value::objs(crate::domain::ObjSet::from_vec(subs)));
                }
                let mut insts: Vec<ObjId> = Vec::new();
                // (on demand, a base class of very many: any instance of a subclass has any
                // attributes — each subclass's `__init__` run for any call made its methods' runs
                // slow; a whole run makes each once)
                let many = self.entry.on_demand && subs.len() > MANY_SUBCLASSES;
                for c in subs {
                    // (the instance made with the defaults is made too, whichever run needs a
                    // receiver first: its `__init__` run's verdicts carry much of a library's
                    // coverage — pip, sktime)
                    if self.entry.any_instance && !torch && !self.entry.on_demand && !self.entry.instances.contains_key(&c) {
                        let saved = (self.entry.any_instance, self.entry.used_defaults);
                        self.entry.any_instance = false;
                        let cname = self.heap.class_info(c).name.clone();
                        let i = self.fresh_instance(c, state, &format!("entry:{}", cname));
                        self.entry.instances.insert(c, i);
                        (self.entry.any_instance, self.entry.used_defaults) = saved;
                    }
                    // (a run for any call takes its instance from `base`)
                    let any = self.entry.any_instance;
                    let cache = match (any, plain) {
                        (false, _) => &mut self.entry.instances,
                        (true, true) => &mut self.entry.general_instances,
                        (true, false) => &mut self.entry.any_instances,
                    };
                    let made = cache.get(&c).copied();
                    self.entry.used_defaults |= !any;
                    let inst = match made {
                        Some(i) => i,
                        None => {
                            let cname = self.heap.class_info(c).name.clone();
                            let home = if any && !plain { &mut *base } else { &mut *state };
                            let i = if any && many && c != cls {
                                let ci = self.heap.class_info(c);
                                Some(self.open_instance(c, &ci, home, &format!("entry:{}", cname)))
                            } else {
                                self.fresh_instance(c, home, &format!("entry:{}", cname))
                            };
                            match (any, plain) {
                                (false, _) => self.entry.instances.insert(c, i),
                                (true, true) => self.entry.general_instances.insert(c, i),
                                (true, false) => self.entry.any_instances.insert(c, i),
                            };
                            i
                        }
                    };
                    insts.extend(inst);
                }
                // (no `__init__` with its defaults completes: an instance some other call made)
                if insts.is_empty() {
                    let ci = self.heap.class_info(cls);
                    let home = if plain { &mut *state } else { &mut *base };
                    insts.push(self.open_instance(cls, &ci, home, &format!("entry:{}", ci.name)));
                }
                self_val = Some(Value::objs(crate::domain::ObjSet::from_vec(insts)));
                if !params.is_empty() {
                    params.remove(0);
                }
            }
        }
        self_val
    }

    /// The open arguments of an entry run (`defaults_only`: a parameter with a default takes it):
    /// (positional, keyword, `*args`, `**kwargs`).
    #[allow(clippy::type_complexity)]
    fn entry_arguments(&mut self, oid: ObjId, fi: &FuncInfo, fparams: &'static ast::Parameters, params: Vec<&'static ast::Parameter>, defaults_only: bool, st: &mut State) -> (Vec<Value>, IndexMap<StrId, Value>, Vec<Value>, Vec<Value>) {
        let mut args: Vec<Value> = Vec::new();
        let mut star_args: Vec<Value> = Vec::new();
        let mut star_kwargs: Vec<Value> = Vec::new();
        if self.entry.torch_modules.contains(&fi.module) {
            for (i, a) in params.iter().enumerate() {
                crate::models::shape::note_param(crate::models::shape::sym(oid, a.name.id.as_str()), i as u32);
            }
        }
        let mut kwargs: IndexMap<StrId, Value> = IndexMap::new();
        for a in params {
            if [&fparams.vararg, &fparams.kwarg].iter().any(|v| v.as_deref().map(|v| std::ptr::eq(v, a)).unwrap_or(false)) {
                // a caller may pass any arguments here
                let v = Value::obj(self.param_source(fi, a, st));
                if fparams.vararg.as_deref().map(|v| std::ptr::eq(v, a)).unwrap_or(false) {
                    star_args.push(v);
                } else {
                    star_kwargs.push(v);
                }
                continue;
            }
            let v = self.open_param(fi, a, defaults_only, st);
            // (keyword-only parameters are passed by keyword)
            if fparams.kwonlyargs.iter().any(|k| std::ptr::eq(&k.parameter, a)) {
                kwargs.insert(s(a.name.id.as_str()), v);
            } else {
                args.push(v);
            }
        }
        (args, kwargs, star_args, star_kwargs)
    }

    /// Keep what a run of `fi` for any call read of its own parameters (`reqs` since `start`):
    /// its contract (`Findings::contracts`).
    fn note_contract(&mut self, fi: &FuncInfo, reqs: &Reqs, start: &Reqs) {
        let own = format!("param {}.", fi.py_qualname());
        let c = self.findings.contracts.get_or_insert_with(State::new);
        for (src, k) in reqs.diff_keys(start) {
            if !crate::domain::source_label(src).starts_with(&own) {
                continue;
            }
            let Some(info) = reqs.get(&src, &k) else { continue };
            let nv = match c.reqs.get(&src, &k) {
                Some(cur) => cur.join(&info),
                None => info,
            };
            c.reqs.set(&src, k, nv);
        }
    }

    /// Keep the schema requirements an entry run found (a requirement-only run: those of the
    /// function's own parameters).
    fn keep_entry_requirements(&self, fi: &FuncInfo, quiet: bool, st: &State, state: &mut State) {
        let own = format!("param {}.", fi.py_qualname());
        let span = (fi.node.node().line, fi.node.node().end_line);
        let in_main = fi.module == self.program.main_module;
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
    pub(super) fn param_source(&mut self, fi: &FuncInfo, a: &ast::Parameter, st: &mut State) -> ObjId {
        self.unknown_source(&format!("param {}.{}", fi.py_qualname(), a.name.id), SourceKind::Param, st, &format!("param {}", a.name.id), "", Ns::Item, vec![])
    }

    /// Parameter `a` of an entry run: its default or whatever a caller passes (only the
    /// default with `default_only`), as annotated; a number default in a torch module is not
    /// joined in (the parameter stays a name dims can be written in).
    pub(super) fn open_param(&mut self, fi: &FuncInfo, a: &'static ast::Parameter, default_only: bool, st: &mut State) -> Value {
        let d = fi.defaults.get(&s(a.name.id.as_str())).cloned();
        if let (Some(d), true) = (&d, default_only) {
            return d.clone();
        }
        // (with a default, only a torch annotation is taken: an instance for a defaulted
        // parameter of a user class made runs cost coverage, click 186 -> 151 sites)
        let ann = a.annotation.as_deref();
        let given = match ann.and_then(|ann| if d.is_some() { crate::models::torch::t_param(self, ann, fi, a.name.id.as_str(), st) } else { self.param_from_annotation(ann, fi, a.name.id.as_str(), st) }) {
            Some(v) => v,
            None => {
                let o = self.param_source(fi, a, st);
                // (`x: int`: an int, as the annotation says; `x: int | None` an int or None)
                match ann.and_then(builtin_annotation) {
                    Some((t, none)) => {
                        self.heap.obj_mut(o).ty = Some(t);
                        if none { Value::obj(o).join(&self.none_v) } else { Value::obj(o) }
                    }
                    None => Value::obj(o),
                }
            }
        };
        match d {
            Some(d) if !(self.entry.torch_modules.contains(&fi.module) && d.pts.is_empty() && d.kv.as_lit().map(intern::key_is_number).unwrap_or(false)) => d.join(&given),
            _ => given,
        }
    }

    pub(in crate::interp) fn param_from_annotation(&mut self, ann: &'static ast::Expr, fi: &FuncInfo, pname: &str, state: &mut State) -> Option<Value> {
        let node = Self::strip_annotation(ann)?;
        if let Some(v) = crate::models::torch::t_param(self, ann, fi, pname, state) {
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
                        if let Some((req, opt)) = crate::models::dataclasses::typed_dict_keys(self, oid) {
                            fields = req.union(&opt);
                            declared = Row::from_data(crate::domain::RowData { present: fields.clone(), definite: req.clone(), likely: req, ..Default::default() });
                        }
                    }
                    let inst = self.alloc(
                        Kind::Instance,
                        &format!("param:{}.{}:{}#{}", fi.qualname, pname, ci.name, oid),
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
                        self.m.td_param_src.insert(inst, src);
                    }
                    return Some(Value::obj(inst));
                }
                // (None: a caller's instance, made otherwise)
                return self.fresh_instance(oid, state, &format!("entry:{}.{}", fi.qualname, pname)).map(Value::obj);
            }
            if o == Kind::External && self.heap.external_qual(oid).ends_with("DataFrame") {
                let src = Source::new(&format!("param {}.{}", fi.py_qualname(), pname), SourceKind::Param);
                let fid = self.alloc(
                    Kind::Frame,
                    &format!("param:{}.{}", fi.qualname, pname),
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

    /// An instance of `cls_oid` as its `__init__` makes it (None: no path of it completes;
    /// `state` is then as it was): given any arguments in a general run (`any_instance`), else
    /// its defaults. Made once of each kind per class.
    pub(in crate::interp) fn fresh_instance(&mut self, cls_oid: ObjId, state: &mut State, ctx: &str) -> Option<ObjId> {
        let ci = self.heap.class_info(cls_oid);
        if self.entry.fresh_in_progress.contains(&cls_oid) {
            // self-referential annotation: open instance
            return Some(self.unknown_source(&format!("instance {}", ci.name), SourceKind::Class, state, &format!("{} (recursive)", ci.name), "", Ns::Attr, vec![]));
        }
        let any = self.entry.any_instance;
        self.entry.fresh_in_progress.insert(cls_oid);
        let ctx = if any { format!("any:{}", ctx) } else { ctx.to_string() };
        let r = self.fresh_instance_inner(cls_oid, &ci, state, &ctx);
        self.entry.fresh_in_progress.remove(&cls_oid);
        self.entry.used_defaults |= !any;
        r
    }

    /// An instance of `cls_oid` whose attributes are unknown (`instance C`).
    fn open_instance(&mut self, cls_oid: ObjId, ci: &ClassInfo, state: &mut State, ctx: &str) -> ObjId {
        self.instance_with(cls_oid, ci, Row::with_source(Source::new(&format!("instance {}", ci.name), SourceKind::Class)), state, ctx)
    }

    fn instance_with(&mut self, cls_oid: ObjId, ci: &ClassInfo, row: Row, state: &mut State, ctx: &str) -> ObjId {
        self.alloc(
            Kind::Instance,
            // (the class's own object: two classes may share a name)
            &format!("instance:{}#{}:{}", ci.name, cls_oid, ctx),
            state,
            &format!("{}()", ci.name),
            Payload::Cls(cls_oid),
            Some(row),
            Ns::Attr,
            Some(true),
            Some(ctx),
        )
    }

    fn fresh_instance_inner(&mut self, cls_oid: ObjId, ci: &ClassInfo, state: &mut State, ctx: &str) -> Option<ObjId> {
        let has_init = self.class_lookup(cls_oid, "__init__").is_some();
        let has_fields = self.heap.mro(cls_oid).iter().any(|c| self.heap.class_info_opt(*c).map(|ci| ci.has_fields()).unwrap_or(false));
        // a class that never initialises its own attributes is only used through subclasses
        // (any construction of a pydantic model without an `__init__` of the program's: what its
        // validation sets — a RootModel's `root` — is not in the fields)
        let pydantic = self.entry.any_instance && !has_init && self.heap.mro(cls_oid).iter().any(|c| self.heap.class_info_opt(*c).is_some_and(|ci| ci.lib.pydantic));
        let row = if ci.open_base || !(has_init || has_fields) || pydantic { Row::with_source(Source::new(&format!("instance {}", ci.name), SourceKind::Class)) } else { Row::new() };
        let inst = self.instance_with(cls_oid, ci, row, state, ctx);
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
        let pre = state.copy();
        if let Some(init_cls) = self.class_lookup(cls_oid, "__init__") {
            let inits = self.heap.cls_slots.get(&(init_cls, Ns::Attr, key_str_lit("__init__"))).cloned().unwrap_or_default();
            for init in inits.pts.iter() {
                if self.heap.kind(init) != Kind::Function {
                    continue;
                }
                let fi = self.heap.func_info(init);
                let Some(params_all) = fi.node.params() else { continue };
                let saved_scope = self.entry.scope;
                if self.entry.torch_modules.contains(&fi.module) {
                    self.entry.scope = Some(init);
                }
                // (any arguments, as annotated, or the defaults: the instance any construction
                // may make, so that a method run on it holds for any call; or the defaults, as a
                // caller that passes nothing)
                let params = scopemod::all_params(params_all).into_iter().skip(1).collect();
                let (args, kwargs, star_args, star_kwargs) = self.entry_arguments(init, &fi, params_all, params, !self.entry.any_instance, state);
                self.entry.scope = saved_scope;
                let fake = if self.entry.torch_modules.contains(&fi.module) { Node::synthetic(fi.node.node().module, frontend::node(ci.node).line) } else { Node::synthetic(fi.node.node().module, fi.node.node().line) };
                // (made with its defaults: with its verdicts, also for an entry run that has
                // none — made once, whichever entry run needs the instance first; with any
                // arguments, as quiet as the run it is made for)
                let saved_quiet = self.findings.quiet;
                self.findings.quiet &= self.entry.any_instance;
                // (with any arguments, a run of `__init__` for any call; with its defaults,
                // not one of the program's calls either: no hover facts)
                let any = self.entry.any_instance;
                let saved_general = std::mem::replace(&mut self.entry.general, any.then_some(init));
                let saved_nofacts = std::mem::replace(&mut self.entry.no_facts, (!any).then_some(init));
                self.call_user(init, Some(Value::obj(inst)), args, kwargs, star_args, star_kwargs, state, &fake, true);
                self.entry.general = saved_general;
                self.entry.no_facts = saved_nofacts;
                self.findings.quiet = saved_quiet;
            }
        }
        if !state.reachable {
            *state = pre;
            return None;
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
        if self.entry.any_instance && self.entry.for_any {
            self.methods_may_have_run(inst, state);
        }
        Some(inst)
    }
}

/// What an entry run is for: verdicts (a function nothing calls), its own requirements (a
/// function the program calls: quiet), hover facts only (`facts_runs`), or hover facts and its
/// own requirements and verdicts for any call, asked for on demand (`general_run_of`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Run {
    Verdicts,
    Quiet,
    FactsOnly,
    OnDemand,
}

/// The builtin type an annotation names (`int`, `"str"`, `list[int]`, `Optional[str]`, `int |
/// None`), and whether it allows None: a type whose instances a run for any call may take its
/// parameter to be (`float` is left out: an int may be given for it).
fn builtin_annotation(ann: &ast::Expr) -> Option<(&'static str, bool)> {
    const TYPES: [(&str, &str); 14] = [("bool", "bool"), ("bytes", "bytes"), ("int", "int"), ("str", "str"), ("list", "list"), ("dict", "dict"), ("set", "set"), ("tuple", "tuple"), ("frozenset", "frozenset"), ("List", "list"), ("Dict", "dict"), ("Set", "set"), ("Tuple", "tuple"), ("FrozenSet", "frozenset")];
    let is_none = |e: &ast::Expr| matches!(e, ast::Expr::NoneLiteral(_)) || matches!(e, ast::Expr::Name(n) if n.id.as_str() == "None");
    let named = |e: &ast::Expr| TYPES.iter().find(|(n, _)| *n == dotted_last(e)).map(|(_, t)| *t);
    match ann {
        ast::Expr::Name(_) | ast::Expr::Attribute(_) => named(ann).map(|t| (t, false)),
        ast::Expr::StringLiteral(sl) => frontend::parse_expression(sl.value.to_str()).and_then(builtin_annotation),
        // (`list[int]`: a list; `Optional[str]`: a str or None)
        ast::Expr::Subscript(sub) if dotted_last(&sub.value) == "Optional" => builtin_annotation(&sub.slice).map(|(t, _)| (t, true)),
        ast::Expr::Subscript(sub) => named(&sub.value).filter(|t| !matches!(*t, "bool" | "bytes" | "int" | "str")).map(|t| (t, false)),
        ast::Expr::BinOp(b) if b.op == ast::Operator::BitOr && (is_none(&b.left) || is_none(&b.right)) => {
            builtin_annotation(if is_none(&b.right) { &b.left } else { &b.right }).map(|(t, _)| (t, true))
        }
        _ => None,
    }
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
