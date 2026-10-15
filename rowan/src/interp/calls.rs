//! Calls: dispatch, unknown calls, construction, user functions, bindings (port of the
//! "calls" section of `rowan/interp.py`).

use indexmap::IndexMap;
use ruff_python_ast as ast;

use crate::domain::{chain_extend, join_values, Ns, Row, Source, SourceKind, Value, DATA};
use crate::frontend::Node;
use crate::intern::{self, key_str_lit, s, KeySet, StrId};
use crate::state::{Kind, State};
use crate::domain::ObjId;

use super::{is_generator, Analyzer, Call, Frame, FrameKind, FuncInfo, FuncNode, Payload, MAX_CLONES};


impl Analyzer {
    // ---------------------------------------------------------------- calls
    pub fn call_value(&mut self, fv: &Value, node: &Node, call_ast: Option<&'static ast::ExprCall>, state: &mut State, override_args: Option<Vec<Value>>) -> Value {
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
        let bulk = !fv.pts.is_empty() && fv.pts.iter().all(|o| match &self.heap.obj(o).payload {
            Payload::Str(q) if self.heap.kind(o) == Kind::External => super::BULK_CONSUMERS.contains(&q.as_str()),
            Payload::Method(q, _) => super::BULK_CONSUMERS.contains(&q.as_str()),
            _ => false,
        });
        for o in fv.pts.iter() {
            if let Payload::Method(_, r) | Payload::Bound(_, r) = &self.heap.obj(o).payload {
                let r = r.clone();
                self.force_gen(&r, state, true);
            }
        }
        // (the program's own code takes them as they are: its loops run them)
        let user = !fv.pts.is_empty() && fv.pts.iter().all(|o| matches!(self.heap.kind(o), Kind::Function | Kind::Bound | Kind::Class));
        for a in args.iter().chain(kwargs.values()).chain(star_args.iter()).chain(star_kwargs.iter()).filter(|_| !user) {
            self.force_gen(a, state, !bulk);
        }
        if fv.pts.is_empty() {
            return self.unknown_call("?", None, &args, &kwargs, &star_args, &star_kwargs, state, node);
        }
        let targets: Vec<u32> = fv.pts.to_vec();
        if targets.len() == 1 {
            return self.call_one(targets[0], &args, &kwargs, &star_args, &star_kwargs, state, node, call_ast);
        }
        // (past the hard deadline: one unknown call for all of them)
        if self.budget.over_hard_deadline() && targets.iter().all(|o| matches!(self.heap.kind(*o), Kind::Function | Kind::Bound)) {
            return self.unknown_call("?", None, &args, &kwargs, &star_args, &star_kwargs, state, node);
        }
        // (one method past its budget on each of several receivers: one call past the budget
        // with all of them as the receiver, not one per receiver)
        let (targets, batch) = match self.skipped_on_receivers(&targets) {
            Some((f, recv, rest)) if rest.is_empty() => return self.skipped_on(f, &recv, &args, &kwargs, &star_args, &star_kwargs, state, node),
            Some((f, recv, rest)) => (rest, Some((f, recv))),
            None => (targets, None),
        };
        // (several functions a value picks — a method of each of several receivers is the
        // receiver's own: not picked)
        let picked = targets.iter().all(|o| !matches!(self.heap.kind(*o), Kind::Bound) && !matches!(self.heap.obj(*o).payload, Payload::Method(..) | Payload::Bound(..))).then(|| self.node_text(node));
        self.alternatives(targets.len() + usize::from(batch.is_some()), false, state, &mut |an, i, st| {
            match (targets.get(i), &batch) {
                (Some(&oid), _) => an.call_picked(oid, &picked, &args, &kwargs, &star_args, &star_kwargs, st, node, call_ast),
                (None, Some((f, recv))) => an.skipped_on(*f, recv, &args, &kwargs, &star_args, &star_kwargs, st, node),
                (None, None) => unreachable!(),
            }
        })
    }

