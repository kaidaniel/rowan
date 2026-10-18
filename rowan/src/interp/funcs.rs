//! Functions, names and targets: making a function object at its `def` or lambda (closure
//! cells, defaults) and applying its decorators; calling a method a class defines (`run_dunder`,
//! for builtins that call one); reading and binding names (locals, cells, globals, class
//! bodies, builtins); binding targets (attributes, items, unpacking, starred targets).
//! Iterating a value is `iter.rs`.

use std::rc::Rc;

use indexmap::IndexMap;
use ruff_python_ast as ast;

use crate::domain::{kv_literals, BOT, DATA, KeyVal, Ns, ObjId, Value};
use crate::frontend::{self, Node};
use crate::intern::{self, key_int, key_str_lit, s, str_get, KeyId, KeySet, StrId};
use crate::source::SourceKind;
use crate::state::{Kind, State};

use super::iter::Elem;
use super::{Analyzer, Args, FrameKind, FuncInfo, FuncNode, KNOWN_IDENTITY_DECORATORS, Payload, is_builtin_name};

/// The key a closure cell's value is under (`Ns::Cell`).
pub fn cell_key() -> KeyId {
    key_str_lit("v")
}

impl Analyzer {
    // ----------------------------------------------------------- functions
    /// The function object of a `def` or lambda evaluated here: its qualified name, the cells
    /// it closes over (the enclosing frame's, and a running comprehension's variables), its
    /// defaults evaluated now.
    pub(in crate::interp) fn make_function(&mut self, node: FuncNode, state: &mut State, name: Option<&str>) -> Value {
        let fr_module = self.frame().module;
        let fr_kind = self.frame().kind;
        let fr_label = self.frame().label.clone();
        let fr_cells = self.frame().cells.clone();
        let fr_class = self.frame().class_oid;
        let mi = self.heap.module_info(fr_module);
        // (a node the scope analysis did not collect, which should not happen: the module's scope)
        let scope = mi.scopes.of_node(node.addr()).unwrap_or_else(|| mi.scopes.module());
        let fname = match name {
            Some(n) => n.to_string(),
            None => match node {
                FuncNode::Def(f) => f.name.id.to_string(),
                FuncNode::Lambda(_) => "<lambda>".to_string(),
            },
        };
        let qual = if fr_kind == FrameKind::Module { fname.clone() } else { format!("{}.{}", fr_label, fname) };
        let free: Vec<String> = mi.scopes.get(scope).free.iter().cloned().collect();
        let mut closure: IndexMap<StrId, u32> = IndexMap::new();
        for n in free {
            let sid = s(&n);
            if let Some(&cid) = fr_cells.get(&sid) {
                closure.insert(sid, cid);
            }
        }
        // a function made inside a comprehension closes over the comprehension's variables
        // (at module level they are not "free" in the scope analysis: look at the uses)
        if !self.cx.comp_cells.is_empty() {
            let sc = mi.scopes.get(scope);
            for n in sc.uses.iter().filter(|n| !sc.is_local(n)) {
                let sid = s(n);
                if closure.contains_key(&sid) {
                    continue;
                }
                if let Some(cid) = self.cx.comp_cells.iter().rev().find_map(|m| m.get(&sid).copied()) {
                    closure.insert(sid, cid);
                }
            }
        }
        let mut defaults: IndexMap<StrId, Value> = IndexMap::new();
        if let Some(params) = node.params() {
            for p in params.posonlyargs.iter().chain(params.args.iter()).chain(params.kwonlyargs.iter()) {
                if let Some(d) = &p.default {
                    let v = self.eval(d, state);
                    defaults.insert(s(p.parameter.name.id.as_str()), v);
                }
            }
        }
        let info = FuncInfo {
            node,
            name: fname,
            qualname: qual.clone(),
            module: fr_module,
            scope,
            closure,
            defaults,
            cls: if fr_kind == FrameKind::Class { fr_class } else { None },
            is_property: std::cell::Cell::new(false),
            is_static: std::cell::Cell::new(false),
            is_classmethod: std::cell::Cell::new(false),
            is_contextmanager: std::cell::Cell::new(false),
            is_cached: std::cell::Cell::new(false),
            is_memo: std::cell::Cell::new(false),
        };
        let n = node.node();
        let has_closure = !info.closure.is_empty();
        let oid = self.alloc_at(Kind::Function, &n, state, &format!("def {}", qual), Payload::Func(Rc::new(info)), None, Ns::Item, None, None);
        // a closure made inside a concrete call (not an entry run): its own analysis with
        // open parameters comes from the closure the factory's entry run makes
        let creator = self.cx.frames.iter().rev().find_map(|f| f.func);
        if self.m.torch_on && has_closure && creator.is_some() && self.entry.run != creator {
            self.entry.concrete_closures.insert(oid);
        }
        if self.m.torch_on {
            self.m.t_funcs_by_qual.entry(qual.clone()).or_default().push(oid);
        }
        Value::obj(oid)
    }

