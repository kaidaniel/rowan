//! Calls: dispatch, unknown calls, construction, user functions, bindings (port of the
//! "calls" section of `rowan/interp.py`).

use indexmap::IndexMap;
use rustc_hash::FxHashSet;
use ruff_python_ast as ast;

use crate::domain::{chain_extend, join_values, KeyVal, Ns, Row, Source, SourceKind, Value, DATA};
use crate::frontend::Node;
use crate::intern::{self, key_str_lit, s, KeySet, StrId};
use crate::state::{Kind, State};
use crate::domain::ObjId;

use super::{is_generator, Analyzer, Call, Frame, FrameKind, FuncInfo, FuncNode, Payload, MAX_ACTIVE, MAX_CLONES, REC_MAX_ITER};

fn widen_value(v: &Value) -> Value {
    if let KeyVal::Lits(ks) = &v.kv {
        if ks.len() > 2 {
            return Value::with_kv(v.pts.clone(), KeyVal::data("recursion"));
        }
    }
    v.clone()
}


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
        self.alternatives(targets.len(), false, state, &mut |an, i, st| {
            let oid = targets[i];
            // (a groupby method of one of several groupby objects: its results are its own)
            let own = matches!(&an.heap.obj(oid).payload, Payload::Method(q, r) if q.starts_with("groupby.") && r.pts.iter().all(|g| an.heap.kind(g) == Kind::Groupby));
            let saved = if own { an.alloc_extra.replace(format!("gbm{}", oid)) } else { an.alloc_extra.clone() };
            let r = an.call_one(oid, &args, &kwargs, &star_args, &star_kwargs, st, node, call_ast);
            an.alloc_extra = saved;
            r
        })
    }

    /// Alternatives: `run(i)` for each of `n` callees, each on its own copy of the state, the
    /// states joined (`keep`: the state as it is is one more, where none runs); the results
    /// of those that return (a callee that raises on every path returns nothing: `A(x)` whose
    /// `__init__` always fails gives no A).
    pub fn alternatives(&mut self, n: usize, keep: bool, state: &mut State, run: &mut dyn FnMut(&mut Analyzer, usize, &mut State) -> Value) -> Value {
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
    #[allow(clippy::too_many_arguments)]
    fn call_alternatives(&mut self, fns: &[u32], recv: u32, args: &[Value], kwargs: &IndexMap<StrId, Value>, star_args: &[Value], star_kwargs: &[Value], state: &mut State, node: &Node, call_ast: Option<&'static ast::ExprCall>) -> Value {
        self.alternatives(fns.len(), false, state, &mut |an, i, st| an.call_user(fns[i], Some(Value::obj(recv)), args.to_vec(), kwargs.clone(), star_args.to_vec(), star_kwargs.to_vec(), st, node, call_ast, false))
    }

    pub fn call_one(&mut self, oid: u32, args: &[Value], kwargs: &IndexMap<StrId, Value>, star_args: &[Value], star_kwargs: &[Value], state: &mut State, node: &Node, call_ast: Option<&'static ast::ExprCall>) -> Value {
        let kind = self.heap.kind(oid);
        // torch: a layer object / Sequential, or a user nn.Module (its forward)
        if matches!(kind, Kind::Namespace | Kind::List) && self.t_layer_type(oid).is_some() {
            let r = self.t_layer_call(oid, args, kwargs, node, state);
            return self.t_forward_hooks(oid, args, r, state, node);
        }
        if kind == Kind::Instance && self.torch_on && self.t_is_module_instance(oid) {
            if let Some(cls) = self.heap.instance_class(oid) {
                if self.class_lookup(cls, "__call__").is_none() {
                    if let Some(m) = self.class_lookup(cls, "forward") {
                        let f = self.cls_slots.get(&(m, Ns::Attr, key_str_lit("forward"))).cloned().unwrap_or_default();
                        let fns: Vec<u32> = f.pts.iter().filter(|x| self.heap.kind(*x) == Kind::Function).collect();
                        let r = self.call_alternatives(&fns, oid, args, kwargs, star_args, star_kwargs, state, node, call_ast);
                        return self.t_forward_hooks(oid, args, r, state, node);
                    }
                }
            }
        }
        match kind {
            Kind::Function if self.heap.func_info(oid).is_memo.get() && star_args.is_empty() && star_kwargs.is_empty() => self.memo_call(oid, args, kwargs, state, node, call_ast),
            Kind::Function => {
                let fi = self.heap.func_info(oid);
                if fi.cls.is_some() && !fi.is_static.get() && !args.is_empty() && !fi.is_classmethod.get() {
                    // unbound method called with explicit self: Base.method(self, ...)
                    return self.call_user(oid, Some(args[0].clone()), args[1..].to_vec(), kwargs.clone(), star_args.to_vec(), star_kwargs.to_vec(), state, node, call_ast, false);
                }
                if fi.is_classmethod.get() {
                    if let Some(cls) = fi.cls {
                        // `Cls.make(...)`: the class is the first argument
                        return self.call_user(oid, Some(Value::obj(cls)), args.to_vec(), kwargs.clone(), star_args.to_vec(), star_kwargs.to_vec(), state, node, call_ast, false);
                    }
                }
                self.call_user(oid, None, args.to_vec(), kwargs.clone(), star_args.to_vec(), star_kwargs.to_vec(), state, node, call_ast, false)
            }
            Kind::Bound => {
                let Payload::Bound(f, self_v) = self.heap.obj(oid).payload.clone() else { return Value::new() };
                self.call_user(f, Some(self_v), args.to_vec(), kwargs.clone(), star_args.to_vec(), star_kwargs.to_vec(), state, node, call_ast, false)
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
            Kind::Method => {
                let Payload::Method(qual, recv) = self.heap.obj(oid).payload.clone() else { return Value::new() };
                // several groupby objects (`groupby` over one of several columns): the model runs
                // once per object, each making its own results
                if qual.starts_with("groupby.") && recv.pts.len() > 1 && recv.pts.iter().all(|o| self.heap.kind(o) == Kind::Groupby) {
                    let gs: Vec<ObjId> = recv.pts.to_vec();
                    return self.alternatives(gs.len(), false, state, &mut |an, i, st| {
                        let Some(mo) = an.method_value(&qual, &Value::obj(gs[i])).single() else { return Value::new() };
                        let saved = an.alloc_extra.replace(format!("gb{}", gs[i]));
                        let r = an.call_one(mo, args, kwargs, star_args, star_kwargs, st, node, call_ast);
                        an.alloc_extra = saved;
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
                let r = match (pure, self.models.lookup(&qual)) {
                    (Some(v), _) => v,
                    (None, Some(m)) => self.run_model(m, state, &call),
                    (None, None) => return self.unknown_call(&qual, Some(&recv), &call.args, kwargs, star_args, star_kwargs, state, node),
                };
                if call_ast.is_some() {
                    crate::models::may_type_error(self, &qual, &call.args, kwargs, &r, state);
                }
                r
            }
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
                        let f = self.cls_slots.get(&(m, Ns::Attr, key_str_lit("__call__"))).cloned().unwrap_or_default();
                        if f.pts.is_empty() {
                            return Value::new();
                        }
                        let fns: Vec<u32> = f.pts.iter().filter(|x| self.heap.kind(*x) == Kind::Function).collect();
                        return self.call_alternatives(&fns, oid, args, kwargs, star_args, star_kwargs, state, node, call_ast);
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

    /// A call of a library function or class (a model, or unknown code).
    #[allow(clippy::too_many_arguments)]
    pub fn call_external(&mut self, qual: &str, args: &[Value], kwargs: &IndexMap<StrId, Value>, star_args: &[Value], star_kwargs: &[Value], state: &mut State, node: &Node, call_ast: Option<&'static ast::ExprCall>) -> Value {
        let qual = qual.to_string();
        let call = Call { qual: qual.clone(), recv: None, args: args.to_vec(), kwargs: kwargs.clone(), star_args: star_args.to_vec(), star_kwargs: star_kwargs.to_vec(), node: *node, call_ast, site: format!("L{}", node.line), line: node.line };
        if let Some(v) = crate::models::pure::try_pure(self, state, &call) {
            return v;
        }
        // a builtin exception class: an exception object with its args
        if let Some(n) = qual.strip_prefix("builtins.").filter(|n| super::exprs::is_exception_name(n)) {
            if kwargs.is_empty() && star_args.is_empty() && !self.models.has_exact(&qual) {
                let n = n.to_string();
                return self.exception_object(&n, args.to_vec(), node, state);
            }
        }
        // `f.__get__(obj)` (a library function bound by hand: attrs's
        // `object.__setattr__.__get__(self)`): f with obj as its first argument
        if let Some(base) = qual.strip_suffix(".__get__").filter(|_| !args.is_empty() && kwargs.is_empty()) {
            let f = self.heap.alloc_raw(Kind::External, &format!("ext:{}", base), "", true, base, Payload::Str(base.to_string()));
            let pc = Call { qual: "functools.partial".into(), args: vec![Value::obj(f), args[0].clone()], ..call.clone() };
            if let Some(m) = self.models.lookup("functools.partial") {
                return self.run_model(m, state, &pc);
            }
        }
        if let Some(m) = self.models.lookup(&qual) {
            return self.run_model(m, state, &call);
        }
        self.unknown_call(&qual, None, args, kwargs, star_args, star_kwargs, state, node)
    }

    /// A model call: objects it allocates at one site stay distinct from each other (one
    /// weak object) even while only the model's own code holds them.
    pub fn run_model(&mut self, m: crate::models::Model, state: &mut State, call: &Call) -> Value {
        if self.model_depth == 0 {
            self.model_epoch += 1;
        }
        self.model_depth += 1;
        let r = m(self, state, call);
        self.model_depth -= 1;
        self.after_model_consume(call, &r, state);
        r
    }

    pub fn construct(&mut self, cls: u32, args: &[Value], kwargs: &IndexMap<StrId, Value>, star_args: &[Value], star_kwargs: &[Value], state: &mut State, node: &Node) -> Value {
        if self.abstract_unimplemented(cls) {
            self.raise_here("TypeError", state);
            return Value::new();
        }
        // `Color(3)`: the Enum member with that value, else ValueError
        if let (Some(ms), Some(a)) = (self.enum_members.get(&cls).cloned(), args.first()) {
            return self.enum_lookup(cls, &ms, a, node, state);
        }
        // a TypedDict class makes a plain dict
        if let Some(v) = self.typed_dict_construct(cls, args, kwargs, star_args, star_kwargs, state, node) {
            return v;
        }
        // a class with its own `__new__`: the instance is what it returns, and `__init__` runs
        // on it when it is an instance of the class
        if let Some(owner) = self.class_lookup(cls, "__new__") {
            let fv = self.cls_slots.get(&(owner, Ns::Attr, key_str_lit("__new__"))).cloned().unwrap_or_default();
            let fns: Vec<ObjId> = fv.pts.iter().filter(|f| self.heap.kind(*f) == Kind::Function).collect();
            if !fns.is_empty() {
                let mut nargs = vec![Value::obj(cls)];
                nargs.extend(args.iter().cloned());
                let mut out = Value::new();
                for f in fns {
                    let r = self.call_user(f, None, nargs.clone(), kwargs.clone(), star_args.to_vec(), star_kwargs.to_vec(), state, node, None, false);
                    out = out.join(&r);
                }
                if let Some(init) = self.class_lookup(cls, "__init__") {
                    let iv = self.cls_slots.get(&(init, Ns::Attr, key_str_lit("__init__"))).cloned().unwrap_or_default();
                    let insts: Vec<ObjId> = out.pts.iter().filter(|o| self.heap.kind(*o) == Kind::Instance && self.heap.instance_class(*o).is_some_and(|c| self.heap.mro(c).contains(&cls))).collect();
                    for inst in insts {
                        for f in iv.pts.iter().filter(|f| self.heap.kind(*f) == Kind::Function).collect::<Vec<_>>() {
                            self.call_user(f, Some(Value::obj(inst)), args.to_vec(), kwargs.clone(), star_args.to_vec(), star_kwargs.to_vec(), state, node, None, false);
                        }
                    }
                }
                return out;
            }
        }
        let inst = self.alloc_instance(cls, node, state);
        // library classes: TypedDict, pydantic models, dict subclasses, dataclasses
        if let Some(v) = self.construct_library(cls, inst, args, kwargs, star_args, star_kwargs, state, node) {
            return v;
        }
        if let Some(init) = self.class_lookup(cls, "__init__") {
            let fv = self.cls_slots.get(&(init, Ns::Attr, key_str_lit("__init__"))).cloned().unwrap_or_default();
            self.fresh_init.insert(inst);
            for f in fv.pts.iter() {
                if self.heap.kind(f) == Kind::Function {
                    self.call_user(f, Some(Value::obj(inst)), args.to_vec(), kwargs.clone(), star_args.to_vec(), star_kwargs.to_vec(), state, node, None, false);
                }
            }
            self.fresh_init.remove(&inst);
        } else if self.builtin_exc_base(cls) {
            self.exception_init(cls, inst, args, node, state);
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



    pub(super) fn receiver_ctx(&self, self_v: &Option<Value>, site: &str) -> (String, bool) {
        if let Some(sv) = self_v {
            if let Some(r) = sv.single() {
                if self.heap.kind(r) == Kind::Instance {
                    let recv = self.heap.obj(r);
                    // per receiver and call site: two calls of one method on one receiver
                    // allocate distinct objects
                    let ctx = if !recv.single {
                        site.to_string()
                    } else if self.heap.clones(&recv.site) >= MAX_CLONES {
                        String::new()
                    } else {
                        format!("o{}/{}", recv.id, site)
                    };
                    return (ctx, recv.single);
                }
            }
        }
        (site.to_string(), true)
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
    pub fn call_user(&mut self, foid: u32, self_v: Option<Value>, args: Vec<Value>, kwargs: IndexMap<StrId, Value>, star_args: Vec<Value>, star_kwargs: Vec<Value>, state: &mut State, node: &Node, _call_ast: Option<&'static ast::ExprCall>, entry: bool) -> Value {
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
            && self.model_depth == 0
            && star_args.is_empty()
            && star_kwargs.is_empty()
            && !self.gens.cm_hook.as_ref().is_some_and(|h| !h.ran && h.depth == self.frames.len())
            && matches!(self.heap.func_info(foid).node, FuncNode::Def(f) if is_generator(&f.body));
        if lazy {
            return self.make_lazy_gen(foid, self_v, args, kwargs, state, node);
        }
        // (user code called back by a model allocates as user code)
        let saved_md = std::mem::replace(&mut self.model_depth, 0);
        // (the arguments, for the unknown call that replaces this one when the hard deadline cuts it)
        let was_cut = self.budget.hard_cut;
        let keep = (self_v.clone(), args.clone(), kwargs.clone(), star_args.clone(), star_kwargs.clone());
        let r = self.call_user_inner(foid, self_v, args, kwargs, star_args, star_kwargs, state, node, _call_ast, entry);
        self.model_depth = saved_md;
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
    fn call_user_inner(&mut self, foid: u32, self_v: Option<Value>, args: Vec<Value>, kwargs: IndexMap<StrId, Value>, star_args: Vec<Value>, star_kwargs: Vec<Value>, state: &mut State, node: &Node, _call_ast: Option<&'static ast::ExprCall>, entry: bool) -> Value {
        let forced = self.gens.force_at.take();
        // a singledispatch function: the implementation for the first argument's type
        if self.sdispatch.contains_key(&foid) && self.sd_bypass.take() != Some(foid) && !args.is_empty() && star_args.is_empty() {
            return self.sdispatch_call(foid, self_v, args, kwargs, star_kwargs, state, node, entry);
        }
        if !state.narrowed.is_empty() {
            state.narrowed.clear();
        }
        let fi = self.heap.func_info(foid);
        let fnode = fi.node;
        let site = match forced.as_ref() {
            Some((site, _, _)) => site.clone(),
            None => self.callee_site(node),
        };
        self.analysed_funcs.insert(foid);
        let mut key = (foid, if !entry { site.clone() } else { format!("entry:{}", site) });
        // literal arguments that differ from the running activations': not the same activation
        // (fib(9) inside fib(10)), up to a depth
        let mut lit_key = false;
        if !entry && !args.is_empty() && kwargs.is_empty() && star_args.is_empty() && star_kwargs.is_empty() {
            // (a tensor of a fully known shape counts like a literal: halving until length 1)
            let lits: Option<Vec<String>> = args
                .iter()
                .map(|a| {
                    if let Some(k) = a.kv.as_lit().filter(|_| a.pts.is_empty()) {
                        return Some(intern::key_repr(k));
                    }
                    if self.torch_on {
                        if let Some(sh) = crate::models::torch::tensor_shape(a) {
                            if sh.rest.is_none() && sh.suffix.is_empty() && sh.assume.is_empty() {
                                let dims: Option<Vec<i64>> = sh.prefix.iter().map(|d| crate::models::shape::as_lit(*d)).collect();
                                return dims.map(|ds| format!("T{:?}", ds));
                            }
                        }
                    }
                    None
                })
                .collect();
            let depth = self.active_by_func.get(&foid).map(|v| v.len()).unwrap_or(0);
            if let Some(ls) = lits.filter(|_| depth < 40) {
                key.1 = format!("{}|{}", key.1, ls.join(","));
                lit_key = true;
            }
        }
        let act = self.active_by_func.get(&foid).cloned().unwrap_or_default();
        if !self.active.contains_key(&key) && act.len() >= MAX_ACTIVE && !lit_key {
            key = act.last().unwrap().clone();
        }
        if self.active.contains_key(&key) {
            self.rec_hit.insert(key.clone());
            // widen the entry arguments with the ones of this recursive call (§6.3)
            if let Some((e_self, e_args, e_kwargs)) = self.rec_entry.get(&key).cloned() {
                let n_self = match (&e_self, &self_v) {
                    (Some(a), Some(b)) => Some(a.join(b)),
                    _ => e_self.clone(),
                };
                let mut n_args: Vec<Value> = Vec::new();
                for (i, a) in e_args.iter().enumerate() {
                    n_args.push(if i < args.len() { a.join(&args[i]) } else { a.clone() });
                }
                if args.len() > e_args.len() {
                    n_args.extend(args[e_args.len()..].iter().cloned());
                }
                let mut n_kwargs = e_kwargs.clone();
                for (k, v) in kwargs.iter() {
                    let nv = match n_kwargs.get(k) {
                        Some(cur) => cur.join(v),
                        None => v.clone(),
                    };
                    n_kwargs.insert(*k, nv);
                }
                let n_args: Vec<Value> = n_args.iter().map(widen_value).collect();
                let n_kwargs: IndexMap<StrId, Value> = n_kwargs.iter().map(|(k, v)| (*k, widen_value(v))).collect();
                if n_self != e_self || n_args != e_args || n_kwargs != e_kwargs {
                    self.rec_entry.insert(key.clone(), (n_self, n_args, n_kwargs));
                    self.rec_widened.insert(key.clone());
                }
            }
            // the heap of this call, where it differs from the activation's start on objects
            // older than it that the call reaches: the activation starts from it too (else a
            // deeper call's paths that the first heap rules out would never be analysed; the
            // objects made since have no facts there, which reads as unknown)
            if let Some((pre, grown, mark)) = self.rec_heap.get(&key) {
                let (pre, mark) = (pre.clone(), *mark);
                let had = grown.is_some();
                let roots: Vec<Value> = self_v.iter().chain(args.iter()).chain(kwargs.values()).cloned().collect();
                let fp = state.footprint(&pre);
                let old = |o: ObjId| self.heap.born_before(o, mark);
                let mut targets: Vec<ObjId> = fp.rows.iter().map(|k| k.0).chain(fp.slots.iter().map(|k| k.0)).chain(fp.elems.iter().copied()).filter(|o| old(*o)).collect();
                targets.sort_unstable();
                targets.dedup();
                let reach = if targets.is_empty() { FxHashSet::default() } else { self.reach_among(&roots, &targets, state) };
                let fp = crate::state::Footprint { rows: fp.rows.into_iter().filter(|k| reach.contains(&k.0)).collect(), slots: fp.slots.into_iter().filter(|k| reach.contains(&k.0)).collect(), elems: fp.elems.into_iter().filter(|o| reach.contains(o)).collect() };
                if !fp.rows.is_empty() || !fp.slots.is_empty() || !fp.elems.is_empty() {
                    let mut g = self.rec_heap.get_mut(&key).and_then(|e| e.1.take()).unwrap_or_else(|| pre.copy());
                    let before = g.copy();
                    g.join_heap_fp(state, &fp);
                    let d = g.footprint(&before);
                    if !had || !d.rows.is_empty() || !d.slots.is_empty() || !d.elems.is_empty() {
                        self.rec_widened.insert(key.clone());
                    }
                    if let Some(e) = self.rec_heap.get_mut(&key) {
                        e.1 = Some(g);
                    }
                }
            }
            if let Some((ret, exit_state, fp)) = self.rec_table.get(&key) {
                let ret = ret.clone();
                state.join_heap_fp(exit_state, fp);
                return ret;
            }
            return Value::new();
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
            eprintln!("run {} at {} frames={} chain={:?}", fi.qualname, site, self.frames.len(), self.call_chain);
        }
        // context for allocations inside the callee (§3)
        let (ctx, ctx_single) = match forced {
            Some((_, ctx, single)) => (ctx, single),
            None => self.receiver_ctx(&self_v, &site),
        };
        // (a generator's object: the one its run returns, keyed as `yield_value` finds it)
        let gen_id = matches!(fnode, FuncNode::Def(f) if is_generator(&f.body)).then(|| self.heap.alloc_raw(Kind::List, &format!("gen:{}", foid), &ctx, true, &format!("generator {}", fi.qualname), Payload::None));
        let fr_single = self.frame().single && self.frame().loop_depth == 0 && ctx_single;
        let fr_single_nl = self.frame().single_nl && ctx_single;
        let mut new_fr = Frame::new(FrameKind::Function, fi.scope, fi.module, &ctx, fr_single, &fi.qualname);
        new_fr.single_nl = fr_single_nl;
        new_fr.func = Some(foid);
        new_fr.code = fi.node.addr();
        {
            let mi = self.heap.module_info(fi.module);
            let mut cells: Vec<String> = mi.scopes.get(fi.scope).cells.iter().cloned().collect();
            cells.sort();
            for name in cells {
                let cid = self.heap.alloc_raw(Kind::Cell, &format!("cell:{}.{}", fi.qualname, name), &ctx, fr_single, &format!("cell {}", name), Payload::None);
                new_fr.cells.insert(s(&name), cid);
            }
        }
        for (n, c) in fi.closure.iter() {
            new_fr.cells.insert(*n, *c);
        }
        let saved_env = std::mem::take(&mut state.env);
        let saved_unbound = std::mem::take(&mut state.unbound);
        self.env_stack.push(saved_env.clone());
        let saved_ret = state.ret.take();
        // (the caller's recorded returns are not the callee's: its end must not join them in)
        let saved_exits = { state.exits.take() };
        let mut saved_yields: Option<(ObjId, Option<super::YieldStates>)> = None;
        self.frames.push(new_fr);
        if let Some(ygid) = gen_id {
            // the yielded values of this activation
            let gid = self.alloc(Kind::List, &format!("gen:{}", foid), state, &format!("generator {}", fi.qualname), Payload::None, None, Ns::Item, None, Some(&ctx));
            self.heap.set_llen(gid, state, Some(0));
            state.elems.insert(gid, KeyVal::Lits(KeySet::new()));
            // (a new activation: not consumed yet, its own item states; a running one's
            // yields are put back after it)
            // (keyed as `yield_value` finds it: the site's object, not a variant `alloc` made)
            saved_yields = Some((ygid, self.gens.yield_states.remove(&ygid)));
            state.slots.remove(&gid, &(Ns::Attr, key_str_lit("__cursor")));
            state.slots.remove(&gid, &(Ns::Attr, key_str_lit("\u{1}effpos")));
            self.gens.pos_states.remove(&gid);
            self.gens.pos_conflict.retain(|(g, _)| *g != gid);
            self.gens.eff.remove(&gid);
        }
        let saved_chain = std::mem::take(&mut self.call_chain);
        self.call_chain = chain_extend(&saved_chain, &[node.line]);
        self.active.insert(key.clone(), true);
        self.active_by_func.entry(foid).or_default().push(key.clone());
        self.rec_entry.insert(key.clone(), (self_v.clone(), args.clone(), kwargs.clone()));

        let pre_bind = state.copy();
        // (objects made from here on, by birth: a run again of code makes them anew)
        let born_mark = self.heap.serial();
        let pre_rc = std::rc::Rc::new(pre_bind.copy());
        self.rec_heap.insert(key.clone(), (pre_rc.clone(), None, born_mark));
        if gen_id.is_none() {
            if let Some(fr) = self.frames.last_mut() {
                fr.pre_call = Some(super::PreCall(self.heap.serial(), pre_rc));
            }
        }
        let gen_cm = self.gens.cm_hook.as_ref().is_some_and(|h| !h.ran && h.depth + 1 == self.frames.len());
        self.bind_params(&fi, &self_v, &args, &kwargs, &star_args, &star_kwargs, state, node);
        let mut entry_state = state.copy();
        let mut ret = Value::new();
        let mut st_final: Option<State> = None;
        let mut vmark = self.findings.mark();
        for _round in 0..REC_MAX_ITER {
            // (a round of a recursion before its fixpoint: its accesses' verdicts are superseded)
            self.findings.drop_since(vmark);
            vmark = self.findings.mark();
            self.budget.runs += 1;
            let mut st = entry_state.copy();
            if let Some(g) = gen_id {
                self.gens.reads.push((g, false, Default::default()));
            }
            match fnode {
                FuncNode::Lambda(l) => {
                    let v = self.eval(&l.body, &mut st);
                    st.ret = Some(v);
                    st.record_exit();
                    st.reachable = false;
                }
                FuncNode::Def(f) => self.with_gen_restart(&mut st, true, &mut |an, s| an.exec_block(&f.body, s)),
            }
            if gen_id.is_some() {
                self.gens.last_reads = self.gens.reads.pop().map(|r| r.2);
            }
            ret = st.ret.clone().unwrap_or_default();
            if st.reachable {
                ret = ret.join(&self.none_v);
            }
            // no normal completion (every path raises): the caller is unreachable after the call
            let completed = st.reachable || st.exits.is_some();
            if !completed && crate::interp::debug_level() == 2 {
                eprintln!("  callee {} did not complete (called at L{})", fi.qualname, node.line);
            }
            if let Some(ex) = st.exits.take() {
                let mut exs = ex.copy();
                st = if st.reachable { st.join(&mut exs) } else { exs };
                st.exits = None;
            }
            st.ret = Some(ret.clone());
            st.reachable = completed;
            if self.rec_widened.contains(&key) {
                self.rec_widened.remove(&key);
                let (w_self, w_args, w_kwargs) = self.rec_entry.get(&key).cloned().unwrap();
                let fp = st.footprint(&entry_state);
                self.rec_table.insert(key.clone(), (ret.clone(), st.copy(), fp));
                state.replace_with(pre_bind.copy());
                if let Some((p, Some(g), _)) = self.rec_heap.get(&key) {
                    let fp = g.footprint(p);
                    state.join_heap_fp(g, &fp);
                }
                self.bind_params(&fi, &w_self, &w_args, &w_kwargs, &star_args, &star_kwargs, state, node);
                entry_state = state.copy();
                continue;
            }
            if self.rec_hit.contains(&key) {
                if let Some((pr, pst, _)) = self.rec_table.get(&key) {
                    if *pr == ret && pst.same(&st) {
                        st_final = Some(st);
                        break;
                    }
                }
                let fp = st.footprint(&entry_state);
                self.rec_table.insert(key.clone(), (ret.clone(), st.copy(), fp));
                continue;
            }
            st_final = Some(st);
            break;
        }
        let converged = st_final.is_some();
        // (the result stabilised even if the state did not: it stands)
        let ret_stable = self.rec_table.get(&key).map(|(pr, _, _)| *pr == ret).unwrap_or(false);
        let mut st = match st_final {
            Some(s) => s,
            None => {
                // did not converge: the last state, widened (its result may be any
                // value, the records it touched may hold other keys)
                let mut s0 = match self.rec_table.get(&key) {
                    Some((_, pst, _)) => pst.copy(),
                    None => entry_state.copy(),
                };
                let fp = s0.footprint(&entry_state);
                let site = s(&format!("recursion@{}", fi.qualname));
                for (o, ns) in fp.rows.iter().copied().collect::<Vec<_>>() {
                    let r = s0.row(o, ns).mutate(site);
                    s0.set_row(o, ns, r);
                }
                s0
            }
        };
        if !converged && !ret_stable {
            ret = ret.join(&Value::data(&format!("recursion@{}", fi.qualname)));
        }
        // the generator's body runs here, eagerly, but its caller sees the heap only where the
        // body is suspended: before the first item, at a yield, or at its end. The objects that
        // existed before the call and that the body changed hold the join of those states.
        let gen_yields = gen_id.filter(|_| !gen_cm).map(|gid| (gid, self.gens.yield_states.get(&gid).map(|v| v.states().map(|s| s.copy()).collect::<Vec<State>>()).unwrap_or_default()));
        // (a generator that ran to its end: the state there, for a consumer that uses it up)
        let finite_end = if gen_yields.is_some() && st.reachable { Some(st.copy()) } else { None };
        // a generator whose body never ends (`while True: yield …`): the caller resumes from
        // one of its yields, and the yields go on (their number is unknown)
        if let Some(gid) = gen_id.filter(|_| !st.reachable) {
            let joined = self.gens.yield_states.remove(&gid).and_then(|v| {
                let mut it = v.list.into_iter().map(|(_, s)| s);
                let mut acc = it.next()?;
                for mut s in it {
                    acc = acc.join(&mut s);
                }
                Some(acc)
            });
            if let Some(mut ys) = joined {
                let mut p = self.heap.prefix(gid, &ys).unwrap_or(0);
                // (the yields joined: every position some yield made is there — the items
                // the endless loop is known to give first)
                while ys.has_slot(gid, Ns::Pos, intern::key_int(p as i64)) {
                    p += 1;
                }
                if p > 0 {
                    self.heap.obj_mut(gid).known_prefix = Some(p);
                }
                ys.lens.insert(gid, (None, p, 0));
                st = ys;
            }
        }
        if let Some((gid, ys)) = gen_yields {
            self.gen_suspension_join(&mut st, &pre_bind, &ys, gid, born_mark);
            // the effects per item: before the first, the objects are as before the call
            if st.reachable {
                let fp = st.footprint(&pre_bind);
                let old = |o: ObjId| self.heap.born_before(o, born_mark) && o != gid;
                let rows: Vec<(ObjId, Ns)> = fp.rows.iter().copied().filter(|(o, _)| old(*o)).collect();
                let slots: Vec<(ObjId, crate::state::SlotKey)> = fp.slots.iter().copied().filter(|(o, _)| old(*o)).collect();
                let elems: Vec<ObjId> = fp.elems.iter().copied().filter(|o| old(*o)).collect();
                let mut lens: Vec<ObjId> = slots.iter().map(|(o, _)| *o).chain(elems.iter().copied()).collect();
                lens.sort_unstable();
                lens.dedup();
                if !rows.is_empty() || !slots.is_empty() || !elems.is_empty() {
                    let at = self.gens.pos_states.remove(&gid).unwrap_or_default();
                    let ge = super::GenEffects { rows, slots, elems, lens, pre: pre_bind.copy(), at, end: finite_end, join: st.copy() };
                    crate::interp::gens::gen_copy_facts(&mut st, &pre_bind, &ge);
                    st.set_slot(gid, Ns::Attr, key_str_lit("\u{1}effpos"), Value::lit(key_str_lit("pre")), true);
                    self.gens.eff.insert(gid, std::rc::Rc::new(ge));
                }
            }
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
        self.active.remove(&key);
        if let Some(lst) = self.active_by_func.get_mut(&foid) {
            lst.pop();
        }
        self.rec_hit.remove(&key);
        self.rec_entry.remove(&key);
        self.rec_widened.remove(&key);
        self.rec_heap.remove(&key);
        self.rec_table.remove(&key);
        self.call_chain = saved_chain;
        self.frames.pop();
        self.env_stack.pop();
        state.env = saved_env;
        state.unbound = saved_unbound;
        state.ret = saved_ret;
        if saved_exits.is_some() {
            state.exits = saved_exits;
        }
        ret
    }

    /// An exception raised here (`cur_exc`, of class `cur_exc_cls`) enters the innermost
    /// running try: its handlers and finally start from this state (with the try frame's
    /// locals when the raise is inside a callee). False: no try is running.
    pub fn record_raise_state(&mut self, state: &mut State) -> bool {
        let exc = self.cur_exc.take().unwrap_or_else(|| Value::data("exception"));
        let cls = self.cur_exc_cls.take().or_else(|| self.exc_value_names(&exc));
        let Some(t) = self.tries.last() else { return false };
        let (fl, el) = (t.frames, t.envs);
        let mut here = state.copy();
        // the try is in a `@contextmanager` generator whose with body is running (also from a
        // callee of the body): the generator's names
        let in_cm = self.gens.cm_frames.iter().rev().find(|(d, _, el0)| *d == fl && el <= *el0).map(|(_, env, _)| env.clone());
        if let Some(env) = in_cm {
            here.env = env;
            here.narrowed = Default::default();
        } else if self.frames.len() < fl {
            if let Some((_, env, _)) = self.gens.cm_frames.iter().rev().find(|(d, _, _)| *d == fl) {
                here.env = env.clone();
                here.narrowed = Default::default();
            }
        } else if self.frames.len() > fl {
            here.env = self.env_stack.get(el).cloned().unwrap_or_else(|| t.pre.env.clone());
            here.narrowed = t.pre.narrowed.clone();
            here.ret = t.pre.ret.clone();
            here.exits = t.pre.exits.clone();
        }
        // (the same state, class and exception as the last raise recorded in this try — a loop
        // of calls raising at one point: nothing to add)
        if let Some((ls, lc, le)) = &t.last {
            if *lc == cls && *le == exc && ls.same(&here) {
                return true;
            }
        }
        self.tries.last_mut().unwrap().last = Some((here.copy(), cls.clone(), exc.clone()));
        // (joined per class: a handler takes every raise of a class or none of them; what
        // callees made is collected when the try ends)
        let recs = &mut self.tries.last_mut().unwrap().recs;
        match recs.iter_mut().find(|r| r.0 == cls) {
            Some(r) => {
                r.1.add(here);
                r.2 = r.2.join(&exc);
            }
            None => {
                let mut j = crate::state::Joins::default();
                j.add(here);
                recs.push((cls, j, exc));
            }
        }
        true
    }

    /// Does some running try's guard catch exceptions of this class?
    pub fn class_guarded(&self, c: &super::ExcNames) -> bool {
        self.guards().any(|g| g.contains("*") || c.0.iter().any(|n| g.contains(n)) || (!c.1 && (g.contains("Exception") || g.contains("BaseException"))))
    }

    /// The class names of an exception value (one class; None: not known).
    pub fn exc_value_names(&self, v: &Value) -> Option<super::ExcNames> {
        if !v.kv.is_bot() || v.pts.is_empty() {
            return None;
        }
        let mut out: Option<super::ExcNames> = None;
        for o in v.pts.iter() {
            let names = if let Some(c) = self.heap.instance_class(o) {
                self.exc_class_names(c)?
            } else if let Some(n) = self.exc_objs.get(&o) {
                super::builtin_exc_names(n)
            } else if self.heap.kind(o) == Kind::External && self.heap.class_info_opt(o).is_none() {
                // (a raised class: `raise KeyError`)
                super::builtin_exc_names(self.heap.external_qual(o).rsplit('.').next().unwrap_or(""))
            } else {
                return None;
            };
            match &out {
                None => out = Some(names),
                Some(prev) if *prev == names => {}
                Some(_) => return None,
            }
        }
        out
    }

    /// The names of a user exception class's MRO, with the builtin bases' names.
    pub fn exc_class_names(&self, cls: u32) -> Option<super::ExcNames> {
        let mro = self.heap.mro(cls);
        let mut names: Vec<String> = Vec::new();
        let mut complete = true;
        for c in mro.iter() {
            let ci = self.heap.class_info_opt(*c)?;
            names.push(ci.name.clone());
            let user: Vec<String> = ci.bases.iter().filter_map(|b| self.heap.class_info_opt(*b).map(|x| x.name.clone())).collect();
            for e in ci.node.arguments.as_ref().map(|a| a.args.iter().collect::<Vec<_>>()).unwrap_or_default() {
                let n = super::dotted_last(e);
                if user.contains(&n) || n == "object" {
                    continue;
                }
                let (bn, full) = super::builtin_exc_names(&n);
                complete &= full;
                for b in bn.iter() {
                    if !names.contains(b) {
                        names.push(b.clone());
                    }
                }
            }
        }
        Some((std::rc::Rc::new(names), complete))
    }

    /// An exception named `name` is raised here (the rest of the path is dead).
    pub fn raise_here(&mut self, name: &str, state: &mut State) {
        self.may_raise_here(name, state);
        state.reachable = false;
    }

    /// An exception named `name` may be raised here: a running try records this state for its
    /// handlers and finally; otherwise the enclosing call raises.
    pub fn may_raise_here(&mut self, name: &str, state: &mut State) {
        let cls = self.cur_exc_cls.take().unwrap_or_else(|| super::builtin_exc_names(name));
        // a builtin exception a model raises, for a handler that may catch it: an object of its
        // class, one per class (its args not known)
        if self.cur_exc.is_none() && self.class_guarded(&cls) && self.exc_node.is_some() {
            let n = name.rsplit('.').next().unwrap_or(name);
            if super::builtin_exc_names(n).1 && super::is_builtin_name(n) {
                let o = self.heap.alloc_raw(Kind::Namespace, &format!("exc:{}", n), "", false, n, Payload::None);
                crate::state::mark_merged(o);
                if !state.has_row(o, Ns::Attr) {
                    state.set_row(o, Ns::Attr, crate::domain::Row::with_source(crate::domain::Source::new(n, crate::domain::SourceKind::Data)));
                }
                state.set_slot(o, Ns::Attr, key_str_lit("args"), Value::data("args"), false);
                self.exc_objs.insert(o, n.to_string());
                self.cur_exc = Some(Value::obj(o));
            }
        }
        self.cur_exc_cls = Some(cls);
        self.record_raise_state(state);
        self.cur_exc_cls = None;
    }

    /// Code that may raise anything ran here (unknown code, a model that does not say what it
    /// raises): a running try records this state.
    pub fn flag_may_raise(&mut self, state: &mut State) {
        self.cur_exc = None;
        self.cur_exc_cls = Some(super::any_exc());
        self.record_raise_state(state);
        self.cur_exc_cls = None;
    }

    /// A raise a try context recorded that leaves it (no handler of it surely catches it): the
    /// enclosing try records it (with no try running, it leaves the program).
    pub fn reraise_rec(&mut self, rec: super::RaiseRec) {
        let (cls, mut st, exc) = rec;
        self.cur_exc = if exc.pts.is_empty() { None } else { Some(exc) };
        self.cur_exc_cls = cls;
        self.record_raise_state(&mut st);
        self.cur_exc = None;
        self.cur_exc_cls = None;
    }

    pub fn in_method_frame(&mut self, foid: u32, self_v: Value, args: Vec<Value>, state: &mut State, f: &mut dyn FnMut(&mut Analyzer, &mut State)) {
        let fi = self.heap.func_info(foid);
        let ctx = self.frame().ctx.clone();
        let mut new_fr = Frame::new(FrameKind::Function, fi.scope, fi.module, &ctx, false, &fi.qualname);
        new_fr.func = Some(foid);
        for (n, c) in fi.closure.iter() {
            new_fr.cells.insert(*n, *c);
        }
        let saved_env = std::mem::take(&mut state.env);
        let saved_unbound = std::mem::take(&mut state.unbound);
        self.env_stack.push(saved_env.clone());
        self.frames.push(new_fr);
        let fake = Node::synthetic(fi.node.node().module, fi.node.node().line);
        self.bind_params(&fi, &Some(self_v), &args, &IndexMap::new(), &[], &[], state, &fake);
        f(self, state);
        self.frames.pop();
        self.env_stack.pop();
        state.env = saved_env;
        state.unbound = saved_unbound;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn bind_params(&mut self, fi: &FuncInfo, self_v: &Option<Value>, args: &[Value], kwargs: &IndexMap<StrId, Value>, star_args: &[Value], star_kwargs: &[Value], state: &mut State, node: &Node) {
        let bound = self.compute_bindings(fi, self_v, args, kwargs, star_args, star_kwargs, state, node);
        for (name, v) in bound {
            self.store_name(name, v, state);
        }
    }

    /// Parameter name -> argument value (allocating the *args tuple / **kwargs dict in `state`).
    #[allow(clippy::too_many_arguments)]
    pub fn compute_bindings(&mut self, fi: &FuncInfo, self_v: &Option<Value>, args: &[Value], kwargs: &IndexMap<StrId, Value>, star_args: &[Value], star_kwargs: &[Value], state: &mut State, node: &Node) -> IndexMap<StrId, Value> {
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
            // `**kwargs` holding the name wins over the default (joined with it when the key
            // is only possibly there)
            let nk = intern::key(intern::Key::Str(intern::str_get(*name)));
            let kw_present = star_kwargs.iter().any(|sk| sk.pts.iter().any(|x| state.row(x, Ns::Item).present.contains(nk)));
            let kw_definite = !star_kwargs.is_empty() && star_kwargs.iter().all(|sk| sk.pts.iter().all(|x| state.row(x, Ns::Item).definite.contains(nk)));
            if let (Some(d), false) = (fi.defaults.get(name), kw_present) {
                bound.insert(*name, d.clone());
            } else if !star_kwargs.is_empty() {
                let mut vals: Vec<Value> = star_kwargs.iter().flat_map(|sk| sk.pts.iter().map(|x| state.slot(x, Ns::Item, nk)).collect::<Vec<_>>()).collect();
                if fi.defaults.get(name).is_none() && !kw_definite {
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
                    v = match fi.defaults.get(name) {
                        Some(d) => v.join(d),
                        None => v.join(&Value::data("kw")),
                    };
                }
                bound.insert(*name, v);
            } else {
                bound.insert(*name, Value::data(&format!("missing arg {}", intern::str_get(*name))));
            }
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
}

impl Analyzer {

}