    /// One of several callees a value picks (`call_value`; `picked`: the call's text, when the
    /// value picks among functions).
    #[allow(clippy::too_many_arguments)]
    fn call_picked(&mut self, oid: ObjId, picked: &Option<String>, args: &[Value], kwargs: &IndexMap<StrId, Value>, star_args: &[Value], star_kwargs: &[Value], st: &mut State, node: &Node, call_ast: Option<&'static ast::ExprCall>) -> Value {
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
        let r = self.call_one(oid, args, kwargs, star_args, star_kwargs, st, node, call_ast);
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

    /// Among `targets`, bound methods of one function on several receivers whose calls would
    /// not run (past the function's budget, no recursion of it running): the function, the
    /// receivers, and the other targets.
    fn skipped_on_receivers(&self, targets: &[ObjId]) -> Option<(ObjId, Value, Vec<ObjId>)> {
        let bound = |o: ObjId| match &self.heap.obj(o).payload {
            Payload::Bound(f, r) if self.heap.kind(o) == Kind::Bound && r.pts.len() == 1 => Some((*f, r)),
            _ => None,
        };
        let skips = |f: ObjId| {
            let fi = self.heap.func_info(f);
            let runs = self.m.sdispatch.contains_key(&f) || fi.is_memo.get() || matches!(fi.node, FuncNode::Def(d) if is_generator(&d.body));
            !runs && self.budget.exhausted(fi.node.addr()) && self.recursion.depth(f) == 0
        };
        let f = targets.iter().find_map(|o| bound(*o).map(|b| b.0).filter(|f| skips(*f)))?;
        let (mine, rest): (Vec<ObjId>, Vec<ObjId>) = targets.iter().partition(|o| bound(**o).is_some_and(|b| b.0 == f));
        (mine.len() > 1).then(|| (f, join_values(mine.iter().filter_map(|o| bound(*o).map(|b| b.1.clone())).collect::<Vec<Value>>()), rest))
    }

    /// A call of method `f` past its budget on `recv` (one or several receivers).
    #[allow(clippy::too_many_arguments)]
    fn skipped_on(&mut self, f: ObjId, recv: &Value, args: &[Value], kwargs: &IndexMap<StrId, Value>, star_args: &[Value], star_kwargs: &[Value], state: &mut State, node: &Node) -> Value {
        let q = self.heap.func_info(f).qualname.clone();
        self.findings.note(format!("{} ran out of its budget: later calls are unknown code", q));
        self.budget.out.insert(f);
        self.entry.analysed.insert(f);
        self.skipped_call(f, &q, Some(recv), args, kwargs, star_args, star_kwargs, state, node)
    }

    /// Several definitions of one method (conditional ones) on one receiver.
    #[allow(clippy::too_many_arguments)]
    fn call_alternatives(&mut self, fns: &[u32], recv: u32, args: &[Value], kwargs: &IndexMap<StrId, Value>, star_args: &[Value], star_kwargs: &[Value], state: &mut State, node: &Node) -> Value {
        self.alternatives(fns.len(), false, state, &mut |an, i, st| an.call_user(fns[i], Some(Value::obj(recv)), args.to_vec(), kwargs.clone(), star_args.to_vec(), star_kwargs.to_vec(), st, node, false))
    }

    pub fn call_one(&mut self, oid: u32, args: &[Value], kwargs: &IndexMap<StrId, Value>, star_args: &[Value], star_kwargs: &[Value], state: &mut State, node: &Node, call_ast: Option<&'static ast::ExprCall>) -> Value {
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
                        let f = self.heap.cls_slots.get(&(m, Ns::Attr, key_str_lit("forward"))).cloned().unwrap_or_default();
                        let fns: Vec<u32> = f.pts.iter().filter(|x| self.heap.kind(*x) == Kind::Function).collect();
                        let r = self.call_alternatives(&fns, oid, args, kwargs, star_args, star_kwargs, state, node);
                        return crate::models::torch::t_forward_hooks(self, oid, args, r, state, node);
                    }
                }
            }
        }
        match kind {
            Kind::Function if self.heap.func_info(oid).is_memo.get() && star_args.is_empty() && star_kwargs.is_empty() => crate::models::singledispatch::memo_call(self, oid, args, kwargs, state, node),
            Kind::Function => {
                let fi = self.heap.func_info(oid);
                if fi.cls.is_some() && !fi.is_static.get() && !args.is_empty() && !fi.is_classmethod.get() {
                    // unbound method called with explicit self: Base.method(self, ...)
                    return self.call_user(oid, Some(args[0].clone()), args[1..].to_vec(), kwargs.clone(), star_args.to_vec(), star_kwargs.to_vec(), state, node, false);
                }
                if fi.is_classmethod.get() {
                    if let Some(cls) = fi.cls {
                        // `Cls.make(...)`: the class is the first argument
                        return self.call_user(oid, Some(Value::obj(cls)), args.to_vec(), kwargs.clone(), star_args.to_vec(), star_kwargs.to_vec(), state, node, false);
                    }
                }
                self.call_user(oid, None, args.to_vec(), kwargs.clone(), star_args.to_vec(), star_kwargs.to_vec(), state, node, false)
            }
            Kind::Bound => {
                let Payload::Bound(f, self_v) = self.heap.obj(oid).payload.clone() else { return Value::new() };
                self.call_user(f, Some(self_v), args.to_vec(), kwargs.clone(), star_args.to_vec(), star_kwargs.to_vec(), state, node, false)
            }
            Kind::Class => self.construct(oid, args, kwargs, star_args, star_kwargs, state, node),
            Kind::External => {
                let qual = self.heap.external_qual(oid);
                if let Some(v) = crate::models::builtin_dunder(self, &qual, args, kwargs, star_args, star_kwargs, state, node) {
                    return v;
                }
                let r = self.call_external(&qual, args, kwargs, star_args, star_kwargs, state, node, call_ast);
                if call_ast.is_some() {
                    crate::models::may_type_error(self, &qual, args, kwargs, &r, state);
                }
                r
            }
            Kind::Method => self.call_method_object(oid, args, kwargs, star_args, star_kwargs, state, node, call_ast),
            Kind::Unknown => {
                let q = format!("call of {}", self.heap.obj(oid).label);
                self.unknown_call(&q, Some(&Value::obj(oid)), args, kwargs, star_args, star_kwargs, state, node)
            }
            Kind::Partial => {
                let call = Call { qual: "partial.__call__".into(), recv: Some(Value::obj(oid)), args: args.to_vec(), kwargs: kwargs.clone(), star_args: star_args.to_vec(), star_kwargs: star_kwargs.to_vec(), node: *node, call_ast, site: format!("L{}", node.line), line: node.line };
                crate::models::partial_call(self, state, &call)
            }
            Kind::Instance => {
                if let Some(cls) = self.heap.instance_class(oid) {
                    if let Some(m) = self.class_lookup(cls, "__call__") {
                        let f = self.heap.cls_slots.get(&(m, Ns::Attr, key_str_lit("__call__"))).cloned().unwrap_or_default();
                        if f.pts.is_empty() {
                            return Value::new();
                        }
                        let fns: Vec<u32> = f.pts.iter().filter(|x| self.heap.kind(*x) == Kind::Function).collect();
                        return self.call_alternatives(&fns, oid, args, kwargs, star_args, star_kwargs, state, node);
                    }
                }
                let q = format!("call of {}", self.heap.obj(oid).label);
                self.unknown_call(&q, Some(&Value::obj(oid)), args, kwargs, star_args, star_kwargs, state, node)
            }
            _ => {
                let q = format!("call of {}", self.heap.obj(oid).label);
                self.unknown_call(&q, None, args, kwargs, star_args, star_kwargs, state, node)
            }
        }
    }

    /// A call of a library method value (`dict.get` bound to its receiver): its model (a str
    /// method on known text: exactly), else unknown code; over several groupby objects, once each.
    #[allow(clippy::too_many_arguments)]
    fn call_method_object(&mut self, oid: u32, args: &[Value], kwargs: &IndexMap<StrId, Value>, star_args: &[Value], star_kwargs: &[Value], state: &mut State, node: &Node, call_ast: Option<&'static ast::ExprCall>) -> Value {
        let Payload::Method(qual, recv) = self.heap.obj(oid).payload.clone() else { return Value::new() };
        // several groupby objects (`groupby` over one of several columns): the model runs
        // once per object, each making its own results
        if qual.starts_with("groupby.") && recv.pts.len() > 1 && recv.pts.iter().all(|o| self.heap.kind(o) == Kind::Groupby) {
            let gs: Vec<ObjId> = recv.pts.to_vec();
            return self.alternatives(gs.len(), false, state, &mut |an, i, st| {
                let Some(mo) = an.method_value(&qual, &Value::obj(gs[i])).single() else { return Value::new() };
                let saved = an.cx.alloc_extra.replace(format!("gb{}", gs[i]));
                let r = an.call_one(mo, args, kwargs, star_args, star_kwargs, st, node, call_ast);
                an.cx.alloc_extra = saved;
                r
            });
        }
        let mut args = args.to_vec();
        if qual.starts_with("dict.") || qual.starts_with("set.") {
            if let Some(a) = args.first_mut() {
                *a = self.keyify(a);
            }
        }
        let call = Call { qual: qual.clone(), recv: Some(recv.clone()), args, kwargs: kwargs.clone(), star_args: star_args.to_vec(), star_kwargs: star_kwargs.to_vec(), node: *node, call_ast, site: format!("L{}", node.line), line: node.line };
        let pure = if qual.starts_with("str.") { crate::models::pure::try_pure(self, state, &call) } else { None };
        let r = match (pure, self.program.models.lookup(&qual)) {
            (Some(v), _) => v,
            (None, Some(m)) => self.run_model(m, state, &call),
            (None, None) => return self.unknown_call(&qual, Some(&recv), &call.args, kwargs, star_args, star_kwargs, state, node),
        };
        // (a str method of a str: what it returns is known by the method)
        let r = match self.value_type(&recv) {
            Some(t @ ("str" | "bytes")) => self.as_type(r, crate::models::str_method_type(&qual, t), false),
            _ => r,
        };
        if call_ast.is_some() {
            crate::models::may_type_error(self, &qual, &call.args, kwargs, &r, state);
        }
        r
    }

    /// A call of a library function or class (a model, or unknown code).
    #[allow(clippy::too_many_arguments)]
    pub fn call_external(&mut self, qual: &str, args: &[Value], kwargs: &IndexMap<StrId, Value>, star_args: &[Value], star_kwargs: &[Value], state: &mut State, node: &Node, call_ast: Option<&'static ast::ExprCall>) -> Value {
        let qual = qual.to_string();
        let call = Call { qual: qual.clone(), recv: None, args: args.to_vec(), kwargs: kwargs.clone(), star_args: star_args.to_vec(), star_kwargs: star_kwargs.to_vec(), node: *node, call_ast, site: format!("L{}", node.line), line: node.line };
        if let Some(v) = crate::models::pure::try_pure(self, state, &call) {
            return self.as_type(v, crate::models::result_type(self, &qual, args), false);
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
            return self.as_type(r, crate::models::result_type(self, &qual, args), false);
        }
        let r = self.unknown_call(&qual, None, args, kwargs, star_args, star_kwargs, state, node);
        // (`input()`: a str all the same)
        if qual == "builtins.input" {
            for o in r.pts.iter() {
                self.heap.obj_mut(o).ty = Some("str");
            }
        }
        r
    }

    /// A model call: objects it allocates at one site stay distinct from each other (one
    /// weak object) even while only the model's own code holds them.
    pub(in crate::interp) fn run_model(&mut self, m: crate::models::Model, state: &mut State, call: &Call) -> Value {
        if self.cx.model_depth == 0 {
            self.cx.model_epoch += 1;
        }
        self.cx.model_depth += 1;
        let r = m(self, state, call);
        self.cx.model_depth -= 1;
        crate::models::itertools::after_model_consume(self, call, &r, state);
        r
    }

    pub fn construct(&mut self, cls: u32, args: &[Value], kwargs: &IndexMap<StrId, Value>, star_args: &[Value], star_kwargs: &[Value], state: &mut State, node: &Node) -> Value {
        if crate::models::dataclasses::abstract_unimplemented(self, cls) {
            self.raise_here("TypeError", state);
            return Value::new();
        }
        // `Color(3)`: the Enum member with that value, else ValueError
        if let (Some(ms), Some(a)) = (self.m.enum_members.get(&cls).cloned(), args.first()) {
            return crate::models::enums::enum_lookup(self, cls, &ms, a, node, state);
        }
        // a TypedDict class makes a plain dict
        if let Some(v) = crate::models::dataclasses::typed_dict_construct(self, cls, args, kwargs, star_args, star_kwargs, state, node) {
            return v;
        }
        // a class with its own `__new__`: the instance is what it returns, and `__init__` runs
        // on it when it is an instance of the class
        if let Some(owner) = self.class_lookup(cls, "__new__") {
            let fv = self.heap.cls_slots.get(&(owner, Ns::Attr, key_str_lit("__new__"))).cloned().unwrap_or_default();
            let fns: Vec<ObjId> = fv.pts.iter().filter(|f| self.heap.kind(*f) == Kind::Function).collect();
            if !fns.is_empty() {
                let mut nargs = vec![Value::obj(cls)];
                nargs.extend(args.iter().cloned());
                let mut out = Value::new();
                for f in fns {
                    let r = self.call_user(f, None, nargs.clone(), kwargs.clone(), star_args.to_vec(), star_kwargs.to_vec(), state, node, false);
                    out = out.join(&r);
                }
                if let Some(init) = self.class_lookup(cls, "__init__") {
                    let iv = self.heap.cls_slots.get(&(init, Ns::Attr, key_str_lit("__init__"))).cloned().unwrap_or_default();
                    let insts: Vec<ObjId> = out.pts.iter().filter(|o| self.heap.kind(*o) == Kind::Instance && self.heap.instance_class(*o).is_some_and(|c| self.heap.mro(c).contains(&cls))).collect();
                    for inst in insts {
                        for f in iv.pts.iter().filter(|f| self.heap.kind(*f) == Kind::Function).collect::<Vec<_>>() {
                            self.call_user(f, Some(Value::obj(inst)), args.to_vec(), kwargs.clone(), star_args.to_vec(), star_kwargs.to_vec(), state, node, false);
                        }
                    }
                }
                return out;
            }
        }
        let inst = self.alloc_instance(cls, node, state);
        // library classes: TypedDict, pydantic models, dict subclasses, dataclasses
        if let Some(v) = crate::models::dataclasses::construct_library(self, cls, inst, args, kwargs, star_args, star_kwargs, state, node) {
            return v;
        }
        if let Some(init) = self.class_lookup(cls, "__init__") {
            let fv = self.heap.cls_slots.get(&(init, Ns::Attr, key_str_lit("__init__"))).cloned().unwrap_or_default();
            self.cx.fresh_init.insert(inst, super::context::Building { mark: self.heap.serial(), at: state.copy(), roots: Vec::new(), holders: Vec::new(), looks: 0, held: false });
            for f in fv.pts.iter() {
                if self.heap.kind(f) == Kind::Function {
                    self.call_user(f, Some(Value::obj(inst)), args.to_vec(), kwargs.clone(), star_args.to_vec(), star_kwargs.to_vec(), state, node, false);
                }
            }
            self.cx.fresh_init.remove(&inst);
        } else if self.builtin_exc_base(cls) {
            crate::models::dataclasses::exception_init(self, cls, inst, args, node, state);
        }
        Value::obj(inst)
    }

    /// A fresh instance of user class `cls` allocated at `node`, without attributes (an
    /// external base's may come from outside the program).
    pub fn alloc_instance(&mut self, cls: ObjId, node: &Node, state: &mut State) -> ObjId {
        let ci = self.heap.class_info(cls);
        let row = if ci.open_base { Row::with_source(Source::new(&format!("instance {}@L{}", ci.name, node.line), SourceKind::Class)) } else { Row::new() };
        // the allocation site includes the class: `cls()` with several possible classes must
        // not collapse onto one instance (prototype: same site, last class wins); by the class
        // object, as two classes may share a name (`type(name, bases, ns)` per choice of bases)
        let site = format!("{}:{}", super::site_string(node), cls);
        let inst = self.alloc(Kind::Instance, &site, state, &format!("{}@L{}", ci.name, node.line), Payload::Cls(cls), Some(row), Ns::Attr, None, None);
        if ci.lib.typed_dict {
            state.set_row(inst, Ns::Item, Row::new());
        } else if ci.open_base && !ci.lib.dict_base {
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

    #[allow(clippy::too_many_arguments)]
    pub fn call_user(&mut self, foid: u32, self_v: Option<Value>, args: Vec<Value>, kwargs: IndexMap<StrId, Value>, star_args: Vec<Value>, star_kwargs: Vec<Value>, state: &mut State, node: &Node, entry: bool) -> Value {
        // past the hard deadline a user function is unknown code (its result unknown, its
        // arguments possibly changed, it may raise): the run ends in bounded time
        if self.budget.over_hard_deadline() {
            self.gens.force_at = None;
            let q = self.heap.func_info(foid).qualname.clone();
            self.findings.note(format!("analysis cut at the deadline ({} ms): later calls are unknown code", super::limit("ms", super::DEADLINE_MS)));
            return self.skipped_call(foid, &q, self_v.as_ref(), &args, &kwargs, &star_args, &star_kwargs, state, node);
        }
        // a generator's body runs when it is consumed (`force_gen`), not here
        let lazy = !entry
            && self.gens.force_at.is_none()
            && self.cx.model_depth == 0
            && star_args.is_empty()
            && star_kwargs.is_empty()
            && !self.gens.cm_hook.as_ref().is_some_and(|h| !h.ran && h.depth == self.cx.frames.len())
            && matches!(self.heap.func_info(foid).node, FuncNode::Def(f) if is_generator(&f.body));
        if lazy {
            return self.make_lazy_gen(foid, self_v, args, kwargs, state, node);
        }
        // (user code called back by a model allocates as user code)
        let saved_md = std::mem::replace(&mut self.cx.model_depth, 0);
        // (the arguments, for the unknown call that replaces this one when the hard deadline cuts it)
        let was_cut = self.budget.hard_cut;
        let keep = (self_v.clone(), args.clone(), kwargs.clone(), star_args.clone(), star_kwargs.clone());
        let r = self.call_user_inner(foid, self_v, args, kwargs, star_args, star_kwargs, state, node, entry);
        self.cx.model_depth = saved_md;
        if self.budget.hard_cut && !was_cut {
            let q = self.heap.func_info(foid).qualname.clone();
            self.findings.note(format!("analysis cut at the deadline ({} ms) inside {}: the call is unknown code", super::limit("ms", super::DEADLINE_MS), q));
            state.reachable = true;
            let (sv, a, kw, sa, skw) = keep;
            return self.skipped_call(foid, &q, sv.as_ref(), &a, &kw, &sa, &skw, state, node);
        }
        r
    }

    #[allow(clippy::too_many_arguments)]
    fn call_user_inner(&mut self, foid: u32, self_v: Option<Value>, args: Vec<Value>, kwargs: IndexMap<StrId, Value>, star_args: Vec<Value>, star_kwargs: Vec<Value>, state: &mut State, node: &Node, entry: bool) -> Value {
        let forced = self.gens.force_at.take();
        // a singledispatch function: the implementation for the first argument's type
        if self.m.sdispatch.contains_key(&foid) && self.m.sd_bypass.take() != Some(foid) && !args.is_empty() && star_args.is_empty() {
            return crate::models::singledispatch::sdispatch_call(self, foid, self_v, args, kwargs, star_kwargs, state, node, entry);
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
        let key = self.activation_key(foid, &site, entry, &args, &kwargs, &star_args, &star_kwargs);
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
            return self.skipped_call(foid, &q, self_v.as_ref(), &args, &kwargs, &star_args, &star_kwargs, state, node);
        }
        if super::debug_enabled() {
            eprintln!("run {} at {} frames={} chain={:?}", fi.qualname, site, self.cx.frames.len(), self.cx.call_chain);
        }
        // context for allocations inside the callee (§3)
        let ctx = match forced {
            Some((_, ctx)) => ctx,
            None => self.receiver_ctx(&self_v, &site),
        };
        // (a generator's object: the one its run returns, keyed as `yield_value` finds it)
        let gen_id = matches!(fnode, FuncNode::Def(f) if is_generator(&f.body)).then(|| self.heap.alloc_raw(Kind::List, &format!("gen:{}", foid), &ctx, true, &format!("generator {}", fi.qualname), Payload::None));
        let new_fr = self.callee_frame(foid, &fi, &ctx);
        let saved_env = std::mem::take(&mut state.env);
        let saved_unbound = std::mem::take(&mut state.unbound);
        self.cx.env_stack.push(saved_env.clone());
        let saved_ret = state.ret.take();
        // (the caller's recorded returns are not the callee's: its end must not join them in)
        let saved_exits = { state.exits.take() };
        self.cx.frames.push(new_fr);
        let saved_yields = gen_id.map(|ygid| (ygid, self.gen_activation_begin(ygid, foid, &fi, &ctx, state)));
        let saved_chain = std::mem::take(&mut self.cx.call_chain);
        self.cx.call_chain = chain_extend(&saved_chain, &[node.line]);
        let pre_bind = state.copy();
        // (objects made from here on, by birth: a run again of code makes them anew)
        let born_mark = self.heap.serial();
        let pre_rc = std::rc::Rc::new(pre_bind.copy());
        self.recursion.begin(&key, (self_v.clone(), args.clone(), kwargs.clone()), pre_rc.clone(), born_mark);
        if gen_id.is_none() {
            if let Some(fr) = self.cx.frames.last_mut() {
                fr.pre_call = Some(super::PreCall(self.heap.serial(), pre_rc));
            }
        }
        let gen_cm = self.gens.cm_hook.as_ref().is_some_and(|h| !h.ran && h.depth + 1 == self.cx.frames.len());
        self.bind_params(&fi, &self_v, &args, &kwargs, &star_args, &star_kwargs, state, node);
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
            self.heap.obj_mut(gid).iterator = true;
            ret = Value::obj(gid);
        }
        let rows: Vec<(u32, Row)> = ret.pts.iter().filter(|o| self.heap.is_record_like(*o)).map(|o| (o, state.row(o, Ns::Item))).collect();
        self.findings.summaries.insert((fi.qualname.clone(), if entry { "entry".into() } else { ctx.clone() }), (ret.clone(), rows));
        // cleanup
        self.recursion.end(&key);
        self.cx.call_chain = saved_chain;
        self.cx.frames.pop();
        self.cx.env_stack.pop();
        state.env = saved_env;
        state.unbound = saved_unbound;
        state.ret = saved_ret;
        if saved_exits.is_some() {
            state.exits = saved_exits;
        }
        ret
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
            let mut cells: Vec<String> = mi.scopes.get(fi.scope).cells.iter().cloned().collect();
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
        self.cx.frames.push(new_fr);
        let fake = Node::synthetic(fi.node.node().module, fi.node.node().line);
        self.bind_params(&fi, &Some(self_v), &args, &IndexMap::new(), &[], &[], state, &fake);
        f(self, state);
        self.cx.frames.pop();
        self.cx.env_stack.pop();
        state.env = saved_env;
        state.unbound = saved_unbound;
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::interp) fn bind_params(&mut self, fi: &FuncInfo, self_v: &Option<Value>, args: &[Value], kwargs: &IndexMap<StrId, Value>, star_args: &[Value], star_kwargs: &[Value], state: &mut State, node: &Node) {
        let bound = self.compute_bindings(fi, self_v, args, kwargs, star_args, star_kwargs, state, node);
        for (name, v) in bound {
            self.store_name(name, v, state);
        }
    }

    /// Parameter name -> argument value (allocating the *args tuple / **kwargs dict in `state`).
    #[allow(clippy::too_many_arguments)]
    pub(in crate::interp) fn compute_bindings(&mut self, fi: &FuncInfo, self_v: &Option<Value>, args: &[Value], kwargs: &IndexMap<StrId, Value>, star_args: &[Value], star_kwargs: &[Value], state: &mut State, node: &Node) -> IndexMap<StrId, Value> {
        let mut bound: IndexMap<StrId, Value> = IndexMap::new();
        let Some(a) = fi.node.params() else {
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
        for name in positional.iter().chain(kwonly.iter()) {
            if bound.contains_key(name) {
                continue;
            }
            let v = self.unbound_param_value(fi, *name, star_kwargs, node, state);
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
                let nk = intern::key(intern::Key::Str(intern::str_get(*name)));
                row = row.store(nk, true);
                state.set_slot(did, Ns::Item, nk, v.clone(), true);
            }
            state.set_row(did, Ns::Item, row);
            let kill: KeySet = KeySet::from_iter(positional.iter().chain(kwonly.iter()).map(|n| intern::key(intern::Key::Str(intern::str_get(*n)))));
            for sk in star_kwargs {
                self.copy_into(did, Ns::Item, sk, state, true, &kill);
            }
            bound.insert(s(kw.name.id.as_str()), Value::obj(did));
        }
        bound
    }

    /// The value of a parameter no argument binds: `**kwargs` holding the name wins over the
    /// default (joined with it when the key is only possibly there); a required one only
    /// `**kw` supplies is read from it (a requirement on an input).
    fn unbound_param_value(&mut self, fi: &FuncInfo, name: StrId, star_kwargs: &[Value], node: &Node, state: &mut State) -> Value {
        let nk = intern::key(intern::Key::Str(intern::str_get(name)));
        let kw_present = star_kwargs.iter().any(|sk| sk.pts.iter().any(|x| state.row(x, Ns::Item).present.contains(nk)));
        let kw_definite = !star_kwargs.is_empty() && star_kwargs.iter().all(|sk| sk.pts.iter().all(|x| state.row(x, Ns::Item).definite.contains(nk)));
        if let (Some(d), false) = (fi.defaults.get(&name), kw_present) {
            d.clone()
        } else if !star_kwargs.is_empty() {
            let mut vals: Vec<Value> = star_kwargs.iter().flat_map(|sk| sk.pts.iter().map(|x| state.slot(x, Ns::Item, nk)).collect::<Vec<_>>()).collect();
            if fi.defaults.get(&name).is_none() && !kw_definite {
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
                v = match fi.defaults.get(&name) {
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

