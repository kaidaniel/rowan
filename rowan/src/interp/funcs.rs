//! Functions, names, targets, iteration (port of the middle part of `rowan/interp.py`).

use std::rc::Rc;

use indexmap::IndexMap;
use ruff_python_ast as ast;

use crate::domain::{join_values, KeyVal, KeysOf, Ns, ObjSet, Row, SourceKind, Value, DATA};
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
    pub fn is_pair(&self) -> bool {
        matches!(self, Elem::Pair(..))
    }
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
    /// The element as a plain value; a pair degrades to nothing (the prototype's `Value()`).
    pub fn one_or_empty(&self) -> Value {
        match self {
            Elem::One(v) => v.clone(),
            Elem::Pair(..) => Value::new(),
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
            self.t_funcs_by_qual.entry(qual.clone()).or_default().push(oid);
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
            if q.ends_with("contextmanager") && !crate::interp::disabled("cmhook") {
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
            if matches!(q.as_str(), "functools.singledispatch" | "functools.singledispatchmethod") && !crate::interp::disabled("sdispatch") {
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
            if KNOWN_IDENTITY_DECORATORS.contains(&q.as_str()) || q.ends_with(".getter") || q.ends_with(".deleter") || q.ends_with(".register") {
                continue;
            }
            if q.starts_with("user:") {
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
                if v.empty() && !self.sum_stack.is_empty() && self.is_entry_object(cid) {
                    return self.path_placeholder_value(cid, Ns::Cell, cell_key(), &Row::new(), state);
                }
                return v;
            }
            let scope = self.frame().scope;
            let mi = self.module_info(fr_module);
            if mi.scopes.get(scope).is_local(&str_get(name)) {
                if let Some(v) = state.env.get(&name) {
                    return v.clone();
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
            // a summary reading a module name that is rebound elsewhere: the value is the one at
            // each call site (a deferred load), not the one when the summary was made
            if fr_kind == FrameKind::Function && self.deferred_global(fr_module, &str_get(name), &v) {
                return self.path_placeholder_value(fr_module, Ns::Attr, intern::key(intern::Key::Str(str_get(name))), &Row::new(), state);
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

    /// A module name a summary reads as a deferred load (its value at each call site): data
    /// rebound elsewhere, or a list / set that functions mutate (their effects land at the
    /// positions the call site has, not those of the state the summary was made in).
    pub fn deferred_global(&self, module: u32, name: &str, v: &Value) -> bool {
        if !self.is_entry_object(module) || crate::interp::disabled("globalrd") {
            return false;
        }
        let data_only = v.pts.iter().all(|o| matches!(self.kind(o), Kind::Dict | Kind::List | Kind::Tuple | Kind::Set | Kind::Instance));
        let mutable_seq = !v.pts.is_empty() && v.pts.iter().all(|o| matches!(self.kind(o), Kind::List | Kind::Set) || (self.kind(o) == Kind::Dict && !crate::interp::disabled("globaldict"))) && !crate::interp::disabled("globalseq") && self.mutated_in_functions(module, name);
        data_only && (mutable_seq || self.module_info(module).scopes.rebound.contains(name))
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
        // flow-sensitive state is re-bound every iteration anyway); the prototype keeps it weak
        let strong = self.frame().loop_depth == 0 || self.opts.recency;
        state.set_slot(fr_module, Ns::Attr, key, v, strong);
        let r = state.row(fr_module, Ns::Attr).store(key, strong);
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
                    if obj.pts.len() > 1 && obj.pts.iter().all(|o| self.kind(o) == Kind::Dict) && !crate::interp::disabled("namekeys") {
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
                    let mut sv = state.slot(oid, Ns::Pos, key_int(i as i64));
                    if sv.empty() && self.is_entry_object(oid) {
                        // an entry tuple inside a summary: its positions are deferred loads
                        sv = self.load_item(&Value::obj(oid), Ns::Item, &Value::lit(key_int(i as i64)), node, state, false);
                    }
                    parts[i] = parts[i].join(&sv);
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
            } else if kind == Kind::Ph {
                // unpacking a placeholder: its elements (an iteration event for the call site)
                let e = self.iterate(&Value::obj(oid), node, state).elem.flat();
                for i in 0..n {
                    parts[i] = parts[i].join(&e);
                }
            } else {
                // an iterable whose items are known by position (zip / enumerate of known lists)
                if matches!(kind, Kind::Zip | Kind::Enumerate) && !crate::interp::disabled("unpackzip") {
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
        if v.pts.is_empty() {
            // positional slots for literal tuple keys: `a, b = ("x", 1)` / `for k, ns in keys`
            // where the keys are tuple literals (Options::positional)
            if self.opts.positional {
                if let Some(cols) = tuple_key_columns(&v.kv, n) {
                    return cols;
                }
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
    pub fn iterate(&mut self, v: &Value, node: &Node, state: &mut State) -> Iteration {
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
                    let cursor = state.slot_opt(oid, Ns::Attr, key_str_lit("__cursor")).and_then(|v| v.kv.as_lit()).and_then(intern::key_as_int).unwrap_or(0).max(0) as usize;
                    match (ll, items.as_mut()) {
                        (Some(n), Some(its)) => {
                            for i in cursor.min(n)..n {
                                its.push(state.slot(oid, Ns::Pos, key_int(i as i64)));
                            }
                        }
                        _ => items = None,
                    }
                    elems.push(self.elem_of(&Value::obj(oid), node, state));
                }
                Kind::Dict | Kind::Frame => {
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
                    let vals = join_values(recv.pts.iter().map(|r| self.all_values(r, Ns::Item, state)));
                    let lit = if recv.pts.len() == 1 { self.literal_dict_items(recv.pts.first().unwrap(), state) } else { None };
                    match (lit, items.as_mut()) {
                        (Some(l), Some(its)) => {
                            for (k, val) in l {
                                match vk {
                                    ViewKind::Keys => its.push(Value::lit(k)),
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
                        ViewKind::Keys => elems.push(Value::kv_only(kv)),
                        ViewKind::Values => elems.push(vals),
                        _ => {
                            pair = true;
                            pairs.push((Value::kv_only(kv), vals));
                        }
                    }
                }
                // itertools.count / cycle / repeat: no end
                Kind::Namespace if self.infinites.contains_key(&oid) => {
                    items = None;
                    exhaustive = false;
                    let inf = self.infinites[&oid].clone();
                    elems.push(self.infinite_elem(&inf));
                }
                Kind::Zip => {
                    let Payload::Zip(ks, vs) = self.obj(oid).payload.clone() else { continue };
                    let mut ki = self.iterate(&ks, node, state);
                    let mut vi = self.iterate(&vs, node, state);
                    // an infinite side yields as many items as the other has
                    if let (Some(inf), Some(n)) = (self.infinite_of(&ks), vi.items.as_ref().map(|x| x.len())) {
                        ki.items = self.infinite_take(&inf, n);
                    }
                    if let (Some(inf), Some(n)) = (self.infinite_of(&vs), ki.items.as_ref().map(|x| x.len())) {
                        vi.items = self.infinite_take(&inf, n);
                    }
                    // a strict zip that did not raise: both sides as long as the known one
                    let strict = self.zip_strict.contains(&oid);
                    let (ki_items, vi_items) = match (ki.items.clone(), vi.items.clone()) {
                        (Some(k), None) if strict => {
                            let e = vi.elem.flat_or_one();
                            let n = k.len();
                            (Some(k), Some(vec![e; n]))
                        }
                        (None, Some(v)) if strict => {
                            let e = ki.elem.flat_or_one();
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
                    pairs.push((ki.elem.flat_or_one(), vi.elem.flat_or_one()));
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
                    pairs.push((Value::data("index"), ii.elem.one_or_empty()));
                }
                Kind::Class if self.enum_members.contains_key(&oid) => {
                    // an Enum class: its members in order
                    let ms: Vec<Value> = self.enum_members[&oid].iter().map(|(_, m)| Value::obj(*m)).collect();
                    if let Some(its) = items.as_mut() {
                        its.extend(ms.iter().cloned());
                    }
                    elems.push(join_values(ms));
                }
                Kind::Instance if self.instance_class(oid).and_then(|c| self.class_lookup(c, "__iter__")).is_some() && !crate::interp::disabled("useriter") => {
                    // a user iterable: what its `__iter__` returns
                    let r = self.call_method_with(&Value::obj(oid), "__iter__", Vec::new(), node, state).unwrap_or_default();
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
                Kind::Ph => {
                    items = None;
                    exhaustive = false;
                    self.record_event("iter", oid);
                    if self.obj(oid).depth >= super::limit("path", PATH_DEPTH as usize) as u32 {
                        elems.push(Value::data("deep"));
                        continue;
                    }
                    let eid = self.path_placeholder(oid, Ns::Item, STAR, state, None, "");
                    elems.push(Value::with_kv(ObjSet::one(eid), KeyVal::Param(eid)));
                }
                Kind::Unknown => {
                    items = None;
                    exhaustive = false;
                    if self.obj(oid).depth >= super::limit("path", PATH_DEPTH as usize) as u32 {
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
                        let r = Row::from_data(crate::domain::RowData { sources: srcs, mutators: row.mutators.clone(), ..Default::default() });
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
                if ks.len() <= UNROLL_MAX {
                    items = Some(intern::sorted_by_str(ks.iter()).into_iter().map(Value::lit).collect());
                }
                elems.push(Value::kv_only(KeyVal::lits(ks)));
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
            return Iteration { items, elem: Elem::Pair(kacc, vacc), exhaustive: exh };
        }
        let elem = join_values(elems.iter().cloned());
        let exh = exhaustive && elems.len() == 1 && matches!(elem.kv, KeyVal::KeysOf(_));
        Iteration { items, elem: Elem::One(elem), exhaustive: exh }
    }

    /// Requirements on the loop's own elements are per element, not per program run.
    pub fn promote_elem_reqs(&mut self, exit_s: &mut State, body_s: &State, elem: &Value, state: &State) {
        if !body_s.reachable {
            return;
        }
        let mut srcs: Vec<crate::domain::SourceId> = Vec::new();
        for oid in elem.pts.iter() {
            for ns in [Ns::Item, Ns::Attr] {
                for (src, _) in state.row(oid, ns).sources.iter() {
                    if !crate::domain::source_is_data(src) && !srcs.contains(&src) {
                        srcs.push(src);
                    }
                }
            }
        }
        for src in srcs {
            let Some(d) = body_s.reqs.inner(&src) else { continue };
            for (k, info) in d.iter() {
                let Some(cur_d) = exit_s.reqs.inner(&src) else { continue };
                if info.always {
                    if let Some(cur) = cur_d.get(k) {
                        if !cur.always {
                            let nv = crate::domain::ReqInfo { always: true, ..cur.clone() };
                            exit_s.reqs.writable(&src).insert(*k, nv);
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
    pub fn literal_dict_items(&mut self, oid: u32, state: &State) -> Option<Vec<(KeyId, Value)>> {
        if self.kind(oid) == Kind::Ph {
            self.record_event("iter", oid);
            return None;
        }
        let row = state.row(oid, Ns::Item);
        // an empty closed dict iterates zero times (known), like an empty list
        if row.open() || row.present != row.definite || row.present.len() > UNROLL_MAX {
            return None;
        }
        if state.has_slot(oid, Ns::Item, STAR) {
            return None;
        }
        // insertion order when it is known (else a fixed order by key)
        let keys = row.ordered_keys().unwrap_or_else(|| intern::sorted_by_str(row.present.iter()));
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

    /// Finite literal key set denoted by a value, or None.
    pub fn lit_keys(&mut self, v: &Value, state: &State) -> Option<KeySet> {
        if v.kv.is_literal() && v.pts.is_empty() {
            return Some(crate::domain::kv_literals(&v.kv));
        }
        let mut out = KeySet::new();
        for oid in v.pts.iter() {
            match self.kind(oid) {
                Kind::Ph => {
                    self.record_event("concrete", oid);
                    return None;
                }
                Kind::List | Kind::Tuple | Kind::Set => match state.elems.get(&oid) {
                    Some(kv) if kv.is_literal() => out = out.union(&crate::domain::kv_literals(&kv)),
                    _ => {
                        if self.is_entry_object(oid) && !crate::interp::disabled("entrymapping") {
                            self.record_event("concrete", oid);
                        }
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
            if self.kind(oid) == Kind::Ph {
                self.record_event("concrete", oid);
                return None;
            }
            if self.kind(oid) != Kind::Dict {
                return None;
            }
            let row = state.row(oid, Ns::Item);
            if row.symbolic() && self.is_entry_object(oid) && !crate::interp::disabled("entrymapping") {
                // a dict from before the call (a module constant): known at the call site
                self.record_event("concrete", oid);
                return None;
            }
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
        let label = format!("{} of {}@L{}", tag, self.obj(oid).label, node.line);
        let payload = self.obj(oid).payload.clone();
        // (the original's id in the site: a deep copy's nested copies are distinct objects)
        let nid = self.alloc(kind, &format!("{}:{}:{}:{}", tag, self.site_of(node), kind.name(), oid), state, &label, payload, None, if kind == Kind::Instance { Ns::Attr } else { Ns::Item }, None, None);
        seen.insert(oid, nid);
        if self.is_entry_object(oid) {
            self.obj_mut(nid).copied_from = Some(oid);
        }
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

impl Elem {
    /// For zip: the element of the inner iteration as one value (a pair's key joined with
    /// its value — the prototype uses the key part for pairs, `kelem`).
    pub fn flat_or_one(&self) -> Value {
        match self {
            Elem::One(v) => v.clone(),
            Elem::Pair(k, _) => k.clone(),
        }
    }
}
