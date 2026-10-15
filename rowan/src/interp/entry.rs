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

#[derive(Default)]
pub struct Entries {
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
}

impl Analyzer {
    pub fn run(&mut self) {
        let mut state = State::new();
        let main = self.program.main_module;
        self.run_module(main, &mut state);
        if self.program.analyse_entry_points {
            self.analyse_uncalled(&mut state);
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

    /// Entry-point rule (§6.4): functions never called get analysed with open parameters.
    pub(super) fn analyse_uncalled(&mut self, state: &mut State) {
        self.entry.called = (self.heap.len() as ObjId, self.entry.analysed.clone());
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
        // the general runs (hover facts only) of functions whose entry run took an instance
        // made with `__init__`'s defaults
        if self.findings.facts_on || !self.findings.row_check_requests.is_empty() {
            for oid in std::mem::take(&mut self.entry.facts_runs) {
                if self.budget.over_hard_deadline() {
                    break;
                }
                let fi = self.heap.func_info(oid);
                self.entry_run(oid, &fi, Run::FactsOnly, state);
            }
        }
    }

    pub(super) fn analyse_uncalled_range(&mut self, from: ObjId, n: ObjId, state: &mut State, quiet_pass: bool) {
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
                crate::models::fastapi::fa_entry_run(self, oid, &fi, state);
                continue;
            }
            self.entry_run(oid, &fi, if quiet { Run::Quiet } else { Run::Verdicts }, state);
        }
    }

    /// The entry run of `oid` with open parameters, from `state`; the requirements it finds on
    /// the inputs are kept in `state` (not those of a facts-only run).
    fn entry_run(&mut self, oid: ObjId, fi: &FuncInfo, run: Run, state: &mut State) {
        let quiet = run != Run::Verdicts;
        let Some(fparams) = fi.node.params() else {
            // a lambda without parameters
            if let FuncNode::Lambda(_) = fi.node {
                let mut st = state.copy();
                st.ret = None;
                let mi = self.heap.module_info(fi.module);
                self.cx.frames.push(Frame::new(FrameKind::Module, mi.scopes.module(), fi.module, &format!("entry:{}", fi.qualname), true, "<entry>"));
                let fake = Node::synthetic(mi.idx, fi.node.node().line);
                self.call_user(oid, None, vec![], IndexMap::new(), vec![], vec![], &mut st, &fake, true);
                self.cx.frames.pop();
            }
            return;
        };
        if debug_enabled() {
            eprintln!("[rowan] entry {} L{} t={:.1}s objs={} verdicts={} quiet={}", fi.qualname, fi.node.node().line, self.budget.now(), self.heap.len(), self.findings.mark(), quiet);
        }
        // a closure made by a factory's entry run starts from that run's state (its cells)
        let mut st = match self.entry.closure_states.get(&oid) {
            Some(cs) => {
                let mut c = cs.copy();
                c.reachable = true;
                c.exits = None;
                c
            }
            None => state.copy(),
        };
        st.ret = None;
        let mi = self.heap.module_info(fi.module);
        self.cx.frames.push(Frame::new(FrameKind::Module, mi.scopes.module(), fi.module, &format!("entry:{}", fi.qualname), true, "<entry>"));
        self.entry.scope = Some(oid);
        // (a run with verdicts takes instances made with `__init__`'s defaults, as a caller
        // that passes nothing — its verdicts are those; the others, and torch modules', any
        // instance)
        let saved_any = std::mem::replace(&mut self.entry.any_instance, quiet || self.entry.torch_modules.contains(&fi.module));
        let saved_used = std::mem::replace(&mut self.entry.used_defaults, false);
        let saved_q = std::mem::replace(&mut self.findings.quiet, quiet);
        let mut params = scopemod::all_params(fparams);
        let self_val = self.entry_receiver(fi, &mut params, &mut st, state);
        let (args, kwargs, star_args, star_kwargs) = self.entry_arguments(oid, fi, fparams, params, &mut st);
        self.findings.quiet = saved_q;
        let defaults = std::mem::replace(&mut self.entry.used_defaults, saved_used);
        self.entry.any_instance = saved_any;
        self.entry.scope = None;
        if defaults && run == Run::Verdicts && (self.findings.facts_on || !self.findings.row_check_requests.is_empty()) {
            self.entry.facts_runs.push(oid);
        }
        let fake = Node::synthetic(mi.idx, fi.node.node().line);
        let saved_entry = self.entry.run.replace(oid);
        let saved_general = std::mem::replace(&mut self.entry.general, (!defaults).then_some(oid));
        let saved_nofacts = std::mem::replace(&mut self.entry.no_facts, defaults.then_some(oid));
        let before = self.heap.len() as ObjId;
        let saved_quiet = std::mem::replace(&mut self.findings.quiet, quiet);
        let called_torch = self.entry.analysed.contains(&oid) && self.entry.torch_modules.contains(&fi.module);
        let saved_qr = std::mem::replace(&mut self.findings.quiet_records, called_torch);
        self.call_user(oid, self_val, args, kwargs, star_args, star_kwargs, &mut st, &fake, true);
        self.findings.quiet_records = saved_qr;
        self.findings.quiet = saved_quiet;
        self.entry.run = saved_entry;
        self.entry.general = saved_general;
        self.entry.no_facts = saved_nofacts;
        if quiet {
            // functions made by a requirement-only run are not the program's
            let after = self.heap.len() as ObjId;
            for o in before..after {
                if self.heap.kind(o) == Kind::Function {
                    self.entry.done.insert(o);
                }
            }
        }
        if self.entry.torch_modules.contains(&fi.module) {
            let after = self.heap.len() as ObjId;
            let made: Vec<ObjId> = (before..after).filter(|o| self.heap.kind(*o) == Kind::Function && !self.heap.func_info(*o).closure.is_empty()).collect();
            if !made.is_empty() {
                let snap = std::rc::Rc::new(st.copy());
                for o in made {
                    self.entry.closure_states.insert(o, snap.clone());
                }
            }
        }
        self.cx.frames.pop();
        if run != Run::FactsOnly {
            self.keep_entry_requirements(fi, quiet, &st, state);
        }
    }

