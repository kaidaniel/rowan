//! Functions, names, targets, iteration (port of the middle part of `rowan/interp.py`).

use std::rc::Rc;

use indexmap::IndexMap;
use ruff_python_ast as ast;

use crate::domain::{join_values, KeyVal, KeysOf, Ns, ObjId, ObjSet, Row, SourceKind, Value, DATA};
use crate::frontend::{self, Node};
use crate::intern::{self, key_int, key_str_lit, s, str_get, KeyId, KeySet, StrId, STAR};
use crate::state::{Kind, State};

use super::{is_builtin_name, Analyzer, FrameKind, FuncInfo, FuncNode, Payload, ViewKind, KNOWN_IDENTITY_DECORATORS, PATH_DEPTH, UNROLL_MAX};

/// The abstract element of an iteration: a value, or a (key, value) pair.
#[derive(Clone, Debug)]
pub enum Elem {
    One(Value),
    Pair(Value, Value),
}

impl Elem {
    pub fn key_part(&self) -> Value {
        match self {
            Elem::One(v) => v.clone(),
            Elem::Pair(k, _) => k.clone(),
        }
    }
    pub fn value_part(&self) -> Value {
        match self {
            Elem::One(v) => v.clone(),
            Elem::Pair(_, v) => v.clone(),
        }
    }
    /// Both parts joined (what a pair looks like when forced into one value).
    pub fn flat(&self) -> Value {
        match self {
            Elem::One(v) => v.clone(),
            Elem::Pair(k, v) => k.join(v),
        }
    }
}

pub struct Iteration {
    pub items: Option<Vec<Value>>,
    pub elem: Elem,
    pub exhaustive: bool,
}

pub fn cell_key() -> KeyId {
    key_str_lit("v")
}

