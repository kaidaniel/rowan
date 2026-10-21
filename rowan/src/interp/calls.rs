//! Calls: `call_value` evaluates the arguments and dispatches on what the callee may be — the
//! program's functions and methods (`call_user`: the body runs at the call site, in a context of
//! the innermost call sites, within its function's budget), classes (`construct`: `__new__`,
//! the instance, `__init__`), library functions and methods (`call_external`, a model or an
//! unknown call), and several of these at once (`several_callees`: alternatives, one state
//! each). Also the frames calls run in and how arguments bind to parameters.

use indexmap::IndexMap;
use ruff_python_ast as ast;

use crate::domain::{chain_extend, join_values, kv_literals, Ns, ObjId, Row, Value, DATA};
use crate::frontend::Node;
use crate::intern::{self, key_str_lit, s, KeySet, StrId};
use crate::source::{Source, SourceKind};
use crate::state::{Kind, NAME_KEY_DROPS, State};

use super::{Analyzer, Args, Call, Frame, FrameKind, FuncInfo, FuncNode, MAX_CLONES, Payload};

impl Analyzer {
    // ---------------------------------------------------------------- calls
    /// A call of `fv` at `node`: the arguments of `call_ast` evaluated (or `override_args`),
    /// generators among them run (`force_gen`), then each callee `fv` may be called.
    /// Is every callee among `fv` one that takes all of an iterable's items at once?
    pub(super) fn bulk_consumer(&self, fv: &Value) -> bool {
        !fv.pts.is_empty() && fv.pts.iter().all(|o| match &self.heap.obj(o).payload {
            Payload::Str(q) if self.heap.kind(o) == Kind::External => super::BULK_CONSUMERS.contains(&q.as_str()),
            Payload::Method(q, _) => super::BULK_CONSUMERS.contains(&q.as_str()),
            _ => false,
        })
    }

    pub fn call_value(&mut self, fv: &Value, node: &Node, call_ast: Option<&'static ast::ExprCall>, state: &mut State, override_args: Option<Vec<Value>>) -> Value {
        let forced = self.force(fv.clone(), state);
        let fv = &forced;
        let mut args: Vec<Value> = Vec::new();
        let mut kwargs: IndexMap<StrId, Value> = IndexMap::new();
        let mut star_args: Vec<Value> = Vec::new();
        let mut star_kwargs: Vec<Value> = Vec::new();
        if let Some(oa) = override_args {
            args = oa;
        } else if let Some(c) = call_ast {
            for a in c.arguments.args.iter() {
                if let ast::Expr::Starred(st) = a {
                    star_args.push(self.eval(&st.value, state));
                } else {
                    args.push(self.eval(a, state));
                }
            }
            for kw in c.arguments.keywords.iter() {
                match &kw.arg {
                    None => star_kwargs.push(self.eval(&kw.value, state)),
                    Some(name) => {
                        let v = self.eval(&kw.value, state);
                        kwargs.insert(s(name.id.as_str()), v);
                    }
                }
            }
        }
        // an argument raised: the call never happens
        if !state.reachable {
            return Value::new();
        }
        // (what a running generator reads through library code: its arguments, its receiver)
        if self.gens.reads.iter().any(|r| r.1) && !fv.pts.iter().all(|o| matches!(self.heap.kind(o), Kind::Function | Kind::Bound | Kind::Class)) {
            let objs: Vec<ObjId> = fv.pts.iter().filter_map(|o| match &self.heap.obj(o).payload {
                Payload::Method(_, r) => Some(r.pts.to_vec()),
                _ => None,
            }).flatten().chain(args.iter().chain(kwargs.values()).chain(star_args.iter()).flat_map(|a| a.pts.iter())).collect();
            for o in objs {
                self.gens.read(o, None);
            }
        }
        // (generators passed or called through a method run now; unless the callee takes all
        // their items at once, code runs between the items)
        let bulk = self.bulk_consumer(fv);
        for o in fv.pts.iter() {
            if let Payload::Method(_, r) | Payload::Bound(_, r) = &self.heap.obj(o).payload {
                let r = r.clone();
                self.force_gen(&r, state, true);
            }
        }
        // (the program's own code takes them as they are: its loops run them; so does `iter`,
        // which gives them back)
        let user = !fv.pts.is_empty() && fv.pts.iter().all(|o| matches!(self.heap.kind(o), Kind::Function | Kind::Bound | Kind::Class) || matches!(&self.heap.obj(o).payload, Payload::Str(q) if q == "builtins.iter" && self.heap.kind(o) == Kind::External));
        for a in args.iter().chain(kwargs.values()).chain(star_args.iter()).chain(star_kwargs.iter()).filter(|_| !user) {
            self.force_gen(a, state, !bulk);
            self.sync_live(a, state, false);
        }
        let a = Args::new(&args, &kwargs, &star_args, &star_kwargs);
        // (None, a number, a str… is not callable: TypeError)
        let lits = kv_literals(&fv.kv);
        if !lits.is_empty() && lits.iter().all(|k| !intern::with_key(k, |k| matches!(k, intern::Key::Member(..) | intern::Key::Inv(..)))) {
            self.may_raise_here("TypeError", state);
            if fv.pts.is_empty() {
                state.reachable = false;
                return Value::new();
            }
        }
        if fv.pts.is_empty() {
            return self.unknown_call("?", None, a, state, node);
        }
        let targets: Vec<u32> = fv.pts.to_vec();
        if targets.len() == 1 {
            return self.call_one(targets[0], a, state, node, call_ast);
        }
        // (past the hard deadline: one unknown call for all of them)
        if self.budget.over_hard_deadline() && targets.iter().all(|o| matches!(self.heap.kind(*o), Kind::Function | Kind::Bound)) {
            return self.unknown_call("?", None, a, state, node);
        }
        let (unknown, targets) = self.unknown_callees(targets, call_ast);
        if targets.is_empty() {
            let (q, recv) = unknown.expect("several unknown callees");
            return self.unknown_call(&q, Some(&recv), a, state, node);
        }
        if unknown.is_none() && targets.len() > 1 && self.budget.past_hover_work(self.cx.frames.len(), self.hover_work()) {
            if let Some(r) = self.skipped_on_demand(&targets, a, state, node, call_ast) {
                return r;
            }
        }
        self.several_callees(targets, unknown, a, state, node, call_ast)
    }

    /// Several callees (functions, methods, classes; `unknown`: unknown code among them, one
    /// call): alternatives, one state each — methods of one function on several receivers
    /// together, those out of their budget one call that does not run (`budget_skipped`), and
    /// once none runs any more (on demand past the work its run may do, past the deadline)
    /// the rest one such call.
    fn several_callees(&mut self, targets: Vec<ObjId>, unknown: Option<(String, Value)>, a: Args, state: &mut State, node: &Node, call_ast: Option<&'static ast::ExprCall>) -> Value {
        let (groups, targets) = self.receiver_groups(targets);
        let (skipped, groups, targets) = self.budget_skipped(groups, targets);
        // (several functions a value picks — a method of each of several receivers is the
        // receiver's own: not picked)
        let picked = (groups.is_empty() && skipped.is_empty() && targets.iter().all(|o| !matches!(self.heap.kind(*o), Kind::Bound) && !matches!(self.heap.obj(*o).payload, Payload::Method(..) | Payload::Bound(..)))).then(|| self.node_text(node));
        let (n, m, k) = (targets.len(), groups.len(), usize::from(!skipped.is_empty()));
        // (the callees left once none runs any more — on demand past the work its run may do,
        // past the deadline: one call that does not run, for all of them)
        let callee = |an: &Analyzer, j: usize| -> Option<(ObjId, Option<Value>)> {
            match targets.get(j) {
                Some(&o) => an.function_callee(o),
                None => groups.get(j - n).map(|(f, r, _)| (*f, Some(r.clone()))),
            }
        };
        let mut rest_skipped: Vec<usize> = Vec::new();
        self.alternatives(n + m + k + usize::from(unknown.is_some()), false, state, &mut |an, i, st| {
            if rest_skipped.contains(&i) {
                st.reachable = false;
                return Value::new();
            }
            if i < n + m && rest_skipped.is_empty() && (an.budget.past_hover_work(an.cx.frames.len(), an.hover_work()) || an.budget.over_hard_deadline()) {
                let rest: Vec<usize> = (i..n + m).filter(|j| callee(an, *j).is_some()).collect();
                if rest.len() > 1 && rest[0] == i {
                    let fns: Vec<(ObjId, Option<Value>)> = rest.iter().filter_map(|j| callee(an, *j)).collect();
                    rest_skipped = rest;
                    return an.skipped_calls(&fns, a, st, node, call_ast);
                }
            }
            if let Some(&oid) = targets.get(i) {
                return an.call_picked(oid, &picked, a, st, node, call_ast);
            }
            if let Some((f, recv, _)) = groups.get(i - n) {
                return an.call_user(*f, Some(recv.clone()), a, st, node, false);
            }
            if i - n - m < k {
                return an.skipped_calls(&skipped, a, st, node, call_ast);
            }
            let (q, recv) = unknown.as_ref().expect("an unknown callee");
            an.unknown_call(q, Some(recv), a, st, node)
        })
    }

