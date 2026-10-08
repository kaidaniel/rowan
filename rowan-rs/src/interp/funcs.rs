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
            generator: std::cell::Cell::new(None),
        };
        let n = node.node();
        let oid = self.alloc_at(Kind::Function, &n, state, &format!("def {}", qual), Payload::Func(Rc::new(info)), None, Ns::Item, None, None);
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
            if q.ends_with("property") || q == "builtins.property" {
                if let Some(fi) = &fi {
                    fi.is_property.set(true);
                }
                continue;
            }
            if q.ends_with("staticmethod") {
                if let Some(fi) = &fi {
                    fi.is_static.set(true);
                }
                continue;
            }
            if q.ends_with("classmethod") {
                if let Some(fi) = &fi {
                    fi.is_classmethod.set(true);
                }
                continue;
            }
            if KNOWN_IDENTITY_DECORATORS.contains(&q.as_str()) || q.ends_with(".setter") || q.ends_with(".getter") || q.ends_with(".deleter") || q.ends_with(".register") {
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
                let at = if self.opts.fixes { frontend::node(d) } else { fnode };
                fv = self.call_value(&dv, &at, None, state, Some(vec![fv.clone()]));
                continue;
            }
            let txt = if q.is_empty() { frontend::text(&d.expression) } else { q.clone() };
            self.note(format!("unknown decorator {} at L{}: treated as identity", txt, fnode.line));
        }
        fv
    }

    pub fn call_method_if_defined(&mut self, v: &Value, name: &str, node: &Node, state: &mut State) -> Option<Value> {
        let mut outs: Vec<Value> = Vec::new();
        for oid in v.pts.iter() {
            if self.kind(oid) == Kind::Instance {
                if let Some(cls) = self.instance_class(oid) {
                    if let Some(m) = self.class_lookup(cls, name) {
                        let fv = self.cls_slots.get(&(m, Ns::Attr, key_str_lit(name))).cloned().unwrap_or_default();
                        for f in fv.pts.iter() {
                            if self.kind(f) == Kind::Function {
                                outs.push(self.call_user(f, Some(Value::obj(oid)), vec![], IndexMap::new(), vec![], vec![], state, node, None, false));
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

    pub fn store_name(&mut self, name: StrId, v: Value, state: &mut State) {
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
                self.store_name(name, v.clone(), state);
                if self.facts_on {
                    let nd = frontend::node(t);
                    self.record_fact(&nd, name, v, state);
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

    pub fn unpack(&mut self, v: &Value, n: usize, state: &mut State, node: &Node) -> Vec<Value> {
        let mut parts: Vec<Value> = (0..n).map(|_| Value::new()).collect();
        for oid in v.pts.iter() {
            let kind = self.kind(oid);
            let ll = self.obj(oid).literal_len;
            if matches!(kind, Kind::Tuple | Kind::List) && ll.is_some() {
                for i in 0..n {
                    parts[i] = parts[i].join(&state.slot(oid, Ns::Pos, key_int(i as i64)));
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
                    let ll = self.obj(oid).literal_len;
                    match (ll, items.as_mut()) {
                        (Some(n), Some(its)) => {
                            for i in 0..n {
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
                    if matches!(vk, ViewKind::Rows | ViewKind::Elems) {
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
                Kind::Zip => {
                    let Payload::Zip(ks, vs) = self.obj(oid).payload.clone() else { continue };
                    let ki = self.iterate(&ks, node, state);
                    let vi = self.iterate(&vs, node, state);
                    match (ki.items, vi.items, items.as_mut()) {
                        (Some(kit), Some(vit), Some(its)) if kit.len() == vit.len() => {
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
                    let Payload::Enumerate(inner) = self.obj(oid).payload.clone() else { continue };
                    let ii = self.iterate(&inner, node, state);
                    items = None;
                    pair = true;
                    pairs.push((Value::data("index"), ii.elem.one_or_empty()));
                }
                Kind::Ph => {
                    items = None;
                    exhaustive = false;
                    self.record_event("iter", oid);
                    if self.obj(oid).depth >= PATH_DEPTH {
                        elems.push(Value::data("deep"));
                        continue;
                    }
                    let eid = self.path_placeholder(oid, Ns::Item, STAR, state, None, "");
                    elems.push(Value::with_kv(ObjSet::one(eid), KeyVal::Param(eid)));
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
            if v.kv.is_literal() {
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
            if its.is_empty() {
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
        if row.open() || row.present != row.definite || row.present.len() > UNROLL_MAX || row.present.is_empty() {
            return None;
        }
        if state.has_slot(oid, Ns::Item, STAR) {
            return None;
        }
        Some(intern::sorted_by_str(row.present.iter()).into_iter().map(|k| (k, state.slot(oid, Ns::Item, k))).collect())
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
                    _ => return None,
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
        let nid = self.alloc(kind, &format!("{}:{}:{}", tag, self.site_of(node), kind.name()), state, &label, payload, None, if kind == Kind::Instance { Ns::Attr } else { Ns::Item }, None, None);
        seen.insert(oid, nid);
        let ll = self.obj(oid).literal_len;
        self.obj_mut(nid).literal_len = ll;
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
