//! Functions, names, targets, iteration (port of the middle part of `rowan/interp.py`).

use std::rc::Rc;

use indexmap::IndexMap;
use ruff_python_ast as ast;

use crate::domain::{join_values, KeyVal, KeysOf, Ns, ObjId, ObjSet, SourceKind, Value, DATA};
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
    pub(in crate::interp) fn value_part(&self) -> Value {
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

/// The label of `enumerate`'s index when it is not known: an int.
pub const ENUM_INDEX: &str = "index";

/// What `iterate` gathers over the objects a value may be: their items in order (None: not
/// known), their elements and (key, value) pairs, and whether one record's keys are all of it.
struct ItemsAcc {
    items: Option<Vec<Value>>,
    elems: Vec<Value>,
    pairs: Vec<(Value, Value)>,
    pair: bool,
    exhaustive: bool,
}

impl Analyzer {
    // ----------------------------------------------------------- functions
    pub(in crate::interp) fn make_function(&mut self, node: FuncNode, state: &mut State, name: Option<&str>) -> Value {
        let fr_module = self.frame().module;
        let fr_kind = self.frame().kind;
        let fr_label = self.frame().label.clone();
        let fr_cells = self.frame().cells.clone();
        let fr_class = self.frame().class_oid;
        let mi = self.heap.module_info(fr_module);
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
        Some(self.alternatives(calls.len(), false, state, &mut |an, i, st| an.call_user(calls[i].1, Some(Value::obj(calls[i].0)), args.clone(), IndexMap::new(), vec![], vec![], st, node, false)))
    }

    /// The (receiver, function) pairs of the instances among `v` whose class defines one of
    /// `names` (the first defined, in order), and the rest of `v`.
    fn method_calls(&self, v: &Value, names: &[&str]) -> (Vec<(ObjId, ObjId)>, Value) {
        let mut calls = Vec::new();
        let mut rest = Value::kv_only(v.kv.clone());
        for o in v.pts.iter() {
            let cls = self.heap.instance_class(o).filter(|_| self.heap.kind(o) == Kind::Instance);
            let found = cls.and_then(|c| names.iter().find_map(|n| Some((self.class_lookup(c, n)?, *n))));
            match found {
                Some((m, n)) => {
                    let fv = self.heap.cls_slots.get(&(m, Ns::Attr, key_str_lit(n))).cloned().unwrap_or_default();
                    calls.extend(fv.pts.iter().filter(|f| self.heap.kind(*f) == Kind::Function).map(|f| (o, f)));
                }
                None => rest.pts = rest.pts.insert(o),
            }
        }
        (calls, rest)
    }

    /// A builtin Python carries out by a dunder of its argument (`str(x)` calls
    /// `x.__str__()`): on the instances among `v` whose class defines one of `names` (the
    /// first defined, in order) that method runs with `args`, one alternative each (and one
    /// where none runs, for the rest of `v`). Returns the results joined and the rest of `v`;
    /// None when no instance defines one.
    pub fn run_dunder(&mut self, v: &Value, names: &[&str], args: Vec<Value>, node: &Node, state: &mut State) -> Option<(Value, Value)> {
        let (calls, rest) = self.method_calls(v, names);
        if calls.is_empty() {
            return None;
        }
        let keep = !rest.empty();
        let r = self.alternatives(calls.len(), keep, state, &mut |an, i, st| an.call_user(calls[i].1, Some(Value::obj(calls[i].0)), args.clone(), IndexMap::new(), vec![], vec![], st, node, false));
        Some((r, rest))
    }

    // ------------------------------------------------------------- names
    /// A read of a name bound on some paths only: possibly a NameError / UnboundLocalError.
    /// After it the program went on, so the name is bound on this path.
    fn unbound_read(&mut self, name: StrId, module: crate::domain::ObjId, node: &Node, state: &mut State, exc: &str) {
        if node.line == 0 {
            return;
        }
        let key = intern::key(intern::Key::Str(str_get(name)));
        let cls = crate::domain::Classification::with_note(crate::domain::Tier::Possibly, "is only assigned on some paths");
        self.verdict(node, key, &cls, module, Ns::Name, None, None);
        self.may_raise_here(exc, state);
        if state.unbound.contains(&name) {
            state.unbound = std::sync::Arc::new(state.unbound.iter().copied().filter(|n| *n != name).collect());
        }
    }

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
                    let key = intern::key(intern::Key::Str(str_get(name)));
                    let cls = crate::domain::Classification::with_note(crate::domain::Tier::Missing, "is not assigned on any path to here");
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
        let oid = self.unknown_source(&format!("name {}", nm), SourceKind::Name, state, &format!("?{}", nm), "", Ns::Item, vec![]);
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
        let key = intern::key(intern::Key::Str(str_get(name)));
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
            self.heap.cls_slots.insert(k, nv);
            return;
        }
        // a module-level binding inside a loop is strong under the recency abstraction (the
        // flow-sensitive state is re-bound every iteration anyway)
        state.set_slot(fr_module, Ns::Attr, key, v, true);
        let r = state.row(fr_module, Ns::Attr).store(key, true);
        state.set_row(fr_module, Ns::Attr, r);
    }

    // ------------------------------------------------------------ targets
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
                    self.findings.t_fact_write = true;
                    self.record_fact(&nd, name, v, state);
                    self.findings.t_fact_write = false;
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

    pub fn unpack(&mut self, v: &Value, n: usize, state: &mut State, node: &Node) -> Vec<Value> {
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

    // ------------------------------------------------------------ iteration
    /// The items of an iterator object through `__next__`, when every call is decided (None:
    /// not decided, or it changes other objects than itself and the items it returns).
    fn unroll_next(&mut self, it: ObjId, node: &Node, state: &mut State) -> Option<Vec<Value>> {
        let m = self.load_attr(&Value::obj(it), "__next__", node, state, false);
        if m.pts.is_empty() {
            return None;
        }
        // (a `__next__` found not unrollable once is not probed again: the probes cost work)
        let fkey: Vec<ObjId> = m.pts.iter().map(|b| match &self.heap.obj(b).payload {
            Payload::Bound(f, _) => *f,
            _ => b,
        }).collect();
        if fkey.iter().any(|f| self.gens.no_unroll_next.contains(f)) {
            return None;
        }
        let r = self.unroll_next_inner(it, &m, node, state);
        if r.is_none() {
            self.gens.no_unroll_next.extend(fkey);
        }
        r
    }

    fn unroll_next_inner(&mut self, it: ObjId, m: &Value, node: &Node, state: &mut State) -> Option<Vec<Value>> {
        let m = m.clone();
        let mut cur = state.copy();
        let mut seq: Vec<Value> = Vec::new();
        for _ in 0..super::UNROLL_MAX {
            let mark = self.heap.len() as ObjId;
            let mut probe = cur.copy();
            // (a frame that catches StopIteration only, as the for loop does: other raises and
            // the reads in the call behave as in the loop; the classes raised as a try records
            // them)
            let mut pre = probe.copy();
            self.enter_try(rustc_hash::FxHashSet::from_iter(["StopIteration".to_string()]), &mut pre);
            let v = self.call_value(&m, node, None, &mut probe, Some(Vec::new()));
            let out = self.leave_try();
            let (flagged, raised_at) = (out.any(), out.raised());
            let recs = out.recs;
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
                let pid = self.heap.alloc_raw(Kind::Pair, &format!("pair:{}:{}", self.site_of(node), tag), &ctx, false, "pair", Payload::Pair(k.clone(), v.clone()));
                Value::obj(pid)
            }
        }
    }

    pub fn iterate(&mut self, v: &Value, node: &Node, state: &mut State) -> Iteration {
        self.force_gen(v, state, false);
        for o in v.pts.iter() {
            self.gens.read(o, None);
        }
        // one of several containers (or strings): their items are alternatives, not one
        // sequence — joined position by position when every one is a sequence of one known
        // length, else not known (the elements are still the join)
        let n_alts = v.pts.len() + usize::from(!v.kv.is_bot());
        let multi_lits = v.pts.is_empty() && matches!(&v.kv, KeyVal::Lits(ks) if ks.len() > 1);
        let alts_mode = n_alts > 1 || multi_lits;
        let alt_items: Option<Vec<Value>> = if alts_mode { self.alternative_items(v, state) } else { None };
        let mut acc = ItemsAcc { items: Some(Vec::new()), elems: Vec::new(), pairs: Vec::new(), pair: false, exhaustive: true };
        let line_data = Value::data(&format!("L{}", node.line));
        for oid in v.pts.iter() {
            let kind = self.heap.kind(oid);
            match kind {
                Kind::List | Kind::Tuple | Kind::Set => self.iter_sequence(oid, node, state, &mut acc),
                Kind::Dict | Kind::Frame => self.iter_mapping(oid, node, state, &mut acc),
                Kind::View => self.iter_view(oid, node, state, &mut acc),
                // an sqlite cursor: its remaining rows
                Kind::Namespace if self.m.sql_cursors.contains_key(&oid) => self.iter_cursor(oid, node, state, &mut acc),
                // itertools.count / cycle / repeat: no end
                Kind::Namespace if self.m.infinites.contains_key(&oid) => {
                    acc.items = None;
                    acc.exhaustive = false;
                    let inf = self.m.infinites[&oid].clone();
                    acc.elems.push(crate::models::itertools::infinite_elem(self, &inf));
                }
                Kind::Zip => self.iter_zip(oid, node, state, &mut acc),
                Kind::Enumerate => self.iter_enumerate(oid, node, state, &mut acc),
                Kind::Class if self.m.enum_members.contains_key(&oid) => {
                    // an Enum class: its members in order
                    let ms: Vec<Value> = self.m.enum_members[&oid].iter().map(|(_, m)| Value::obj(*m)).collect();
                    if let Some(its) = acc.items.as_mut() {
                        its.extend(ms.iter().cloned());
                    }
                    acc.elems.push(join_values(ms));
                }
                Kind::Instance if self.heap.instance_class(oid).and_then(|c| self.class_lookup(c, "__iter__")).is_some() => self.iter_user(oid, node, state, &mut acc),
                Kind::Unknown => self.iter_unknown(oid, node, state, &mut acc),
                _ => {
                    acc.items = None;
                    acc.exhaustive = false;
                    acc.elems.push(line_data.clone());
                }
            }
        }
        let ItemsAcc { mut items, mut elems, pairs, pair, mut exhaustive } = acc;
        if v.pts.is_empty() {
            self.iter_scalar(v, state, &line_data, &mut items, &mut elems, &mut exhaustive);
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

    /// The items of one of several containers (or strings), position by position: when every
    /// one is a list / tuple / string of one known length (at most `UNROLL_MAX`), the join of
    /// the alternatives at each position.
    fn alternative_items(&self, v: &Value, state: &State) -> Option<Vec<Value>> {
        let mut lists: Option<Vec<Vec<Value>>> = Some(Vec::new());
        for o in v.pts.iter() {
            let seq = matches!(self.heap.kind(o), Kind::List | Kind::Tuple) && !self.heap.obj(o).iterator;
            match (seq, self.heap.llen(o, state), lists.as_mut()) {
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
        lists.filter(|ls| !ls.is_empty() && ls.iter().all(|l| l.len() == ls[0].len())).map(|ls| (0..ls[0].len()).map(|i| join_values(ls.iter().map(|l| l[i].clone()).collect::<Vec<_>>())).collect())
    }

    /// Iterating a value of no object (a str, several literals, a tensor's rows, unknown): its
    /// characters / literals as the elements (and items, when few and known).
    fn iter_scalar(&mut self, v: &Value, state: &State, line_data: &Value, items: &mut Option<Vec<Value>>, elems: &mut Vec<Value>, exhaustive: &mut bool) {
        *items = None;
        *exhaustive = false;
        let one_str = v.kv.as_lit().and_then(intern::key_as_str);
        if let Some(text) = one_str {
            // one known string: its characters
            let chars: Vec<KeyId> = text.chars().map(|ch| intern::key_str_lit(&ch.to_string())).collect();
            if chars.len() <= UNROLL_MAX {
                *items = Some(chars.iter().map(|k| Value::lit(*k)).collect());
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
                    *items = Some(intern::sorted_by_str(ks.iter()).into_iter().map(Value::lit).collect());
                }
                elems.push(Value::kv_only(KeyVal::lits(ks)));
            }
        } else if let Some(e) = self.m.torch_on.then(|| crate::models::torch::t_iter_elem(self, v, state)).flatten() {
            elems.push(e);
        } else {
            elems.push(line_data.clone());
        }
    }

    /// An unknown iterable: one element object standing for each of its elements.
    fn iter_unknown(&mut self, oid: ObjId, _node: &Node, state: &mut State, acc: &mut ItemsAcc) {
        // (a slice of an unknown sequence: its elements)
        if let Some(b) = self.slice_base(oid) {
            return self.iter_unknown(b, _node, state, acc);
        }
        acc.items = None;
        acc.exhaustive = false;
        if self.heap.obj(oid).depth >= PATH_DEPTH {
            acc.elems.push(Value::data("deep"));
            return;
        }
        let row = state.row(oid, Ns::Item);
        let label = format!("elem of {}", self.heap.obj(oid).label);
        let eid = self.heap.alloc_raw(Kind::Unknown, &format!("elem:{}", oid), "", false, &label, Payload::None);
        self.fill_placeholder(eid, oid, Ns::Item, STAR, &row, true, state);
        acc.elems.push(Value::obj(eid));
    }

    /// An instance of a class with `__iter__`: what its iterator's `__next__` gives (in order, when
    /// decided call by call), or what the iterable `__iter__` returns gives.
    fn iter_user(&mut self, oid: ObjId, node: &Node, state: &mut State, acc: &mut ItemsAcc) {
        // a user iterable: what its `__iter__` returns
        let r = self.call_method_with(&Value::obj(oid), "__iter__", Vec::new(), node, state).unwrap_or_default();
        // an iterator (`return self`, an object with `__next__`): its items are what
        // `__next__` returns (iterating it again would ask `__iter__` forever)
        let iters: Vec<u32> = r.pts.iter().filter(|p| *p == oid || (self.heap.kind(*p) == Kind::Instance && self.heap.instance_class(*p).and_then(|c| self.class_lookup(c, "__next__")).is_some())).collect();
        // (one iterator whose `__next__` is decided call by call — an item, or surely
        // StopIteration — and changes nothing but its own state and the items it makes:
        // its items, in order, up to UNROLL_MAX)
        if iters.len() == 1 && r.pts.len() == 1 && r.kv.is_bot() && acc.items.is_some() {
            let it = iters[0];
            if let Some(seq) = self.unroll_next(it, node, state) {
                if let Some(its) = acc.items.as_mut() {
                    its.extend(seq.iter().cloned());
                }
                acc.elems.push(join_values(seq));
                return;
            }
        }
        if !iters.is_empty() {
            acc.items = None;
            acc.exhaustive = false;
            // `__next__` once per item, any number of times (none: StopIteration at once)
            let its = Value::objs(iters.iter().copied().collect());
            let mut nexts = Value::new();
            let (head, _, _) = self.loop_fixpoint(state, &mut |an, s| {
                let (v, rest) = an.run_dunder(&its, &["__next__"], Vec::new(), node, s).unwrap_or((Value::new(), its.clone()));
                nexts = nexts.join(&v);
                if !rest.empty() {
                    nexts = nexts.join(&Value::data("next"));
                }
            });
            state.replace_with(head);
            acc.elems.push(nexts);
            let rest = Value::with_kv(r.pts.filter(|p| !iters.contains(&p)), r.kv.clone());
            if rest.empty() {
                return;
            }
            let sub = self.iterate(&rest, node, state);
            match sub.elem {
                Elem::One(e) => acc.elems.push(e),
                Elem::Pair(k, v) => {
                    acc.pair = true;
                    acc.pairs.push((k, v));
                }
            }
            return;
        }
        let sub = self.iterate(&r, node, state);
        match (sub.items, acc.items.as_mut()) {
            (Some(si), Some(its)) => its.extend(si),
            _ => acc.items = None,
        }
        acc.exhaustive = false;
        match sub.elem {
            Elem::One(e) => acc.elems.push(e),
            Elem::Pair(k, v) => {
                acc.pair = true;
                acc.pairs.push((k, v));
            }
        }
    }

    /// `enumerate(it, start)`: (index, item) pairs.
    fn iter_enumerate(&mut self, oid: ObjId, node: &Node, state: &mut State, acc: &mut ItemsAcc) {
        let Payload::Enumerate(inner, start) = self.heap.obj(oid).payload.clone() else { return };
        let ii = self.iterate(&inner, node, state);
        match (ii.items, acc.items.as_mut(), start) {
            (Some(inner_items), Some(its), Some(start)) => {
                // known items: (index, item) pairs with literal indices
                let ctx = self.frame().ctx.clone();
                for (i, item) in inner_items.into_iter().enumerate() {
                    let idx = Value::lit(key_int(start + i as i64));
                    let pid = self.heap.alloc_raw(Kind::Pair, &format!("pair:{}:{}", self.site_of(node), its.len()), &ctx, true, "pair", Payload::Pair(idx.clone(), item.clone()));
                    self.heap.obj_mut(pid).payload = Payload::Pair(idx, item);
                    its.push(Value::obj(pid));
                }
            }
            _ => acc.items = None,
        }
        acc.pair = true;
        let iv = self.elem_value(&ii.elem, node, "en");
        acc.pairs.push((Value::data(ENUM_INDEX), iv));
    }

    /// `zip(a, b)`: (a item, b item) pairs, as many as the shorter side has.
    fn iter_zip(&mut self, oid: ObjId, node: &Node, state: &mut State, acc: &mut ItemsAcc) {
        let Payload::Zip(ks, vs) = self.heap.obj(oid).payload.clone() else { return };
        let mut ki = self.iterate(&ks, node, state);
        let mut vi = self.iterate(&vs, node, state);
        // an infinite side (or a generator known that far) yields as many items as the
        // other has
        if let (None, Some(n)) = (&ki.items, vi.items.as_ref().map(|x| x.len())) {
            if let Some(its) = crate::models::itertools::take_known(self, &ks, n, state) {
                ki.items = Some(its);
            }
        }
        if let (None, Some(n)) = (&vi.items, ki.items.as_ref().map(|x| x.len())) {
            if let Some(its) = crate::models::itertools::take_known(self, &vs, n, state) {
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
        match (ki_items, vi_items, acc.items.as_mut()) {
            (Some(kit), Some(vit), Some(its)) => {
                // zip stops at the shorter input
                let ctx = self.frame().ctx.clone();
                for (k, val) in kit.into_iter().zip(vit.into_iter()) {
                    let pid = self.heap.alloc_raw(Kind::Pair, &format!("pair:{}:{}", self.site_of(node), its.len()), &ctx, true, "pair", Payload::Pair(k.clone(), val.clone()));
                    self.heap.obj_mut(pid).payload = Payload::Pair(k, val);
                    its.push(Value::obj(pid));
                }
            }
            _ => acc.items = None,
        }
        acc.pair = true;
        let (kv1, vv1) = (self.elem_value(&ki.elem, node, "zk"), self.elem_value(&vi.elem, node, "zv"));
        acc.pairs.push((kv1, vv1));
    }

    /// An sqlite cursor: its remaining rows.
    fn iter_cursor(&mut self, oid: ObjId, node: &Node, state: &mut State, acc: &mut ItemsAcc) {
        if let Some(rows) = crate::models::sqlite::cursor_rows(self, state, node, oid) {
            state.set_slot(oid, Ns::Attr, key_str_lit("\u{1}pos"), Value::data("pos"), true);
            let it = self.iterate(&rows, node, state);
            match (it.items, acc.items.as_mut()) {
                (Some(xs), Some(its)) => its.extend(xs),
                _ => acc.items = None,
            }
            acc.elems.push(it.elem.flat());
        }
    }

    /// A view: keys, values or (key, value) items of its dict; a frame's rows; an object's attributes.
    fn iter_view(&mut self, oid: ObjId, _node: &Node, state: &mut State, acc: &mut ItemsAcc) {
        let Payload::View(vk, recv) = self.heap.obj(oid).payload.clone() else { return };
        if matches!(vk, ViewKind::Rows | ViewKind::Elems | ViewKind::Attrs) {
            acc.items = None;
            acc.exhaustive = false;
            if vk == ViewKind::Rows {
                acc.pair = true;
                acc.pairs.push((Value::data("index"), recv));
            } else {
                acc.elems.push(recv);
            }
            return;
        }
        let kv = KeyVal::KeysOf(KeysOf::new(recv.pts.clone(), Ns::Item));
        let vals = join_values(recv.pts.iter().map(|r| self.record_values(r, state)).collect::<Vec<_>>());
        let lit = if recv.pts.len() == 1 { self.literal_dict_items(recv.pts.first().unwrap(), state) } else { None };
        match (lit, acc.items.as_mut()) {
            (Some(l), Some(its)) => {
                for (k, val) in l {
                    match vk {
                        ViewKind::Keys | ViewKind::Proxy => its.push(Value::lit(k)),
                        ViewKind::Values => its.push(val),
                        _ => {
                            let pid = self.heap.alloc_raw(Kind::Pair, &format!("pair:{}:{}", oid, intern::key_repr(k)), "", true, "pair", Payload::Pair(Value::lit(k), val.clone()));
                            self.heap.obj_mut(pid).payload = Payload::Pair(Value::lit(k), val);
                            its.push(Value::obj(pid));
                        }
                    }
                }
            }
            _ => acc.items = None,
        }
        match vk {
            ViewKind::Keys | ViewKind::Proxy => acc.elems.push(Value::kv_only(kv)),
            ViewKind::Values => acc.elems.push(vals),
            _ => {
                acc.pair = true;
                acc.pairs.push((Value::kv_only(kv), vals));
            }
        }
    }

    /// A dict or frame: its keys (in order when literal).
    fn iter_mapping(&mut self, oid: ObjId, _node: &Node, state: &mut State, acc: &mut ItemsAcc) {
        // (a dict with literal keys, a module constant: its items)
        let lit = self.literal_dict_items(oid, state);
        match (lit, acc.items.as_mut()) {
            (Some(l), Some(its)) => its.extend(l.into_iter().map(|(k, _)| Value::lit(k))),
            _ => acc.items = None,
        }
        acc.elems.push(Value::kv_only(KeyVal::KeysOf(KeysOf::new(ObjSet::one(oid), Ns::Item))));
    }

    /// A list, tuple or set: its positions from the cursor on (a generator partly consumed).
    fn iter_sequence(&mut self, oid: ObjId, node: &Node, state: &mut State, acc: &mut ItemsAcc) {
        let ll = self.heap.llen(oid, state);
        // a generator partly consumed by next(): iteration starts at its cursor
        let cursor = self.cursor_of(oid, state);
        match (ll, acc.items.as_mut(), cursor) {
            (Some(n), Some(its), Some(cursor)) => {
                for i in cursor.min(n)..n {
                    its.push(state.slot(oid, Ns::Pos, key_int(i as i64)));
                }
            }
            _ => acc.items = None,
        }
        acc.elems.push(self.elem_of(&Value::obj(oid), node, state));
    }

    /// Requirements on the loop's own elements are per element, not per program run.
    pub(in crate::interp) fn promote_elem_reqs(&mut self, exit_s: &mut State, body_s: &State, elem: &Value, state: &State) {
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
                if matches!(self.heap.kind(oid), Kind::List | Kind::Tuple) {
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

    /// [(key, value)] for a closed dict whose keys are all definite literals (unrollable).
    /// A loop over a closed dict (or a view of one) whose keys are known in their order but
    /// some only maybe there: its items, each with whether it surely comes (a `for` runs a
    /// maybe item on one path and skips it on the other).
    pub fn maybe_dict_items(&mut self, v: &Value, state: &State) -> Option<Vec<(Value, bool)>> {
        let o = v.single().filter(|_| v.kv.is_bot())?;
        let (d, vk) = match (self.heap.kind(o), &self.heap.obj(o).payload) {
            (Kind::Dict, _) => (o, ViewKind::Keys),
            (Kind::View, Payload::View(vk, recv)) if matches!(vk, ViewKind::Keys | ViewKind::Values | ViewKind::Items | ViewKind::Proxy) => (recv.single().filter(|r| recv.kv.is_bot() && self.heap.kind(*r) == Kind::Dict)?, *vk),
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
                    let pid = self.heap.alloc_raw(Kind::Pair, &format!("pair:{}:{}", o, intern::key_repr(k)), "", true, "pair", Payload::Pair(Value::lit(k), val.clone()));
                    self.heap.obj_mut(pid).payload = Payload::Pair(Value::lit(k), val);
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
            match self.heap.kind(oid) {
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
    pub(in crate::interp) fn record_values(&mut self, oid: u32, state: &mut State) -> Value {
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
            match self.heap.kind(oid) {
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
            if self.heap.kind(oid) != Kind::Dict {
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
        let o_kind = self.heap.kind(oid);
        // a tuple is its own copy (`copy.copy`), and its own deep copy when nothing in it is
        // copied (CPython returns the same object)
        if o_kind == Kind::Tuple && (!deep || state.slots_of(oid).iter().all(|(_, v)| v.pts.iter().all(|x| !matches!(self.heap.kind(x), Kind::Dict | Kind::List | Kind::Tuple | Kind::Set | Kind::Instance)))) {
            seen.insert(oid, oid);
            return oid;
        }
        let kind = if matches!(o_kind, Kind::Dict | Kind::List | Kind::Tuple | Kind::Set | Kind::Instance | Kind::Frame) { o_kind } else { Kind::Dict };
        let tag = if deep { "deep" } else { "copy" };
        // (the original's id in the site: a deep copy's nested copies are distinct objects; a
        // copy of a copy made at this node counts as a copy of the first original)
        let prefix = format!("{}:{}:", tag, self.site_of(node));
        let site_root = self.heap.obj(oid).site.strip_prefix(&prefix).and_then(|rest| rest.split(['#', ':']).nth(1)).and_then(|r| r.parse::<u32>().ok());
        let root = match (self.heap.copy_root.get(&oid), site_root) {
            (Some(&r), _) if self.heap.obj(oid).site.starts_with(&prefix) => r,
            (_, Some(r)) if (r as usize) < self.heap.len() => r,
            _ => oid,
        };
        // (a copy of a copy made elsewhere is named by the first original: `copy of X@L5`)
        let mut base: &str = &self.heap.obj(root).label;
        while let Some(rest) = base.strip_prefix("copy of ").or_else(|| base.strip_prefix("deep of ")) {
            base = rest.rsplit_once("@L").filter(|(_, l)| l.chars().all(|c| c.is_ascii_digit())).map(|(b, _)| b).unwrap_or(rest);
        }
        let label = format!("{} of {}@L{}", tag, base, node.line);
        let payload = self.heap.obj(oid).payload.clone();
        let nid = self.alloc(kind, &format!("{}{}:{}", prefix, kind.name(), root), state, &label, payload, None, if kind == Kind::Instance { Ns::Attr } else { Ns::Item }, None, None);
        self.heap.copy_root.insert(nid, root);
        seen.insert(oid, nid);
        let ll = self.heap.llen(oid, state);
        self.heap.set_llen(nid, state, ll);
        let nsingle = self.heap.obj(nid).single;
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
                    if matches!(self.heap.kind(x), Kind::Dict | Kind::List | Kind::Tuple | Kind::Set | Kind::Instance) {
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

