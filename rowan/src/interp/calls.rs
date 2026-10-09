//! Calls: dispatch, unknown calls, construction, user functions, bindings (port of the
//! "calls" section of `rowan/interp.py`).

use std::rc::Rc;

use indexmap::IndexMap;
use ruff_python_ast as ast;

use crate::domain::{chain_extend, join_rows, join_values, Classification, KeyVal, Ns, ReqInfo, Row, Source, SourceKind, Tier, Value, DATA};
use crate::frontend::Node;
use crate::intern::{self, key_str_lit, s, KeyId, KeySet, StrId};
use crate::state::{Footprint, Kind, State};
use crate::domain::ObjId;

use super::funcs::cell_key;
use super::{is_generator, Analyzer, Call, Frame, FrameKind, FuncInfo, FuncNode, Payload, Verdict, MAX_ACTIVE, MAX_CLONES, MAX_MUTATED, NOEFFECT_METHOD_NAMES, REC_MAX_ITER};

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Sig {
    foid: u32,
    site: String,
    entry: bool,
    self_v: Option<Value>,
    args: Vec<Value>,
    kwargs: Vec<(StrId, Value)>,
    star_args: Vec<Value>,
    star_kwargs: Vec<Value>,
    heap: Vec<(u32, Option<Row>, Option<Row>, Vec<((Ns, KeyId), Value)>, Option<KeyVal>, bool)>,
    guards: Vec<Vec<String>>,
    fr_single: bool,
    ctx: String,
}