impl Analyzer {
    // ----------------------------------------------------------- functions
    pub fn make_function(&mut self, node: FuncNode, state: &mut State, name: Option<&str>) -> Value {
        let fr_module = self.frame().module;
        let fr_kind = self.frame().kind;
        let fr_label = self.frame().label.clone();
        let fr_cells = self.frame().cells.clone();
        let fr_class = self.frame().class_oid;
        let mi = self.module_info(fr_module);
        let scope = mi.scopes.of_node(node.addr());
        let scope = match scope {
            Some(sc) => sc,
            None => {
                // not collected (should not happen): a detached scope
                let mut sc = crate::scopes::Scopes::default();
                sc.add_detached(crate::scopes::ScopeKind::Function);
                mi.scopes.module()
            }
        };
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
        if !self.comp_cells.is_empty() {
            let sc = mi.scopes.get(scope);
            for n in sc.uses.iter().filter(|n| !sc.is_local(n)) {
                let sid = s(n);
                if closure.contains_key(&sid) {
                    continue;
                }
                if let Some(cid) = self.comp_cells.iter().rev().find_map(|m| m.get(&sid).copied()) {
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
            generator: std::cell::Cell::new(None),
        };
        let n = node.node();
        let has_closure = !info.closure.is_empty();
        let oid = self.alloc_at(Kind::Function, &n, state, &format!("def {}", qual), Payload::Func(Rc::new(info)), None, Ns::Item, None, None);
        // a closure made inside a concrete call (not an entry run): its own analysis with
        // open parameters comes from the closure the factory's entry run makes
        let creator = self.frames.iter().rev().find_map(|f| f.func);
        if self.torch_on && has_closure && creator.is_some() && self.entry_run != creator {
            self.t_concrete_closures.insert(oid);
        }
        if self.torch_on {
            self.m.t_funcs_by_qual.entry(qual.clone()).or_default().push(oid);
        }
        Value::obj(oid)
    }

    pub fn decorator_qualname(&mut self, d: &'static ast::Expr, state: &mut State) -> Option<String> {
        let target = match d {
            ast::Expr::Call(c) => &*c.func,
            other => other,
        };
        let v = self.eval(target, state);
        for oid in v.pts.iter() {
            match self.kind(oid) {
                Kind::External => return Some(self.external_qual(oid)),
                Kind::Function => return Some(format!("user:{}", self.func_info(oid).qualname)),
                Kind::Class => return Some(format!("class:{}", self.class_info(oid).name)),
                Kind::Method => {
                    if let Payload::Method(q, _) = &self.obj(oid).payload {
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

    pub fn apply_decorators(&mut self, decorators: &'static [ast::Decorator], fnode: Node, mut fv: Value, state: &mut State) -> Value {
        for d in decorators.iter().rev() {
            let q = self.decorator_qualname(&d.expression, state).unwrap_or_default();
            let fobj = fv.pts.first();
            let fi = fobj.and_then(|o| self.func_info_opt(o));
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
                            self.prop_setters.insert((cls, nk), f);
                            if let Some(getter) = self.cls_slots.get(&(cls, Ns::Attr, nk)).cloned() {
                                if !getter.pts.is_empty() {
                                    fv = getter;
                                }
                            }
                        }
                    }
                }
                continue;
            }
            // `@singledispatch`: implementations registered on it are chosen by the type of the
            // first argument
            if matches!(q.as_str(), "functools.singledispatch" | "functools.singledispatchmethod") {
                if let Some(f) = fobj.filter(|o| self.kind(*o) == Kind::Function) {
                    self.sdispatch.insert(f, Vec::new());
                    if q.ends_with("method") {
                        self.sdispatch_method.insert(f);
                    }
                }
                continue;
            }
            if q == "sdispatch.register" {
                let dv = match &d.expression {
                    ast::Expr::Call(c) => self.eval(&c.func, state),
                    other => self.eval(other, state),
                };
                let base = dv.pts.iter().find_map(|m| match &self.obj(m).payload {
                    Payload::Method(_, b) => b.single(),
                    _ => None,
                });
                let impl_f = fobj.filter(|o| self.kind(*o) == Kind::Function);
                if let (Some(base), Some(impl_f)) = (base, impl_f) {
                    // the type: `@f.register(T)`, else the first parameter's annotation
                    let ty = match &d.expression {
                        ast::Expr::Call(c) if !c.arguments.args.is_empty() => Some(self.eval(&c.arguments.args[0], state)),
                        _ => self.sdispatch_annotation(impl_f, self.sdispatch_method.contains(&base), state),
                    };
                    if let (Some(ty), Some(reg)) = (ty, self.sdispatch.get_mut(&base)) {
                        reg.push((ty, impl_f));
                    }
                }
                continue;
            }
            if matches!(q.as_str(), "functools.lru_cache" | "functools.cache") {
                if let (Some(fi), Some(f)) = (&fi, fobj) {
                    fi.is_memo.set(true);
                    // the (empty) cache exists from the definition on, on every path
                    let memo = self.alloc_raw(Kind::Dict, &format!("memo:{}", f), "", true, "lru_cache", Payload::None);
                    state.set_row(memo, Ns::Item, crate::domain::Row::new());
                }
                continue;
            }
            if q.starts_with("fastapi.") {
                // `@app.get("/x")`: the route decorator registers the handler
                let dv = self.eval(&d.expression, state);
                crate::models::fastapi::apply_route(self, &dv, &fv);
                continue;
            }
            // `@functools.wraps(fn)`: the wrapper takes fn's name, qualname, doc, __wrapped__
            if q == "functools.wraps" {
                if let (ast::Expr::Call(cl), Some(f)) = (&d.expression, fobj) {
                    if let Some(a0) = cl.arguments.args.first() {
                        let wv = self.eval(a0, state);
                        self.wrapped.insert(f, wv);
                    }
                }
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
                !tv.pts.is_empty() && tv.pts.iter().all(|o| matches!(self.kind(o), Kind::Function | Kind::Bound))
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
            self.note(format!("unknown decorator {} at L{}: treated as identity", txt, fnode.line));
        }
        fv
    }

    pub fn call_method_if_defined(&mut self, v: &Value, name: &str, node: &Node, state: &mut State) -> Option<Value> {
        self.call_method_with(v, name, Vec::new(), node, state)
    }

    /// `call_method_if_defined` with positional arguments.
    pub fn call_method_with(&mut self, v: &Value, name: &str, args: Vec<Value>, node: &Node, state: &mut State) -> Option<Value> {
        let mut outs: Vec<Value> = Vec::new();
        for oid in v.pts.iter() {
            if self.kind(oid) == Kind::Instance {
                if let Some(cls) = self.instance_class(oid) {
                    if let Some(m) = self.class_lookup(cls, name) {
                        let fv = self.cls_slots.get(&(m, Ns::Attr, key_str_lit(name))).cloned().unwrap_or_default();
                        for f in fv.pts.iter() {
                            if self.kind(f) == Kind::Function {
                                outs.push(self.call_user(f, Some(Value::obj(oid)), args.clone(), IndexMap::new(), vec![], vec![], state, node, None, false));
                            }
                        }
                    }
                }
            }
        }
        if outs.is_empty() {
            None
        } else {
            Some(join_values(outs))
        }
    }

    // ------------------------------------------------------------- names
    /// A read of a name bound on some paths only: possibly a NameError / UnboundLocalError.
    /// After it the program went on, so the name is bound on this path.
    fn unbound_read(&mut self, name: StrId, module: crate::domain::ObjId, node: &Node, state: &mut State, exc: &str) {
        if node.line == 0 {
            return;
        }
        let key = intern::key(intern::Key::Str(str_get(name)));
        let cls = crate::domain::Classification::with_note(crate::domain::Tier::Possibly, "bound on some paths only");
        self.verdict(node, key, &cls, module, Ns::Name, None, None);
        self.may_raise_here(exc, state);
        if state.unbound.contains(&name) {
            state.unbound = std::sync::Arc::new(state.unbound.iter().copied().filter(|n| *n != name).collect());
        }
    }

    pub fn load_name(&mut self, name: StrId, node: &Node, state: &mut State) -> Value {
        let _ = node;
        for ov in self.frame().overlays.iter().rev() {
            if let Some(v) = ov.get(&name) {
                return v.clone();
            }
        }
        let fr_kind = self.frame().kind;
        let fr_module = self.frame().module;
        if fr_kind == FrameKind::Function {
            if let Some(&cid) = self.frame().cells.get(&name) {
                let v = state.slot(cid, Ns::Cell, cell_key());
                return v;
            }
            let scope = self.frame().scope;
            let mi = self.module_info(fr_module);
            if mi.scopes.get(scope).is_local(&str_get(name)) {
                if let Some(v) = state.env.get(&name).cloned() {
                    if state.unbound.contains(&name) {
                        self.unbound_read(name, fr_module, node, state, "UnboundLocalError");
                    }
                    return v;
                }
                if node.line > 0 {
                    let key = intern::key(intern::Key::Str(str_get(name)));
                    let cls = crate::domain::Classification::with_note(crate::domain::Tier::Missing, "bound on no path to here");
                    self.verdict(node, key, &cls, fr_module, Ns::Name, None, None);
                    self.raise_here("UnboundLocalError", state);
                }
                return Value::data(&format!("unbound {}", str_get(name)));
            }
        }
        if fr_kind == FrameKind::Class {
            let cls = self.frame().class_oid.unwrap();
            if let Some(v) = state.slot_opt(cls, Ns::Attr, intern::key(intern::Key::Str(str_get(name)))) {
                return v;
            }
            if let Some(&cid) = self.frame().cells.get(&name) {
                return state.slot(cid, Ns::Cell, cell_key());
            }
        }
        if let Some(v) = state.slot_opt(fr_module, Ns::Attr, intern::key(intern::Key::Str(str_get(name)))) {
            if fr_kind == FrameKind::Module {
                let k = intern::key(intern::Key::Str(str_get(name)));
                let row = state.row(fr_module, Ns::Attr);
                if row.present.contains(k) && !row.definite.contains(k) && row.mutators.is_empty() && !row.top && row.sources.is_empty() {
                    self.unbound_read(name, fr_module, node, state, "NameError");
                    state.set_row(fr_module, Ns::Attr, row.narrow_in(k, true));
                }
            }
            return v;
        }
        let nm = str_get(name);
        if is_builtin_name(&nm) {
            let oid = self.alloc_raw(Kind::External, &format!("ext:builtins.{}", nm), "", true, &nm, Payload::Str(format!("builtins.{}", nm)));
            return Value::obj(oid);
        }
        let oid = self.unknown_source(&format!("name {}", nm), SourceKind::Name, state, &format!("?{}", nm), "", Ns::Item, vec![]);
        Value::obj(oid)
    }


    /// A narrowed value for a name (a test on it): its value changes, not whether it is bound.
    pub fn rebind_name(&mut self, name: StrId, v: Value, state: &mut State) {
        let module = self.frame().module;
        let at_module = self.frame().kind == FrameKind::Module;
        let row = if at_module { Some(state.row(module, Ns::Attr)) } else { None };
        let unbound = state.unbound.clone();
        // (keys stored through the name hold on whichever object it is: narrowed to one dict,
        // they are that dict's — `t is a` after `t[k] = …` with t one of a, b)
        let keys: Vec<KeyId> = state.name_keys.iter().filter(|(n, _)| *n == name).map(|(_, k)| *k).collect();
        let one = v.single().filter(|o| self.kind(*o) == Kind::Dict && v.kv.is_bot());
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
                    state.set_row(o, Ns::Item, r.narrow_in(k, true));
                }
            }
        }
    }

    pub fn store_name(&mut self, name: StrId, v: Value, state: &mut State) {
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
        let key = intern::key(intern::Key::Str(str_get(name)));
        {
            let fr = self.frame_mut();
            for ov in fr.overlays.iter_mut().rev() {
                if ov.contains_key(&name) {
                    ov.insert(name, v);
                    return;
                }
            }
        }
        if fr_kind == FrameKind::Function {
            if let Some(&cid) = self.frame().cells.get(&name) {
                let strong = self.obj(cid).single;
                state.set_slot(cid, Ns::Cell, cell_key(), v, strong);
                return;
            }
            let scope = self.frame().scope;
            let mi = self.module_info(fr_module);
            if mi.scopes.get(scope).globals_decl.contains(&*str_get(name)) {
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
            let nv = match self.cls_slots.get(&k) {
                Some(cur) => cur.join(&v),
                None => v,
            };
            self.cls_slots.insert(k, nv);
            return;
        }
        // a module-level binding inside a loop is strong under the recency abstraction (the
        // flow-sensitive state is re-bound every iteration anyway)
        state.set_slot(fr_module, Ns::Attr, key, v, true);
        let r = state.row(fr_module, Ns::Attr).store(key, true);
        state.set_row(fr_module, Ns::Attr, r);
    }

    // ------------------------------------------------------------ targets
    pub fn bind_target(&mut self, t: &'static ast::Expr, v: &Value, state: &mut State) {
        match t {
            ast::Expr::Name(n) => {
                let name = s(n.id.as_str());
                if self.torch_on {
                    self.t_name_result(name, v);
                }
                self.store_name(name, v.clone(), state);
                if self.facts_on {
                    let nd = frontend::node(t);
                    self.t_fact_write = true;
                    self.record_fact(&nd, name, v, state);
                    self.t_fact_write = false;
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
                    if obj.pts.len() > 1 && obj.pts.iter().all(|o| self.kind(o) == Kind::Dict) {
                        state.add_name_key(s(nm.id.as_str()), k);
                    }
                }
            }
            ast::Expr::Tuple(ast::ExprTuple { elts, .. }) | ast::Expr::List(ast::ExprList { elts, .. })
                if self.torch_on && {
                    let star = elts.iter().position(|e| matches!(e, ast::Expr::Starred(_)));
                    let n = frontend::node(t);
                    match self.t_unpack(v, elts.len(), star, &n, state) {
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
        let known = v.single().filter(|o| matches!(self.kind(*o), Kind::List | Kind::Tuple)).and_then(|o| self.llen(o, state).filter(|l| *l >= si + after).map(|l| (o, l)));
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

    pub fn unpack(&mut self, v: &Value, n: usize, state: &mut State, node: &Node) -> Vec<Value> {
        let mut parts: Vec<Value> = (0..n).map(|_| Value::new()).collect();
        for oid in v.pts.iter() {
            let kind = self.kind(oid);
            let ll = self.llen(oid, state);
            if matches!(kind, Kind::Tuple | Kind::List) && ll.is_some() {
                for i in 0..n {
                    let sv = state.slot(oid, Ns::Pos, key_int(i as i64));
                    parts[i] = parts[i].join(&sv);
                }
            } else if let Some(cls) = self.instance_class(oid).filter(|c| kind == Kind::Instance && self.is_named_tuple_class(*c)) {
                // a NamedTuple: its fields in order
                let fs = self.dataclass_fields_mro(cls);
                for i in 0..n {
                    let v = match fs.get(i) {
                        Some((f, _)) => state.slot(oid, Ns::Attr, intern::key_str_lit(f)),
                        None => Value::data(&format!("unpack@L{}", node.line)),
                    };
                    parts[i] = parts[i].join(&v);
                }
            } else if kind == Kind::Pair {
                if let Payload::Pair(k, val) = self.obj(oid).payload.clone() {
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
            let lits = crate::domain::kv_literals(&v.kv);
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

    pub fn bind_iter_target(&mut self, target: &'static ast::Expr, elem: &Elem, state: &mut State) {
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
                    let pid = self.alloc_raw(Kind::Pair, &format!("pair:{}", addr), "", true, "pair", Payload::Pair(k.clone(), val.clone()));
                    self.obj_mut(pid).payload = Payload::Pair(k.clone(), val.clone());
                    self.bind_target(target, &Value::obj(pid), state);
                }
            }
            Elem::One(v) => self.bind_target(target, v, state),
        }
    }

    // ------------------------------------------------------------ iteration
    /// The items of an iterator object through `__next__`, when every call is decided (None:
    /// not decided, or it changes other objects than itself and the items it returns).
    fn unroll_next(&mut self, it: ObjId, node: &Node, state: &mut State) -> Option<Vec<Value>> {
        let m = self.load_attr(&Value::obj(it), "__next__", node, state, false);
        if m.pts.is_empty() {
            return None;
        }
        // (a `__next__` found not unrollable once is not probed again: the probes cost work)
        let fkey: Vec<ObjId> = m.pts.iter().map(|b| match &self.obj(b).payload {
            Payload::Bound(f, _) => *f,
            _ => b,
        }).collect();
        if fkey.iter().any(|f| self.no_unroll_next.contains(f)) {
            return None;
        }
        let r = self.unroll_next_inner(it, &m, node, state);
        if r.is_none() {
            self.no_unroll_next.extend(fkey);
        }
        r
    }

    fn unroll_next_inner(&mut self, it: ObjId, m: &Value, node: &Node, state: &mut State) -> Option<Vec<Value>> {
        let m = m.clone();
        let mut cur = state.copy();
        let mut seq: Vec<Value> = Vec::new();
        for _ in 0..super::UNROLL_MAX {
            let mark = self.objects.len() as ObjId;
            let mut probe = cur.copy();
            // (a frame that catches StopIteration only, as the for loop does: other raises and
            // the reads in the call behave as in the loop; the classes raised as a try records
            // them)
            let mut pre = probe.copy();
            self.enter_try(rustc_hash::FxHashSet::from_iter(["StopIteration".to_string()]), &mut pre);
            let v = self.call_value(&m, node, None, &mut probe, Some(Vec::new()));
            let out = self.leave_try();
            let (flagged, raised_at, recs) = (out.flagged, out.raised(), out.recs);
            let only_stop = !recs.is_empty() && recs.iter().all(|(c, _, _)| c.as_ref().is_some_and(|(names, _)| names.iter().any(|n| n == "StopIteration")));
            let level = if flagged { 1 } else if raised_at.is_none() && probe.reachable { 0 } else if !probe.reachable && raised_at.is_some() { 2 } else { 1 };
            // (effects only on the iterator, the objects its attributes hold — its own state,
            // like the list it pops from — and on objects the call made)
            let fp = probe.footprint(&cur);
            let held: Vec<ObjId> = cur.slots_of(it).iter().filter(|((ns, _), _)| *ns == Ns::Attr).flat_map(|(_, v)| v.pts.to_vec()).collect();
            let own = |o: ObjId| o == it || o >= mark || held.contains(&o);
            if fp.rows.iter().any(|(o, _)| !own(*o)) || fp.slots.iter().any(|(o, _)| !own(*o)) || fp.elems.iter().any(|o| !own(*o)) {
                return None;
            }
            match level {
                0 => {
                    // (the items so far live in the state while the next call runs: a record
                    // made at the same site again is then a new object, not the earlier item)
                    probe.env.insert(s(&format!("\u{1}next{}", seq.len())), v.clone());
                    seq.push(v);
                    cur = probe;
                }
                2 if only_stop => {
                    // (the loop goes on from where StopIteration was raised)
                    if let Some(mut rs) = raised_at.and_then(|_| crate::interp::join_raise_recs(&recs)) {
                        rs.reachable = true;
                        cur = rs;
                    }
                    for i in 0..seq.len() {
                        cur.env.shift_remove(&s(&format!("\u{1}next{}", i)));
                    }
                    state.replace_with(cur);
                    return Some(seq);
                }
                _ => return None,
            }
        }
        None
    }

    /// An iteration's element as one value: a (key, value) pair is a pair object (what
    /// `enumerate` / `zip` / a starred item hand on, unpacked later).
    pub fn elem_value(&mut self, e: &Elem, node: &Node, tag: &str) -> Value {
        match e {
            Elem::One(v) => v.clone(),
            Elem::Pair(k, v) => {
                let ctx = self.frame().ctx.clone();
                let pid = self.alloc_raw(Kind::Pair, &format!("pair:{}:{}", self.site_of(node), tag), &ctx, false, "pair", Payload::Pair(k.clone(), v.clone()));
                Value::obj(pid)
            }
        }
    }

    pub fn iterate(&mut self, v: &Value, node: &Node, state: &mut State) -> Iteration {
        // one of several containers (or strings): their items are alternatives, not one
        // sequence — joined position by position when every one is a sequence of one known
        // length, else not known (the elements are still the join)
        let n_alts = v.pts.len() + usize::from(!v.kv.is_bot());
        let multi_lits = v.pts.is_empty() && matches!(&v.kv, KeyVal::Lits(ks) if ks.len() > 1);
        let alts_mode = n_alts > 1 || multi_lits;
        let mut alt_items: Option<Vec<Value>> = None;
        if alts_mode {
            let mut lists: Option<Vec<Vec<Value>>> = Some(Vec::new());
            for o in v.pts.iter() {
                let seq = matches!(self.kind(o), Kind::List | Kind::Tuple) && !self.obj(o).iterator;
                match (seq, self.llen(o, state), lists.as_mut()) {
                    (true, Some(n), Some(ls)) if n <= UNROLL_MAX => ls.push((0..n).map(|i| state.slot(o, Ns::Pos, key_int(i as i64))).collect()),
                    _ => lists = None,
                }
            }
            match (&v.kv, lists.as_mut()) {
                (KeyVal::Bot, _) => {}
                (kv, Some(ls)) if kv.is_literal() => {
                    for k in crate::domain::kv_literals(kv).iter() {
                        match intern::key_as_str(k) {
                            Some(s) if s.chars().count() <= UNROLL_MAX => ls.push(s.chars().map(|ch| Value::lit(intern::key_str_lit(&ch.to_string()))).collect()),
                            _ => {
                                lists = None;
                                break;
                            }
                        }
                    }
                }
                _ => lists = None,
            }
            alt_items = lists.filter(|ls| !ls.is_empty() && ls.iter().all(|l| l.len() == ls[0].len())).map(|ls| (0..ls[0].len()).map(|i| join_values(ls.iter().map(|l| l[i].clone()).collect::<Vec<_>>())).collect());
        }
        let mut items: Option<Vec<Value>> = Some(Vec::new());
        let mut elems: Vec<Value> = Vec::new();
        let mut pairs: Vec<(Value, Value)> = Vec::new();
        let mut pair = false;
        let mut exhaustive = true;
        let line_data = Value::data(&format!("L{}", node.line));
        for oid in v.pts.iter() {
            let kind = self.kind(oid);
            match kind {
                Kind::List | Kind::Tuple | Kind::Set => {
                    let ll = self.llen(oid, state);
                    // a generator partly consumed by next(): iteration starts at its cursor
                    let cursor = self.cursor_of(oid, state);
                    match (ll, items.as_mut(), cursor) {
                        (Some(n), Some(its), Some(cursor)) => {
                            for i in cursor.min(n)..n {
                                its.push(state.slot(oid, Ns::Pos, key_int(i as i64)));
                            }
                        }
                        _ => items = None,
                    }
                    elems.push(self.elem_of(&Value::obj(oid), node, state));
                }
                Kind::Dict | Kind::Frame => {
                    // (a dict from before a summarised call, a module constant: its items are
                    // known where the call is)
                    let lit = self.literal_dict_items(oid, state);
                    match (lit, items.as_mut()) {
                        (Some(l), Some(its)) => its.extend(l.into_iter().map(|(k, _)| Value::lit(k))),
                        _ => items = None,
                    }
                    elems.push(Value::kv_only(KeyVal::KeysOf(KeysOf::new(ObjSet::one(oid), Ns::Item))));
                }
                Kind::View => {
                    let Payload::View(vk, recv) = self.obj(oid).payload.clone() else { continue };
                    if matches!(vk, ViewKind::Rows | ViewKind::Elems | ViewKind::Attrs) {
                        items = None;
                        exhaustive = false;
                        if vk == ViewKind::Rows {
                            pair = true;
                            pairs.push((Value::data("index"), recv));
                        } else {
                            elems.push(recv);
                        }
                        continue;
                    }
                    let kv = KeyVal::KeysOf(KeysOf::new(recv.pts.clone(), Ns::Item));
                    let vals = join_values(recv.pts.iter().map(|r| self.record_values(r, state)).collect::<Vec<_>>());
                    let lit = if recv.pts.len() == 1 { self.literal_dict_items(recv.pts.first().unwrap(), state) } else { None };
                    match (lit, items.as_mut()) {
                        (Some(l), Some(its)) => {
                            for (k, val) in l {
                                match vk {
                                    ViewKind::Keys | ViewKind::Proxy => its.push(Value::lit(k)),
                                    ViewKind::Values => its.push(val),
                                    _ => {
                                        let pid = self.alloc_raw(Kind::Pair, &format!("pair:{}:{}", oid, intern::key_repr(k)), "", true, "pair", Payload::Pair(Value::lit(k), val.clone()));
                                        self.obj_mut(pid).payload = Payload::Pair(Value::lit(k), val);
                                        its.push(Value::obj(pid));
                                    }
                                }
                            }
                        }
                        _ => items = None,
                    }
                    match vk {
                        ViewKind::Keys | ViewKind::Proxy => elems.push(Value::kv_only(kv)),
                        ViewKind::Values => elems.push(vals),
                        _ => {
                            pair = true;
                            pairs.push((Value::kv_only(kv), vals));
                        }
                    }
                }
                // an sqlite cursor: its remaining rows
                Kind::Namespace if self.m.sql_cursors.contains_key(&oid) => {
                    if let Some(rows) = crate::models::sqlite::cursor_rows(self, state, node, oid) {
                        state.set_slot(oid, Ns::Attr, key_str_lit("\u{1}pos"), Value::data("pos"), true);
                        let it = self.iterate(&rows, node, state);
                        match (it.items, items.as_mut()) {
                            (Some(xs), Some(its)) => its.extend(xs),
                            _ => items = None,
                        }
                        elems.push(it.elem.flat());
                    }
                }
                // itertools.count / cycle / repeat: no end
                Kind::Namespace if self.m.infinites.contains_key(&oid) => {
                    items = None;
                    exhaustive = false;
                    let inf = self.m.infinites[&oid].clone();
                    elems.push(self.infinite_elem(&inf));
                }
                Kind::Zip => {
                    let Payload::Zip(ks, vs) = self.obj(oid).payload.clone() else { continue };
                    let mut ki = self.iterate(&ks, node, state);
                    let mut vi = self.iterate(&vs, node, state);
                    // an infinite side (or a generator known that far) yields as many items as the
                    // other has
                    if let (None, Some(n)) = (&ki.items, vi.items.as_ref().map(|x| x.len())) {
                        if let Some(its) = self.take_known(&ks, n, state) {
                            ki.items = Some(its);
                        }
                    }
                    if let (None, Some(n)) = (&vi.items, ki.items.as_ref().map(|x| x.len())) {
                        if let Some(its) = self.take_known(&vs, n, state) {
                            vi.items = Some(its);
                        }
                    }
                    // a strict zip that did not raise: both sides as long as the known one
                    let strict = self.m.zip_strict.contains(&oid);
                    let (ki_items, vi_items) = match (ki.items.clone(), vi.items.clone()) {
                        (Some(k), None) if strict => {
                            let e = self.elem_value(&vi.elem, node, "zv");
                            let n = k.len();
                            (Some(k), Some(vec![e; n]))
                        }
                        (None, Some(v)) if strict => {
                            let e = self.elem_value(&ki.elem, node, "zk");
                            let n = v.len();
                            (Some(vec![e; n]), Some(v))
                        }
                        (k, v) => (k, v),
                    };
                    match (ki_items, vi_items, items.as_mut()) {
                        (Some(kit), Some(vit), Some(its)) => {
                            // zip stops at the shorter input
                            let ctx = self.frame().ctx.clone();
                            for (k, val) in kit.into_iter().zip(vit.into_iter()) {
                                let pid = self.alloc_raw(Kind::Pair, &format!("pair:{}:{}", self.site_of(node), its.len()), &ctx, true, "pair", Payload::Pair(k.clone(), val.clone()));
                                self.obj_mut(pid).payload = Payload::Pair(k, val);
                                its.push(Value::obj(pid));
                            }
                        }
                        _ => items = None,
                    }
                    pair = true;
                    let (kv1, vv1) = (self.elem_value(&ki.elem, node, "zk"), self.elem_value(&vi.elem, node, "zv"));
                    pairs.push((kv1, vv1));
                }
                Kind::Enumerate => {
                    let Payload::Enumerate(inner, start) = self.obj(oid).payload.clone() else { continue };
                    let ii = self.iterate(&inner, node, state);
                    match (ii.items, items.as_mut(), start) {
                        (Some(inner_items), Some(its), Some(start)) => {
                            // known items: (index, item) pairs with literal indices
                            let ctx = self.frame().ctx.clone();
                            for (i, item) in inner_items.into_iter().enumerate() {
                                let idx = Value::lit(key_int(start + i as i64));
                                let pid = self.alloc_raw(Kind::Pair, &format!("pair:{}:{}", self.site_of(node), its.len()), &ctx, true, "pair", Payload::Pair(idx.clone(), item.clone()));
                                self.obj_mut(pid).payload = Payload::Pair(idx, item);
                                its.push(Value::obj(pid));
                            }
                        }
                        _ => items = None,
                    }
                    pair = true;
                    let iv = self.elem_value(&ii.elem, node, "en");
                    pairs.push((Value::data("index"), iv));
                }
                Kind::Class if self.enum_members.contains_key(&oid) => {
                    // an Enum class: its members in order
                    let ms: Vec<Value> = self.enum_members[&oid].iter().map(|(_, m)| Value::obj(*m)).collect();
                    if let Some(its) = items.as_mut() {
                        its.extend(ms.iter().cloned());
                    }
                    elems.push(join_values(ms));
                }
                Kind::Instance if self.instance_class(oid).and_then(|c| self.class_lookup(c, "__iter__")).is_some() => {
                    // a user iterable: what its `__iter__` returns
                    let r = self.call_method_with(&Value::obj(oid), "__iter__", Vec::new(), node, state).unwrap_or_default();
                    // an iterator (`return self`, an object with `__next__`): its items are what
                    // `__next__` returns (iterating it again would ask `__iter__` forever)
                    let iters: Vec<u32> = r.pts.iter().filter(|p| *p == oid || (self.kind(*p) == Kind::Instance && self.instance_class(*p).and_then(|c| self.class_lookup(c, "__next__")).is_some())).collect();
                    // (one iterator whose `__next__` is decided call by call — an item, or surely
                    // StopIteration — and changes nothing but its own state and the items it makes:
                    // its items, in order, up to UNROLL_MAX)
                    if iters.len() == 1 && r.pts.len() == 1 && r.kv.is_bot() && items.is_some() {
                        let it = iters[0];
                        if let Some(seq) = self.unroll_next(it, node, state) {
                            if let Some(its) = items.as_mut() {
                                its.extend(seq.iter().cloned());
                            }
                            elems.push(join_values(seq));
                            continue;
                        }
                    }
                    if !iters.is_empty() {
                        items = None;
                        exhaustive = false;
                        let mut none_taken = state.copy();
                        let mut nexts = Value::new();
                        for it in &iters {
                            if self.instance_class(*it).and_then(|c| self.class_lookup(c, "__next__")).is_some() {
                                nexts = nexts.join(&self.call_method_with(&Value::obj(*it), "__next__", Vec::new(), node, state).unwrap_or_default());
                            } else {
                                nexts = nexts.join(&Value::data("next"));
                            }
                        }
                        // (the loop may take no item: StopIteration at once)
                        let j = if state.reachable { state.join(&mut none_taken) } else { none_taken };
                        state.replace_with(j);
                        elems.push(nexts);
                        let rest = Value::with_kv(r.pts.filter(|p| !iters.contains(&p)), r.kv.clone());
                        if rest.empty() {
                            continue;
                        }
                        let sub = self.iterate(&rest, node, state);
                        match sub.elem {
                            Elem::One(e) => elems.push(e),
                            Elem::Pair(k, v) => {
                                pair = true;
                                pairs.push((k, v));
                            }
                        }
                        continue;
                    }
                    let sub = self.iterate(&r, node, state);
                    match (sub.items, items.as_mut()) {
                        (Some(si), Some(its)) => its.extend(si),
                        _ => items = None,
                    }
                    exhaustive = false;
                    match sub.elem {
                        Elem::One(e) => elems.push(e),
                        Elem::Pair(k, v) => {
                            pair = true;
                            pairs.push((k, v));
                        }
                    }
                }
                Kind::Unknown => {
                    items = None;
                    exhaustive = false;
                    if self.obj(oid).depth >= PATH_DEPTH {
                        elems.push(Value::data("deep"));
                        continue;
                    }
                    let row = state.row(oid, Ns::Item);
                    let label = format!("elem of {}", self.obj(oid).label);
                    let eid = self.alloc_raw(Kind::Unknown, &format!("elem:{}", oid), "", false, &label, Payload::None);
                    if let Some(sym) = self.sym_access(oid, Ns::Item, STAR) {
                        if !self.symbols.contains_key(&eid) {
                            let scope = self.symbol_scope.get(&oid).copied().unwrap_or(0);
                            self.set_symbol(eid, sym, scope);
                        }
                    }
                    self.obj_mut(eid).iter_elem = true;
                    let d = self.obj(oid).depth + 1;
                    self.obj_mut(eid).depth = d;
                    if !state.has_row(eid, Ns::Item) {
                        let srcs = row.sources.map(|(src, _)| (crate::domain::source_extend(src, STAR), intern::EMPTY_LENS));
                        // (an element was taken on this path: its source's requirements are
                        // known here, none so far — see the nested-source join)
                        for (cs, _) in srcs.iter() {
                            state.reqs.mark(&cs, crate::domain::ReqInfo::default());
                        }
                        // (nothing known of the unknown value's content: neither of its elements')
                        let r = if srcs.is_empty() && row.mutators.is_empty() {
                            Row::top()
                        } else {
                            Row::from_data(crate::domain::RowData { sources: srcs, mutators: row.mutators.clone(), top: row.top, ..Default::default() })
                        };
                        state.set_row(eid, Ns::Item, r.clone());
                        state.set_row(eid, Ns::Attr, r);
                    }
                    elems.push(Value::obj(eid));
                }
                _ => {
                    items = None;
                    exhaustive = false;
                    elems.push(line_data.clone());
                }
            }
        }
        if v.pts.is_empty() {
            items = None;
            exhaustive = false;
            let one_str = v.kv.as_lit().and_then(intern::key_as_str);
            if let Some(text) = one_str {
                // one known string: its characters
                let chars: Vec<KeyId> = text.chars().map(|ch| intern::key_str_lit(&ch.to_string())).collect();
                if chars.len() <= UNROLL_MAX {
                    items = Some(chars.iter().map(|k| Value::lit(*k)).collect());
                }
                let ks: KeySet = chars.into_iter().collect();
                elems.push(Value::kv_only(KeyVal::lits(ks)));
            } else if v.kv.is_literal() {
                let ks = crate::domain::kv_literals(&v.kv);
                // (one of several strings: its characters)
                if ks.iter().all(intern::key_is_str) {
                    let chars: KeySet = ks.iter().flat_map(|k| intern::key_as_str(k).map(|s| s.chars().map(|ch| intern::key_str_lit(&ch.to_string())).collect::<Vec<_>>()).unwrap_or_default()).collect();
                    elems.push(Value::kv_only(KeyVal::lits(chars)));
                } else {
                    if ks.len() <= UNROLL_MAX {
                        items = Some(intern::sorted_by_str(ks.iter()).into_iter().map(Value::lit).collect());
                    }
                    elems.push(Value::kv_only(KeyVal::lits(ks)));
                }
            } else {
                elems.push(line_data.clone());
            }
        }
        if let Some(its) = &items {
            // no items *and* no containers: nothing is known; empty containers iterate zero times
            if its.is_empty() && v.pts.is_empty() {
                items = None;
            }
        }
        if pair {
            let mut kacc = join_values(pairs.iter().map(|p| p.0.clone()));
            let vacc = join_values(pairs.iter().map(|p| p.1.clone()));
            for e in &elems {
                kacc = kacc.join(e);
            }
            let exh = exhaustive && pairs.len() == 1;
            if alts_mode {
                items = alt_items;
            }
            return Iteration { items, elem: Elem::Pair(kacc, vacc), exhaustive: exh };
        }
        let elem = join_values(elems.iter().cloned());
        let exh = exhaustive && elems.len() == 1 && matches!(elem.kv, KeyVal::KeysOf(_));
        if alts_mode {
            items = alt_items;
        }
        Iteration { items, elem: Elem::One(elem), exhaustive: exh }
    }

    /// Requirements on the loop's own elements are per element, not per program run.
    pub fn promote_elem_reqs(&mut self, exit_s: &mut State, body_s: &State, elem: &Value, state: &State) {
        if !body_s.reachable {
            return;
        }
        let mut srcs: Vec<crate::domain::SourceId> = Vec::new();
        // (the element, and the items it holds two levels down: the group of a `groupby` pair
        // iterated in its turn reads each of its rows once per element too)
        let mut objs: Vec<ObjId> = elem.pts.iter().collect();
        let mut level: Vec<ObjId> = objs.clone();
        for _ in 0..2 {
            let mut next: Vec<ObjId> = Vec::new();
            for oid in level {
                if matches!(self.kind(oid), Kind::List | Kind::Tuple) {
                    state.slots.each(&oid, &mut |sk, v| {
                        if matches!(sk.0, Ns::Elem | Ns::Pos) {
                            next.extend(v.pts.iter());
                        }
                    });
                }
            }
            next.retain(|o| !objs.contains(o));
            objs.extend(next.iter().copied());
            level = next;
        }
        for oid in objs {
            for ns in [Ns::Item, Ns::Attr] {
                for (src, _) in state.row(oid, ns).sources.iter() {
                    if !crate::domain::source_is_data(src) && !srcs.contains(&src) {
                        srcs.push(src);
                    }
                }
            }
        }
        for src in srcs {
            for (k, info) in body_s.reqs.for_key(&src) {
                if info.always {
                    if let Some(cur) = exit_s.reqs.get(&src, &k) {
                        if !cur.always {
                            exit_s.reqs.set(&src, k, crate::domain::ReqInfo { always: true, ..cur });
                        }
                    }
                }
            }
        }
    }

    /// Keys proven on a loop element that is a view of a record are proven on the record.
    pub fn promote_elem_narrowing(&mut self, exit_s: &mut State, body_s: &State, elem: &Value) {
        if !body_s.reachable {
            return;
        }
        for oid in elem.pts.iter() {
            let Some(src) = self.obj(oid).derived_from else { continue };
            if !self.obj(src).single {
                continue;
            }
            for ns in [Ns::Item, Ns::Attr] {
                let proven = body_s.row(oid, ns).definite.intersection(&body_s.row(src, ns).definite);
                let mut row = exit_s.row(src, ns);
                for k in proven.difference(&row.definite).iter() {
                    row = row.narrow_in(k, true);
                }
                exit_s.set_row(src, ns, row);
            }
        }
    }

    /// [(key, value)] for a closed dict whose keys are all definite literals (unrollable).
    /// A loop over a closed dict (or a view of one) whose keys are known in their order but
    /// some only maybe there: its items, each with whether it surely comes (a `for` runs a
    /// maybe item on one path and skips it on the other).
    pub fn maybe_dict_items(&mut self, v: &Value, state: &State) -> Option<Vec<(Value, bool)>> {
        let o = v.single().filter(|_| v.kv.is_bot())?;
        let (d, vk) = match (self.kind(o), &self.obj(o).payload) {
            (Kind::Dict, _) => (o, ViewKind::Keys),
            (Kind::View, Payload::View(vk, recv)) if matches!(vk, ViewKind::Keys | ViewKind::Values | ViewKind::Items | ViewKind::Proxy) => (recv.single().filter(|r| recv.kv.is_bot() && self.kind(*r) == Kind::Dict)?, *vk),
            _ => return None,
        };
        let row = state.row(d, Ns::Item);
        if !row.closed() || row.present == row.definite || row.present.len() > UNROLL_MAX || state.has_slot(d, Ns::Item, STAR) || self.m.counters.contains(&d) {
            return None;
        }
        let keys = row.ordered_keys()?;
        let mut out = Vec::new();
        for k in keys {
            let sure = row.definite.contains(k);
            let val = state.slot(d, Ns::Item, k);
            let item = match vk {
                ViewKind::Keys | ViewKind::Proxy => Value::lit(k),
                ViewKind::Values => val,
                _ => {
                    let pid = self.alloc_raw(Kind::Pair, &format!("pair:{}:{}", o, intern::key_repr(k)), "", true, "pair", Payload::Pair(Value::lit(k), val.clone()));
                    self.obj_mut(pid).payload = Payload::Pair(Value::lit(k), val);
                    Value::obj(pid)
                }
            };
            out.push((item, sure));
        }
        Some(out)
    }

    pub fn literal_dict_items(&mut self, oid: u32, state: &State) -> Option<Vec<(KeyId, Value)>> {
        let row = state.row(oid, Ns::Item);
        // an empty closed dict iterates zero times (known), like an empty list
        if row.open() || row.present != row.definite || row.present.len() > UNROLL_MAX {
            return None;
        }
        // (a Counter's STAR slot is the count of a missing key, not a key)
        if state.has_slot(oid, Ns::Item, STAR) && !self.m.counters.contains(&oid) {
            return None;
        }
        // insertion order when it is known (a guess would make order-dependent results wrong)
        let keys = match row.ordered_keys() {
            Some(k) => k,
            None if row.present.len() <= 1 => intern::sorted_by_str(row.present.iter()),
            None => return None,
        };
        Some(keys.into_iter().map(|k| (k, state.slot(oid, Ns::Item, k))).collect())
    }

    pub fn elem_of(&mut self, v: &Value, node: &Node, state: &State) -> Value {
        let mut acc = Value::new();
        for oid in v.pts.iter() {
            match self.kind(oid) {
                Kind::List | Kind::Tuple | Kind::Set => {
                    let e = state.slot(oid, Ns::Elem, STAR);
                    let kv = state.elems.get(&oid).unwrap_or(DATA);
                    acc = acc.join(&Value::with_kv(e.pts, kv));
                }
                Kind::Dict | Kind::Frame => acc = acc.join(&Value::kv_only(KeyVal::KeysOf(KeysOf::new(ObjSet::one(oid), Ns::Item)))),
                _ => acc = acc.join(&Value::data(&format!("L{}", node.line))),
            }
        }
        if v.pts.is_empty() {
            acc = Value::data(&format!("L{}", node.line));
        }
        acc
    }

    pub fn all_values(&self, oid: u32, ns: Ns, state: &State) -> Value {
        state.all_slots(oid, ns)
    }

    /// The values a record holds, including what its unknown keys hold (a parameter's dict,
    /// an open record: its content under an unknown key).
    pub fn record_values(&mut self, oid: u32, state: &mut State) -> Value {
        let v = state.all_slots(oid, Ns::Item);
        let row = state.row(oid, Ns::Item);
        // (keys stored under unknown names keep their values in the `*` slot: only content
        // from outside — a parameter, an input, unknown code — is not there)
        let outside = row.top || row.sources.iter().any(|(src, _)| !crate::domain::source_is_data(src));
        if row.open() && outside {
            return v.join(&self.path_placeholder_value(oid, Ns::Item, STAR, &row, state));
        }
        v
    }

    /// Finite literal key set denoted by a value, or None.
    pub fn lit_keys(&mut self, v: &Value, state: &State) -> Option<KeySet> {
        if v.kv.is_literal() && v.pts.is_empty() {
            return Some(crate::domain::kv_literals(&v.kv));
        }
        let mut out = KeySet::new();
        for oid in v.pts.iter() {
            match self.kind(oid) {
                Kind::List | Kind::Tuple | Kind::Set => match state.elems.get(&oid) {
                    Some(kv) if kv.is_literal() => out = out.union(&crate::domain::kv_literals(&kv)),
                    _ => {
                        return None;
                    }
                },
                Kind::Dict => {
                    let row = state.row(oid, Ns::Item);
                    if row.closed() && row.present == row.definite {
                        out = out.union(&row.present);
                    } else {
                        return None;
                    }
                }
                _ => return None,
            }
        }
        if v.pts.is_empty() {
            None
        } else {
            Some(out)
        }
    }

    pub fn literal_mapping(&mut self, v: &Value, state: &State) -> Option<Vec<(KeyId, KeyId)>> {
        let mut out: Vec<(KeyId, KeyId)> = Vec::new();
        for oid in v.pts.iter() {
            if self.kind(oid) != Kind::Dict {
                return None;
            }
            let row = state.row(oid, Ns::Item);
            if row.open() || row.present != row.definite {
                return None;
            }
            for k in row.present.iter() {
                let sv = state.slot(oid, Ns::Item, k);
                match sv.kv.as_lit() {
                    Some(l) => out.push((k, l)),
                    None => return None,
                }
            }
        }
        if v.pts.is_empty() {
            None
        } else {
            Some(out)
        }
    }

    pub fn copy_object(&mut self, oid: u32, node: &Node, state: &mut State, deep: bool, seen: &mut IndexMap<u32, u32>) -> u32 {
        if let Some(&n) = seen.get(&oid) {
            return n;
        }
        let o_kind = self.kind(oid);
        let kind = if matches!(o_kind, Kind::Dict | Kind::List | Kind::Tuple | Kind::Set | Kind::Instance | Kind::Frame) { o_kind } else { Kind::Dict };
        let tag = if deep { "deep" } else { "copy" };
        // (the original's id in the site: a deep copy's nested copies are distinct objects; a
        // copy of a copy made at this node counts as a copy of the first original)
        let prefix = format!("{}:{}:", tag, self.site_of(node));
        // (a clone of such a copy, made by a summary's instantiation, has the same site)
        let site_root = self.obj(oid).site.strip_prefix(&prefix).and_then(|rest| rest.split(['#', ':']).nth(1)).and_then(|r| r.parse::<u32>().ok());
        let root = match (self.copy_root.get(&oid), site_root) {
            (Some(&r), _) if self.obj(oid).site.starts_with(&prefix) => r,
            (_, Some(r)) if (r as usize) < self.objects.len() => r,
            _ => oid,
        };
        let label = format!("{} of {}@L{}", tag, self.obj(root).label, node.line);
        let payload = self.obj(oid).payload.clone();
        let nid = self.alloc(kind, &format!("{}{}:{}", prefix, kind.name(), root), state, &label, payload, None, if kind == Kind::Instance { Ns::Attr } else { Ns::Item }, None, None);
        self.copy_root.insert(nid, root);
        seen.insert(oid, nid);
        let ll = self.llen(oid, state);
        self.set_llen(nid, state, ll);
        let nsingle = self.obj(nid).single;
        for ns in [Ns::Item, Ns::Attr] {
            if state.has_row(oid, ns) {
                let prev = state.rows.get(&(nid, ns));
                let r = match prev {
                    Some(p) if !nsingle => p.join(&state.row(oid, ns)),
                    _ => state.row(oid, ns),
                };
                state.set_row(nid, ns, r);
            }
        }
        if let Some(e) = state.elems.get(&oid) {
            state.elems.insert(nid, e);
        }
        for ((ns, k), mut v) in state.slots_of(oid) {
            if deep && seen.len() < 64 {
                let mut newp: Vec<u32> = Vec::new();
                for x in v.pts.iter() {
                    if matches!(self.kind(x), Kind::Dict | Kind::List | Kind::Tuple | Kind::Set | Kind::Instance) {
                        newp.push(self.copy_object(x, node, state, true, seen));
                    } else {
                        newp.push(x);
                    }
                }
                v = Value::with_kv(ObjSet::from_vec(newp), v.kv);
            }
            state.set_slot(nid, ns, k, v, true);
        }
        nid
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