    /// On demand, past the work its run may do, several functions of the program none of which
    /// runs here: one call that does not run, for all of them. A class among them whose instance
    /// only its `__init__` makes (`plain_init`) gives its instance, made here, as that
    /// `__init__`'s receiver — `cls(**params)` for any of a thousand classes. None: a callee
    /// is none of those.
    fn skipped_on_demand(&mut self, targets: &[ObjId], a: Args, state: &mut State, node: &Node, call_ast: Option<&'static ast::ExprCall>) -> Option<Value> {
        let callees: Vec<(Option<ObjId>, Option<Value>, bool)> = targets.iter().map(|&o| match (self.heap.kind(o), &self.heap.obj(o).payload) {
            (Kind::Function, Payload::Func(_)) => Some((Some(o), None, false)),
            (Kind::Bound, Payload::Bound(f, r)) if self.heap.kind(*f) == Kind::Function => Some((Some(*f), Some(r.clone()), false)),
            (Kind::Class, _) => self.plain_init(o).map(|init| (init, Some(Value::obj(o)), true)),
            _ => None,
        }).collect::<Option<_>>()?;
        let funcs = callees.iter().any(|(_, _, class)| !class);
        let mut fns: Vec<(ObjId, Option<Value>)> = Vec::new();
        let mut made = Value::new();
        for (f, r, class) in callees {
            let r = match r.as_ref().and_then(|c| c.single()).filter(|_| class) {
                Some(cls) => {
                    let inst = Value::obj(self.alloc_instance(cls, node, state));
                    made = made.join(&inst);
                    Some(inst)
                }
                None => r,
            };
            fns.extend(f.map(|f| (f, r)));
        }
        let r = if fns.is_empty() { Value::new() } else { self.skipped_calls(&fns, a, state, node, call_ast) };
        Some(if made.pts.is_empty() || funcs { r.join(&made) } else { made })
    }

    /// Among a call's callees, the values rowan knows nothing of, or their methods: any of them is
    /// any code, so several are one unknown call given all of them (each its own would walk all
    /// it is given again, on a copy of the state): its name and receivers, and the other callees.
    #[allow(clippy::type_complexity)]
    fn unknown_callees(&self, targets: Vec<ObjId>, call_ast: Option<&'static ast::ExprCall>) -> (Option<(String, Value)>, Vec<ObjId>) {
        let unknown_code = |o: ObjId| match (self.heap.kind(o), &self.heap.obj(o).payload) {
            (Kind::Unknown, _) => Some(Value::obj(o)),
            (Kind::Method, Payload::Method(q, r)) if q.starts_with("unknown.") => Some(r.clone()),
            _ => None,
        };
        let (unknown, mut targets): (Vec<ObjId>, Vec<ObjId>) = targets.into_iter().partition(|o| unknown_code(*o).is_some());
        let unknown = match unknown.len() {
            0 => None,
            1 => {
                targets.push(unknown[0]);
                None
            }
            _ => {
                let q = match call_ast.and_then(|c| crate::syntax::chain_text(&c.func)) {
                    Some(text) => format!("call of {}", text),
                    None => format!("call of {}", self.heap.obj(unknown[0]).label),
                };
                Some((q, join_values(unknown.iter().filter_map(|o| unknown_code(*o)).collect::<Vec<_>>())))
            }
        };
        (unknown, targets)
    }

    /// A method on several receivers is one call with all of them as the receiver, not one per
    /// receiver, when the call would not run (past the function's budget, no recursion of it
    /// running) or in a run on demand (a base class's method run for the instances of every
    /// subclass); and its receivers that each stand for several objects (`merged`: stores into
    /// them only join) are one call, as a run for each of them would be no more exact where it
    /// stores, while nested calls on several receivers would multiply. The groups (function,
    /// receivers, how many), and the other callees.
    #[allow(clippy::type_complexity)]
    fn receiver_groups(&self, targets: Vec<ObjId>) -> (Vec<(ObjId, Value, usize)>, Vec<ObjId>) {
        let on_demand = self.budget.hover.is_some();
        let skips = |an: &Analyzer, f: ObjId| an.out_of_budget(f);
        let bound = |o: ObjId| match (self.heap.kind(o), &self.heap.obj(o).payload) {
            (Kind::Bound, Payload::Bound(f, r)) => Some((*f, r.clone())),
            _ => None,
        };
        let merged = |r: &Value| !r.pts.is_empty() && r.pts.iter().all(|o| !self.heap.obj(o).single);
        // per function: all its receivers, and those that stand for several objects
        let mut all: Vec<(ObjId, Value, usize, Value, usize)> = Vec::new();
        for (f, r) in targets.iter().filter_map(|o| bound(*o)) {
            let m = merged(&r);
            match all.iter_mut().find(|g| g.0 == f) {
                Some(g) => {
                    g.1 = g.1.join(&r);
                    g.2 += 1;
                    if m {
                        g.3 = g.3.join(&r);
                        g.4 += 1;
                    }
                }
                None => all.push((f, r.clone(), 1, if m { r } else { Value::new() }, usize::from(m))),
            }
        }
        // (whole: every receiver of the function in the group; else its merged ones)
        let mut groups: Vec<(ObjId, Value, usize, bool)> = Vec::new();
        for (f, r, n, mr, mn) in all {
            if n > 1 && (on_demand || skips(self, f)) {
                groups.push((f, r, n, true));
            } else if mn > 1 {
                groups.push((f, mr, mn, false));
            }
        }
        let targets: Vec<ObjId> = targets
            .into_iter()
            .filter(|o| match bound(*o) {
                Some((f, r)) => !groups.iter().any(|g| g.0 == f && (g.3 || merged(&r))),
                None => true,
            })
            .collect();
        (groups.into_iter().map(|(f, r, n, _)| (f, r, n)).collect(), targets)
    }

    /// A callee that is a plain function (not a class's) or a bound method: the function and
    /// its receiver.
    fn function_callee(&self, o: ObjId) -> Option<(ObjId, Option<Value>)> {
        match (self.heap.kind(o), &self.heap.obj(o).payload) {
            (Kind::Function, Payload::Func(_)) if self.heap.func_info(o).cls.is_none() => Some((o, None)),
            (Kind::Bound, Payload::Bound(f, r)) if self.heap.kind(*f) == Kind::Function => Some((*f, Some(r.clone()))),
            _ => None,
        }
    }

    /// Would a call of `f` not run, its function out of its budget (`call_user_inner`)?
    pub(super) fn out_of_budget(&self, f: ObjId) -> bool {
        let fi = self.heap.func_info(f);
        let runs = self.m.sdispatch.contains_key(&f) || fi.is_memo.get() || fi.node.is_generator();
        !runs && self.budget.exhausted(fi.node.addr()) && self.recursion.depth(f) == 0
    }

    /// Among a call's callees (functions, methods, receiver groups), those out of their budget,
    /// when several are: they make one call that does not run (`skipped_calls`; each its own
    /// walked all it is given again — `self.__init__(**params)` on any of a thousand
    /// instances). More than `PICK_MAX` of them: all of them (a registry's entry point, any of
    /// a thousand: timm's `create_model`). The rest stay.
    #[allow(clippy::type_complexity)]
    fn budget_skipped(&mut self, groups: Vec<(ObjId, Value, usize)>, targets: Vec<ObjId>) -> (Vec<(ObjId, Option<Value>)>, Vec<(ObjId, Value, usize)>, Vec<ObjId>) {
        let callee = |an: &Analyzer, o: ObjId| an.function_callee(o);
        let many = groups.len() + targets.iter().filter(|o| callee(self, **o).is_some()).count() > super::PICK_MAX;
        let skip = |an: &Analyzer, f: ObjId| many || an.out_of_budget(f);
        let n = groups.iter().filter(|g| skip(self, g.0)).count() + targets.iter().filter(|o| callee(self, **o).is_some_and(|(f, _)| skip(self, f))).count();
        if n < 2 {
            return (Vec::new(), groups, targets);
        }
        let mut fns: Vec<(ObjId, Option<Value>)> = Vec::new();
        let (out, groups): (Vec<_>, Vec<_>) = groups.into_iter().partition(|g| skip(self, g.0));
        fns.extend(out.into_iter().map(|(f, r, _)| (f, Some(r))));
        let mut rest = Vec::new();
        for o in targets {
            match callee(self, o).filter(|(f, _)| skip(self, *f)) {
                Some(c) => fns.push(c),
                None => rest.push(o),
            }
        }
        let out: Vec<ObjId> = fns.iter().map(|(f, _)| *f).filter(|f| self.out_of_budget(*f)).collect();
        for f in &out {
            self.budget.out.insert(*f);
            let q = self.heap.func_info(*f).qualname.clone();
            self.findings.note(format!("{} ran out of its budget: later calls are unknown code", q));
        }
        (fns, groups, rest)
    }