    /// What a decorator expression is, for `apply_decorators`: a library function's qualified
    /// name, `user:F` for the program's function, `class:C` for its class, a FastAPI or
    /// singledispatch method's name; None otherwise.
    pub fn decorator_qualname(&mut self, d: &'static ast::Expr, state: &mut State) -> Option<String> {
        let target = match d {
            ast::Expr::Call(c) => &*c.func,
            other => other,
        };
        let v = self.eval(target, state);
        for oid in v.pts.iter() {
            match self.heap.kind(oid) {
                Kind::External => return Some(self.heap.external_qual(oid)),
                Kind::Function => return Some(format!("user:{}", self.heap.func_info(oid).qualname)),
                Kind::Class => return Some(format!("class:{}", self.heap.class_info(oid).name)),
                Kind::Method => {
                    if let Payload::Method(q, _) = &self.heap.obj(oid).payload {
                        if q.starts_with("fastapi.") || q.starts_with("sdispatch.") {
                            return Some(q.clone());
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// The decorators of a `def`, innermost first: `property`, `staticmethod`, `classmethod`,
    /// `contextmanager` and setters mark the function; functools and FastAPI decorators go to
    /// their models; the program's own decorators are called with the function; known identity
    /// decorators change nothing; any other is noted and taken to return the function.
    pub(in crate::interp) fn apply_decorators(&mut self, decorators: &'static [ast::Decorator], fnode: Node, mut fv: Value, state: &mut State) -> Value {
        for d in decorators.iter().rev() {
            let q = self.decorator_qualname(&d.expression, state).unwrap_or_default();
            let fobj = fv.pts.first();
            let fi = fobj.and_then(|o| self.heap.func_info_opt(o));
            if q.ends_with("property") || q == "builtins.property" || q.ends_with("computed_field") {
                if let Some(fi) = &fi {
                    fi.is_property.set(true);
                    if q.ends_with("cached_property") {
                        fi.is_cached.set(true);
                    }
                }
                continue;
            }
            if q.ends_with("staticmethod") {
                if let Some(fi) = &fi {
                    fi.is_static.set(true);
                }
                continue;
            }
            if q.ends_with("contextmanager") {
                if let Some(fi) = &fi {
                    fi.is_contextmanager.set(true);
                    continue;
                }
            }
            if q.ends_with("classmethod") {
                if let Some(fi) = &fi {
                    fi.is_classmethod.set(true);
                }
                continue;
            }
            let is_setter = q.ends_with(".setter") || matches!(&d.expression, ast::Expr::Attribute(a) if a.attr.id.as_str() == "setter");
            if is_setter {
                // `@name.setter`: remember the setter, keep the getter bound to the name
                if let (Some(cls), Some(f)) = (self.frame().class_oid, fobj) {
                    if let ast::Expr::Attribute(a) = &d.expression {
                        if let ast::Expr::Name(n) = &*a.value {
                            let nk = key_str_lit(n.id.as_str());
                            self.heap.prop_setters.insert((cls, nk), f);
                            if let Some(getter) = self.heap.cls_slots.get(&(cls, Ns::Attr, nk)).cloned() {
                                if !getter.pts.is_empty() {
                                    fv = getter;
                                }
                            }
                        }
                    }
                }
                continue;
            }
            // functools: singledispatch (and its `register`), lru_cache / cache, wraps
            if crate::models::singledispatch::functools_decorator(self, &q, d, fobj, fi.as_deref(), state) {
                continue;
            }
            if q.starts_with("fastapi.") {
                // `@app.get("/x")`: the route decorator registers the handler
                let dv = self.eval(&d.expression, state);
                crate::models::fastapi::apply_route(self, &dv, &fv);
                continue;
            }
            // a decorator that is user code (a bound method of a user object, a function held in
            // a variable): applied like one (`@registry.register("csv")` fills the registry)
            let user_target = !q.starts_with("user:") && {
                let target: &'static ast::Expr = match &d.expression {
                    ast::Expr::Call(c) => &c.func,
                    other => other,
                };
                let mut probe = state.copy();
                let tv = self.eval(target, &mut probe);
                !tv.pts.is_empty() && tv.pts.iter().all(|o| matches!(self.heap.kind(o), Kind::Function | Kind::Bound))
            };
            if !user_target && (KNOWN_IDENTITY_DECORATORS.contains(&q.as_str()) || q.ends_with(".getter") || q.ends_with(".deleter") || q.ends_with(".register")) {
                continue;
            }
            if q.starts_with("user:") || user_target {
                let (target, call_ast) = match &d.expression {
                    ast::Expr::Call(c) => (&*c.func, Some(c)),
                    other => (other, None),
                };
                let mut dv = self.eval(target, state);
                if let Some(c) = call_ast {
                    dv = self.call_value(&dv, &frontend::node(&d.expression), Some(c), state, None); // decorator factory
                }
                // the decorator application's call site: the decorator itself with `fixes`
                // (stacked decorators applied at the def's site shared one clone context)
                let at = frontend::node(d);
                fv = self.call_value(&dv, &at, None, state, Some(vec![fv.clone()]));
                continue;
            }
            let txt = if q.is_empty() { frontend::text(&d.expression) } else { q.clone() };
            self.findings.note(format!("line {}: decorator `{}` is not modelled (taken to return the function unchanged)", fnode.line, txt));
        }
        fv
    }

    /// `call_method_with` and no arguments.
    pub fn call_method_if_defined(&mut self, v: &Value, name: &str, node: &Node, state: &mut State) -> Option<Value> {
        self.call_method_with(v, name, Vec::new(), node, state)
    }

    /// `call_method_if_defined` with positional arguments: on the instances among `v` whose
    /// class defines `name`, one alternative each (None: none does).
    pub fn call_method_with(&mut self, v: &Value, name: &str, args: Vec<Value>, node: &Node, state: &mut State) -> Option<Value> {
        let calls = self.method_calls(v, &[name]).0;
        if calls.is_empty() {
            return None;
        }
        Some(self.alternatives(calls.len(), false, state, &mut |an, i, st| an.call_user(calls[i].1, Some(Value::obj(calls[i].0)), Args::pos(&args), st, node, false)))
    }

    /// The (receiver, function) pairs of the instances among `v` whose class defines one of
    /// `names` (the first defined, in order), and the rest of `v`.
    fn method_calls(&self, v: &Value, names: &[&str]) -> (Vec<(ObjId, ObjId)>, Value) {
        let mut calls = Vec::new();
        let mut rest = Value::kv_only(v.kv.clone());
        let mut others = Vec::new();
        for o in v.pts.iter() {
            let cls = self.heap.instance_class(o).filter(|_| self.heap.kind(o) == Kind::Instance);
            let found = cls.and_then(|c| names.iter().find_map(|n| Some((self.class_lookup(c, n)?, *n))));
            match found {
                Some((m, n)) => {
                    let fv = self.heap.class_attr(m, n);
                    calls.extend(fv.pts.iter().filter(|f| self.heap.kind(*f) == Kind::Function).map(|f| (o, f)));
                }
                None => others.push(o),
            }
        }
        rest.pts = crate::intern::IdSet::from_vec(others);
        (calls, rest)
    }

    /// A builtin Python carries out by a dunder of its argument (`str(x)` calls
    /// `x.__str__()`): on the instances among `v` whose class defines one of `names` (the
    /// first defined, in order) that method runs with `args`, one alternative each (and one
    /// where none runs, for the rest of `v`); those that would not run, when several are, one
    /// call that does not run (each function with its receivers, as `budget_skipped` makes for
    /// a method of several receivers: `f"{obj}"` of any of a thousand instances). Returns the
    /// results joined and the rest of `v`; None when no instance defines one.
    pub fn run_dunder(&mut self, v: &Value, names: &[&str], args: Vec<Value>, node: &Node, state: &mut State) -> Option<(Value, Value)> {
        let (calls, rest) = self.method_calls(v, names);
        if calls.is_empty() {
            return None;
        }
        let keep = !rest.empty();
        let mut done: Vec<usize> = Vec::new();
        let r = self.alternatives(calls.len(), keep, state, &mut |an, i, st| {
            if done.contains(&i) {
                st.reachable = false;
                return Value::new();
            }
            // (the rest that would not run now, when several: one call for them)
            let past = an.budget.past_hover_work(an.cx.frames.len(), an.hover_work()) || an.budget.over_hard_deadline();
            let skip: Vec<usize> = (i..calls.len()).filter(|j| past || an.out_of_budget(calls[*j].1)).collect();
            if skip.len() > 1 && skip[0] == i {
                let mut fns: Vec<(ObjId, Option<Value>)> = Vec::new();
                for &(o, f) in skip.iter().map(|j| &calls[*j]) {
                    match fns.iter_mut().find(|(g, _)| *g == f) {
                        Some((_, r)) => *r = Some(r.take().unwrap_or_default().join(&Value::obj(o))),
                        None => fns.push((f, Some(Value::obj(o)))),
                    }
                }
                done = skip;
                return an.skipped_calls(&fns, Args::pos(&args), st, node, None);
            }
            an.call_user(calls[i].1, Some(Value::obj(calls[i].0)), Args::pos(&args), st, node, false)
        });
        Some((r, rest))
    }

    // ------------------------------------------------------------- names
    /// A read of a name bound on some paths only: possibly a NameError / UnboundLocalError.
    /// After it the program went on, so the name is bound on this path.
    fn unbound_read(&mut self, name: StrId, module: ObjId, node: &Node, state: &mut State, exc: &str) {
        if node.line == 0 {
            return;
        }
        let key = intern::key_of_str(name);
        let cls = crate::domain::Classification::with_note(crate::domain::Tier::Possibly, "is only assigned on some paths");
        self.verdict(node, key, &cls, module, Ns::Name, None, None);
        self.may_raise_here(exc, state);
        if state.unbound.contains(&name) {
            state.unbound = std::sync::Arc::new(state.unbound.iter().copied().filter(|n| *n != name).collect());
        }
    }

    /// The value of a name here: a comprehension's variable, a local or cell of the running
    /// function (an unbound one raises), a class body's name, the module's global (one bound on
    /// some paths only may raise NameError), `__name__`, a builtin, else an unknown.
    pub fn load_name(&mut self, name: StrId, node: &Node, state: &mut State) -> Value {
        if self.frame().comp_names.iter().any(|ns| ns.contains(&name)) {
            return state.env.get(&name).cloned().unwrap_or_default();
        }
        let fr_kind = self.frame().kind;
        let fr_module = self.frame().module;
        if fr_kind == FrameKind::Function {
            if let Some(&cid) = self.frame().cells.get(&name) {
                let v = state.slot(cid, Ns::Cell, cell_key());
                return v;
            }
            let scope = self.frame().scope;
            let mi = self.heap.module_info(fr_module);
            if mi.scopes.get(scope).is_local(&str_get(name)) {
                if let Some(v) = state.env.get(&name).cloned() {
                    if state.unbound.contains(&name) {
                        self.unbound_read(name, fr_module, node, state, "UnboundLocalError");
                    }
                    return v;
                }
                if node.line > 0 {
                    let key = intern::key_of_str(name);
                    let cls = crate::domain::Classification::with_note(crate::domain::Tier::Missing, "is not assigned on any path to here");
                    self.verdict(node, key, &cls, fr_module, Ns::Name, None, None);
                    self.raise_here("UnboundLocalError", state);
                }
                return Value::data(&format!("unbound {}", str_get(name)));
            }
        }
        if fr_kind == FrameKind::Class {
            let cls = self.frame().class_oid.unwrap();
            if let Some(v) = state.slot_opt(cls, Ns::Attr, intern::key_of_str(name)) {
                return v;
            }
            if let Some(&cid) = self.frame().cells.get(&name) {
                return state.slot(cid, Ns::Cell, cell_key());
            }
        }
        if let Some(v) = state.slot_opt(fr_module, Ns::Attr, intern::key_of_str(name)) {
            if fr_kind == FrameKind::Module {
                let k = intern::key_of_str(name);
                let row = state.row(fr_module, Ns::Attr);
                if row.present.contains(k) && !row.definite.contains(k) && row.mutators.is_empty() && !row.top && row.sources.is_empty() {
                    self.unbound_read(name, fr_module, node, state, "NameError");
                    state.set_row_narrowed(fr_module, Ns::Attr, row.narrow_in(k, true));
                }
            }
            return v;
        }
        let nm = str_get(name);
        // (the module's own name: `__main__` for the program's entry)
        if &*nm == "__name__" {
            let own = if fr_module == self.program.main_module { "__main__".to_string() } else { self.heap.module_info(fr_module).name.clone() };
            return Value::lit_str(&own);
        }
        if is_builtin_name(&nm) {
            let oid = self.heap.alloc_raw(Kind::External, &format!("ext:builtins.{}", nm), "", true, &nm, Payload::Str(format!("builtins.{}", nm)));
            return Value::obj(oid);
        }
        let oid = self.unknown_source(&format!("name {}", nm), SourceKind::Name, state, &format!("?{}", nm));
        Value::obj(oid)
    }


    /// A narrowed value for a name (a test on it): its value changes, not whether it is bound.
    pub(in crate::interp) fn rebind_name(&mut self, name: StrId, v: Value, state: &mut State) {
        let module = self.frame().module;
        let at_module = self.frame().kind == FrameKind::Module;
        let row = if at_module { Some(state.row(module, Ns::Attr)) } else { None };
        let unbound = state.unbound.clone();
        // (keys stored through the name hold on whichever object it is: narrowed to one dict,
        // they are that dict's — `t is a` after `t[k] = …` with t one of a, b)
        let keys: Vec<KeyId> = state.name_keys.iter().filter(|(n, _)| *n == name).map(|(_, k)| *k).collect();
        let one = v.single().filter(|o| self.heap.kind(*o) == Kind::Dict && v.kv.is_bot());
        self.store_name(name, v, state);
        if let Some(r) = row {
            state.set_row(module, Ns::Attr, r);
        }
        state.unbound = unbound;
        for k in keys {
            state.add_name_key(name, k);
            if let Some(o) = one {
                let r = state.row(o, Ns::Item);
                if !r.definite.contains(k) {
                    state.set_row_narrowed(o, Ns::Item, r.narrow_in(k, true));
                }
            }
        }
    }

    /// Binds a name in the running frame: a comprehension's variable, a cell, a `global`, a
    /// local, a class attribute (also the class's flow-insensitive slot), or a module global.
    /// Facts about the name's old value (keys stored through it, tests on it) go.
    pub(in crate::interp) fn store_name(&mut self, name: StrId, v: Value, state: &mut State) {
        state.drop_name_keys(Some(name));
        // a fact about a test reading this name no longer holds once it is rebound
        if !state.facts.is_empty() && state.facts.iter().any(|f| f.deps.iter().any(|(n, _)| *n == name)) {
            let kept: Vec<_> = state.facts.iter().filter(|f| !f.deps.iter().any(|(n, _)| *n == name)).cloned().collect();
            state.facts = std::sync::Arc::new(kept);
        }
        if !state.narrowed.is_empty() {
            let nm = str_get(name);
            let prefix = format!("{}.", nm);
            state.narrowed.retain(|k, _| {
                let ks = str_get(*k);
                !(*ks == *nm || ks.starts_with(&prefix))
            });
        }
        let fr_kind = self.frame().kind;
        let fr_module = self.frame().module;
        let key = intern::key_of_str(name);
        if self.frame().comp_names.iter().any(|ns| ns.contains(&name)) {
            state.env.insert(name, v);
            return;
        }
        if fr_kind == FrameKind::Function {
            if let Some(&cid) = self.frame().cells.get(&name) {
                let strong = self.heap.obj(cid).single;
                state.set_slot(cid, Ns::Cell, cell_key(), v, strong);
                return;
            }
            let scope = self.frame().scope;
            let mi = self.heap.module_info(fr_module);
            if mi.scopes.get(scope).globals_decl.contains(&*str_get(name)) {
                self.gens.global_held.extend(v.pts.iter().filter(|o| self.gens.lazy.contains_key(o)));
                state.set_slot(fr_module, Ns::Attr, key, v, true);
                return;
            }
            if state.unbound.contains(&name) {
                state.unbound = std::sync::Arc::new(state.unbound.iter().copied().filter(|n| *n != name).collect());
            }
            state.env.insert(name, v);
            return;
        }
        if fr_kind == FrameKind::Class {
            let cls = self.frame().class_oid.unwrap();
            state.set_slot(cls, Ns::Attr, key, v.clone(), true);
            let r = state.row(cls, Ns::Attr).store(key, true);
            state.set_row(cls, Ns::Attr, r);
            let k = (cls, Ns::Attr, key);
            let nv = match self.heap.cls_slots.get(&k) {
                Some(cur) => cur.join(&v),
                None => v,
            };
            self.heap.set_cls_slot(k, nv);
            return;
        }
        // a module-level binding inside a loop is strong under the recency abstraction (the
        // flow-sensitive state is re-bound every iteration anyway)
        state.set_slot(fr_module, Ns::Attr, key, v, true);
        let r = state.row(fr_module, Ns::Attr).store(key, true);
        state.set_row(fr_module, Ns::Attr, r);
    }

    // ------------------------------------------------------------ targets
    /// Assigns `v` to an assignment target: a name (with the torch model's names for dims and
    /// the hover facts), an attribute, an item, a tuple or list of targets (unpacking), a
    /// starred target.
    pub(in crate::interp) fn bind_target(&mut self, t: &'static ast::Expr, v: &Value, state: &mut State) {
        match t {
            ast::Expr::Name(n) => {
                let name = s(n.id.as_str());
                let named = if self.m.torch_on { crate::models::torch::t_name_number(self, name, v).or_else(|| crate::models::torch::t_name_tensor(self, name, v)) } else { None };
                let v = named.as_ref().unwrap_or(v);
                if self.m.torch_on {
                    crate::models::torch::t_name_result(self, name, v);
                }
                self.store_name(name, v.clone(), state);
                if self.findings.facts_on {
                    let nd = frontend::node(t);
                    self.findings.fact_write = true;
                    self.record_fact(&nd, name, v, state);
                    self.findings.fact_write = false;
                }
            }
            ast::Expr::Attribute(a) => {
                let obj = self.eval(&a.value, state);
                let n = frontend::node(t);
                self.store_attr(&obj, a.attr.id.as_str(), v, &n, state);
            }
            ast::Expr::Subscript(sub) => {
                let obj = self.eval(&sub.value, state);
                let key = self.eval_key(&sub.slice, state);
                let n = frontend::node(t);
                self.store_item(&obj, Ns::Item, &key, v, &n, state);
                // `t[k] = v`: whichever object `t` is has k now
                if let (ast::Expr::Name(nm), Some(k)) = (&*sub.value, key.kv.as_lit().filter(|_| key.pts.is_empty())) {
                    if obj.pts.len() > 1 && obj.pts.iter().all(|o| self.heap.kind(o) == Kind::Dict) {
                        state.add_name_key(s(nm.id.as_str()), k);
                    }
                }
            }
            ast::Expr::Tuple(ast::ExprTuple { elts, .. }) | ast::Expr::List(ast::ExprList { elts, .. })
                if self.m.torch_on && {
                    let star = elts.iter().position(|e| matches!(e, ast::Expr::Starred(_)));
                    let n = frontend::node(t);
                    match crate::models::torch::t_unpack(self, v, elts.len(), star, &n, state) {
                        Some(parts) => {
                            for (sub, pv) in elts.iter().zip(parts.iter()) {
                                self.bind_target(sub, pv, state);
                            }
                            true
                        }
                        None => false,
                    }
                } => {}
            ast::Expr::Tuple(ast::ExprTuple { elts, .. }) | ast::Expr::List(ast::ExprList { elts, .. })
                if elts.iter().any(|e| matches!(e, ast::Expr::Starred(_))) =>
            {
                self.bind_starred(elts, v, state, &frontend::node(t));
            }
            ast::Expr::Tuple(ast::ExprTuple { elts, .. }) | ast::Expr::List(ast::ExprList { elts, .. }) => {
                let n = frontend::node(t);
                self.check_unpack_len(v, elts.len(), state);
                let parts = self.unpack(v, elts.len(), state, &n);
                for (sub, pv) in elts.iter().zip(parts.iter()) {
                    if let ast::Expr::Starred(st) = sub {
                        self.bind_target(&st.value, pv, state);
                    } else {
                        self.bind_target(sub, pv, state);
                    }
                }
            }
            ast::Expr::Starred(st) => self.bind_target(&st.value, v, state),
            _ => {}
        }
    }

    /// `a, *mid, z = v`: the targets before the star take the leading positions, those after
    /// it the trailing ones, and the starred target a fresh list of the middle. Without a known
    /// length the leading targets keep their positions and the rest take any element.
    fn bind_starred(&mut self, elts: &'static [ast::Expr], v: &Value, state: &mut State, n: &Node) {
        let si = elts.iter().position(|e| matches!(e, ast::Expr::Starred(_))).unwrap_or(0);
        let after = elts.len() - si - 1;
        let known = v.single().filter(|o| matches!(self.heap.kind(*o), Kind::List | Kind::Tuple)).and_then(|o| self.heap.llen(o, state).filter(|l| *l >= si + after).map(|l| (o, l)));
        let (lead, mid, trail): (Vec<Value>, Value, Vec<Value>) = if let Some((oid, len)) = known {
            let at = |i: usize, state: &State| state.slot(oid, Ns::Pos, key_int(i as i64));
            let lead = (0..si).map(|i| at(i, state)).collect();
            let trail = (len - after..len).map(|i| at(i, state)).collect();
            let vals: Vec<Value> = (si..len - after).map(|i| at(i, state)).collect();
            let kv = state.elems.get(&oid).unwrap_or(DATA);
            (lead, self.alloc_positional(Kind::List, n, Some(vals), Value::new(), kv, state), trail)
        } else {
            let lead = self.unpack(v, si, state, n);
            let e = self.iterate(v, n, state).elem.flat();
            let mid = self.alloc_positional(Kind::List, n, None, e.clone(), DATA, state);
            (lead, mid, (0..after).map(|_| e.clone()).collect())
        };
        for (sub, pv) in elts[..si].iter().zip(lead.iter()) {
            self.bind_target(sub, pv, state);
        }
        self.bind_target(&elts[si], &mid, state);
        for (sub, pv) in elts[si + 1..].iter().zip(trail.iter()) {
            self.bind_target(sub, pv, state);
        }
    }

    /// What of `v` Python can iterate: its literals that are numbers, None, … raise TypeError
    /// (a `for`, a comprehension, unpacking, `list(v)`); with nothing else, the path ends here.
    pub fn iterable_part(&mut self, v: &Value, state: &mut State) -> Value {
        let ks = kv_literals(&v.kv);
        let not_iterable = |k: KeyId| intern::with_key(k, |k| matches!(k, intern::Key::Int(_) | intern::Key::Float(_) | intern::Key::Bool(_) | intern::Key::None | intern::Key::Ellipsis | intern::Key::Dec(..) | intern::Key::Frac(..)));
        if !v.kv.is_literal() || !ks.iter().any(not_iterable) {
            return v.clone();
        }
        let rest: intern::KeySet = ks.iter().filter(|k| !not_iterable(*k)).collect();
        let out = Value::with_kv(v.pts.clone(), if rest.is_empty() { BOT } else { KeyVal::lits(rest) });
        if out.empty() {
            self.raise_here("TypeError", state);
        } else {
            self.may_raise_here("TypeError", state);
        }
        out
    }

    /// Unpacking into `n` names raises ValueError for a sequence of another known length (a
    /// list, tuple or generator's items, a known string): on every path when each part does,
    /// on some when a part's length is not known.
    fn check_unpack_len(&mut self, v: &Value, n: usize, state: &mut State) {
        self.force_gen(v, state, false);
        let mut parts: Vec<Option<bool>> = v.pts.iter().map(|o| match (self.heap.kind(o), self.heap.llen(o, state)) {
            (Kind::List | Kind::Tuple, Some(l)) => Some(l != n),
            // (a dict's item, an enumerate's or zip's of two)
            (Kind::Pair, _) => Some(n != 2),
            _ => None,
        }).collect();
        if !v.kv.is_bot() {
            let ks = kv_literals(&v.kv);
            if v.kv.is_literal() && ks.iter().all(intern::key_is_str) {
                parts.extend(ks.iter().map(|k| Some(intern::key_as_str(k).is_some_and(|t| t.chars().count() != n))));
            } else {
                parts.push(None);
            }
        }
        // (of a length not known: it may raise)
        if parts.iter().all(|p| *p == Some(true)) && !parts.is_empty() {
            self.raise_here("ValueError", state);
        } else if parts.iter().any(|p| *p != Some(false)) {
            self.may_raise_here("ValueError", state);
        }
    }

    /// The `n` values unpacking `v` gives (`a, b = v`): positions of a list or tuple of known
    /// length, a NamedTuple's fields, a pair's parts, the known items of a `zip` / `enumerate`,
    /// the characters of strings of length `n`, the parts of literal tuples; else each any
    /// element.
    pub fn unpack(&mut self, v: &Value, n: usize, state: &mut State, node: &Node) -> Vec<Value> {
        let v = &self.iterable_part(v, state);
        self.force_gen(v, state, false);
        let mut parts: Vec<Value> = (0..n).map(|_| Value::new()).collect();
        for oid in v.pts.iter() {
            let kind = self.heap.kind(oid);
            let ll = self.heap.llen(oid, state);
            if matches!(kind, Kind::Tuple | Kind::List) && ll.is_some() {
                for i in 0..n {
                    let sv = state.slot(oid, Ns::Pos, key_int(i as i64));
                    parts[i] = parts[i].join(&sv);
                }
            } else if let Some(cls) = self.heap.instance_class(oid).filter(|c| kind == Kind::Instance && crate::models::dataclasses::is_named_tuple_class(self, *c)) {
                // a NamedTuple: its fields in order
                let fs = crate::models::dataclasses::dataclass_fields_mro(self, cls);
                for i in 0..n {
                    let v = match fs.get(i) {
                        Some((f, _)) => state.slot(oid, Ns::Attr, intern::key_str_lit(f)),
                        None => Value::data(&format!("unpack@L{}", node.line)),
                    };
                    parts[i] = parts[i].join(&v);
                }
            } else if kind == Kind::Pair {
                if let Payload::Pair(k, val) = self.heap.obj(oid).payload.clone() {
                    if n >= 1 {
                        parts[0] = parts[0].join(&k);
                    }
                    if n >= 2 {
                        parts[1] = parts[1].join(&val);
                    }
                }
            } else {
                // an iterable whose items are known by position (zip / enumerate of known lists)
                if matches!(kind, Kind::Zip | Kind::Enumerate) {
                    let it = self.iterate(&Value::obj(oid), node, state);
                    if let Some(items) = it.items.filter(|v| v.len() == n) {
                        for i in 0..n {
                            parts[i] = parts[i].join(&items[i]);
                        }
                        continue;
                    }
                }
                let e = self.elem_of(&Value::obj(oid), node, state);
                for i in 0..n {
                    parts[i] = parts[i].join(&e);
                }
            }
        }
        // known strings of n characters: their characters (`k, v = "ab"`)
        if v.pts.is_empty() && v.kv.is_literal() {
            let lits = kv_literals(&v.kv);
            let strs: Option<Vec<Vec<char>>> = lits.iter().map(|k| intern::key_as_str(k).map(|s| s.chars().collect::<Vec<char>>()).filter(|cs| cs.len() == n)).collect();
            if let Some(strs) = strs.filter(|s| !s.is_empty()) {
                let mut out: Vec<Value> = (0..n).map(|_| Value::new()).collect();
                for cs in &strs {
                    for i in 0..n {
                        out[i] = out[i].join(&Value::lit(intern::key_str_lit(&cs[i].to_string())));
                    }
                }
                return out;
            }
        }
        if v.pts.is_empty() {
            // positional slots for literal tuple keys: `a, b = ("x", 1)` / `for k, ns in keys`
            // where the keys are tuple literals
            if let Some(cols) = tuple_key_columns(&v.kv, n) {
                return cols;
            }
            parts = (0..n).map(|_| Value::data(&format!("unpack@L{}", node.line))).collect();
        }
        parts
    }

    /// Binds a loop's target to an iteration's element (a pair to two targets part by part, or
    /// as a pair object to one).
    pub(in crate::interp) fn bind_iter_target(&mut self, target: &'static ast::Expr, elem: &Elem, state: &mut State) {
        match elem {
            Elem::Pair(k, val) => {
                let two = match target {
                    ast::Expr::Tuple(t) if t.elts.len() == 2 => Some(&t.elts),
                    ast::Expr::List(l) if l.elts.len() == 2 => Some(&l.elts),
                    _ => None,
                };
                if let Some(elts) = two {
                    self.bind_target(&elts[0], k, state);
                    self.bind_target(&elts[1], val, state);
                } else {
                    let addr = target as *const ast::Expr as usize;
                    let pid = self.heap.alloc_raw(Kind::Pair, &format!("pair:{}", addr), "", true, "pair", Payload::Pair(k.clone(), val.clone()));
                    self.heap.obj_mut(pid).payload = Payload::Pair(k.clone(), val.clone());
                    self.bind_target(target, &Value::obj(pid), state);
                }
            }
            Elem::One(v) => self.bind_target(target, v, state),
        }
    }
}

/// `n` per-position key values of a (set of) literal tuple key(s) of arity `n`, or None.
fn tuple_key_columns(kv: &KeyVal, n: usize) -> Option<Vec<Value>> {
    let tuples: Vec<Vec<KeyId>> = match kv {
        KeyVal::Lit(k) => vec![intern::with_key(*k, |kk| if let intern::Key::Tuple(t) = kk { Some(t.clone()) } else { None })?],
        KeyVal::Lits(ks) => {
            let mut out = Vec::new();
            for k in ks.iter() {
                out.push(intern::with_key(k, |kk| if let intern::Key::Tuple(t) = kk { Some(t.clone()) } else { None })?);
            }
            out
        }
        _ => return None,
    };
    if tuples.is_empty() || tuples.iter().any(|t| t.len() != n) {
        return None;
    }
    let mut cols: Vec<Value> = Vec::new();
    for i in 0..n {
        let ks: KeySet = KeySet::from_iter(tuples.iter().map(|t| t[i]));
        cols.push(Value::kv_only(KeyVal::lits(ks)));
    }
    Some(cols)
}