pub struct CacheHit {
    pub raises: bool,
    pub ret: Value,
    pub fp_state: State,
    pub fp: Footprint,
    pub verdicts: Vec<Verdict>,
}

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
        if override_args.is_some() {
            self.model_call += 1;
            let r = self.call_value_inner(fv, node, call_ast, state, override_args);
            self.model_call -= 1;
            return r;
        }
        self.call_value_inner(fv, node, call_ast, state, None)
    }

    fn call_value_inner(&mut self, fv: &Value, node: &Node, call_ast: Option<&'static ast::ExprCall>, state: &mut State, override_args: Option<Vec<Value>>) -> Value {
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
        if fv.pts.is_empty() {
            return self.unknown_call("?", None, &args, &kwargs, &star_args, &star_kwargs, state, node);
        }
        let targets: Vec<u32> = fv.pts.to_vec();
        if targets.len() == 1 {
            return self.call_one(targets[0], &args, &kwargs, &star_args, &star_kwargs, state, node, call_ast);
        }
        let mut pre = state.copy();
        let mut outs: Vec<Value> = Vec::new();
        let mut states: Vec<State> = Vec::new();
        for oid in targets {
            let mut st = pre.copy();
            outs.push(self.call_one(oid, &args, &kwargs, &star_args, &star_kwargs, &mut st, node, call_ast));
            states.push(st);
        }
        let mut out = states.remove(0);
        for mut st in states {
            out = out.join(&mut st);
        }
        state.replace_with(out);
        join_values(outs)
    }

    pub fn call_one(&mut self, oid: u32, args: &[Value], kwargs: &IndexMap<StrId, Value>, star_args: &[Value], star_kwargs: &[Value], state: &mut State, node: &Node, call_ast: Option<&'static ast::ExprCall>) -> Value {
        let kind = self.kind(oid);
        // torch: a layer object / Sequential, or a user nn.Module (its forward)
        if matches!(kind, Kind::Namespace | Kind::List) && self.t_layer_type(oid).is_some() {
            return self.t_layer_call(oid, args, kwargs, node, state);
        }
        if kind == Kind::Instance && self.torch_on && self.t_is_module_instance(oid) {
            if let Some(cls) = self.instance_class(oid) {
                if self.class_lookup(cls, "__call__").is_none() {
                    if let Some(m) = self.class_lookup(cls, "forward") {
                        let f = self.cls_slots.get(&(m, Ns::Attr, key_str_lit("forward"))).cloned().unwrap_or_default();
                        let mut outs = Vec::new();
                        for x in f.pts.iter() {
                            if self.kind(x) == Kind::Function {
                                outs.push(self.call_user(x, Some(Value::obj(oid)), args.to_vec(), kwargs.clone(), star_args.to_vec(), star_kwargs.to_vec(), state, node, call_ast, false));
                            }
                        }
                        return join_values(outs);
                    }
                }
            }
        }
        match kind {
            Kind::Function if self.func_info(oid).is_memo.get() && self.sum_stack.is_empty() && star_args.is_empty() && star_kwargs.is_empty() && !super::disabled("memo") => {
                // a memoised function with known arguments: the first call's result is the
                // result of every later call with the same arguments
                let lit = |v: &Value| v.kv.as_lit().filter(|_| v.pts.is_empty());
                let pos: Option<Vec<KeyId>> = args.iter().map(lit).collect();
                let kws: Option<Vec<(StrId, KeyId)>> = kwargs.iter().map(|(k, v)| lit(v).map(|x| (*k, x))).collect();
                // the cache is a hidden dict in the (flow-sensitive) state: on paths where the
                // first call happened the result is reused, elsewhere the call runs
                let key = pos.zip(kws).map(|(p, k)| {
                    let mut parts = p;
                    for (n, v) in k {
                        parts.push(intern::key_tuple(vec![key_str_lit(&intern::str_get(n)), v]));
                    }
                    intern::key_tuple(parts)
                });
                let Some(mk) = key else {
                    return self.call_user(oid, None, args.to_vec(), kwargs.clone(), vec![], vec![], state, node, call_ast, false);
                };
                let memo = self.alloc_raw(Kind::Dict, &format!("memo:{}", oid), "", true, "lru_cache", Payload::None);
                let row = state.row(memo, Ns::Item);
                if row.definite.contains(mk) {
                    return state.slot(memo, Ns::Item, mk);
                }
                let r = self.call_user(oid, None, args.to_vec(), kwargs.clone(), vec![], vec![], state, node, call_ast, false);
                let out = if row.present.contains(mk) { r.join(&state.slot(memo, Ns::Item, mk)) } else { r };
                let row = state.row(memo, Ns::Item).store(mk, true);
                state.set_row(memo, Ns::Item, row);
                state.set_slot(memo, Ns::Item, mk, out.clone(), true);
                out
            }
            Kind::Function => {
                let fi = self.func_info(oid);
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
                let Payload::Bound(f, self_v) = self.obj(oid).payload.clone() else { return Value::new() };
                self.call_user(f, Some(self_v), args.to_vec(), kwargs.clone(), star_args.to_vec(), star_kwargs.to_vec(), state, node, call_ast, false)
            }
            Kind::Class => self.construct(oid, args, kwargs, star_args, star_kwargs, state, node),
            Kind::External => {
                let qual = self.external_qual(oid);
                let call = Call { qual: qual.clone(), recv: None, args: args.to_vec(), kwargs: kwargs.clone(), star_args: star_args.to_vec(), star_kwargs: star_kwargs.to_vec(), node: *node, call_ast, site: format!("L{}", node.line), line: node.line };
                if let Some(m) = self.models.lookup(&qual) {
                    if qual.starts_with("pandas.") && !self.sum_stack.is_empty() && crate::models::pandas::frame_producer(&qual).is_none() {
                        // a frame built from a parameter's contents: decided per call site
                        for v in args.iter().chain(kwargs.values()) {
                            self.concrete_events(v, state, 2);
                        }
                        let vals: Vec<&Value> = args.iter().chain(kwargs.values()).collect();
                        crate::models::pandas::symbolic_frames_event(self, state, &vals);
                    }
                    return m(self, state, &call);
                }
                self.unknown_call(&qual, None, args, kwargs, star_args, star_kwargs, state, node)
            }
            Kind::Method => {
                let Payload::Method(qual, recv) = self.obj(oid).payload.clone() else { return Value::new() };
                // `super().__init__(…)` / `Base.__init__(self, …)` reaching a dataclass base
                if let Some(rest) = qual.strip_prefix("dcinit.") {
                    let (cls, self_v, args): (u32, Value, Vec<Value>) = match rest.strip_prefix("class.") {
                        Some(c) => (c.parse().unwrap_or(0), args.first().cloned().unwrap_or_default(), args.iter().skip(1).cloned().collect()),
                        None => (rest.parse().unwrap_or(0), recv.clone(), args.to_vec()),
                    };
                    if cls != 0 {
                        for o in self_v.pts.iter() {
                            if matches!(self.kind(o), Kind::Instance | Kind::Ph) {
                                self.dataclass_init(cls, o, &args, kwargs, star_kwargs, state, node);
                            }
                        }
                    }
                    return self.none_v.clone();
                }
                let call = Call { qual: qual.clone(), recv: Some(recv.clone()), args: args.to_vec(), kwargs: kwargs.clone(), star_args: star_args.to_vec(), star_kwargs: star_kwargs.to_vec(), node: *node, call_ast, site: format!("L{}", node.line), line: node.line };
                if let Some(m) = self.models.lookup(&qual) {
                    return m(self, state, &call);
                }
                self.unknown_call(&qual, Some(&recv), args, kwargs, star_args, star_kwargs, state, node)
            }
            Kind::Unknown => {
                let q = format!("call of {}", self.obj(oid).label);
                self.unknown_call(&q, Some(&Value::obj(oid)), args, kwargs, star_args, star_kwargs, state, node)
            }
            Kind::Partial => {
                let call = Call { qual: "partial.__call__".into(), recv: Some(Value::obj(oid)), args: args.to_vec(), kwargs: kwargs.clone(), star_args: star_args.to_vec(), star_kwargs: star_kwargs.to_vec(), node: *node, call_ast, site: format!("L{}", node.line), line: node.line };
                crate::models::partial_call(self, state, &call)
            }
            Kind::Ph => {
                self.record_event("call", oid);
                let q = format!("call of {}", self.obj(oid).label);
                self.unknown_call(&q, Some(&Value::obj(oid)), args, kwargs, star_args, star_kwargs, state, node)
            }
            Kind::Instance => {
                if let Some(cls) = self.instance_class(oid) {
                    if let Some(m) = self.class_lookup(cls, "__call__") {
                        let f = self.cls_slots.get(&(m, Ns::Attr, key_str_lit("__call__"))).cloned().unwrap_or_default();
                        if f.pts.is_empty() {
                            return Value::new();
                        }
                        let mut outs = Vec::new();
                        for x in f.pts.iter() {
                            if self.kind(x) == Kind::Function {
                                outs.push(self.call_user(x, Some(Value::obj(oid)), args.to_vec(), kwargs.clone(), star_args.to_vec(), star_kwargs.to_vec(), state, node, call_ast, false));
                            }
                        }
                        return join_values(outs);
                    }
                }
                let q = format!("call of {}", self.obj(oid).label);
                self.unknown_call(&q, Some(&Value::obj(oid)), args, kwargs, star_args, star_kwargs, state, node)
            }
            _ => {
                let q = format!("call of {}", self.obj(oid).label);
                self.unknown_call(&q, None, args, kwargs, star_args, star_kwargs, state, node)
            }
        }
    }

    pub fn unknown_call(&mut self, qual: &str, recv: Option<&Value>, args: &[Value], kwargs: &IndexMap<StrId, Value>, star_args: &[Value], star_kwargs: &[Value], state: &mut State, node: &Node) -> Value {
        let site = format!("{}@L{}", qual, node.line);
        // unknown code may raise
        if crate::interp::debug_level() == 2 {
            eprintln!("  unknown call {} in {} (may raise)", site, self.frames.last().map(|f| f.label.clone()).unwrap_or_default());
        }
        if let Some(t) = self.raise_stack.last_mut() {
            *t = true;
        }
        if let Some(m) = self.may_raise.last_mut() {
            *m = true;
        }
        // unknown code may remove keys stored through names
        state.drop_name_keys(None);
        // unknown code may change what an earlier test read: facts about it no longer hold
        if !state.facts.is_empty() {
            let mut objs: Vec<u32> = recv.map(|r| r.pts.to_vec()).unwrap_or_default();
            for v in args.iter().chain(kwargs.values()).chain(star_args.iter()).chain(star_kwargs.iter()) {
                objs.extend(v.pts.iter());
            }
            self.kill_facts_on(&objs, state);
        }
        let name = qual.rsplit('.').next().unwrap_or(qual);
        if NOEFFECT_METHOD_NAMES.contains(&name) && recv.map(|r| self.record_objects(r).is_empty()).unwrap_or(true) {
            return Value::data(&format!("L{}", node.line));
        }
        if !state.narrowed.is_empty() {
            state.narrowed.clear();
        }
        let mut vals: Vec<Value> = args.to_vec();
        vals.extend(kwargs.values().cloned());
        vals.extend(star_args.iter().cloned());
        vals.extend(star_kwargs.iter().cloned());
        if let Some(r) = recv {
            vals.push(r.clone());
        }
        let touched = self.reachable_records(&vals, state, 1);
        let site_id = s(&site);
        for (oid, ns) in touched.iter().take(MAX_MUTATED) {
            let r = state.row(*oid, *ns).mutate(site_id);
            state.set_row(*oid, *ns, r);
        }
        let arg_recs: Vec<(u32, Ns)> = touched.iter().copied().filter(|(oid, _)| vals.iter().any(|v| v.pts.contains(*oid))).collect();
        let rid = self.alloc(Kind::Unknown, &format!("ret:{}", site), state, &format!("result of {}", site), Payload::None, None, Ns::Item, Some(true), None);
        // for people: the call as written and the known objects it was given (unknown code may
        // return one of them; given unknowns add nothing to `?`)
        {
            let text = crate::frontend::text_at(node);
            let mut given: Vec<String> = Vec::new();
            for (o, _) in &arg_recs {
                if matches!(self.kind(*o), Kind::Unknown | Kind::Ph) {
                    continue;
                }
                let g = self.obj(*o).label.clone();
                if !given.contains(&g) {
                    given.push(g);
                }
            }
            if !text.is_empty() {
                let t = text.split_whitespace().collect::<Vec<_>>().join(" ");
                let scope = self.frame().func.unwrap_or(0);
                self.set_symbol(rid, t.clone(), scope);
                self.origins.insert(rid, (t, node.line, given));
            }
        }
        if !arg_recs.is_empty() {
            let row = join_rows(arg_recs.iter().map(|(o, n)| state.row(*o, *n)));
            let mut d = row.data();
            d.definite = KeySet::new();
            d.absent = KeySet::new();
            d.mutators = d.mutators.insert(site_id);
            d.lterms = d.lterms.union(&d.dterms);
            d.dterms = Default::default();
            d.pstores = d.pstores.map(|(r, _, l)| (r, false, l));
            state.set_row(rid, Ns::Item, Row::from_data(d));
            // as an object the result is unknown: its attributes come from the call
            let src = Source::new(&site, SourceKind::Call);
            state.set_row(rid, Ns::Attr, Row::with_source(src).mutate(site_id));
            for (oid, ns) in &arg_recs {
                for (k, v) in state.obj_slots(*oid, *ns) {
                    state.set_slot(rid, Ns::Item, k, v, false);
                }
            }
        } else {
            let src = Source::new(&site, SourceKind::Call);
            state.set_row(rid, Ns::Item, Row::with_source(src));
            state.set_row(rid, Ns::Attr, Row::with_source(src));
        }
        self.note(format!("unknown call {}", site));
        Value::obj(rid)
    }

    pub fn reachable_records(&self, vals: &[Value], state: &State, depth: u32) -> Vec<(u32, Ns)> {
        let mut out: Vec<(u32, Ns)> = Vec::new();
        let mut seen: rustc_hash::FxHashSet<u32> = rustc_hash::FxHashSet::default();
        let mut work: Vec<(u32, u32)> = vals.iter().flat_map(|v| v.pts.iter().map(|o| (o, 0u32)).collect::<Vec<_>>()).collect();
        while let Some((oid, d)) = work.pop() {
            if !seen.insert(oid) {
                continue;
            }
            let k = self.kind(oid);
            if matches!(k, Kind::Dict | Kind::Frame | Kind::Unknown | Kind::Ph) {
                out.push((oid, Ns::Item));
            }
            if matches!(k, Kind::Instance | Kind::Unknown | Kind::Ph) {
                out.push((oid, Ns::Attr));
            }
            if d < depth {
                for (_, v) in state.slots_of(oid) {
                    work.extend(v.pts.iter().map(|x| (x, d + 1)));
                }
            }
        }
        out
    }

    pub fn construct(&mut self, cls: u32, args: &[Value], kwargs: &IndexMap<StrId, Value>, star_args: &[Value], star_kwargs: &[Value], state: &mut State, node: &Node) -> Value {
        let ci = self.class_info(cls);
        // a TypedDict class makes a plain dict
        if ci.typed_dict && !super::disabled("typeddictcall") {
            let call = Call { qual: "builtins.dict".into(), recv: None, args: args.to_vec(), kwargs: kwargs.clone(), star_args: star_args.to_vec(), star_kwargs: star_kwargs.to_vec(), node: *node, call_ast: None, site: format!("L{}", node.line), line: node.line };
            if let Some(m) = self.models.lookup("builtins.dict") {
                return m(self, state, &call);
            }
        }
        let row = if ci.open_base { Row::with_source(Source::new(&format!("instance {}@L{}", ci.name, node.line), SourceKind::Class)) } else { Row::new() };
        // the allocation site includes the class: `cls()` with several possible classes must
        // not collapse onto one instance (prototype: same site, last class wins)
        let site = format!("{}:{}", super::site_string(node), ci.name);
        let inst = self.alloc(Kind::Instance, &site, state, &format!("{}@L{}", ci.name, node.line), Payload::Cls(cls), Some(row), Ns::Attr, None, None);
        if ci.typed_dict {
            state.set_row(inst, Ns::Item, Row::new());
        }
        // a dict subclass without its own `__init__`: dict's constructor fills its items
        let dict_based = self.mro(cls).iter().any(|c| self.class_info_opt(*c).map(|x| x.dict_base).unwrap_or(false));
        if dict_based && self.class_lookup(cls, "__init__").is_none() && !super::disabled("dictsub") {
            state.set_row(inst, Ns::Item, Row::new());
            if let Some(a0) = args.first() {
                if !self.record_objects(a0).is_empty() {
                    self.copy_into(inst, Ns::Item, a0, state, true, &crate::intern::KeySet::new());
                } else {
                    let r = state.row(inst, Ns::Item).data_store(Source::new(&format!("{}@L{}", ci.name, node.line), SourceKind::Data));
                    state.set_row(inst, Ns::Item, r);
                }
            }
            for (k, v) in kwargs.iter() {
                self.store_key(inst, Ns::Item, &KeyVal::Lit(intern::key(intern::Key::Str(intern::str_get(*k)))), v, true, node, state);
            }
            for sk in star_kwargs {
                self.copy_into(inst, Ns::Item, sk, state, true, &crate::intern::KeySet::new());
            }
            return Value::obj(inst);
        }
        // the generated `__init__` of a dataclass base applies to a subclass without its own
        let dc_init = ci.has_fields() || {
            let mut owner: Option<bool> = None;
            for c in self.mro(cls) {
                if self.cls_slots.contains_key(&(c, Ns::Attr, key_str_lit("__init__"))) {
                    owner = Some(false);
                    break;
                }
                if self.class_info_opt(c).map(|x| x.has_fields()).unwrap_or(false) {
                    owner = Some(true);
                    break;
                }
            }
            owner == Some(true)
        };
        if dc_init {
            self.dataclass_init(cls, inst, args, kwargs, star_kwargs, state, node);
            return Value::obj(inst);
        }
        if let Some(init) = self.class_lookup(cls, "__init__") {
            let fv = self.cls_slots.get(&(init, Ns::Attr, key_str_lit("__init__"))).cloned().unwrap_or_default();
            self.fresh_init.insert(inst);
            for f in fv.pts.iter() {
                if self.kind(f) == Kind::Function {
                    self.note_ctor_field_classes(init, f, args, kwargs);
                    self.call_user(f, Some(Value::obj(inst)), args.to_vec(), kwargs.clone(), star_args.to_vec(), star_kwargs.to_vec(), state, node, None, false);
                }
            }
            self.fresh_init.remove(&inst);
        }
        Value::obj(inst)
    }

    /// The generated `__init__` of a dataclass / TypedDict / NamedTuple `cls` run on `inst`
    /// (a fresh instance, or `self` when called explicitly as `Base.__init__(self, …)` /
    /// `super().__init__(…)` from a subclass).
    pub fn dataclass_init(&mut self, cls: u32, inst: ObjId, args: &[Value], kwargs: &IndexMap<StrId, Value>, star_kwargs: &[Value], state: &mut State, node: &Node) {
        let ci = self.class_info(cls);
        let ns = if ci.typed_dict { Ns::Item } else { Ns::Attr };
        {
            let fields = self.dataclass_fields_mro(cls);
            for (i, (fname, default)) in fields.iter().enumerate() {
                let fk = key_str_lit(fname);
                let mut v: Option<Value> = None;
                if i < args.len() {
                    v = Some(args[i].clone());
                } else if let Some(kv) = kwargs.get(&s(fname)) {
                    v = Some(kv.clone());
                } else if !star_kwargs.is_empty() {
                    // `**data` supplies the field when it has the key (over the default); a
                    // field without a default must come from it: a read (a requirement on an
                    // open input)
                    let present = star_kwargs.iter().any(|sk| sk.pts.iter().any(|x| state.row(x, Ns::Item).present.contains(fk)));
                    let definite = star_kwargs.iter().all(|sk| !sk.pts.is_empty() && sk.pts.iter().all(|x| state.row(x, Ns::Item).definite.contains(fk)));
                    let mut acc = join_values(star_kwargs.iter().flat_map(|sk| sk.pts.iter().map(|x| state.slot(x, Ns::Item, fk)).collect::<Vec<_>>()));
                    match default {
                        Some(d) if !definite => {
                            let dv = self.dataclass_default(d, node, state);
                            acc = if present { acc.join(&dv) } else { dv.join(&acc) };
                        }
                        Some(_) => {}
                        None if !definite && !super::disabled("kwreq") => {
                            for sk in star_kwargs {
                                let recs: Vec<u32> = self.record_objects(sk).into_iter().filter(|o| state.row(*o, Ns::Item).open()).collect();
                                if !recs.is_empty() {
                                    let lv = self.load_item(&Value::with_kv(crate::domain::ObjSet::from_vec(recs), sk.kv.clone()), Ns::Item, &Value::lit(fk), node, state, true);
                                    acc = acc.join(&lv);
                                }
                            }
                        }
                        None => {}
                    }
                    v = Some(acc);
                } else if let Some(d) = default {
                    v = Some(self.dataclass_default(d, node, state));
                }
                if v.is_none() && star_kwargs.is_empty() {
                    if !ci.typed_dict {
                        self.verdict(node, fk, &Classification::with_note(Tier::Missing, "dataclass field not supplied"), inst, ns, None, None);
                    }
                    continue;
                }
                let r = state.row(inst, ns).store(fk, true);
                state.set_row(inst, ns, r);
                let v = v.filter(|x| !x.empty()).unwrap_or_else(|| Value::data("field"));
                state.set_slot(inst, ns, fk, v, true);
            }
            if ci.typed_dict {
                for sk in star_kwargs {
                    self.copy_into(inst, Ns::Item, sk, state, true, &KeySet::new());
                }
            }
        }
        // the generated __init__ ends by calling __post_init__ when the class defines one
        if let Some(pc) = self.class_lookup(cls, "__post_init__") {
            let fv = self.cls_slots.get(&(pc, Ns::Attr, key_str_lit("__post_init__"))).cloned().unwrap_or_default();
            for f in fv.pts.iter() {
                if self.kind(f) == Kind::Function {
                    self.call_user(f, Some(Value::obj(inst)), Vec::new(), IndexMap::new(), Vec::new(), Vec::new(), state, node, None, false);
                }
            }
        }
    }

    /// `self.f = p` statements of `__init__` whose `p` is a parameter: record the classes of
    /// the instances passed for `p` here, so `self.f` resolves its methods by class later.
    fn note_ctor_field_classes(&mut self, init_cls: u32, init: u32, args: &[Value], kwargs: &IndexMap<StrId, Value>) {
        let fi = self.func_info(init);
        let FuncNode::Def(f) = fi.node else { return };
        let params: Vec<String> = crate::scopes::all_params(&f.parameters).into_iter().skip(1).map(|p| p.name.id.to_string()).collect();
        for st in &f.body {
            let ast::Stmt::Assign(a) = st else { continue };
            if a.targets.len() != 1 {
                continue;
            }
            let ast::Expr::Attribute(t) = &a.targets[0] else { continue };
            let ast::Expr::Name(base) = &*t.value else { continue };
            if base.id.as_str() != "self" {
                continue;
            }
            let ast::Expr::Name(src) = &*a.value else { continue };
            let pname = src.id.as_str();
            let v = match params.iter().position(|p| p == pname) {
                Some(i) if i < args.len() => Some(args[i].clone()),
                _ => kwargs.get(&s(pname)).cloned(),
            };
            let Some(v) = v else { continue };
            let classes: Vec<u32> = v.pts.iter().filter_map(|o| if self.kind(o) == Kind::Instance { self.instance_class(o) } else { None }).collect();
            let mixed = classes.is_empty() || v.pts.iter().any(|o| self.kind(o) != Kind::Instance);
            let entry = self.ctor_field_classes.entry((init_cls, t.attr.id.to_string())).or_default();
            if mixed {
                // not an instance (or not only instances): the field has no single class
                entry.insert(u32::MAX);
            }
            entry.extend(classes);
        }
    }

    /// The default of a dataclass field: `field(default_factory=f)` calls `f` (a fresh
    /// container per construction site), `field(default=x)` and plain defaults evaluate.
    pub fn dataclass_default(&mut self, d: &'static ast::Expr, node: &Node, state: &mut State) -> Value {
        if let ast::Expr::Call(c) = d {
            if super::dotted_last(&c.func) == "field" {
                for kw in c.arguments.keywords.iter() {
                    let Some(arg) = &kw.arg else { continue };
                    match arg.id.as_str() {
                        "default_factory" => {
                            let f = self.eval(&kw.value, state);
                            return self.call_value(&f, node, None, state, Some(vec![]));
                        }
                        "default" => return self.eval(&kw.value, state),
                        _ => {}
                    }
                }
                return Value::data("field");
            }
        }
        self.eval(d, state)
    }

    /// Dataclass fields in constructor order: base classes' fields first (MRO, nearest last).
    pub fn dataclass_fields_mro(&self, cls: u32) -> Vec<(String, Option<&'static ast::Expr>)> {
        let mut out: Vec<(String, Option<&'static ast::Expr>)> = Vec::new();
        for c in self.mro(cls).into_iter().rev() {
            if let Some(ci) = self.class_info_opt(c) {
                for (name, d) in ci.fields() {
                    if let Some(e) = out.iter_mut().find(|(n, _)| *n == name) {
                        e.1 = d;
                    } else {
                        out.push((name, d));
                    }
                }
            }
        }
        out
    }

    fn receiver_ctx(&self, self_v: &Option<Value>, site: &str) -> (String, bool) {
        if let Some(sv) = self_v {
            if let Some(r) = sv.single() {
                if self.kind(r) == Kind::Instance {
                    let recv = self.obj(r);
                    // per receiver and call site: two calls of one method on one receiver
                    // allocate distinct objects
                    let ctx = if !recv.single {
                        site.to_string()
                    } else if self.site_clones.get(&recv.site).copied().unwrap_or(0) >= super::limit("clones", MAX_CLONES) {
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

    #[allow(clippy::too_many_arguments)]
    pub fn call_user(&mut self, foid: u32, self_v: Option<Value>, args: Vec<Value>, kwargs: IndexMap<StrId, Value>, star_args: Vec<Value>, star_kwargs: Vec<Value>, state: &mut State, node: &Node, _call_ast: Option<&'static ast::ExprCall>, entry: bool) -> Value {
        if !state.narrowed.is_empty() {
            state.narrowed.clear();
        }
        let fi = self.func_info(foid);
        let fnode = fi.node;
        // the call context: per call site (line and column) with `fixes`; the prototype used
        // the line only, so two calls on one line shared their clones
        let site = if node.addr != 0 { format!("{}L{}:{}", crate::frontend::site_prefix(node.module), node.line, node.col) } else { format!("{}L{}", crate::frontend::site_prefix(node.module), node.line) };
        // inside a tabulated frame the context is chained to the frame's, so two activations
        // of the caller give distinct objects (bounded depth; beyond it the site alone)
        let site = {
            let fr = self.frame();
            let mut site = site;
            if !fr.symbolic {
                for i in &fr.unroll {
                    site.push_str(&format!("#{}", i));
                }
            }
            if fr.kind != FrameKind::Module && !fr.symbolic && !fr.ctx.is_empty() && !fr.ctx.starts_with("entry") {
                if fr.ctx.matches('/').count() < super::limit("chain", 2) {
                    format!("{}/{}", fr.ctx, site)
                } else {
                    // deeper chains stay distinct through a hash of the prefix: two activations
                    // of the caller keep allocating distinct objects at any depth (bounded by
                    // the tabulation budgets), while the context string stays short
                    let mut h = rustc_hash::FxHasher::default();
                    std::hash::Hash::hash(&fr.ctx, &mut h);
                    format!("h{:x}/{}", std::hash::Hasher::finish(&h) & 0xffff_ffff, site)
                }
            } else {
                site
            }
        };
        self.analysed_funcs.insert(foid);
        // --- symbolic summaries (§7.3/§9): analyse the body once, instantiate per call ---
        let forced = std::mem::take(&mut self.force_tab_once) || self.torch_modules.contains(&fi.module);
        if self.use_summaries && self.model_call == 0 && !forced {
            let bound = self.compute_bindings(&fi, &self_v, &args, &kwargs, &star_args, &star_kwargs, state, node);
            // the classes of the arguments: the summary is specialised on them
            let mut sig: Vec<(String, ObjId)> = Vec::new();
            // (a placeholder with a declared user class counts as an instance of it, so calls
            // inside summaries specialise too)
            let one_class = |an: &Analyzer, v: &Value| -> Option<ObjId> {
                if v.pts.is_empty() {
                    return None;
                }
                let mut cls: Option<ObjId> = None;
                for o in v.pts.iter() {
                    let c = match an.kind(o) {
                        Kind::Instance => an.instance_class(o)?,
                        Kind::Ph => an.ph_info(o).cls.get().filter(|c| an.class_info_opt(*c).is_some())?,
                        _ => return None,
                    };
                    match cls {
                        None => cls = Some(c),
                        Some(x) if x == c => {}
                        _ => return None,
                    }
                }
                cls
            };
            if !super::disabled("sigsum") {
                if let Some(sv) = &self_v {
                    if let Some(c) = one_class(self, sv) {
                        if let Some(FuncNode::Def(f)) = Some(fi.node) {
                            if let Some(p0) = crate::scopes::all_params(&f.parameters).first() {
                                sig.push((p0.name.id.to_string(), c));
                            }
                        }
                    }
                }
                for (name, v) in bound.iter() {
                    if let Some(c) = one_class(self, v) {
                        sig.push((intern::str_get(*name).to_string(), c));
                    }
                }
                sig.sort();
            }
            let summ = self.summarise_sig(foid, state, sig);
            let Some(summ) = summ else {
                return Value::new(); // recursion through the symbolic path: ⊥ summary
            };
            let (call_ctx, ctx_single) = self.receiver_ctx(&self_v, &site);
            let mut bind: IndexMap<u32, Value> = IndexMap::new();
            for (name, ph) in summ.ph_by_name.iter() {
                if let Some(bv) = bound.get(name) {
                    let mut bv = bv.clone();
                    let pcls = self.ph_info(*ph).cls.get();
                    if let Some(pc) = pcls {
                        if !bv.pts.is_empty() {
                            let kept = bv.pts.filter(|o| self.may_be_instance(o, pc));
                            if kept != bv.pts {
                                bv = Value::with_kv(kept, bv.kv.clone());
                            }
                        }
                    }
                    bind.insert(*ph, bv);
                }
            }
            if let (Some(ps), Some(sv)) = (summ.ph_self, &self_v) {
                bind.insert(ps, sv.clone());
            }
            if !self.should_tabulate(&summ, &bind, state) {
                if !self.sum_stack.is_empty() {
                    self.propagate_events(&summ, &bind, state);
                }
                let saved_chain = std::mem::take(&mut self.call_chain);
                self.call_chain = chain_extend(&saved_chain, &[node.line]);
                if summ.raises {
                    if crate::interp::debug_level() == 2 {
                        eprintln!("  summary of {} raises (instantiated at L{} in {})", fi.qualname, node.line, self.frames.last().map(|f| f.label.clone()).unwrap_or_default());
                    }
                    self.note_raise();
                }
                let r = self.instantiate(&summ, &bind, state, node, &call_ctx, ctx_single);
                self.call_chain = saved_chain;
                return r;
            }
        }
        // --- per-call-site (tabulated) analysis ---
        let mut key = (foid, if !entry { site.clone() } else { format!("entry:{}", site) });
        let act = self.active_by_func.get(&foid).cloned().unwrap_or_default();
        if !self.active.contains_key(&key) && act.len() >= MAX_ACTIVE {
            key = act.last().unwrap().clone();
        }
        if self.active.contains_key(&key) {
            self.rec_hit.insert(key.clone());
            // widen the entry arguments with the ones of this recursive call (§9.7)
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
            if let Some((ret, exit_state, fp)) = self.rec_table.get(&key) {
                let ret = ret.clone();
                state.join_heap_fp(exit_state, fp);
                return ret;
            }
            return Value::new();
        }
        // summary cache: same callee, same call site, same observable input → replay the effects
        let sig = self.call_signature(foid, &site, entry, &self_v, &args, &kwargs, &star_args, &star_kwargs, state);
        if let Some(sig) = &sig {
            if let Some(hit) = self.call_cache.get(sig).cloned() {
                if super::debug_enabled() {
                    eprintln!("cache hit for {} at {}", self.func_info(foid).qualname, site);
                }
                if hit.raises {
                    self.note_raise();
                }
                for k in &hit.fp.rows {
                    if let Some(r) = hit.fp_state.rows.get(k) {
                        state.rows.insert(*k, r);
                    }
                }
                for (oid, sk) in &hit.fp.slots {
                    if let Some(v) = hit.fp_state.slots.get(oid, sk) {
                        state.slots.set(oid, *sk, v);
                    }
                }
                for k in &hit.fp.elems {
                    if let Some(v) = hit.fp_state.elems.get(k) {
                        state.elems.insert(*k, v);
                    }
                }
                // recent objects the replayed call allocated: folded at the loop's back-edge
                // like the originals, or a later activation resets them under live references
                if !self.recent_stack.is_empty() {
                    let mut oids: Vec<ObjId> = hit.fp.rows.iter().map(|(o, _)| *o).collect();
                    oids.extend(hit.fp.slots.iter().map(|(o, _)| *o));
                    oids.extend(hit.fp.elems.iter().copied());
                    let mut recents: Vec<(ObjId, Kind, String, String)> = Vec::new();
                    for o in oids {
                        if !self.has_obj(o) {
                            continue;
                        }
                        let ob = self.obj(o);
                        if let Some(base) = ob.site.strip_suffix("#recent") {
                            recents.push((o, ob.kind, base.to_string(), ob.ctx.clone()));
                        }
                    }
                    let top = self.recent_stack.last_mut().unwrap();
                    for r in recents {
                        if !top.iter().any(|(x, _, _, _)| *x == r.0) {
                            top.push(r);
                        }
                    }
                }
                for (src, d) in hit.fp_state.reqs.items() {
                    for (k, info) in d.iter() {
                        let site_id = info.sites.first().unwrap_or(s(&site));
                        state.require(src, *k, site_id, info.alternatives.clone(), info.caveats.clone(), info.alt_keys.clone());
                        if !info.always {
                            let tgt = state.reqs.writable(&src);
                            if let Some(cur) = tgt.get(k).cloned() {
                                tgt.insert(*k, ReqInfo { always: false, ..cur });
                            }
                        }
                    }
                }
                let chain = chain_extend(&self.call_chain, &[node.line]);
                for v in &hit.verdicts {
                    let full = chain_extend(&chain, &v.chain);
                    let mut nv = v.clone();
                    nv.chain = full;
                    let dk = nv.dedupe_key();
                    if !self.verdict_keys.contains(&dk) {
                        self.verdict_keys.insert(dk);
                        self.verdicts.push(nv);
                    }
                }
                let ret = hit.ret.clone();
                let rows: Vec<(u32, Row)> = ret.pts.iter().filter(|o| self.is_record_like(*o)).map(|o| (o, state.row(o, Ns::Item))).collect();
                self.summaries.entry((fi.qualname.clone(), site.clone())).or_insert((ret.clone(), rows));
                return ret;
            }
        }
        // context for allocations inside the callee (§6.1)
        let (ctx, ctx_single) = self.receiver_ctx(&self_v, &site);
        let fr_single = self.frame().single && self.frame().loop_depth == 0 && ctx_single;
        let fr_single_nl = self.frame().single_nl && ctx_single;
        let mut new_fr = Frame::new(FrameKind::Function, fi.scope, fi.module, &ctx, fr_single, &fi.qualname);
        new_fr.single_nl = fr_single_nl;
        new_fr.func = Some(foid);
        {
            let mi = self.module_info(fi.module);
            let mut cells: Vec<String> = mi.scopes.get(fi.scope).cells.iter().cloned().collect();
            cells.sort();
            for name in cells {
                let cid = self.alloc_raw(Kind::Cell, &format!("cell:{}.{}", fi.qualname, name), &ctx, fr_single, &format!("cell {}", name), Payload::None);
                new_fr.cells.insert(s(&name), cid);
            }
        }
        for (n, c) in fi.closure.iter() {
            new_fr.cells.insert(*n, *c);
        }
        let saved_env = std::mem::take(&mut state.env);
        self.env_stack.push(saved_env.clone());
        self.raise_stack.push(false);
        let saved_ret = state.ret.take();
        self.frames.push(new_fr);
        if let FuncNode::Def(f) = fnode {
            if is_generator(&f.body) {
                // the yielded values of this activation
                let gid = self.alloc(Kind::List, &format!("gen:{}", foid), state, &format!("generator {}", fi.qualname), Payload::None, None, Ns::Item, None, Some(&ctx));
                self.obj_mut(gid).literal_len = Some(0);
                state.elems.insert(gid, KeyVal::Lits(KeySet::new()));
            }
        }
        let saved_chain = std::mem::take(&mut self.call_chain);
        self.call_chain = chain_extend(&saved_chain, &[node.line]);
        self.active.insert(key.clone(), true);
        self.active_by_func.entry(foid).or_default().push(key.clone());
        self.rec_entry.insert(key.clone(), (self_v.clone(), args.clone(), kwargs.clone()));
        let verdict_mark = self.verdicts.len();
        let chain_len = self.call_chain.len();

        let mut pre_bind = state.copy();
        self.bind_params(&fi, &self_v, &args, &kwargs, &star_args, &star_kwargs, state, node);
        let mut entry_state = state.copy();
        let mut ret = Value::new();
        let mut st_final: Option<State> = None;
        for _ in 0..REC_MAX_ITER {
            let mut st = entry_state.copy();
            match fnode {
                FuncNode::Lambda(l) => {
                    let v = self.eval(&l.body, &mut st);
                    st.ret = Some(v);
                    st.record_exit();
                    st.reachable = false;
                }
                FuncNode::Def(f) => self.exec_block(&f.body, &mut st),
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
                let mut exs = ex.copy_ref();
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
        let mut st = match st_final {
            Some(s) => s,
            None => {
                // did not converge: use the last tabulated state
                match self.rec_table.get(&key) {
                    Some((_, pst, _)) => pst.copy_ref(),
                    None => entry_state.copy(),
                }
            }
        };
        if let Some(sig) = sig {
            if !self.rec_hit.contains(&key) {
                let mut rel = State::new();
                for (src, d) in st.reqs.items() {
                    let base = pre_bind.reqs.inner(&src);
                    for (k, info) in d.iter() {
                        let same = base.as_ref().and_then(|b| b.get(k)).map(|b| b == info).unwrap_or(false);
                        if !same {
                            rel.reqs.writable(&src).insert(*k, info.clone());
                        }
                    }
                }
                let fp = st.footprint(&pre_bind);
                let mut fp_state = State::new();
                for k in &fp.rows {
                    if let Some(r) = st.rows.get(k) {
                        fp_state.rows.insert(*k, r);
                    }
                }
                for (oid, sk) in &fp.slots {
                    if let Some(v) = st.slots.get(oid, sk) {
                        fp_state.slots.set(oid, *sk, v);
                    }
                }
                for k in &fp.elems {
                    if let Some(v) = st.elems.get(k) {
                        fp_state.elems.insert(*k, v);
                    }
                }
                fp_state.reqs = rel.reqs;
                let cur_chain = self.call_chain.clone();
                let rel_verdicts: Vec<Verdict> = self.verdicts[verdict_mark..]
                    .iter()
                    .map(|v| {
                        let mut nv = v.clone();
                        if v.chain.len() >= chain_len && v.chain[..chain_len] == cur_chain[..] {
                            nv.chain = v.chain[chain_len..].to_vec();
                        }
                        nv
                    })
                    .collect();
                let raises = self.raise_stack.last().copied().unwrap_or(false);
                self.call_cache.insert(sig, Rc::new(CacheHit { raises, ret: ret.clone(), fp_state, fp, verdicts: rel_verdicts }));
            }
        }
        state.set_to(&mut st);
        if let FuncNode::Def(f) = fnode {
            if is_generator(&f.body) {
                let gid = self.alloc_raw(Kind::List, &format!("gen:{}", foid), &ctx, true, &format!("generator {}", fi.qualname), Payload::None);
                ret = Value::obj(gid);
            }
        }
        let rows: Vec<(u32, Row)> = ret.pts.iter().filter(|o| self.is_record_like(*o)).map(|o| (o, state.row(o, Ns::Item))).collect();
        self.summaries.insert((fi.qualname.clone(), ctx.clone()), (ret.clone(), rows));
        // cleanup
        self.active.remove(&key);
        if let Some(lst) = self.active_by_func.get_mut(&foid) {
            lst.pop();
        }
        self.rec_hit.remove(&key);
        self.rec_entry.remove(&key);
        self.rec_widened.remove(&key);
        self.call_chain = saved_chain;
        self.frames.pop();
        self.env_stack.pop();
        let raised = self.raise_stack.pop().unwrap_or(false);
        if raised {
            if crate::interp::debug_level() == 2 {
                eprintln!("  callee {} raised (tabulated at L{})", fi.qualname, node.line);
            }
            self.note_raise();
        }
        state.env = saved_env;
        state.ret = saved_ret;
        ret
    }

    /// Drop the facts whose test read one of `objs`.
    pub fn kill_facts_on(&mut self, objs: &[u32], state: &mut State) {
        if objs.is_empty() || state.facts.is_empty() {
            return;
        }
        let kept: Vec<_> = state.facts.iter().filter(|f| !f.deps.iter().any(|(_, v)| v.pts.iter().any(|o| objs.contains(&o)))).cloned().collect();
        if kept.len() != state.facts.len() {
            state.facts = std::sync::Arc::new(kept);
        }
    }

    /// An exception raised here is caught by the innermost enclosing try: its handler starts
    /// from this state (with the try frame's locals when the raise is inside a callee).
    pub fn record_raise_state(&mut self, state: &mut State) -> bool {
        let Some((fl, el, pre)) = self.try_marks.last() else { return false };
        if self.try_sum_depth.last().map(|d| self.sum_stack.len() > *d).unwrap_or(false) && !crate::interp::disabled("sumraise") {
            // inside a callee's summary: its rows are relative to that summary; the call's
            // instantiation decides whether it raises here
            return false;
        }
        let mut here = state.copy();
        if self.frames.len() < *fl {
            // the try is in a `@contextmanager` generator whose with body is running
            if let Some((_, env)) = self.cm_frames.iter().rev().find(|(d, _)| *d == *fl) {
                here.env = env.clone();
                here.narrowed = Default::default();
            }
        } else if self.frames.len() > *fl {
            here.env = self.env_stack.get(*el).cloned().unwrap_or_else(|| pre.env.clone());
            here.narrowed = pre.narrowed.clone();
            here.ret = pre.ret.clone();
            here.exits = pre.exits.clone();
        }
        let exc = self.cur_exc.take().unwrap_or_else(|| Value::data("exception"));
        if let Some(rv) = self.raise_values.last_mut() {
            *rv = Some(match rv.take() {
                None => exc,
                Some(prev) => prev.join(&exc),
            });
        }
        let Some(slot) = self.raise_states.last_mut() else { return false };
        *slot = Some(match slot.take() {
            None => here,
            Some(mut prev) => prev.join(&mut here),
        });
        true
    }

    /// A builtin exception (`KeyError(key)`, …) as a value: open attributes, known `args`.
    pub fn exception_object(&mut self, cls_name: &str, args: Vec<Value>, node: &Node, state: &mut State) -> Value {
        let t = self.alloc_positional(Kind::Tuple, node, Some(args), Value::new(), DATA, state);
        let row = Row::with_source(crate::domain::Source::new(cls_name, crate::domain::SourceKind::Data));
        let oid = self.alloc_at(Kind::Namespace, node, state, &format!("{}@L{}", cls_name, node.line), Payload::None, Some(row), Ns::Attr, None, None);
        let strong = self.obj(oid).single;
        self.store_key(oid, Ns::Attr, &KeyVal::Lit(key_str_lit("args")), &t, strong, node, state);
        Value::obj(oid)
    }

    /// An exception named `name` is raised here (the rest of the path is dead).
    pub fn raise_here(&mut self, name: &str, state: &mut State) {
        self.may_raise_here(name, state);
        state.reachable = false;
    }

    /// An exception named `name` may be raised here: a handler that catches it starts from
    /// this state; otherwise the enclosing call raises.
    pub fn may_raise_here(&mut self, name: &str, state: &mut State) {
        let caught = self.guards.iter().any(|g| g.contains("*") || g.contains("Exception") || g.contains("BaseException") || g.contains(name));
        if caught && self.record_raise_state(state) {
            return;
        }
        if let Some(t) = self.raise_stack.last_mut() {
            *t = true;
        }
        if let Some(m) = self.may_raise.last_mut() {
            *m = true;
        }
    }

    /// The call that just completed may raise: the enclosing call and try body learn it.
    pub fn note_raise(&mut self) {
        if crate::interp::debug_level() == 2 {
            eprintln!("  raise noted in {} (chain {:?})", self.frames.last().map(|f| f.label.clone()).unwrap_or_default(), self.call_chain);
        }
        if let Some(t) = self.raise_stack.last_mut() {
            *t = true;
        }
        if let Some(m) = self.may_raise.last_mut() {
            *m = true;
        }
    }

    pub fn in_method_frame(&mut self, foid: u32, self_v: Value, args: Vec<Value>, state: &mut State, f: &mut dyn FnMut(&mut Analyzer, &mut State)) {
        let fi = self.func_info(foid);
        let ctx = self.frame().ctx.clone();
        let mut new_fr = Frame::new(FrameKind::Function, fi.scope, fi.module, &ctx, false, &fi.qualname);
        new_fr.func = Some(foid);
        for (n, c) in fi.closure.iter() {
            new_fr.cells.insert(*n, *c);
        }
        let saved_env = std::mem::take(&mut state.env);
        self.env_stack.push(saved_env.clone());
        self.frames.push(new_fr);
        let fake = Node::synthetic(fi.node.node().module, fi.node.node().line);
        self.bind_params(&fi, &Some(self_v), &args, &IndexMap::new(), &[], &[], state, &fake);
        f(self, state);
        self.frames.pop();
        self.env_stack.pop();
        state.env = saved_env;
    }

    /// Hashable description of everything a callee can observe, or None if too large.
    #[allow(clippy::too_many_arguments)]
    pub fn call_signature(&self, foid: u32, site: &str, entry: bool, self_v: &Option<Value>, args: &[Value], kwargs: &IndexMap<StrId, Value>, star_args: &[Value], star_kwargs: &[Value], state: &State) -> Option<Sig> {
        let fi = self.func_info(foid);
        let mut vals: Vec<Value> = args.to_vec();
        vals.extend(kwargs.values().cloned());
        vals.extend(star_args.iter().cloned());
        vals.extend(star_kwargs.iter().cloned());
        if let Some(sv) = self_v {
            vals.push(sv.clone());
        }
        for cid in fi.closure.values() {
            vals.push(state.slot(*cid, Ns::Cell, cell_key()));
        }
        let mut roots: Vec<u32> = vals.iter().flat_map(|v| v.pts.to_vec()).collect();
        roots.push(fi.module);
        let mut seen: Vec<u32> = Vec::new();
        let mut seen_set: rustc_hash::FxHashSet<u32> = rustc_hash::FxHashSet::default();
        let mut work: Vec<(u32, u32)> = roots.into_iter().map(|o| (o, 0)).collect();
        while let Some((oid, d)) = work.pop() {
            if seen_set.contains(&oid) {
                continue;
            }
            if seen.len() > 400 {
                return None;
            }
            seen_set.insert(oid);
            seen.push(oid);
            if d >= 3 {
                continue;
            }
            for (_, v) in state.slots_of(oid) {
                for x in v.pts.iter() {
                    if !seen_set.contains(&x) {
                        work.push((x, d + 1));
                    }
                }
            }
        }
        seen.sort_unstable();
        let mut heap = Vec::with_capacity(seen.len());
        for oid in seen {
            heap.push((oid, state.rows.get(&(oid, Ns::Item)), state.rows.get(&(oid, Ns::Attr)), state.slots_of(oid), state.elems.get(&oid), self.obj(oid).single));
        }
        let guards: Vec<Vec<String>> = self
            .guards
            .iter()
            .map(|g| {
                let mut v: Vec<String> = g.iter().cloned().collect();
                v.sort();
                v
            })
            .collect();
        let mut kw: Vec<(StrId, Value)> = kwargs.iter().map(|(k, v)| (*k, v.clone())).collect();
        kw.sort_by_key(|(k, _)| intern::str_get(*k));
        Some(Sig {
            foid,
            site: site.to_string(),
            entry,
            self_v: self_v.clone(),
            args: args.to_vec(),
            kwargs: kw,
            star_args: star_args.to_vec(),
            star_kwargs: star_kwargs.to_vec(),
            heap,
            guards,
            fr_single: self.frame().single && self.frame().loop_depth == 0,
            ctx: self.frame().ctx.clone(),
        })
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
        // `f(*xs)` with known items: they are positional arguments; a placeholder `*args`
        // forwarded from a summary is decided per call site
        let mut star_args: Vec<Value> = star_args.to_vec();
        let mut expanded: Vec<Value> = Vec::new();
        star_args.retain(|sa| {
            for o in sa.pts.iter() {
                if self.kind(o) == Kind::Ph {
                    self.record_event("iter", o);
                }
            }
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
        for sk in star_kwargs {
            for o in sk.pts.iter() {
                if self.kind(o) == Kind::Ph {
                    self.record_event("iter", o);
                }
            }
        }
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
                if fi.defaults.get(name).is_none() && !kw_definite && !super::disabled("kwreq") {
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
            self.obj_mut(tid).literal_len = if star_args.is_empty() { Some(rest_pos.len()) } else { None };
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
                // `**kw` forwarded from a placeholder (or a copy of one): the new dict is not
                // concrete either; its events reach the enclosing summary
                if let Some(s1) = sk.single() {
                    let src = if self.kind(s1) == Kind::Ph { Some(s1) } else { self.obj(s1).copied_from.filter(|c| self.kind(*c) == Kind::Ph) };
                    if let Some(src) = src {
                        if self.is_entry_object(src) {
                            self.obj_mut(did).copied_from = Some(src);
                        }
                    }
                }
            }
            bound.insert(s(kw.name.id.as_str()), Value::obj(did));
        }
        bound
    }
}