    /// One of several callees a value picks (`call_value`; `picked`: the call's text, when the
    /// value picks among functions).
    fn call_picked(&mut self, oid: ObjId, picked: &Option<String>, a: Args, st: &mut State, node: &Node, call_ast: Option<&'static ast::ExprCall>) -> Value {
        // (a groupby method of one of several groupby objects: its results are its own)
        let own = matches!(&self.heap.obj(oid).payload, Payload::Method(q, r) if q.starts_with("groupby.") && r.pts.iter().all(|g| self.heap.kind(g) == Kind::Groupby));
        let saved = if own { self.cx.alloc_extra.replace(format!("gbm{}", oid)) } else { self.cx.alloc_extra.clone() };
        if let Some(call) = picked {
            let name = match self.heap.kind(oid) {
                Kind::Function => self.heap.func_info(oid).qualname.clone(),
                Kind::Unknown => "code rowan cannot see".to_string(),
                _ => self.heap.obj(oid).label.to_string(),
            };
            self.cx.picked.push((call.clone(), name));
        }
        let r = self.call_one(oid, a, st, node, call_ast);
        if picked.is_some() {
            self.cx.picked.pop();
        }
        self.cx.alloc_extra = saved;
        r
    }

    /// Alternatives: `run(i)` for each of `n` callees, each on its own copy of the state, the
    /// states joined (`keep`: the state as it is is one more, where none runs); the results
    /// of those that return (a callee that raises on every path returns nothing: `A(x)` whose
    /// `__init__` always fails gives no A).
    pub(in crate::interp) fn alternatives(&mut self, n: usize, keep: bool, state: &mut State, run: &mut dyn FnMut(&mut Analyzer, usize, &mut State) -> Value) -> Value {
        if n == 1 && !keep {
            return run(self, 0, state);
        }
        let pre = state.copy();
        let mut outs: Vec<Value> = Vec::new();
        let mut states: Vec<State> = if keep { vec![pre.copy()] } else { Vec::new() };
        for i in 0..n {
            let mut st = pre.copy();
            let r = run(self, i, &mut st);
            if st.reachable {
                outs.push(r);
            }
            states.push(st);
        }
        if let Some(j) = State::join_all(states) {
            state.replace_with(j);
        }
        join_values(outs)
    }

    /// Several definitions of one method (conditional ones) on one receiver.
    fn call_alternatives(&mut self, fns: &[u32], recv: u32, a: Args, state: &mut State, node: &Node) -> Value {
        self.alternatives(fns.len(), false, state, &mut |an, i, st| an.call_user(fns[i], Some(Value::obj(recv)), a, st, node, false))
    }

    /// A call of one callee object: a torch layer or module (its `forward`), a function (a
    /// class's unbound method takes its receiver from the arguments; a classmethod the class), a
    /// bound method, a class, a library function or method, `functools.partial`, an instance
    /// (its `__call__`), or unknown code.
    pub fn call_one(&mut self, oid: u32, a: Args, state: &mut State, node: &Node, call_ast: Option<&'static ast::ExprCall>) -> Value {
        let Args { pos: args, kw: kwargs, star: star_args, star_kw: star_kwargs } = a;
        let kind = self.heap.kind(oid);
        // torch: a layer object / Sequential, or a user nn.Module (its forward)
        if matches!(kind, Kind::Namespace | Kind::List) && crate::models::torch::t_layer_type(self, oid).is_some() {
            let r = crate::models::torch::t_layer_call(self, oid, args, kwargs, node, state);
            return crate::models::torch::t_forward_hooks(self, oid, args, r, state, node);
        }
        if kind == Kind::Instance && self.m.torch_on && crate::models::torch::t_is_module_instance(self, oid) {
            if let Some(cls) = self.heap.instance_class(oid) {
                if self.class_lookup(cls, "__call__").is_none() {
                    if let Some(m) = self.class_lookup(cls, "forward") {
                        let f = self.heap.class_attr(m, "forward");
                        let fns: Vec<u32> = f.pts.iter().filter(|x| self.heap.kind(*x) == Kind::Function).collect();
                        let r = self.call_alternatives(&fns, oid, a, state, node);
                        return crate::models::torch::t_forward_hooks(self, oid, args, r, state, node);
                    }
                }
            }
        }
        match kind {
            Kind::Function if self.heap.func_info(oid).is_memo.get() => crate::models::singledispatch::memo_call(self, oid, None, a, state, node),
            Kind::Function => {
                let fi = self.heap.func_info(oid);
                if fi.cls.is_some() && !fi.is_static.get() && !args.is_empty() && !fi.is_classmethod.get() {
                    // unbound method called with explicit self: Base.method(self, ...)
                    return self.call_user(oid, Some(args[0].clone()), Args::new(&args[1..], &kwargs, &star_args, &star_kwargs), state, node, false);
                }
                if fi.is_classmethod.get() {
                    if let Some(cls) = fi.cls {
                        // `Cls.make(...)`: the class is the first argument
                        return self.call_user(oid, Some(Value::obj(cls)), a, state, node, false);
                    }
                }
                self.call_user(oid, None, a, state, node, false)
            }
            Kind::Bound => {
                let Payload::Bound(f, self_v) = self.heap.obj(oid).payload.clone() else { return Value::new() };
                // (a method's calls past its budget do not run, cached or not)
                if self.heap.func_info(f).is_memo.get() && !self.budget.exhausted(self.heap.func_info(f).node.addr()) {
                    return crate::models::singledispatch::memo_call(self, f, Some(self_v), a, state, node);
                }
                self.call_user(f, Some(self_v), a, state, node, false)
            }
            Kind::Class => self.construct(oid, a, state, node),
            Kind::External => {
                let qual = self.heap.external_qual(oid);
                if let Some(v) = crate::models::builtin_dunder(self, &qual, a, state, node) {
                    return v;
                }
                let r = self.call_external(&qual, a, state, node, call_ast);
                if call_ast.is_some() {
                    crate::models::may_type_error(self, &qual, args, kwargs, &r, state);
                }
                r
            }
            Kind::Method => self.call_method_object(oid, a, state, node, call_ast),
            Kind::Unknown => {
                let q = format!("call of {}", self.heap.obj(oid).label);
                self.unknown_call(&q, Some(&Value::obj(oid)), a, state, node)
            }
            Kind::Partial => {
                let call = Call { qual: "partial.__call__".into(), recv: Some(Value::obj(oid)), args: args.to_vec(), kwargs: kwargs.clone(), star_args: star_args.to_vec(), star_kwargs: star_kwargs.to_vec(), node: *node, call_ast, site: format!("L{}", node.line), line: node.line };
                crate::models::stdlib::partial_call(self, state, &call)
            }
            Kind::Instance => {
                if let Some(cls) = self.heap.instance_class(oid) {
                    if let Some(m) = self.class_lookup(cls, "__call__") {
                        let f = self.heap.class_attr(m, "__call__");
                        if f.pts.is_empty() {
                            return Value::new();
                        }
                        let fns: Vec<u32> = f.pts.iter().filter(|x| self.heap.kind(*x) == Kind::Function).collect();
                        return self.call_alternatives(&fns, oid, a, state, node);
                    }
                }
                let q = format!("call of {}", self.heap.obj(oid).label);
                self.unknown_call(&q, Some(&Value::obj(oid)), a, state, node)
            }
            _ => {
                let q = format!("call of {}", self.heap.obj(oid).label);
                self.unknown_call(&q, None, a, state, node)
            }
        }
    }