    /// The receiver of a method's entry run: an instance of its class or of one of its
    /// subclasses (made once per class, from `state`; `st` starts over from it then).
    fn entry_receiver(&mut self, fi: &FuncInfo, params: &mut Vec<&'static ast::Parameter>, st: &mut State, state: &mut State) -> Option<Value> {
        let mut self_val: Option<Value> = None;
        if let Some(cls) = fi.cls {
            if !fi.is_static.get() {
                // (an instance of the class or of one of its subclasses: what their
                // `__init__`s set may be there; a torch module's method is analysed in
                // its own terms: its attributes as symbols)
                let torch = self.entry.torch_modules.contains(&fi.module);
                let subs: Vec<ObjId> = self.program.user_classes.iter().copied().filter(|c| *c == cls || (!torch && self.heap.mro(*c).contains(&cls))).collect();
                let mut insts: Vec<ObjId> = Vec::new();
                for c in subs {
                    // (the instance made with the defaults is made too, whichever run needs a
                    // receiver first: its `__init__` run's verdicts carry much of a library's
                    // coverage — pip, sktime)
                    if self.entry.any_instance && !torch && !self.entry.instances.contains_key(&c) {
                        let saved = (self.entry.any_instance, self.entry.used_defaults);
                        self.entry.any_instance = false;
                        let cname = self.heap.class_info(c).name.clone();
                        let i = self.fresh_instance(c, state, &format!("entry:{}", cname));
                        self.entry.instances.insert(c, i);
                        (self.entry.any_instance, self.entry.used_defaults) = saved;
                        *st = state.copy();
                        st.ret = None;
                    }
                    let made = if self.entry.any_instance { self.entry.general_instances.get(&c) } else { self.entry.instances.get(&c) };
                    self.entry.used_defaults |= !self.entry.any_instance;
                    let inst = match made {
                        Some(&i) => i,
                        None => {
                            let cname = self.heap.class_info(c).name.clone();
                            let i = self.fresh_instance(c, state, &format!("entry:{}", cname));
                            if self.entry.any_instance {
                                self.entry.general_instances.insert(c, i);
                            } else {
                                self.entry.instances.insert(c, i);
                            }
                            *st = state.copy();
                            st.ret = None;
                            i
                        }
                    };
                    insts.extend(inst);
                }
                // (no `__init__` with its defaults completes: an instance some other call made)
                if insts.is_empty() {
                    let ci = self.heap.class_info(cls);
                    insts.push(self.open_instance(cls, &ci, st, &format!("entry:{}", ci.name)));
                }
                self_val = Some(Value::objs(crate::domain::ObjSet::from_vec(insts)));
                if !params.is_empty() {
                    params.remove(0);
                }
            }
        }
        self_val
    }

    /// The open arguments of an entry run: (positional, keyword, `*args`, `**kwargs`).
    #[allow(clippy::type_complexity)]
    fn entry_arguments(&mut self, oid: ObjId, fi: &FuncInfo, fparams: &'static ast::Parameters, params: Vec<&'static ast::Parameter>, st: &mut State) -> (Vec<Value>, IndexMap<StrId, Value>, Vec<Value>, Vec<Value>) {
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
            let v = self.open_param(fi, a, false, st);
            // (keyword-only parameters are passed by keyword)
            if fparams.kwonlyargs.iter().any(|k| std::ptr::eq(&k.parameter, a)) {
                kwargs.insert(s(a.name.id.as_str()), v);
            } else {
                args.push(v);
            }
        }
        (args, kwargs, star_args, star_kwargs)
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
        let row = if ci.open_base || !(has_init || has_fields) { Row::with_source(Source::new(&format!("instance {}", ci.name), SourceKind::Class)) } else { Row::new() };
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
                let mut args = Vec::new();
                let mut kwargs: IndexMap<StrId, Value> = IndexMap::new();
                let (mut star_args, mut star_kwargs) = (Vec::new(), Vec::new());
                // (any arguments, as annotated, or the defaults: the instance any construction
                // may make, so that a method run on it holds for any call; or the defaults, as a
                // caller that passes nothing)
                for a in scopemod::all_params(params_all).into_iter().skip(1) {
                    if params_all.vararg.as_deref().is_some_and(|v| std::ptr::eq(v, a)) {
                        star_args.push(Value::obj(self.param_source(&fi, a, state)));
                        continue;
                    }
                    if params_all.kwarg.as_deref().is_some_and(|v| std::ptr::eq(v, a)) {
                        star_kwargs.push(Value::obj(self.param_source(&fi, a, state)));
                        continue;
                    }
                    let v = self.open_param(&fi, a, !self.entry.any_instance, state);
                    if params_all.kwonlyargs.iter().any(|k| std::ptr::eq(&k.parameter, a)) {
                        kwargs.insert(s(a.name.id.as_str()), v);
                    } else {
                        args.push(v);
                    }
                }
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
        Some(inst)
    }
}

/// What an entry run is for: verdicts (a function nothing calls), its own requirements (a
/// function the program calls: quiet), or hover facts only (`facts_runs`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Run {
    Verdicts,
    Quiet,
    FactsOnly,
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