    /// A call of a library method value (`dict.get` bound to its receiver): its model (a str
    /// method on known text: exactly), else unknown code; over several groupby objects, once each.
    fn call_method_object(&mut self, oid: u32, a: Args, state: &mut State, node: &Node, call_ast: Option<&'static ast::ExprCall>) -> Value {
        let Args { pos: args, kw: kwargs, star: star_args, star_kw: star_kwargs } = a;
        let Payload::Method(qual, recv) = self.heap.obj(oid).payload.clone() else { return Value::new() };
        // several groupby objects (`groupby` over one of several columns): the model runs
        // once per object, each making its own results
        if qual.starts_with("groupby.") && recv.pts.len() > 1 && recv.pts.iter().all(|o| self.heap.kind(o) == Kind::Groupby) {
            let gs: Vec<ObjId> = recv.pts.to_vec();
            return self.alternatives(gs.len(), false, state, &mut |an, i, st| {
                let Some(mo) = an.method_value(&qual, &Value::obj(gs[i])).single() else { return Value::new() };
                let saved = an.cx.alloc_extra.replace(format!("gb{}", gs[i]));
                let r = an.call_one(mo, a, st, node, call_ast);
                an.cx.alloc_extra = saved;
                r
            });
        }
        let mut args = args.to_vec();
        if qual.starts_with("dict.") || qual.starts_with("set.") {
            if let Some(first) = args.first_mut() {
                *first = self.keyify(first);
            }
        }
        let call = Call { qual: qual.clone(), recv: Some(recv.clone()), args, kwargs: kwargs.clone(), star_args: star_args.to_vec(), star_kwargs: star_kwargs.to_vec(), node: *node, call_ast, site: format!("L{}", node.line), line: node.line };
        let pure = if qual.starts_with("str.") { self.in_model(|an| crate::models::pure::try_pure(an, state, &call)) } else { None };
        let r = match (pure, self.program.models.lookup(&qual)) {
            (Some(v), _) => v,
            (None, Some(m)) => self.run_model(m, state, &call),
            (None, None) => return self.unknown_call(&qual, Some(&recv), Args::new(&call.args, kwargs, star_args, star_kwargs), state, node),
        };
        // (a str method of a str: what it returns is known by the method)
        let r = match self.value_type(&recv) {
            Some(t @ ("str" | "bytes")) => self.as_type(r, crate::models::builtins::str_method_type(&qual, t), false),
            _ => r,
        };
        let r = self.pure_hint(&call, r, state);
        if call_ast.is_some() {
            crate::models::may_type_error(self, &qual, &call.args, kwargs, &r, state);
        }
        // (a method of an object rowan knows nothing of, modelled as a dict's: a list's `pop`
        // raises IndexError, a set's KeyError)
        if matches!(qual.as_str(), "dict.pop" | "dict.popitem") && recv.pts.iter().any(|o| self.heap.kind(o) == Kind::Unknown) {
            self.flag_may_raise(state);
        }
        r
    }

    /// A call of a library function or class (a model, or unknown code).
    pub fn call_external(&mut self, qual: &str, a: Args, state: &mut State, node: &Node, call_ast: Option<&'static ast::ExprCall>) -> Value {
        let Args { pos: args, kw: kwargs, star: star_args, star_kw: star_kwargs } = a;
        let qual = qual.to_string();
        let call = Call { qual: qual.clone(), recv: None, args: args.to_vec(), kwargs: kwargs.clone(), star_args: star_args.to_vec(), star_kwargs: star_kwargs.to_vec(), node: *node, call_ast, site: format!("L{}", node.line), line: node.line };
        if let Some(v) = self.in_model(|an| crate::models::pure::try_pure(an, state, &call)) {
            return self.as_type(v, crate::models::builtins::result_type(self, &qual, args), false);
        }
        // a builtin exception class: an exception object with its args
        if let Some(n) = qual.strip_prefix("builtins.").filter(|n| super::exprs::is_exception_name(n)) {
            if kwargs.is_empty() && star_args.is_empty() && !self.program.models.has_exact(&qual) {
                let n = n.to_string();
                return crate::models::exceptions::exception_object(self, &n, args.to_vec(), node, state);
            }
        }
        // `f.__get__(obj)` (a library function bound by hand: attrs's
        // `object.__setattr__.__get__(self)`): f with obj as its first argument
        if let Some(base) = qual.strip_suffix(".__get__").filter(|_| !args.is_empty() && kwargs.is_empty()) {
            let f = self.heap.alloc_raw(Kind::External, &format!("ext:{}", base), "", true, base, Payload::Str(base.to_string()));
            let pc = Call { qual: "functools.partial".into(), args: vec![Value::obj(f), args[0].clone()], ..call.clone() };
            if let Some(m) = self.program.models.lookup("functools.partial") {
                return self.run_model(m, state, &pc);
            }
        }
        if let Some(m) = self.program.models.lookup(&qual) {
            let r = self.run_model(m, state, &call);
            let r = self.as_type(r, crate::models::builtins::result_type(self, &qual, args), false);
            return self.pure_hint(&call, r, state);
        }
        let r = self.unknown_call(&qual, None, a, state, node);
        // (`input()`: a str all the same)
        if qual == "builtins.input" {
            for o in r.pts.iter() {
                self.heap.obj_mut(o).ty = Some("str");
            }
        }
        r
    }

    /// A call computed exactly on literals (`try_pure`) whose receiver or one argument is an
    /// unknown that may be some literals (`str(port)`, `port` `8080 | ?`): its unknown result
    /// (a scalar, or unknown objects) may be what the call gives on those (`"8080" | ?`). (Run
    /// on a copy of the state: the result the call gave on the unknown covers what it does on
    /// them.)
    fn pure_hint(&mut self, call: &Call, r: Value, state: &State) -> Value {
        let scalar = r.pts.is_empty() && matches!(r.kv, crate::domain::KeyVal::Data(..));
        if !scalar && !(r.kv.is_bot() && !r.pts.is_empty() && r.pts.iter().all(|o| self.heap.kind(o) == Kind::Unknown)) {
            return r;
        }
        let hinted: Vec<usize> = call.recv.iter().chain(call.args.iter()).enumerate().filter(|(_, v)| !self.literals_beside(v).is_empty()).map(|(i, _)| i).collect();
        let [i] = hinted[..] else { return r };
        fn at(c: &mut Call, i: usize) -> &mut Value {
            match (&mut c.recv, i) {
                (Some(r), 0) => r,
                (Some(_), _) => &mut c.args[i - 1],
                (None, _) => &mut c.args[i],
            }
        }
        let mut c = call.clone();
        // (a method of an unknown receiver that may be some texts: the str method on them)
        if let Some(m) = c.qual.strip_prefix("unknown.").filter(|m| super::STR_METHOD_NAMES.contains(m)) {
            c.qual = format!("str.{}", m);
        }
        let hint = self.literals_beside(at(&mut c, i));
        let mut out = r.kv.hint();
        for k in hint.iter() {
            *at(&mut c, i) = Value::lit(k);
            let mut st = state.copy();
            let Some(v) = self.in_model(|an| crate::models::pure::try_pure(an, &mut st, &c)) else { continue };
            if !v.pts.is_empty() || !v.kv.is_literal() {
                return r;
            }
            out = out.union(&kv_literals(&v.kv));
        }
        match &r.kv {
            _ if out.is_empty() || out.len() > crate::domain::HINT_MAX => r,
            crate::domain::KeyVal::Data(l, _) => Value::kv_only(crate::domain::KeyVal::Data(*l, out)),
            _ => Value::with_kv(r.pts.clone(), crate::domain::KeyVal::lits(out)),
        }
    }

    /// A model call: objects it allocates at one site stay distinct from each other (one
    /// weak object) even while only the model's own code holds them.
    pub(in crate::interp) fn run_model(&mut self, m: crate::models::Model, state: &mut State, call: &Call) -> Value {
        // (an argument or receiver of literals beside objects: each part on its own)
        let recv = call.recv.clone().unwrap_or_default();
        let mut vs: Vec<&Value> = vec![&recv];
        vs.extend(call.args.iter());
        if let Some(r) = self.split_mixed(&vs, state, &mut |an, vs, st| {
            let mut c = call.clone();
            if c.recv.is_some() {
                c.recv = Some(vs[0].clone());
            }
            c.args = vs[1..].to_vec();
            an.run_model(m, st, &c)
        }) {
            return r;
        }
        // (an argument of literals beside an unknown object, which may be anything: for the model
        // a value not known — a model reading its literals would leave the unknown's cases out)
        let beside_unknown = |v: &Value| !v.pts.is_empty() && v.kv.is_literal() && kv_literals(&v.kv).iter().any(|k| k != intern::key_none()) && v.pts.iter().any(|o| self.heap.kind(o) == Kind::Unknown);
        let opaque;
        let call = if call.args.iter().any(beside_unknown) {
            let mut c = call.clone();
            for a in c.args.iter_mut().filter(|a| beside_unknown(a)) {
                *a = Value::with_kv(a.pts.clone(), DATA);
            }
            opaque = c;
            &opaque
        } else {
            call
        };
        let sources = intern::sources_len();
        let r = self.in_model(|an| m(an, state, call));
        self.name_sources(sources, call);
        crate::models::itertools::after_model_consume(self, call, &r, state);
        crate::models::builtins::live_after_model(self, m, call, &r, state);
        r
    }

    /// For people: the inputs a model opened at a call as written (sources from id `from` on,
    /// `read_csv@L5`) are named by the call and its file (`pd.read_csv(path)` (line 5 in io.py)).
    fn name_sources(&mut self, from: u32, call: &Call) {
        if call.call_ast.is_none() {
            return;
        }
        let at = format!("@L{}", call.line);
        for id in from..intern::sources_len() {
            let site = intern::with_source(intern::SourceId(id), |s| s.path.is_empty().then(|| s.site.clone()));
            if let Some(site) = site.filter(|s| s.ends_with(&at)) {
                let text = crate::frontend::text_at(&call.node).split_whitespace().collect::<Vec<_>>().join(" ");
                self.names.site_names.entry(intern::s(&site)).or_insert((text, call.node.module, call.line));
            }
        }
    }

    /// Runs a model's code: the objects it makes are held by it until it returns (an
    /// allocation at its node again is another object, not the first one renewed).
    fn in_model<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        if self.cx.model_depth == 0 {
            self.cx.model_epoch += 1;
        }
        self.cx.model_depth += 1;
        let r = f(self);
        self.cx.model_depth -= 1;
        r
    }

    /// A call of a class of the program: an Enum lookup, a TypedDict, a `__new__` of its own
    /// (then `__init__` on what it returns), a library base's construction, or a fresh instance
    /// and its `__init__` (an exception's args when a builtin exception class is its base).
    pub fn construct(&mut self, cls: u32, a: Args, state: &mut State, node: &Node) -> Value {
        let Args { pos: args, kw: kwargs, star: star_args, star_kw: star_kwargs } = a;
        if crate::models::dataclasses::abstract_unimplemented(self, cls) {
            self.raise_here("TypeError", state);
            return Value::new();
        }
        // `Color(3)`: the Enum member with that value, else ValueError
        if let (Some(ms), Some(a)) = (self.m.enum_members.get(&cls).cloned(), args.first()) {
            return crate::models::enums::enum_lookup(self, cls, &ms, a, node, state);
        }
        // a TypedDict class makes a plain dict
        if let Some(v) = crate::models::dataclasses::typed_dict_construct(self, cls, a, state, node) {
            return v;
        }
        // a class with its own `__new__`: the instance is what it returns, and `__init__` runs
        // on it when it is an instance of the class
        if let Some(owner) = self.class_lookup(cls, "__new__") {
            let fv = self.heap.class_attr(owner, "__new__");
            let fns: Vec<ObjId> = fv.pts.iter().filter(|f| self.heap.kind(*f) == Kind::Function).collect();
            if !fns.is_empty() {
                let mut nargs = vec![Value::obj(cls)];
                nargs.extend(args.iter().cloned());
                let mut out = Value::new();
                for f in fns {
                    let r = self.call_user(f, None, Args::new(&nargs, &kwargs, &star_args, &star_kwargs), state, node, false);
                    out = out.join(&r);
                }
                if let Some(init) = self.class_lookup(cls, "__init__") {
                    let iv = self.heap.class_attr(init, "__init__");
                    let insts: Vec<ObjId> = out.pts.iter().filter(|o| self.heap.kind(*o) == Kind::Instance && self.heap.instance_class(*o).is_some_and(|c| self.heap.mro(c).contains(&cls))).collect();
                    for inst in insts {
                        for f in iv.pts.iter().filter(|f| self.heap.kind(*f) == Kind::Function).collect::<Vec<_>>() {
                            self.call_user(f, Some(Value::obj(inst)), a, state, node, false);
                        }
                    }
                }
                return out;
            }
        }
        let inst = self.alloc_instance(cls, node, state);
        // library classes: TypedDict, pydantic models, dict subclasses, dataclasses
        if let Some(v) = crate::models::dataclasses::construct_library(self, cls, inst, a, state, node) {
            return v;
        }
        if let Some(init) = self.class_lookup(cls, "__init__") {
            let fv = self.heap.class_attr(init, "__init__");
            self.cx.fresh_init.insert(inst, super::context::Building { mark: self.heap.serial(), at: state.copy(), roots: Vec::new(), holders: Vec::new(), looks: 0, held: false });
            for f in fv.pts.iter() {
                if self.heap.kind(f) == Kind::Function {
                    self.call_user(f, Some(Value::obj(inst)), a, state, node, false);
                }
            }
            self.cx.fresh_init.remove(&inst);
        } else if self.builtin_exc_base(cls) {
            crate::models::dataclasses::exception_init(self, cls, inst, args, node, state);
        }
        Value::obj(inst)
    }

    /// A class whose call only makes an instance and runs its `__init__` on it (no `__new__`,
    /// no generated or library construction, not abstract): Some(its `__init__`, if the
    /// program defines one); None otherwise.
    fn plain_init(&self, cls: ObjId) -> Option<Option<ObjId>> {
        let ci = self.heap.class_info_opt(cls)?;
        let fields = self.heap.mro(cls).iter().any(|c| self.heap.class_info_opt(*c).is_some_and(|x| x.has_fields() || x.lib.dict_base || x.lib.typed_dict));
        if fields || ci.lib.typed_dict || self.m.enum_members.contains_key(&cls) || self.class_lookup(cls, "__new__").is_some() || crate::models::pydantic::is_validated(self, cls) || self.builtin_exc_base(cls) || crate::models::dataclasses::abstract_unimplemented(self, cls) {
            return None;
        }
        let Some(owner) = self.class_lookup(cls, "__init__") else { return Some(None) };
        let fv = self.heap.cls_slots.get(&(owner, Ns::Attr, key_str_lit("__init__")))?;
        match fv.single() {
            Some(f) if self.heap.kind(f) == Kind::Function && fv.kv.is_bot() => Some(Some(f)),
            _ => None,
        }
    }

    /// A fresh instance of user class `cls` allocated at `node`, without attributes (an
    /// external base's may come from outside the program).
    pub fn alloc_instance(&mut self, cls: ObjId, node: &Node, state: &mut State) -> ObjId {
        let ci = self.heap.class_info(cls);
        let row = if ci.open_base.get() { Row::with_source(Source::new(&format!("instance {}@L{}", ci.name, node.line), SourceKind::Class)) } else { Row::new() };
        // the allocation site includes the class: `cls()` with several possible classes must
        // not collapse onto one instance; by the class object, as two classes may share a name
        // (`type(name, bases, ns)` per choice of bases)
        let site = format!("{}:{}", super::site_string(node), cls);
        let inst = self.alloc(Kind::Instance, &site, state, &format!("{}@L{}", ci.name, node.line), Payload::Cls(cls), Some(row), Ns::Attr, None, None);
        if ci.lib.typed_dict {
            state.set_row(inst, Ns::Item, Row::new());
        } else if ci.open_base.get() && !ci.lib.dict_base {
            // (an external base — `list`, a library class — may hold items the program's code
            // does not show: `append` / `super().__init__(items)` run there)
            state.set_row(inst, Ns::Item, Row::with_source(Source::new(&format!("items of {}@L{}", ci.name, node.line), SourceKind::Class)));
        }
        inst
    }

    /// The allocation context of a call: per receiver (a single one) and call site, so two
    /// calls of one method on one receiver allocate distinct objects. A summary receiver's
    /// call is one call on one of the objects it stands for: its allocations are strong as any
    /// call's (made again while in use, a variant).
    pub(super) fn receiver_ctx(&self, self_v: &Option<Value>, site: &str) -> String {
        let recv = self_v.as_ref().and_then(|sv| sv.single()).filter(|r| self.heap.kind(*r) == Kind::Instance).map(|r| self.heap.obj(r));
        match recv {
            Some(recv) if recv.single && self.heap.clones(&recv.site) >= MAX_CLONES => String::new(),
            Some(recv) if recv.single => format!("o{}/{}", recv.id, site),
            _ => site.to_string(),
        }
    }

    /// The context of a call's allocations: its site (line and column), chained to the
    /// calling frame's context (bounded depth; beyond it a hash of the prefix).
    pub(super) fn callee_site(&self, node: &Node) -> String {
        let site = if node.addr != 0 { format!("{}L{}:{}", crate::frontend::site_prefix(node.module), node.line, node.col) } else { format!("{}L{}", crate::frontend::site_prefix(node.module), node.line) };
        // chained to the calling frame's context, so two activations of the caller give
        // distinct objects
        {
            let fr = self.frame();
            let mut site = site;
            for i in &fr.unroll {
                site.push_str(&format!("#{}", i));
            }
            if fr.kind != FrameKind::Module && !fr.ctx.is_empty() && !fr.ctx.starts_with("entry") {
                if fr.ctx.matches('/').count() < super::CHAIN {
                    format!("{}/{}", fr.ctx, site)
                } else {
                    // deeper chains stay distinct through a hash of the prefix: two activations
                    // of the caller keep allocating distinct objects at any depth, while the
                    // context string stays short
                    let mut h = rustc_hash::FxHasher::default();
                    std::hash::Hash::hash(&fr.ctx, &mut h);
                    format!("h{:x}/{}", std::hash::Hasher::finish(&h), site)
                }
            } else {
                site
            }
        }
    }

    /// A call of the program's function `foid` (`self_v`: a method's receiver; `entry`: its
    /// entry run). Unknown code instead past the work a run on demand may do, past the deadline,
    /// or past its budget (`skipped_call`); a generator function's call makes a lazy generator
    /// (its body runs when consumed); else its body runs here (`call_user_inner`).
    pub fn call_user(&mut self, foid: u32, self_v: Option<Value>, a: Args, state: &mut State, node: &Node, entry: bool) -> Value {
        // (an entry past the library's share: it stops, to be undone)
        if self.budget.lib_abort.get() {
            state.reachable = false;
            return Value::new();
        }
        let Args { pos: args, kw: kwargs, star: star_args, star_kw: star_kwargs } = a;
        // (its run for any call starts from the heap here too; a followed library's code has none)
        let library = self.heap.module_info(self.heap.func_info(foid).module).library;
        if !library {
            self.note_call(foid, self_v.as_ref(), state);
        }
        // a followed library's code: from the program's code, one entry into it (`library_entry`)
        if library && self.budget.lib_entry.is_none() {
            let q = self.heap.func_info(foid).qualname.clone();
            let r = match self.library_entry(|| format!("call {} at {}", q, super::site_string(node)), state, |an, st| an.call_user(foid, self_v.clone(), a, st, node, entry)) {
                Some(r) => r,
                // (past the library's share: a call that does not run)
                None => {
                    self.gens.force_at = None;
                    self.skipped_call(foid, &q, self_v.as_ref(), a, state, node)
                }
            };
            return self.named_by_call(r, node);
        }
        // (an inert body runs a few steps: it runs wherever it is called, `inert`)
        let inert = self.inert(foid).is_some();
        // (within an entry past its allowance, calls of code do not run)
        if !inert && self.budget.past_library_work(self.library_work()) {
            self.gens.force_at = None;
            let q = self.heap.func_info(foid).qualname.clone();
            return self.skipped_call(foid, &q, self_v.as_ref(), a, state, node);
        }
        // (on demand, past the work a function's run may do, its callees' calls are unknown)
        if !inert && self.budget.past_hover_work(self.cx.frames.len(), self.hover_work()) {
            self.gens.force_at = None;
            let q = self.heap.func_info(foid).qualname.clone();
            return self.skipped_call(foid, &q, self_v.as_ref(), a, state, node);
        }
        // past the hard deadline a user function is unknown code (its result unknown, its
        // arguments possibly changed, it may raise): the run ends in bounded time
        if self.budget.over_hard_deadline() {
            self.gens.force_at = None;
            let q = self.heap.func_info(foid).qualname.clone();
            self.findings.note(format!("analysis cut at the deadline ({} ms): later calls are unknown code", super::limit("ms", super::DEADLINE_MS)));
            return self.skipped_call(foid, &q, self_v.as_ref(), a, state, node);
        }
        // a generator's body runs when it is consumed (`force_gen`), not here
        let lazy = !entry
            && self.gens.force_at.is_none()
            && self.cx.model_depth == 0
            && star_args.is_empty()
            && star_kwargs.is_empty()
            && !self.gens.cm_hook.as_ref().is_some_and(|h| !h.ran && h.depth == self.cx.frames.len())
            && self.heap.func_info(foid).node.is_generator();
        if lazy {
            return self.make_lazy_gen(foid, self_v, args.to_vec(), kwargs.clone(), state, node);
        }
        // (user code called back by a model allocates as user code)
        let saved_md = std::mem::replace(&mut self.cx.model_depth, 0);
        // (the receiver, for the unknown call that replaces this one when the hard deadline cuts it)
        let was_cut = self.budget.hard_cut;
        let recv = self_v.clone();
        // (facts of keys stored through names are the caller's names': a callee's names are its
        // own; what it deletes or gives unknown code voids the caller's)
        let names = std::mem::take(&mut state.name_keys);
        let drops = NAME_KEY_DROPS.with(|c| c.get());
        let tally = super::debug_enabled().then(|| {
            let fi = self.heap.func_info(foid);
            let name = || format!("{} ({})", fi.qualname, self.heap.module_info(fi.module).name);
            (fi.node.addr(), self.budget.tally_begin(fi.node.addr(), name))
        });
        let r = self.call_user_inner(foid, self_v, a, state, node, entry);
        if let Some((code, t)) = tally {
            self.budget.tally_end(code, t);
        }
        state.name_keys = if NAME_KEY_DROPS.with(|c| c.get()) == drops { names } else { Default::default() };
        self.cx.model_depth = saved_md;
        if self.budget.hard_cut && !was_cut {
            let q = self.heap.func_info(foid).qualname.clone();
            self.findings.note(format!("analysis cut at the deadline ({} ms) inside {}: the call is unknown code", super::limit("ms", super::DEADLINE_MS), q));
            state.reachable = true;
            return self.skipped_call(foid, &q, recv.as_ref(), a, state, node);
        }
        r
    }

    /// The body run of a call: singledispatch picks its implementation; a call of a running
    /// activation takes its partial result (`recursive_call`); one past its budget is unknown
    /// code. Else the callee's frame is pushed (its allocation context: receiver and call site),
    /// the arguments bound, the body run to its recursion fixpoint (`run_rounds`), and its result
    /// (a generator's object for a generator) recorded as a summary.
    fn call_user_inner(&mut self, foid: u32, self_v: Option<Value>, a: Args, state: &mut State, node: &Node, entry: bool) -> Value {
        let Args { pos: args, kw: kwargs, star: star_args, star_kw: star_kwargs } = a;
        let forced = self.gens.force_at.take();
        // a singledispatch function: the implementation for the first argument's type
        if self.m.sdispatch.contains_key(&foid) && self.m.sd_bypass.take() != Some(foid) && !args.is_empty() && star_args.is_empty() {
            return crate::models::singledispatch::sdispatch_call(self, foid, self_v, args.to_vec(), kwargs.clone(), star_kwargs.to_vec(), state, node, entry);
        }
        if !state.narrowed.is_empty() {
            state.narrowed.clear();
        }
        let fi = self.heap.func_info(foid);
        let fnode = fi.node;
        let site = match forced.as_ref() {
            Some((site, _)) => site.clone(),
            None => self.callee_site(node),
        };
        self.entry.analysed.insert(foid);
        let key = self.activation_key(foid, &site, entry, self_v.as_ref(), a, state);
        if self.recursion.is_active(&key) {
            return self.recursive_call(&key, &self_v, &args, &kwargs, state);
        }
        // every statement a function code runs counts against its budget (entry runs aside: one
        // per function); past it a call is unknown code, so the whole analysis is bounded by the
        // number of functions times the budget
        if !entry && self.budget.exhausted(fi.node.addr()) {
            let q = fi.qualname.clone();
            self.findings.note(format!("{} ran out of its budget: later calls are unknown code", q));
            self.budget.out.insert(foid);
            return self.skipped_call(foid, &q, self_v.as_ref(), a, state, node);
        }
        if super::debug_enabled() {
            eprintln!("run {} at {} frames={} chain={:?}", fi.qualname, site, self.cx.frames.len(), self.cx.call_chain);
        }
        // context for allocations inside the callee
        let ctx = match forced {
            Some((_, ctx)) => ctx,
            None => self.receiver_ctx(&self_v, &site),
        };
        // (a generator's object: the one its run returns, keyed as `yield_value` finds it)
        let gen_id = fnode.is_generator().then(|| self.heap.alloc_raw(Kind::List, &format!("gen:{}", foid), &ctx, true, &format!("generator {}", fi.qualname), Payload::None));
        let new_fr = self.callee_frame(foid, &fi, &ctx);
        let caller = self.take_caller(&fi, state);
        self.cx.frames.push(new_fr);
        let saved_yields = gen_id.map(|ygid| (ygid, self.gen_activation_begin(ygid, foid, &fi, &ctx, state)));
        let saved_chain = std::mem::take(&mut self.cx.call_chain);
        let saved_files = std::mem::take(&mut self.cx.call_files);
        // (a generator expression's run is no call site of its own: its calls are those of the
        // code it is written in)
        (self.cx.call_chain, self.cx.call_files) = match fnode {
            FuncNode::GenExp(_) => (saved_chain.clone(), saved_files.clone()),
            _ => chain_extend(&saved_chain, &saved_files, node.module, node.line),
        };
        let pre_bind = state.copy();
        // (objects made from here on, by birth: a run again of code makes them anew)
        let born_mark = self.heap.serial();
        let pre_rc = std::rc::Rc::new(pre_bind.copy());
        self.recursion.begin(&key, (self_v.clone(), args.to_vec(), kwargs.clone()), pre_rc.clone(), born_mark);
        if gen_id.is_none() {
            if let Some(fr) = self.cx.frames.last_mut() {
                fr.pre_call = Some(super::PreCall(self.heap.mark(), pre_rc));
            }
        }
        let gen_cm = self.gens.cm_hook.as_ref().is_some_and(|h| !h.ran && h.depth + 1 == self.cx.frames.len());
        self.bind_params(&fi, &self_v, a, state, node);
        let (mut ret, mut st) = self.run_rounds(&key, &fi, gen_id, &pre_bind, &star_args, &star_kwargs, state, node);
        if let Some(gid) = gen_id {
            self.gen_activation_end(gid, gen_cm, &mut st, &pre_bind, born_mark);
        }
        if let Some((gid, prev)) = saved_yields {
            match prev {
                Some(v) => {
                    self.gens.yield_states.insert(gid, v);
                }
                None => {
                    self.gens.yield_states.remove(&gid);
                }
            }
        }
        *state = st.copy();
        if let Some(gid) = gen_id {
            // what `yield from` the generator evaluates to: its return value
            state.set_slot(gid, Ns::Attr, key_str_lit("\u{1}genret"), ret.clone(), true);
            // (one-shot: what a consumer takes is gone for the next one)
            self.start_iterator(gid, state);
            ret = Value::obj(gid);
        }
        let rows: Vec<(u32, Row)> = ret.pts.iter().filter(|o| self.heap.is_record(*o)).map(|o| (o, state.row(o, Ns::Item))).collect();
        self.findings.summaries.insert((fi.qualname.clone(), if entry { "entry".into() } else { ctx.clone() }), (ret.clone(), rows));
        // cleanup
        self.recursion.end(&key);
        self.cx.call_chain = saved_chain;
        self.cx.call_files = saved_files;
        self.cx.frames.pop();
        if self.cx.frames.len() <= 1 && self.cx.recent_stack.is_empty() {
            self.heap.forget_repaid();
        }
        self.give_back(caller, state);
        ret
    }

    /// What a callee's run takes out of the caller's state: its names, its possibly-unbound
    /// names, its returns so far (the callee's end must not join them in), and from its path
    /// the tests of the callee's own parameters (another call's).
    fn take_caller(&mut self, fi: &FuncInfo, state: &mut State) -> Caller {
        let env = std::mem::take(&mut state.env);
        self.cx.env_stack.push(env.clone());
        self.cx.unbound_stack.push(state.unbound.clone());
        let path = state.path.clone();
        let own = s(&format!("param {}.", fi.py_qualname()));
        if state.path.iter().any(|(o, _)| *o == own) {
            state.path = std::sync::Arc::new(state.path.iter().filter(|(o, _)| *o != own).copied().collect());
        }
        Caller { env, unbound: std::mem::take(&mut state.unbound), path, ret: state.ret.take(), exits: state.exits.take() }
    }

    /// The caller's part of the state back after the callee's run (`take_caller`); the callee's
    /// returns go, its exits replace the caller's when the caller had some.
    fn give_back(&mut self, c: Caller, state: &mut State) {
        self.cx.env_stack.pop();
        self.cx.unbound_stack.pop();
        state.env = c.env;
        state.unbound = c.unbound;
        state.path = c.path;
        state.ret = c.ret;
        if c.exits.is_some() {
            state.exits = c.exits;
        }
    }

    /// The frame a call of `fi` runs in: context `ctx`, its own cells and its closure's.
    fn callee_frame(&mut self, foid: ObjId, fi: &FuncInfo, ctx: &str) -> Frame {
        let fr_single = self.frame().single && self.frame().loop_depth == 0;
        let fr_single_nl = self.frame().single_nl;
        let mut new_fr = Frame::new(FrameKind::Function, fi.scope, fi.module, ctx, fr_single, &fi.qualname);
        new_fr.single_nl = fr_single_nl;
        new_fr.func = Some(foid);
        new_fr.code = fi.node.addr();
        {
            let mi = self.heap.module_info(fi.module);
            // (a generator expression's own variables are its comprehension's: `comp_cells`)
            let own = !fi.node.is_genexp();
            let mut cells: Vec<String> = mi.scopes.get(fi.scope).cells.iter().filter(|_| own).cloned().collect();
            cells.sort();
            for name in cells {
                let cid = self.heap.alloc_raw(Kind::Cell, &format!("cell:{}.{}", fi.qualname, name), ctx, fr_single, &format!("cell {}", name), Payload::None);
                new_fr.cells.insert(s(&name), cid);
            }
        }
        for (n, c) in fi.closure.iter() {
            new_fr.cells.insert(*n, *c);
        }
        new_fr
    }

    /// `f` runs in a frame of method `foid` with `self_v` and `args` bound, in the caller's
    /// allocation context (narrowing through a `__contains__` that is `return x in y`).
    pub(in crate::interp) fn in_method_frame(&mut self, foid: u32, self_v: Value, args: Vec<Value>, state: &mut State, f: &mut dyn FnMut(&mut Analyzer, &mut State)) {
        let fi = self.heap.func_info(foid);
        let ctx = self.frame().ctx.clone();
        let mut new_fr = Frame::new(FrameKind::Function, fi.scope, fi.module, &ctx, false, &fi.qualname);
        new_fr.func = Some(foid);
        for (n, c) in fi.closure.iter() {
            new_fr.cells.insert(*n, *c);
        }
        let saved_env = std::mem::take(&mut state.env);
        let saved_unbound = std::mem::take(&mut state.unbound);
        self.cx.env_stack.push(saved_env.clone());
        self.cx.unbound_stack.push(saved_unbound.clone());
        self.cx.frames.push(new_fr);
        let fake = Node::synthetic(fi.node.node().module, fi.node.node().line);
        self.bind_params(&fi, &Some(self_v), Args::pos(&args), state, &fake);
        f(self, state);
        self.cx.frames.pop();
        self.cx.env_stack.pop();
        self.cx.unbound_stack.pop();
        state.env = saved_env;
        state.unbound = saved_unbound;
    }

    /// Binds a call's arguments to the parameters of `fi` in the running frame (and records the
    /// parameters' hover facts at the `def`).
    pub(in crate::interp) fn bind_params(&mut self, fi: &FuncInfo, self_v: &Option<Value>, a: Args, state: &mut State, node: &Node) {
        let bound = self.compute_bindings(fi, self_v, a, state, node);
        self.record_param_facts(fi, &bound, state);
        for (name, v) in bound {
            self.store_name(name, v, state);
        }
    }

    /// Parameter name -> argument value (allocating the *args tuple / **kwargs dict in `state`).
    pub(in crate::interp) fn compute_bindings(&mut self, fi: &FuncInfo, self_v: &Option<Value>, a: Args, state: &mut State, node: &Node) -> IndexMap<StrId, Value> {
        let Args { pos: args, kw: kwargs, star: star_args, star_kw: star_kwargs } = a;
        let mut bound: IndexMap<StrId, Value> = IndexMap::new();
        let Some(a) = fi.node.params() else {
            // (a generator expression's iterator)
            if let (FuncNode::GenExp(_), Some(it)) = (fi.node, args.first()) {
                bound.insert(s(".0"), it.clone());
            }
            return bound;
        };
        let positional: Vec<StrId> = a.posonlyargs.iter().chain(a.args.iter()).map(|p| s(p.parameter.name.id.as_str())).collect();
        let kwonly: Vec<StrId> = a.kwonlyargs.iter().map(|p| s(p.parameter.name.id.as_str())).collect();
        let mut args: Vec<Value> = args.to_vec();
        if let Some(sv) = self_v {
            if !positional.is_empty() {
                args.insert(0, sv.clone());
            }
        }
        // `f(*xs)` with known items: they are positional arguments
        let mut star_args: Vec<Value> = star_args.to_vec();
        let mut expanded: Vec<Value> = Vec::new();
        star_args.retain(|sa| {
            let it = self.iterate(sa, node, state);
            match it.items {
                Some(items) if sa.pts.len() == 1 => {
                    expanded.extend(items);
                    false
                }
                _ => true,
            }
        });
        args.extend(expanded);
        let star_args = &star_args[..];
        // the elements of `*xs` (an unknown iterable's are unknown, not none)
        let extra = if !star_args.is_empty() { join_values(star_args.iter().map(|sa| self.iterate(sa, node, state).elem.flat()).collect::<Vec<_>>()) } else { Value::new() };
        for (i, name) in positional.iter().enumerate() {
            if i < args.len() {
                bound.insert(*name, args[i].clone());
            } else if !star_args.is_empty() {
                bound.insert(*name, extra.clone());
            }
        }
        let rest_pos: Vec<Value> = if args.len() > positional.len() { args[positional.len()..].to_vec() } else { Vec::new() };
        for name in positional.iter().chain(kwonly.iter()) {
            if let Some(v) = kwargs.get(name) {
                bound.insert(*name, v.clone());
            }
        }
        let used_kw: Vec<StrId> = bound.keys().copied().filter(|k| kwargs.contains_key(k)).collect();
        let live = self.live_defaults(fi, &positional, &kwonly, state);
        let defaults = live.as_ref().unwrap_or(&fi.defaults);
        for name in positional.iter().chain(kwonly.iter()) {
            if bound.contains_key(name) {
                continue;
            }
            let v = self.unbound_param_value(defaults, *name, star_kwargs, node, state);
            bound.insert(*name, v);
        }
        if let Some(va) = &a.vararg {
            let tid = self.alloc(Kind::Tuple, &format!("varargs:{}@{}", fi.qualname, self.site_of(node)), state, &format!("*{}", va.name.id), Payload::None, None, Ns::Item, None, None);
            let mut acc = if !rest_pos.is_empty() { join_values(rest_pos.iter().cloned()) } else { Value::new() };
            if !star_args.is_empty() {
                acc = acc.join(&extra);
            }
            state.set_slot(tid, Ns::Elem, intern::STAR, Value::with_kv(acc.pts.clone(), DATA), true);
            state.elems.insert(tid, if !rest_pos.is_empty() { acc.kv.clone() } else { DATA });
            self.heap.set_llen(tid, state, if star_args.is_empty() { Some(rest_pos.len()) } else { None });
            for (i, v) in rest_pos.iter().enumerate() {
                state.set_slot(tid, Ns::Pos, intern::key_int(i as i64), v.clone(), true);
            }
            bound.insert(s(va.name.id.as_str()), Value::obj(tid));
        }
        if let Some(kw) = &a.kwarg {
            let did = self.alloc(Kind::Dict, &format!("kwargs:{}@{}", fi.qualname, self.site_of(node)), state, &format!("**{}", kw.name.id), Payload::None, None, Ns::Item, None, None);
            let mut row = Row::new();
            for (name, v) in kwargs.iter() {
                if used_kw.contains(name) {
                    continue;
                }
                let nk = intern::key_of_str(*name);
                row = row.store(nk, true);
                state.set_slot(did, Ns::Item, nk, v.clone(), true);
            }
            state.set_row(did, Ns::Item, row);
            let kill: KeySet = KeySet::from_iter(positional.iter().chain(kwonly.iter()).map(|n| intern::key_of_str(*n)));
            for sk in star_kwargs {
                self.copy_into(did, Ns::Item, sk, state, true, &kill);
            }
            bound.insert(s(kw.name.id.as_str()), Value::obj(did));
        }
        bound
    }

    /// The defaults a call of `fi` (the running frame's function) takes: its own, unless its
    /// `__defaults__` / `__kwdefaults__` were stored or read (`function_defaults_attr`): then
    /// what they hold now (their own as well where the attribute may not be set; an unknown's
    /// or an open dict's: unknown too; a parameter none of them gives a default: none).
    fn live_defaults(&self, fi: &FuncInfo, positional: &[StrId], kwonly: &[StrId], state: &State) -> Option<IndexMap<StrId, Value>> {
        let foid = self.frame().func.filter(|f| self.heap.func_info_opt(*f).is_some_and(|g| std::ptr::eq(&*g, fi)))?;
        let mut out: Option<IndexMap<StrId, Value>> = None;
        for (attr, names) in [("__defaults__", positional), ("__kwdefaults__", kwonly)] {
            let k = intern::key_str_lit(attr);
            let Some(v) = state.slot_opt(foid, Ns::Attr, k) else { continue };
            let mut new: IndexMap<StrId, Value> = IndexMap::new();
            let mut add = |n: StrId, x: Value| {
                let e = new.entry(n).or_default();
                *e = e.join(&x);
            };
            let any = |add: &mut dyn FnMut(StrId, Value)| names.iter().for_each(|n| add(*n, Value::data(attr)));
            // (a tuple's items are the last parameters' defaults)
            let aligned = |items: Vec<Value>, add: &mut dyn FnMut(StrId, Value)| match names.len().checked_sub(items.len()) {
                Some(off) => items.into_iter().enumerate().for_each(|(i, x)| add(names[off + i], x)),
                None => names.iter().for_each(|n| add(*n, Value::data(attr))),
            };
            if !v.kv.is_literal() && !v.kv.is_bot() {
                any(&mut add);
            }
            for lit in crate::domain::kv_literals(&v.kv).iter() {
                match intern::key_get(lit) {
                    intern::Key::None => {}
                    intern::Key::Tuple(xs) if attr == "__defaults__" => aligned(xs.into_iter().map(Value::lit).collect(), &mut add),
                    _ => any(&mut add),
                }
            }
            for o in v.pts.iter() {
                match (self.heap.kind(o), attr) {
                    (Kind::Tuple, "__defaults__") => match self.heap.llen(o, state) {
                        Some(n) => aligned((0..n).map(|i| state.slot(o, Ns::Pos, intern::key_int(i as i64))).collect(), &mut add),
                        None => any(&mut add),
                    },
                    (Kind::Dict, "__kwdefaults__") => {
                        let row = state.row(o, Ns::Item);
                        for n in names {
                            let nk = intern::key_of_str(*n);
                            if row.open() {
                                add(*n, state.slot(o, Ns::Item, nk).join(&Value::data(attr)));
                            } else if row.present.contains(nk) {
                                add(*n, state.slot(o, Ns::Item, nk));
                            }
                        }
                    }
                    _ => any(&mut add),
                }
            }
            let d = out.get_or_insert_with(|| fi.defaults.clone());
            // (where it may not be set, its own defaults too)
            let keep = state.row(foid, Ns::Attr).definite.contains(k);
            for n in names {
                let own = d.shift_remove(n).filter(|_| !keep);
                match (own, new.get(n)) {
                    (Some(a), Some(b)) => d.insert(*n, a.join(b)),
                    (Some(a), None) => d.insert(*n, a),
                    (None, Some(b)) => d.insert(*n, b.clone()),
                    (None, None) => None,
                };
            }
        }
        out
    }

    /// The value of a parameter no argument binds: `**kwargs` holding the name wins over the
    /// default (joined with it when the key is only possibly there); a required one only
    /// `**kw` supplies is read from it (a requirement on an input).
    fn unbound_param_value(&mut self, defaults: &IndexMap<StrId, Value>, name: StrId, star_kwargs: &[Value], node: &Node, state: &mut State) -> Value {
        let nk = intern::key_of_str(name);
        let kw_present = star_kwargs.iter().any(|sk| sk.pts.iter().any(|x| state.row(x, Ns::Item).present.contains(nk)));
        let kw_definite = !star_kwargs.is_empty() && star_kwargs.iter().all(|sk| sk.pts.iter().all(|x| state.row(x, Ns::Item).definite.contains(nk)));
        if let (Some(d), false) = (defaults.get(&name), kw_present) {
            d.clone()
        } else if !star_kwargs.is_empty() {
            let mut vals: Vec<Value> = star_kwargs.iter().flat_map(|sk| sk.pts.iter().map(|x| state.slot(x, Ns::Item, nk)).collect::<Vec<_>>()).collect();
            if defaults.get(&name).is_none() && !kw_definite {
                // a required parameter only `**kw` can supply: read from it (a requirement on
                // an input; a missing key is a TypeError at the call)
                for sk in star_kwargs {
                    let recs: Vec<u32> = self.record_objects(sk).into_iter().filter(|o| state.row(*o, Ns::Item).open()).collect();
                    if !recs.is_empty() {
                        let v = self.load_item(&Value::with_kv(crate::domain::ObjSet::from_vec(recs), sk.kv.clone()), Ns::Item, &Value::lit(nk), node, state, true);
                        vals.push(v);
                    }
                }
            }
            let mut v = if vals.is_empty() { Value::data("kw") } else { join_values(vals) };
            if !kw_definite {
                v = match defaults.get(&name) {
                    Some(d) => v.join(d),
                    None => v.join(&Value::data("kw")),
                };
            }
            v
        } else {
            Value::data(&format!("missing arg {}", intern::str_get(name)))
        }
    }
}

/// The caller's part of the state a callee's run takes out (`Analyzer::take_caller`).
struct Caller {
    env: IndexMap<StrId, Value>,
    unbound: std::sync::Arc<Vec<StrId>>,
    path: std::sync::Arc<Vec<(StrId, StrId)>>,
    ret: Option<Value>,
    exits: Option<std::sync::Arc<State>>,
}
