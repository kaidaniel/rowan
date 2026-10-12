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
use super::{is_generator, Analyzer, Call, Frame, FrameKind, FuncInfo, FuncNode, Payload, Verdict, MAX_ACTIVE, MAX_ARG_JOIN, MAX_CLONES, NOEFFECT_METHOD_NAMES, REC_MAX_ITER};

/// A dataclass field as `__init__` takes it (`dc_specs`).
#[derive(Clone, Debug)]
pub struct DcSpec {
    pub name: String,
    pub default: Option<&'static ast::Expr>,
    pub init: bool,
    pub initvar: bool,
    pub kw_only: bool,
}

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
    heap: Vec<(u32, Option<Row>, Option<Row>, Vec<(crate::state::SlotKey, Value)>, Option<KeyVal>, bool)>,
    guards: Vec<Vec<String>>,
    fr_single: bool,
    ctx: String,
}

pub struct CacheHit {
    pub raises: bool,
    /// some path of the run returned (else the call never completes)
    pub completes: bool,
    /// the lengths the run changed
    pub lens: Vec<ObjId>,
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
        if fv.pts.is_empty() {
            return self.unknown_call("?", None, &args, &kwargs, &star_args, &star_kwargs, state, node);
        }
        let targets: Vec<u32> = fv.pts.to_vec();
        if targets.len() == 1 {
            return self.call_one(targets[0], &args, &kwargs, &star_args, &star_kwargs, state, node, call_ast);
        }
        // (past the hard deadline: one unknown call for all of them)
        if self.over_hard_deadline() && targets.iter().all(|o| matches!(self.kind(*o), Kind::Function | Kind::Bound)) {
            return self.unknown_call("?", None, &args, &kwargs, &star_args, &star_kwargs, state, node);
        }
        let mut pre = state.copy();
        let mut outs: Vec<Value> = Vec::new();
        let mut states: Vec<State> = Vec::new();
        for oid in targets {
            let mut st = pre.copy();
            // (a groupby method of one of several groupby objects: its results are its own)
            let own = matches!(&self.obj(oid).payload, Payload::Method(q, r) if q.starts_with("groupby.") && r.pts.iter().all(|g| self.kind(g) == Kind::Groupby));
            let saved = if own { self.alloc_extra.replace(format!("gbm{}", oid)) } else { self.alloc_extra.clone() };
            outs.push(self.call_one(oid, &args, &kwargs, &star_args, &star_kwargs, &mut st, node, call_ast));
            self.alloc_extra = saved;
            states.push(st);
        }
        // (pairwise: each round joins what the others wrote once, not the growing join again)
        while states.len() > 1 {
            let mut next: Vec<State> = Vec::with_capacity(states.len().div_ceil(2));
            let mut it = states.into_iter();
            while let Some(mut a) = it.next() {
                match it.next() {
                    Some(mut b) => next.push(a.join(&mut b)),
                    None => next.push(a),
                }
            }
            states = next;
        }
        let out = states.pop().unwrap();
        state.replace_with(out);
        join_values(outs)
    }

    /// Several definitions of one method (conditional ones): alternatives, one state each.
    #[allow(clippy::too_many_arguments)]
    fn call_alternatives(&mut self, fns: &[u32], recv: u32, args: &[Value], kwargs: &IndexMap<StrId, Value>, star_args: &[Value], star_kwargs: &[Value], state: &mut State, node: &Node, call_ast: Option<&'static ast::ExprCall>) -> Value {
        if fns.len() == 1 {
            let mut outs = Vec::new();
            for x in fns {
                outs.push(self.call_user(*x, Some(Value::obj(recv)), args.to_vec(), kwargs.clone(), star_args.to_vec(), star_kwargs.to_vec(), state, node, call_ast, false));
            }
            return join_values(outs);
        }
        let mut outs = Vec::new();
        let mut acc: Option<State> = None;
        for x in fns {
            let mut st = state.copy();
            outs.push(self.call_user(*x, Some(Value::obj(recv)), args.to_vec(), kwargs.clone(), star_args.to_vec(), star_kwargs.to_vec(), &mut st, node, call_ast, false));
            acc = Some(match acc {
                None => st,
                Some(mut a) => a.join(&mut st),
            });
        }
        if let Some(a) = acc {
            state.replace_with(a);
        }
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
                        let fns: Vec<u32> = f.pts.iter().filter(|x| self.kind(*x) == Kind::Function).collect();
                        return self.call_alternatives(&fns, oid, args, kwargs, star_args, star_kwargs, state, node, call_ast);
                    }
                }
            }
        }
        match kind {
            Kind::Function if self.func_info(oid).is_memo.get() && star_args.is_empty() && star_kwargs.is_empty() => {
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
                // the hit / miss counts (cache_info), known while every call's outcome is
                let bump = |state: &mut State, name: &str, by_known: bool| {
                    let k = key_str_lit(name);
                    let cur = state.slot_opt(memo, Ns::Attr, k).unwrap_or_else(|| Value::lit(intern::key_int(0)));
                    let nv = match (cur.kv.as_lit().and_then(intern::key_as_int), by_known) {
                        (Some(n), true) => Value::lit(intern::key_int(n + 1)),
                        _ => Value::data("count"),
                    };
                    state.set_slot(memo, Ns::Attr, k, nv, true);
                };
                if row.definite.contains(mk) {
                    bump(state, "hits", true);
                    return state.slot(memo, Ns::Item, mk);
                }
                if row.present.contains(mk) {
                    bump(state, "hits", false);
                    bump(state, "misses", false);
                } else {
                    bump(state, "misses", true);
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
                if let Some(m) = self.models.lookup(&qual) {
                    return self.run_model(m, state, &call);
                }
                self.unknown_call(&qual, None, args, kwargs, star_args, star_kwargs, state, node)
            }
            Kind::Method => {
                let Payload::Method(qual, recv) = self.obj(oid).payload.clone() else { return Value::new() };
                // several groupby objects (`groupby` over one of several columns): the model runs
                // once per object, each making its own results
                if qual.starts_with("groupby.") && recv.pts.len() > 1 && recv.pts.iter().all(|o| self.kind(o) == Kind::Groupby) {
                    let mut outs: Vec<Value> = Vec::new();
                    for g in recv.pts.iter() {
                        let mv = self.method_value(&qual, &Value::obj(g));
                        let Some(mo) = mv.single() else { continue };
                        let saved = self.alloc_extra.replace(format!("gb{}", g));
                        let r = self.call_one(mo, args, kwargs, star_args, star_kwargs, state, node, call_ast);
                        self.alloc_extra = saved;
                        outs.push(r);
                    }
                    return join_values(outs);
                }
                // `super().__init__(…)` reaching a builtin exception base: args, str / repr
                if qual == "excinit" {
                    for o in recv.pts.iter() {
                        if self.kind(o) == Kind::Instance {
                            let name = self.instance_class(o).and_then(|c| self.class_info_opt(c)).map(|ci| ci.name.clone()).unwrap_or_else(|| "Exception".into());
                            let t = self.alloc_positional(Kind::Tuple, node, Some(args.to_vec()), Value::new(), DATA, state);
                            let strong = recv.pts.len() == 1 && self.obj(o).single;
                            self.store_key(o, Ns::Attr, &KeyVal::Lit(key_str_lit("args")), &t, strong, node, state);
                            self.exc_objs.insert(o, name);
                        }
                    }
                    return self.none_v.clone();
                }
                // `super().__init__(…)` / `Base.__init__(self, …)` reaching a dataclass base
                if let Some(rest) = qual.strip_prefix("dcinit.") {
                    let (cls, self_v, args): (u32, Value, Vec<Value>) = match rest.strip_prefix("class.") {
                        Some(c) => (c.parse().unwrap_or(0), args.first().cloned().unwrap_or_default(), args.iter().skip(1).cloned().collect()),
                        None => (rest.parse().unwrap_or(0), recv.clone(), args.to_vec()),
                    };
                    if cls != 0 {
                        for o in self_v.pts.iter() {
                            if matches!(self.kind(o), Kind::Instance) {
                                self.dataclass_init(cls, o, &args, star_args, kwargs, star_kwargs, state, node);
                            }
                        }
                    }
                    return self.none_v.clone();
                }
                let mut args = args.to_vec();
                if qual.starts_with("dict.") || qual.starts_with("set.") {
                    if let Some(a) = args.first_mut() {
                        *a = self.keyify(a);
                    }
                }
                let call = Call { qual: qual.clone(), recv: Some(recv.clone()), args, kwargs: kwargs.clone(), star_args: star_args.to_vec(), star_kwargs: star_kwargs.to_vec(), node: *node, call_ast, site: format!("L{}", node.line), line: node.line };
                if qual.starts_with("str.") {
                    if let Some(v) = crate::models::pure::try_pure(self, state, &call) {
                        return v;
                    }
                }
                if let Some(m) = self.models.lookup(&qual) {
                    return self.run_model(m, state, &call);
                }
                self.unknown_call(&qual, Some(&recv), &call.args, kwargs, star_args, star_kwargs, state, node)
            }
            Kind::Unknown => {
                let q = format!("call of {}", self.obj(oid).label);
                self.unknown_call(&q, Some(&Value::obj(oid)), args, kwargs, star_args, star_kwargs, state, node)
            }
            Kind::Partial => {
                let call = Call { qual: "partial.__call__".into(), recv: Some(Value::obj(oid)), args: args.to_vec(), kwargs: kwargs.clone(), star_args: star_args.to_vec(), star_kwargs: star_kwargs.to_vec(), node: *node, call_ast, site: format!("L{}", node.line), line: node.line };
                crate::models::partial_call(self, state, &call)
            }
            Kind::Instance => {
                if let Some(cls) = self.instance_class(oid) {
                    if let Some(m) = self.class_lookup(cls, "__call__") {
                        let f = self.cls_slots.get(&(m, Ns::Attr, key_str_lit("__call__"))).cloned().unwrap_or_default();
                        if f.pts.is_empty() {
                            return Value::new();
                        }
                        let fns: Vec<u32> = f.pts.iter().filter(|x| self.kind(*x) == Kind::Function).collect();
                        return self.call_alternatives(&fns, oid, args, kwargs, star_args, star_kwargs, state, node, call_ast);
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
        // (a call of the result of a call of the result of … : past two levels the result is
        // the call node's own object, as access paths below an input are bounded; otherwise
        // recursion over unknown values makes new objects without end)
        let deep = qual.matches("result of").count() >= 2;
        let site = if deep {
            let text: String = crate::frontend::text_at(node).chars().take(40).collect();
            format!("call {} at M{}:L{}:{}", text, node.module, node.line, node.col)
        } else {
            format!("{}@L{}", qual, node.line)
        };
        // unknown code may raise
        if crate::interp::debug_level() == 2 {
            eprintln!("  unknown call {} in {} (may raise)", site, self.frames.last().map(|f| f.label.clone()).unwrap_or_default());
        }
        if let Some(t) = self.raise_stack.last_mut() {
            *t = true;
        }
        if let Some(e) = self.esc_cls.last_mut() {
            *e = None;
        }
        self.flag_may_raise(state);
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
        let touched = self.reachable_records(&vals, state, 0);
        let site_id = s(&site);
        let arg_recs: Vec<(u32, Ns)> = touched.iter().copied().filter(|(oid, _)| vals.iter().any(|v| v.pts.contains(*oid))).collect();
        // (the result's object is keyed by the last call site of the context only: with the
        // whole call string every context made its own result and its own tree of parts — a
        // recursive walk over unknown values, rich's pretty printer)
        let rctx: String = self.frame().ctx.rsplit('/').next().unwrap_or("").to_string();
        let rid = self.alloc(Kind::Unknown, &format!("ret:{}", site), state, &format!("result of {}", site), Payload::None, None, Ns::Item, Some(true), Some(&rctx));
        if let Some(r) = recv {
            for o in r.pts.iter() {
                self.note_part(o, rid);
                if let Payload::Method(_, mr) = &self.obj(o).payload {
                    for m in mr.pts.iter() {
                        self.note_part(m, rid);
                    }
                }
            }
        }
        // everything it can reach through what it is given may have changed: keys, values,
        // items, lengths
        self.havoc_reachable(&vals, rid, site_id, state);
        // for people: the call as written and the known objects it was given (unknown code may
        // return one of them; given unknowns add nothing to `?`)
        {
            let text = crate::frontend::text_at(node);
            let mut given: Vec<String> = Vec::new();
            for (o, _) in &arg_recs {
                if matches!(self.kind(*o), Kind::Unknown) {
                    continue;
                }
                let g = self.obj(*o).label.to_string();
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
        // (many records given — a value joined over many contexts: the result is any record,
        // an open row, rather than the join of them all and a copy of all their slots)
        let joinable = arg_recs.len() <= MAX_ARG_JOIN;
        if !arg_recs.is_empty() && joinable {
            let row = join_rows(arg_recs.iter().map(|(o, n)| state.row(*o, *n)));
            let mut d = row.data();
            d.definite = KeySet::new();
            d.absent = KeySet::new();
            d.mutators = d.mutators.insert(site_id);
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
            let mutated = |r: Row| if arg_recs.is_empty() { r } else { r.mutate(site_id) };
            state.set_row(rid, Ns::Item, mutated(Row::with_source(src)));
            state.set_row(rid, Ns::Attr, mutated(Row::with_source(src)));
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
            if matches!(k, Kind::Dict | Kind::Frame | Kind::Unknown) {
                out.push((oid, Ns::Item));
            }
            if matches!(k, Kind::Instance | Kind::Unknown) {
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

    /// The object standing for any value unknown code may have stored: nothing known about it.
    pub fn any_object(&mut self, state: &mut State) -> ObjId {
        let o = self.alloc_raw(Kind::Unknown, "any", "", false, "?", Payload::None);
        for ns in [Ns::Item, Ns::Attr] {
            if !state.has_row(o, ns) {
                state.set_row(o, ns, Row::top());
            }
        }
        o
    }

    /// Unknown code ran on `vals`: every object they reach (through slots, items, elements)
    /// may have changed. Records get the call as a mutator (any key may have been added or
    /// removed), every slot of a mutable object may now hold the call's result `rid` (any value),
    /// lists and sets any items and any length. Modules, classes and functions are not entered.
    pub fn havoc_reachable(&mut self, vals: &[Value], rid: ObjId, site: StrId, state: &mut State) {
        if std::env::var("T1").is_ok() { return; }
        let mut seen: rustc_hash::FxHashSet<ObjId> = rustc_hash::FxHashSet::default();
        let mut work: Vec<ObjId> = vals.iter().flat_map(|v| v.pts.iter()).collect();
        // (one object stands for "anything unknown code stored": a value per call would pile
        // up in every slot it reaches)
        let any = Value::obj(self.any_object(state));
        while let Some(o) = work.pop() {
            if o == rid || o == any.pts.first().unwrap_or(0) || !seen.insert(o) {
                continue;
            }
            let k = self.kind(o);
            if matches!(k, Kind::Module | Kind::Class | Kind::Function | Kind::External | Kind::Method | Kind::Bound) {
                continue;
            }
            let slots = state.slots_of(o);
            for (_, v) in &slots {
                work.extend(v.pts.iter());
            }
            // (a tuple cannot change; an unknown object's content is unknown already)
            if k == Kind::Tuple || (k == Kind::Unknown && !state.has_row(o, Ns::Item) && !state.has_row(o, Ns::Attr)) {
                continue;
            }
            for ns in [Ns::Item, Ns::Attr] {
                if state.has_row(o, ns) || (ns == Ns::Item && matches!(k, Kind::Dict | Kind::Frame)) || (ns == Ns::Attr && k == Kind::Instance) {
                    let r = state.row(o, ns).mutate(site);
                    state.set_row(o, ns, r);
                }
            }
            for (sk, v) in slots {
                if matches!(sk.0, Ns::Item | Ns::Attr | Ns::Pos | Ns::Elem) {
                    state.set_slot(o, sk.0, sk.1, v.join(&any), false);
                }
            }
            if matches!(k, Kind::List | Kind::Set) && !self.obj(o).site.starts_with("gen:") {
                let e = state.slot(o, Ns::Elem, intern::STAR).join(&any);
                state.set_slot(o, Ns::Elem, intern::STAR, e, false);
                let kv = crate::domain::kv_join(&state.elems.get(&o).unwrap_or(DATA), &DATA);
                state.elems.insert(o, kv);
                state.clear_last(o);
                self.set_prefix(o, state, 0);
            }
        }
    }

    /// An ABC with abstract methods no class of its MRO implements: `cls()` raises TypeError.
    pub fn abstract_unimplemented(&self, cls: u32) -> bool {
        let mro = self.mro(cls);
        let is_abc = mro.iter().any(|c| {
            self.class_info_opt(*c).map(|ci| {
                ci.node.arguments.as_ref().map(|a| a.args.iter().any(|b| super::dotted_last(b) == "ABC") || a.keywords.iter().any(|k| k.arg.as_ref().map(|x| x.as_str()) == Some("metaclass") && super::dotted_last(&k.value) == "ABCMeta")).unwrap_or(false)
            }).unwrap_or(false)
        });
        if !is_abc {
            return false;
        }
        // the first definition of each name along the MRO decides
        let mut seen: Vec<String> = Vec::new();
        for c in mro.iter() {
            let Some(ci) = self.class_info_opt(*c) else { continue };
            for st in &ci.node.body {
                let (name, decos) = match st {
                    ast::Stmt::FunctionDef(f) => (f.name.id.to_string(), &f.decorator_list),
                    ast::Stmt::Assign(a) => {
                        for t in &a.targets {
                            if let ast::Expr::Name(n) = t {
                                seen.push(n.id.to_string());
                            }
                        }
                        continue;
                    }
                    _ => continue,
                };
                if seen.contains(&name) {
                    continue;
                }
                seen.push(name);
                if decos.iter().any(|d| super::dotted_last(&d.expression) == "abstractmethod" || matches!(&d.expression, ast::Expr::Call(cl) if super::dotted_last(&cl.func) == "abstractmethod")) {
                    return true;
                }
            }
        }
        false
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

    /// What a model call does to the one-shot iterators it is given: a full consumer uses
    /// them up; a lazy wrapper (zip, islice, map, …) takes from them when it is consumed
    /// itself; anything else leaves their position unknown.
    fn after_model_consume(&mut self, call: &Call, r: &Value, state: &mut State) {
        // (one-shot iterators, and lazy iterators over them)
        let its: Vec<(usize, u32)> = call.args.iter().chain(call.star_args.iter()).enumerate().filter_map(|(i, a)| a.single().filter(|o| a.kv.is_bot() && ((self.obj(*o).iterator && self.obj(*o).single && matches!(self.kind(*o), Kind::List)) || self.lazy_src.contains_key(o))).map(|o| (i, o))).collect();
        if its.is_empty() {
            return;
        }
        let q = call.qual.as_str();
        const FULL: &[&str] = &[
            "builtins.list", "builtins.tuple", "builtins.set", "builtins.frozenset", "builtins.dict", "builtins.sorted", "builtins.sum", "builtins.min", "builtins.max",
            "str.join", "collections.Counter", "collections.deque", "collections.OrderedDict", "collections.defaultdict", "list.extend", "set.update", "dict.update",
            "statistics.mean", "statistics.median", "statistics.fmean", "math.fsum", "math.prod", "bytes.join",
        ];
        const LAZY_ALL: &[&str] = &["builtins.map", "builtins.filter", "builtins.enumerate", "builtins.iter", "itertools.chain", "itertools.starmap", "itertools.takewhile", "itertools.dropwhile", "itertools.accumulate", "itertools.compress", "itertools.groupby", "itertools.pairwise", "itertools.batched"];
        if matches!(q, "builtins.next" | "builtins.iter") && its.len() == 1 && r.single() == Some(its[0].1) {
            return;
        }
        if q == "builtins.next" {
            return;
        }
        if FULL.contains(&q) {
            for (_, o) in its {
                self.exhaust(&Value::obj(o), state);
            }
            return;
        }
        let robj = r.single().filter(|ro| !its.iter().any(|(_, o)| o == ro));
        if q == "itertools.islice" {
            // islice(it, stop) / islice(it, start, stop): start + the items it gives
            let lit = |v: Option<&Value>| v.and_then(|x| x.kv.as_lit().filter(|_| x.pts.is_empty())).and_then(intern::key_as_int);
            let n = match call.args.len() {
                2 => lit(call.args.get(1)),
                n if n >= 3 => lit(call.args.get(2)),
                _ => None,
            };
            if let (Some(ro), Some(n)) = (robj, n) {
                self.lazy_src.insert(ro, vec![(its[0].1, Some(n.max(0) as usize))]);
                return;
            }
        }
        if q == "builtins.zip" {
            if let (Some(ro), Some(pulls)) = (robj, self.zip_pulls(call, state)) {
                self.lazy_src.insert(ro, pulls);
                return;
            }
        }
        if LAZY_ALL.contains(&q) {
            if let Some(ro) = robj {
                self.lazy_src.insert(ro, its.iter().map(|(_, o)| (*o, None)).collect());
                return;
            }
        }
        for (_, o) in its {
            if self.lazy_src.contains_key(&o) {
                self.consume_lazy(&Value::obj(o), false, state);
            } else {
                self.set_cursor(o, None, state);
            }
        }
    }

    /// The items `zip(...)` takes from each one-shot iterator argument when it is consumed:
    /// rounds = the shortest length; in the last round the arguments before the first that
    /// ran out each gave one more.
    fn zip_pulls(&mut self, call: &Call, state: &mut State) -> Option<Vec<(ObjId, Option<usize>)>> {
        if !call.star_args.is_empty() || call.kwargs.keys().any(|k| &*intern::str_get(*k) != "strict") {
            return None;
        }
        // (exact remaining length, or a lower bound)
        let mut lens: Vec<(Option<usize>, usize, Option<u32>)> = Vec::new();
        for a in &call.args {
            let one = a.single().filter(|o| a.kv.is_bot() && self.obj(*o).iterator && self.obj(*o).single && matches!(self.kind(*o), Kind::List));
            match one {
                Some(o) => {
                    let c = self.cursor_of(o, state)?;
                    match self.llen(o, state) {
                        Some(n) => lens.push((Some(n.saturating_sub(c)), n.saturating_sub(c), Some(o))),
                        None => lens.push((None, self.prefix(o, state).unwrap_or(0).saturating_sub(c), Some(o))),
                    }
                }
                None => {
                    let it = self.iterate(a, &call.node, state);
                    let n = it.items.map(|v| v.len())?;
                    lens.push((Some(n), n, None));
                }
            }
        }
        let rounds = lens.iter().filter_map(|(e, _, _)| *e).min()?;
        // (an argument of unknown length must surely outlast the shortest)
        if lens.iter().any(|(e, lb, _)| e.is_none() && *lb <= rounds) {
            return None;
        }
        let first_out = lens.iter().position(|(e, _, _)| *e == Some(rounds))?;
        let mut out = Vec::new();
        for (i, (_, _, o)) in lens.iter().enumerate() {
            if let Some(o) = o {
                out.push((*o, Some(if i < first_out { rounds + 1 } else { rounds })));
            }
        }
        Some(out)
    }

    pub fn construct(&mut self, cls: u32, args: &[Value], kwargs: &IndexMap<StrId, Value>, star_args: &[Value], star_kwargs: &[Value], state: &mut State, node: &Node) -> Value {
        if self.abstract_unimplemented(cls) {
            self.raise_here("TypeError", state);
            return Value::new();
        }
        // `Color(3)`: the Enum member with that value, else ValueError
        if let (Some(ms), Some(a)) = (self.enum_members.get(&cls).cloned(), args.first()) {
            let vk = key_str_lit("value");
            let lits = if a.pts.is_empty() && a.kv.is_literal() { Some(crate::domain::kv_literals(&a.kv)) } else { None };
            let mut out = Value::new();
            match lits {
                Some(ls) => {
                    let mut all_hit = true;
                    let is_flag = self.enum_kind.get(&cls).map(|k| k.flag).unwrap_or(false);
                    let all_bits: i64 = ms.iter().filter_map(|(_, m)| state.slot(*m, Ns::Attr, vk).kv.as_lit().and_then(intern::key_as_int)).fold(0, |a, b| a | b);
                    for l in ls.iter() {
                        let hit = ms.iter().find(|(_, m)| state.slot(*m, Ns::Attr, vk).kv.as_lit() == Some(l));
                        match hit {
                            Some((_, m)) => out = out.join(&Value::obj(*m)),
                            // a Flag: any combination of the members' bits (0 too)
                            None => match intern::key_as_int(l).filter(|i| is_flag && *i >= 0 && i & !all_bits == 0) {
                                Some(i) => {
                                    let fm = self.flag_member(cls, i, node, state);
                                    out = out.join(&Value::obj(fm));
                                }
                                None => all_hit = false,
                            },
                        }
                    }
                    if out.empty() {
                        self.raise_here("ValueError", state);
                        return Value::new();
                    }
                    if !all_hit {
                        self.may_raise_here("ValueError", state);
                    }
                }
                None => {
                    // a member given: itself
                    let given: Vec<u32> = a.pts.iter().filter(|o| ms.iter().any(|(_, m)| m == o)).collect();
                    if !given.is_empty() && given.len() == a.pts.len() && a.kv.is_bot() {
                        return Value::objs(crate::domain::ObjSet::from_vec(given));
                    }
                    self.may_raise_here("ValueError", state);
                    out = join_values(ms.iter().map(|(_, m)| Value::obj(*m)).collect::<Vec<_>>());
                }
            }
            return out;
        }
        let ci = self.class_info(cls);
        // a TypedDict class makes a plain dict
        if ci.typed_dict {
            let call = Call { qual: "builtins.dict".into(), recv: None, args: args.to_vec(), kwargs: kwargs.clone(), star_args: star_args.to_vec(), star_kwargs: star_kwargs.to_vec(), node: *node, call_ast: None, site: format!("L{}", node.line), line: node.line };
            if let Some(m) = self.models.lookup("builtins.dict") {
                return m(self, state, &call);
            }
        }
        // a class with its own `__new__`: the instance is what it returns, and `__init__` runs
        // on it when it is an instance of the class
        if let Some(owner) = self.class_lookup(cls, "__new__") {
            let fv = self.cls_slots.get(&(owner, Ns::Attr, key_str_lit("__new__"))).cloned().unwrap_or_default();
            let fns: Vec<ObjId> = fv.pts.iter().filter(|f| self.kind(*f) == Kind::Function).collect();
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
                    let insts: Vec<ObjId> = out.pts.iter().filter(|o| self.kind(*o) == Kind::Instance && self.instance_class(*o).is_some_and(|c| self.mro(c).contains(&cls))).collect();
                    for inst in insts {
                        for f in iv.pts.iter().filter(|f| self.kind(*f) == Kind::Function).collect::<Vec<_>>() {
                            self.call_user(f, Some(Value::obj(inst)), args.to_vec(), kwargs.clone(), star_args.to_vec(), star_kwargs.to_vec(), state, node, None, false);
                        }
                    }
                }
                return out;
            }
        }
        let inst = self.alloc_instance(cls, node, state);
        // a pydantic model (without its own `__init__`): validation of the keyword input
        if crate::models::pydantic::is_validated(self, cls) {
            let own_init = self.class_lookup(cls, "__init__").map(|c| crate::models::pydantic::is_validated(self, c) && self.cls_slots.contains_key(&(c, Ns::Attr, key_str_lit("__init__")))).unwrap_or(false);
            if !own_init {
                // a pydantic dataclass takes its fields positionally too
                let mut kw = kwargs.clone();
                if crate::models::pydantic::is_validated(self, cls) && !crate::models::pydantic::is_model(self, cls) {
                    let names: Vec<String> = self.dataclass_fields_mro(cls).into_iter().map(|(n, _)| n).collect();
                    for (i, a) in args.iter().enumerate() {
                        if let Some(n) = names.get(i) {
                            kw.insert(intern::s(n), a.clone());
                        }
                    }
                }
                return self.pyd_construct(cls, args, &kw, star_kwargs, state, node);
            }
        }
        // a dict subclass without its own `__init__`: dict's constructor fills its items
        let dict_based = self.mro(cls).iter().any(|c| self.class_info_opt(*c).map(|x| x.dict_base).unwrap_or(false));
        if dict_based && self.class_lookup(cls, "__init__").is_none() {
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
            self.dataclass_init(cls, inst, args, star_args, kwargs, star_kwargs, state, node);
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
        } else if self.builtin_exc_base(cls) {
            // an Exception subclass without its own `__init__`: args are the arguments
            let name = self.class_info(cls).name.clone();
            let t = self.alloc_positional(Kind::Tuple, node, Some(args.to_vec()), Value::new(), DATA, state);
            let strong = self.obj(inst).single;
            self.store_key(inst, Ns::Attr, &KeyVal::Lit(key_str_lit("args")), &t, strong, node, state);
            self.exc_objs.insert(inst, name);
        }
        Value::obj(inst)
    }

    /// A fresh instance of user class `cls` allocated at `node`, without attributes (an
    /// external base's may come from outside the program).
    pub fn alloc_instance(&mut self, cls: ObjId, node: &Node, state: &mut State) -> ObjId {
        let ci = self.class_info(cls);
        let row = if ci.open_base { Row::with_source(Source::new(&format!("instance {}@L{}", ci.name, node.line), SourceKind::Class)) } else { Row::new() };
        // the allocation site includes the class: `cls()` with several possible classes must
        // not collapse onto one instance (prototype: same site, last class wins)
        let site = format!("{}:{}", super::site_string(node), ci.name);
        let inst = self.alloc(Kind::Instance, &site, state, &format!("{}@L{}", ci.name, node.line), Payload::Cls(cls), Some(row), Ns::Attr, None, None);
        if ci.typed_dict {
            state.set_row(inst, Ns::Item, Row::new());
        } else if ci.open_base && !ci.dict_base {
            // (an external base — `list`, a library class — may hold items the program's code
            // does not show: `append` / `super().__init__(items)` run there)
            state.set_row(inst, Ns::Item, Row::with_source(Source::new(&format!("items of {}@L{}", ci.name, node.line), SourceKind::Class)));
        }
        inst
    }

    /// The generated `__init__` of a dataclass / TypedDict / NamedTuple `cls` run on `inst`
    /// (a fresh instance, or `self` when called explicitly as `Base.__init__(self, …)` /
    /// `super().__init__(…)` from a subclass).
    pub fn dataclass_init(&mut self, cls: u32, inst: ObjId, args: &[Value], star_args: &[Value], kwargs: &IndexMap<StrId, Value>, star_kwargs: &[Value], state: &mut State, node: &Node) {
        let ci = self.class_info(cls);
        let ns = if ci.typed_dict { Ns::Item } else { Ns::Attr };
        let mut initvars: Vec<Value> = Vec::new();
        // `*seq` arguments: a sequence of known length gives positional arguments in order; past
        // one of unknown length every further positional field may come from it (its element)
        let mut args_full: Vec<Value> = args.to_vec();
        let mut star_rest: Option<Value> = None;
        for sa in star_args {
            if star_rest.is_some() {
                break;
            }
            // (a list / tuple of known length: its positions)
            if let Some(o) = sa.single().filter(|o| matches!(self.kind(*o), Kind::Tuple | Kind::List) && sa.kv.is_bot()) {
                if let Some(n) = self.llen(o, state) {
                    for i in 0..n {
                        args_full.push(state.slot(o, Ns::Pos, intern::key_int(i as i64)));
                    }
                    continue;
                }
            }
            let it = self.iterate(sa, node, state);
            match it.items {
                Some(items) if it.exhaustive => args_full.extend(items),
                _ => {
                    star_rest = Some(match it.elem {
                        super::funcs::Elem::One(v) => v,
                        super::funcs::Elem::Pair(a, b) => a.join(&b),
                    })
                }
            }
        }
        let args: &[Value] = &args_full;
        {
            let specs = self.dc_specs(cls);
            let mut pos = 0usize;
            let n_positional = specs.iter().filter(|f| f.init && !f.kw_only).count();
            if args.len() > n_positional {
                // more positional arguments than the __init__ takes
                self.raise_here("TypeError", state);
                return;
            }
            for spec in specs.iter() {
                let (fname, default) = (&spec.name, spec.default);
                let fk = key_str_lit(fname);
                let mut v: Option<Value> = None;
                if !spec.init {
                    // `field(init=False)`: its default
                    let d = default.map(|d| self.dataclass_default(d, node, state)).unwrap_or_default();
                    if !d.empty() {
                        let r = state.row(inst, ns).store(fk, true);
                        state.set_row(inst, ns, r);
                        state.set_slot(inst, ns, fk, d, true);
                    }
                    continue;
                }
                let positional = !spec.kw_only && pos < args.len();
                if !spec.kw_only {
                    pos += 1;
                }
                if positional {
                    v = Some(args[pos - 1].clone());
                } else if let Some(kv) = kwargs.get(&s(fname)) {
                    v = Some(kv.clone());
                } else if let (Some(rest), false) = (&star_rest, spec.kw_only) {
                    // (from the sequence of unknown length, or the default when it is short)
                    let r = if rest.empty() { Value::data("field") } else { rest.clone() };
                    v = Some(match default {
                        Some(d) => self.dataclass_default(d, node, state).join(&r),
                        None => r,
                    });
                } else if !star_kwargs.is_empty() {
                    // `**data` supplies the field when it has the key (over the default); a
                    // field without a default must come from it: a read (a requirement on an
                    // open input)
                    let present = star_kwargs.iter().any(|sk| sk.pts.iter().any(|x| state.row(x, Ns::Item).present.contains(fk)));
                    let definite = star_kwargs.iter().all(|sk| !sk.pts.is_empty() && sk.pts.iter().all(|x| state.row(x, Ns::Item).definite.contains(fk)));
                    // a parameter's mapping: whether it has the key is known at the call site
                    let mut acc = join_values(star_kwargs.iter().flat_map(|sk| sk.pts.iter().map(|x| state.slot(x, Ns::Item, fk)).collect::<Vec<_>>()));
                    match default {
                        Some(d) if !definite => {
                            let dv = self.dataclass_default(d, node, state);
                            acc = if present { acc.join(&dv) } else { dv.join(&acc) };
                        }
                        Some(_) => {}
                        None if !definite => {
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
                let v = v.filter(|x| !x.empty()).unwrap_or_else(|| Value::data("field"));
                if spec.initvar {
                    // an InitVar: an argument of __post_init__, no attribute
                    initvars.push(v);
                    continue;
                }
                let r = state.row(inst, ns).store(fk, true);
                state.set_row(inst, ns, r);
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
                    self.call_user(f, Some(Value::obj(inst)), initvars.clone(), IndexMap::new(), Vec::new(), Vec::new(), state, node, None, false);
                }
            }
        }
    }

    /// The fields of a dataclass (TypedDict / NamedTuple) as its `__init__` takes them: bases
    /// first; `InitVar`s (init-only), `field(init=False)`, keyword-only ones (after `_: KW_ONLY`,
    /// `field(kw_only=True)`, `@dataclass(kw_only=True)`).
    pub fn dc_specs(&self, cls: u32) -> Vec<DcSpec> {
        let mut out: Vec<DcSpec> = Vec::new();
        for c in self.mro(cls).into_iter().rev() {
            let Some(ci) = self.class_info_opt(c) else { continue };
            if !ci.has_fields() {
                continue;
            }
            let attrs: Vec<(String, Option<&'static ast::Expr>)> = ci.fields();
            let mut kw_only = ci.dc_kw_only.get();
            for st in &ci.node.body {
                let ast::Stmt::AnnAssign(a) = st else { continue };
                let ast::Expr::Name(n) = &*a.target else { continue };
                let ann_name = match &*a.annotation {
                    ast::Expr::Subscript(sb) => super::dotted_last(&sb.value),
                    other => super::dotted_last(other),
                };
                if ann_name == "KW_ONLY" {
                    kw_only = true;
                    continue;
                }
                let initvar = ann_name == "InitVar";
                let name = n.id.to_string();
                if !initvar && !attrs.iter().any(|(f, _)| *f == name) {
                    continue;
                }
                let default: Option<&'static ast::Expr> = a.value.as_deref();
                let mut init = true;
                let mut fkw = kw_only;
                if let Some(ast::Expr::Call(fc)) = default {
                    if super::dotted_last(&fc.func) == "field" {
                        for k in fc.arguments.keywords.iter() {
                            match (k.arg.as_ref().map(|x| x.as_str()), &k.value) {
                                (Some("init"), ast::Expr::BooleanLiteral(b)) => init = b.value,
                                (Some("kw_only"), ast::Expr::BooleanLiteral(b)) => fkw = b.value,
                                _ => {}
                            }
                        }
                    }
                }
                let spec = DcSpec { name: name.clone(), default, init, initvar, kw_only: fkw };
                match out.iter_mut().find(|x| x.name == name) {
                    Some(e) => *e = spec,
                    None => out.push(spec),
                }
            }
        }
        // classes whose fields are not from a class body (generated / NamedTuple): all plain
        if out.is_empty() {
            out = self.dataclass_fields_mro(cls).into_iter().map(|(name, default)| DcSpec { name, default, init: true, initvar: false, kw_only: false }).collect();
        }
        out
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
        // the default's names are the defining module's (a class from another module, a
        // generated namedtuple): evaluated there
        let here = self.module_info(self.frame().module).idx;
        let owner = crate::frontend::module_of_addr(d as *const ast::Expr as usize);
        if let Some(idx) = owner.filter(|i| *i != here) {
            if let Some(moid) = self.modules.values().copied().find(|m| self.module_info(*m).idx == idx) {
                let mi = self.module_info(moid);
                let (ctx, single) = (self.frame().ctx.clone(), self.frame().single);
                self.frames.push(Frame::new(FrameKind::Module, mi.scopes.module(), moid, &ctx, single, "<default>"));
                let v = self.dataclass_default_here(d, node, state);
                self.frames.pop();
                return v;
            }
        }
        self.dataclass_default_here(d, node, state)
    }

    fn dataclass_default_here(&mut self, d: &'static ast::Expr, node: &Node, state: &mut State) -> Value {
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
                    } else if self.site_clones.get(&*recv.site).copied().unwrap_or(0) >= MAX_CLONES {
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
        // past the hard deadline a user function is unknown code (its result unknown, its
        // arguments possibly changed, it may raise): the run ends in bounded time
        if self.over_hard_deadline() {
            let q = self.func_info(foid).qualname.clone();
            self.note(format!("analysis cut at the deadline ({} ms): later calls are unknown code", super::limit("ms", super::DEADLINE_MS)));
            return self.unknown_call(&q, self_v.as_ref(), &args, &kwargs, &star_args, &star_kwargs, state, node);
        }
        // (user code called back by a model allocates as user code)
        let saved_md = std::mem::replace(&mut self.model_depth, 0);
        // (the arguments, for the unknown call that replaces this one when the hard deadline cuts it)
        let was_cut = self.hard_cut;
        let keep = (self_v.clone(), args.clone(), kwargs.clone(), star_args.clone(), star_kwargs.clone());
        let r = self.call_user_inner(foid, self_v, args, kwargs, star_args, star_kwargs, state, node, _call_ast, entry);
        self.model_depth = saved_md;
        if self.hard_cut && !was_cut {
            let q = self.func_info(foid).qualname.clone();
            self.note(format!("analysis cut at the deadline ({} ms) inside {}: the call is unknown code", super::limit("ms", super::DEADLINE_MS), q));
            state.reachable = true;
            let (sv, a, kw, sa, skw) = keep;
            return self.unknown_call(&q, sv.as_ref(), &a, &kw, &sa, &skw, state, node);
        }
        r
    }

    #[allow(clippy::too_many_arguments)]
    fn call_user_inner(&mut self, foid: u32, self_v: Option<Value>, args: Vec<Value>, kwargs: IndexMap<StrId, Value>, star_args: Vec<Value>, star_kwargs: Vec<Value>, state: &mut State, node: &Node, _call_ast: Option<&'static ast::ExprCall>, entry: bool) -> Value {
        // a singledispatch function: the implementation for the first argument's type
        if self.sdispatch.contains_key(&foid) && self.sd_bypass.take() != Some(foid) && !args.is_empty() && star_args.is_empty() {
            return self.sdispatch_call(foid, self_v, args, kwargs, star_kwargs, state, node, entry);
        }
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
            for i in &fr.unroll {
                site.push_str(&format!("#{}", i));
            }
            if fr.kind != FrameKind::Module && !fr.ctx.is_empty() && !fr.ctx.starts_with("entry") {
                if fr.ctx.matches('/').count() < super::CHAIN {
                    format!("{}/{}", fr.ctx, site)
                } else {
                    // deeper chains stay distinct through a hash of the prefix: two activations
                    // of the caller keep allocating distinct objects at any depth (bounded by
                    // the tabulation budgets), while the context string stays short
                    let mut h = rustc_hash::FxHasher::default();
                    std::hash::Hash::hash(&fr.ctx, &mut h);
                    format!("h{:x}/{}", std::hash::Hasher::finish(&h), site)
                }
            } else {
                site
            }
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
                                let dims: Option<Vec<i64>> = sh.prefix.iter().map(|d| crate::shape::as_lit(*d)).collect();
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
            if let Some((ret, exit_state, fp)) = self.rec_table.get(&key) {
                let ret = ret.clone();
                state.join_heap_fp(exit_state, fp);
                return ret;
            }
            return Value::new();
        }
        // summary cache: same callee, same call site, same observable input → replay the effects
        let sig = self.call_signature(foid, &site, entry, &self_v, &args, &kwargs, &star_args, &star_kwargs, state);
        if let Some(sig) = sig.as_ref() {
            // (a run that raised, under a try that may catch it: the handlers need the state at
            // each raise, which only a run records)
            let hit = self.call_cache.get(sig).cloned().filter(|h| !(h.raises && !self.guards.is_empty()));
            if let Some(hit) = hit {
                if super::debug_enabled() {
                    eprintln!("cache hit for {} at {}", self.func_info(foid).qualname, site);
                }
                if hit.raises {
                    self.note_raise();
                }
                // (the facts the run changed, deletions too)
                for k in &hit.fp.rows {
                    match hit.fp_state.rows.get(k) {
                        Some(r) => state.rows.insert(*k, r),
                        None => {
                            state.rows.remove(k);
                        }
                    }
                }
                for (oid, sk) in &hit.fp.slots {
                    if matches!(sk.0, Ns::Pos | Ns::Elem) {
                        state.clear_last(*oid);
                    }
                    match hit.fp_state.slots.get(oid, sk) {
                        Some(v) => state.slots.set(oid, *sk, v),
                        None => {
                            state.slots.remove(oid, sk);
                        }
                    }
                }
                for k in &hit.fp.elems {
                    match hit.fp_state.elems.get(k) {
                        Some(v) => state.elems.insert(*k, v),
                        None => {
                            state.elems.remove(k);
                        }
                    }
                }
                for k in &hit.lens {
                    match hit.fp_state.lens.get(k) {
                        Some(v) => state.lens.insert(*k, v),
                        None => {
                            state.lens.remove(k);
                        }
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
                            recents.push((o, ob.kind, base.to_string(), ob.ctx.to_string()));
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
                            if let Some(cur) = state.reqs.get(&src, k) {
                                state.reqs.set(&src, *k, ReqInfo { always: false, ..cur });
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
                    if !self.verdict_keys.contains(&dk) && !self.quiet {
                        self.verdict_keys.insert(dk);
                        self.verdicts.push(nv);
                    }
                }
                let ret = hit.ret.clone();
                let rows: Vec<(u32, Row)> = ret.pts.iter().filter(|o| self.is_record_like(*o)).map(|o| (o, state.row(o, Ns::Item))).collect();
                self.summaries.entry((fi.qualname.clone(), site.clone())).or_insert((ret.clone(), rows));
                if !hit.completes {
                    state.reachable = false;
                }
                return ret;
            }
        }
        // every statement a function code runs counts against its budget (entry runs aside: one
        // per function); past it a call is unknown code, so the whole analysis is bounded by the
        // number of functions times the budget
        if !entry && self.tab_count.get(&fi.node.addr()).copied().unwrap_or(0) > super::limit("budget", super::FN_BUDGET) {
            let q = fi.qualname.clone();
            self.note(format!("{} ran out of its budget: later calls are unknown code", q));
            self.budget_out.insert(foid);
            return self.unknown_call(&q, self_v.as_ref(), &args, &kwargs, &star_args, &star_kwargs, state, node);
        }
        if super::debug_enabled() {
            eprintln!("run {} at {} frames={} chain={:?}", fi.qualname, site, self.frames.len(), self.call_chain);
        }
        // context for allocations inside the callee (§3)
        let (ctx, ctx_single) = self.receiver_ctx(&self_v, &site);
        let fr_single = self.frame().single && self.frame().loop_depth == 0 && ctx_single;
        let fr_single_nl = self.frame().single_nl && ctx_single;
        let mut new_fr = Frame::new(FrameKind::Function, fi.scope, fi.module, &ctx, fr_single, &fi.qualname);
        new_fr.single_nl = fr_single_nl;
        new_fr.func = Some(foid);
        new_fr.code = fi.node.addr();
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
        let saved_unbound = std::mem::take(&mut state.unbound);
        self.env_stack.push(saved_env.clone());
        self.raise_stack.push(false);
        self.esc_cls.push(Some(Vec::new()));
        let raise_mark = self.raise_records;
        let saved_ret = state.ret.take();
        // (the caller's recorded returns are not the callee's: its end must not join them in)
        let saved_exits = { state.exits.take() };
        let mut saved_yields: Option<(ObjId, Option<super::YieldStates>)> = None;
        self.frames.push(new_fr);
        if let FuncNode::Def(f) = fnode {
            if is_generator(&f.body) {
                // the yielded values of this activation
                let gid = self.alloc(Kind::List, &format!("gen:{}", foid), state, &format!("generator {}", fi.qualname), Payload::None, None, Ns::Item, None, Some(&ctx));
                self.set_llen(gid, state, Some(0));
                state.elems.insert(gid, KeyVal::Lits(KeySet::new()));
                // (a new activation: not consumed yet, its own item states; a running one's
                // yields are put back after it)
                // (keyed as `yield_value` finds it: the site's object, not a variant `alloc` made)
                let ygid = self.alloc_raw(Kind::List, &format!("gen:{}", foid), &ctx, true, &format!("generator {}", fi.qualname), Payload::None);
                saved_yields = Some((ygid, self.gen_yield_states.remove(&ygid)));
                state.slots.remove(&gid, &(Ns::Attr, key_str_lit("__cursor")));
                state.slots.remove(&gid, &(Ns::Attr, key_str_lit("\u{1}effpos")));
                self.gen_pos_states.remove(&gid);
                self.gen_pos_conflict.retain(|(g, _)| *g != gid);
                self.gen_eff.remove(&gid);
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
        let obj_mark_call = self.objects.len() as ObjId;
        if !matches!(fnode, FuncNode::Def(f) if is_generator(&f.body)) {
            if let Some(fr) = self.frames.last_mut() {
                fr.pre_call = Some(super::PreCall(obj_mark_call, std::rc::Rc::new(pre_bind.copy())));
            }
        }
        let gen_cm = self.cm_hook.as_ref().is_some_and(|h| !h.ran && h.depth + 1 == self.frames.len());
        self.bind_params(&fi, &self_v, &args, &kwargs, &star_args, &star_kwargs, state, node);
        let mut entry_state = state.copy();
        let mut ret = Value::new();
        let mut st_final: Option<State> = None;
        for _round in 0..REC_MAX_ITER {
            self.tab_runs += 1;
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
        let converged = st_final.is_some();
        // (the result stabilised even if the state did not: it stands)
        let ret_stable = self.rec_table.get(&key).map(|(pr, _, _)| *pr == ret).unwrap_or(false);
        let mut st = match st_final {
            Some(s) => s,
            None => {
                // did not converge: the last tabulated state, widened (its result may be any
                // value, the records it touched may hold other keys)
                let mut s0 = match self.rec_table.get(&key) {
                    Some((_, pst, _)) => pst.copy_ref(),
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
        // (at most CACHE_PER_FN entries per function: the cache's memory is bounded too)
        let room = self.cache_count.get(&fnode.addr()).copied().unwrap_or(0) < super::CACHE_PER_FN;
        if let Some(sig) = sig.filter(|_| room) {
            if !self.rec_hit.contains(&key) {
                *self.cache_count.entry(fnode.addr()).or_default() += 1;
                let mut rel = State::new();
                for (src, k) in st.reqs.diff_keys(&pre_bind.reqs) {
                    if let Some(info) = st.reqs.get(&src, &k) {
                        if pre_bind.reqs.get(&src, &k).as_ref() != Some(&info) {
                            rel.reqs.set(&src, k, info);
                        }
                    }
                }
                let mut fp = st.footprint(&pre_bind);
                // (what the call made that its result reaches, whole: a later hit replays it
                // into a state where those objects may have been collected meanwhile)
                let mut work: Vec<ObjId> = ret.pts.iter().filter(|o| *o >= obj_mark_call).collect();
                let mut seen: rustc_hash::FxHashSet<ObjId> = rustc_hash::FxHashSet::default();
                let rows_have: rustc_hash::FxHashSet<(ObjId, Ns)> = fp.rows.iter().copied().collect();
                let slots_have: rustc_hash::FxHashSet<(ObjId, crate::state::SlotKey)> = fp.slots.iter().copied().collect();
                let elems_have: rustc_hash::FxHashSet<ObjId> = fp.elems.iter().copied().collect();
                while let Some(o) = work.pop() {
                    if !seen.insert(o) || seen.len() > 4096 {
                        continue;
                    }
                    for ns in [Ns::Item, Ns::Attr] {
                        if st.has_row(o, ns) && !rows_have.contains(&(o, ns)) {
                            fp.rows.push((o, ns));
                        }
                    }
                    st.slots.each(&o, &mut |sk, v| {
                        if !slots_have.contains(&(o, *sk)) {
                            fp.slots.push((o, *sk));
                        }
                        work.extend(v.pts.iter().filter(|x| *x >= obj_mark_call));
                    });
                    if st.elems.get(&o).is_some() && !elems_have.contains(&o) {
                        fp.elems.push(o);
                    }
                }
                let lens_changed: Vec<ObjId> = st.lens.diff_keys(&pre_bind.lens);
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
                for k in &lens_changed {
                    if let Some(v) = st.lens.get(k) {
                        fp_state.lens.insert(*k, v);
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
                self.call_cache.insert(sig, Rc::new(CacheHit { raises, completes: st.reachable, lens: lens_changed, ret: ret.clone(), fp_state, fp, verdicts: rel_verdicts }));
            }
        }
        // the generator's body runs here, eagerly, but its caller sees the heap only where the
        // body is suspended: before the first item, at a yield, or at its end. The objects that
        // existed before the call and that the body changed hold the join of those states.
        let gen_yields = match fnode {
            FuncNode::Def(f) if is_generator(&f.body) && !gen_cm => {
                let gid = self.alloc_raw(Kind::List, &format!("gen:{}", foid), &ctx, true, &format!("generator {}", fi.qualname), Payload::None);
                Some((gid, self.gen_yield_states.get(&gid).map(|v| v.states().map(|s| s.copy_ref()).collect::<Vec<State>>()).unwrap_or_default()))
            }
            _ => None,
        };
        // (a generator that ran to its end: the state there, for a consumer that uses it up)
        let finite_end = if gen_yields.is_some() && st.reachable { Some(st.copy()) } else { None };
        // a generator whose body never ends (`while True: yield …`): the caller resumes from
        // one of its yields, and the yields go on (their number is unknown)
        if let FuncNode::Def(f) = fnode {
            if is_generator(&f.body) && !st.reachable {
                let gid = self.alloc_raw(Kind::List, &format!("gen:{}", foid), &ctx, true, &format!("generator {}", fi.qualname), Payload::None);
                let joined = self.gen_yield_states.remove(&gid).and_then(|v| {
                    let mut it = v.list.into_iter().map(|(_, s)| s);
                    let mut acc = it.next()?;
                    for mut s in it {
                        acc = acc.join(&mut s);
                    }
                    Some(acc)
                });
                if let Some(mut ys) = joined {
                    let mut p = self.prefix(gid, &ys).unwrap_or(0);
                    // (the yields joined: every position some yield made is there — the items
                    // the endless loop is known to give first)
                    while ys.has_slot(gid, Ns::Pos, intern::key_int(p as i64)) {
                        p += 1;
                    }
                    if p > 0 {
                        self.objects[gid as usize].known_prefix = Some(p);
                    }
                    ys.lens.insert(gid, (None, p, 0));
                    st = ys;
                }
            }
        }
        if let Some((gid, ys)) = gen_yields {
            self.gen_suspension_join(&mut st, &pre_bind, &ys, gid, obj_mark_call);
            // the effects per item: before the first, the objects are as before the call
            if st.reachable {
                let fp = st.footprint(&pre_bind);
                let old = |o: ObjId| o < obj_mark_call && o != gid;
                let rows: Vec<(ObjId, Ns)> = fp.rows.iter().copied().filter(|(o, _)| old(*o)).collect();
                let slots: Vec<(ObjId, crate::state::SlotKey)> = fp.slots.iter().copied().filter(|(o, _)| old(*o)).collect();
                let elems: Vec<ObjId> = fp.elems.iter().copied().filter(|o| old(*o)).collect();
                let mut lens: Vec<ObjId> = slots.iter().map(|(o, _)| *o).chain(elems.iter().copied()).collect();
                lens.sort_unstable();
                lens.dedup();
                if !rows.is_empty() || !slots.is_empty() || !elems.is_empty() {
                    let at = self.gen_pos_states.remove(&gid).unwrap_or_default();
                    let ge = super::GenEffects { rows, slots, elems, lens, pre: pre_bind.copy(), at, end: finite_end, join: st.copy() };
                    crate::interp::exprs::gen_copy_facts_pub(&mut st, &pre_bind, &ge);
                    st.set_slot(gid, Ns::Attr, key_str_lit("\u{1}effpos"), Value::lit(key_str_lit("pre")), true);
                    self.gen_eff.insert(gid, std::rc::Rc::new(ge));
                }
            }
        }
        if let Some((gid, prev)) = saved_yields {
            match prev {
                Some(v) => {
                    self.gen_yield_states.insert(gid, v);
                }
                None => {
                    self.gen_yield_states.remove(&gid);
                }
            }
        }
        state.set_to(&mut st);
        if let FuncNode::Def(f) = fnode {
            if is_generator(&f.body) {
                let gid = self.alloc_raw(Kind::List, &format!("gen:{}", foid), &ctx, true, &format!("generator {}", fi.qualname), Payload::None);
                // what `yield from` the generator evaluates to: its return value
                state.set_slot(gid, Ns::Attr, key_str_lit("\u{1}genret"), ret.clone(), true);
                // (one-shot: what a consumer takes is gone for the next one)
                self.obj_mut(gid).iterator = true;
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
        let callee_cls = self.esc_cls.pop().flatten();
        if raised {
            if crate::interp::debug_level() == 2 {
                eprintln!("  callee {} raised (tabulated at L{})", fi.qualname, node.line);
            }
            // (a raise the enclosing try recorded, with its state and exception, needs no
            // "raised somewhere" flag: the handler gets that exception, not an unknown one; nor
            // do raises of known classes no guard here catches)
            let known = callee_cls.as_ref().filter(|cs| !cs.is_empty() && !cs.iter().any(|c| self.class_guarded(c)));
            if self.raise_records > raise_mark || known.is_some() {
                if let Some(t) = self.raise_stack.last_mut() {
                    *t = true;
                }
                self.merge_esc(callee_cls.clone());
            } else {
                self.note_raise();
            }
        }
        state.env = saved_env;
        state.unbound = saved_unbound;
        state.ret = saved_ret;
        if saved_exits.is_some() {
            state.exits = saved_exits;
        }
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
        let Some((fl, el, pre, tmark)) = self.try_marks.last() else { return false };
        let tmark = *tmark;
        let mut here = state.copy();
        // the try is in a `@contextmanager` generator whose with body is running (also from a
        // callee of the body): the generator's names
        let in_cm = self.cm_frames.iter().rev().find(|(d, _, el0)| *d == *fl && *el <= *el0).map(|(_, env, _)| env.clone());
        if let Some(env) = in_cm {
            here.env = env;
            here.narrowed = Default::default();
        } else if self.frames.len() < *fl {
            if let Some((_, env, _)) = self.cm_frames.iter().rev().find(|(d, _, _)| *d == *fl) {
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
        let cls = self.cur_exc_cls.take().or_else(|| self.exc_value_names(&exc));
        // (the same state, class and exception as the last raise recorded in this try — a loop
        // of summarised calls raising at one call site: nothing to add)
        let depth = self.raise_states.len();
        if depth > 0 {
            let open = self.raise_states.last().is_some_and(|s| s.is_some());
            if let Some((ls, lc, le)) = self.raise_last.get(&depth) {
                if open && *lc == cls && *le == exc && ls.same(&here) {
                    return true;
                }
            }
            self.raise_last.insert(depth, (here.copy_ref(), cls.clone(), exc.clone()));
        }
        // (what callees made since the try began and nothing reaches after the unwind — the
        // try's names, the frames above it, the exception — is garbage in the raise state: a
        // try around a large recursion otherwise joins every callee's objects at every raise)
        {
            let fl = *fl;
            let el = *el;
            let pre_try = pre.copy_ref();
            let mut roots: Vec<ObjId> = Vec::new();
            let push = |v: &Value, roots: &mut Vec<ObjId>| {
                roots.extend(v.pts.iter());
                match &v.kv {
                    KeyVal::KeysOf(ko) => roots.extend(ko.objs.iter()),
                    _ => {}
                }
            };
            for v in here.env.values() {
                push(v, &mut roots);
            }
            for v in here.narrowed.values() {
                push(v, &mut roots);
            }
            if let Some(r) = &here.ret {
                push(r, &mut roots);
            }
            for env in self.env_stack.iter().take(el) {
                for v in env.values() {
                    push(v, &mut roots);
                }
            }
            for fr in self.frames.iter().take(fl) {
                roots.push(fr.module);
                roots.extend(fr.cells.values().copied());
                for ov in &fr.overlays {
                    for v in ov.values() {
                        push(v, &mut roots);
                    }
                }
            }
            for (o, v) in here.lasts.iter() {
                roots.push(*o);
                push(v, &mut roots);
            }
            push(&exc, &mut roots);
            self.collect_since(tmark, &pre_try, roots, &mut here);
        }
        // (a try that keeps its raises one by one joins them where a handler needs them: the
        // running join is only a mark that something was recorded)
        let mut kept = false;
        if let Some((d, recs)) = self.raise_cls.last_mut() {
            if *d == depth {
                recs.push((cls, here.copy(), exc.clone()));
                kept = true;
            }
        }
        if let Some(rv) = self.raise_values.last_mut() {
            *rv = Some(match rv.take() {
                None => exc,
                Some(prev) => prev.join(&exc),
            });
        }
        let Some(slot) = self.raise_states.last_mut() else { return false };
        *slot = Some(match slot.take() {
            _ if kept => State::bottom(),
            None => here,
            Some(mut prev) => prev.join(&mut here),
        });
        self.raise_records += 1;
        true
    }

    /// The heap a generator's caller sees (see the call): the touched objects older than
    /// `mark` (but the generator's own item list) joined over `pre`, the yield states and `st`.
    fn gen_suspension_join(&mut self, st: &mut State, pre: &State, yields: &[State], gid: ObjId, mark: ObjId) {
        if !st.reachable {
            return;
        }
        let fp = st.footprint(pre);
        let old = |o: ObjId| o < mark && o != gid;
        let ys: Vec<&State> = yields.iter().filter(|y| y.reachable).collect();
        // Each fact of `st` is joined with `pre`'s and every yield state's. Yield states are
        // copies of one another changed by the body, sharing structure: each is diffed against
        // the one before it (the first against `pre`), and only the keys the diff names are
        // joined. A fact a state does not change equals the previous state's, joined already
        // (a generator can hold thousands of yield states, and joining every touched fact with
        // each was facts × yields).
        let entry = |an: &Analyzer, s: &State, o: ObjId| -> (Option<usize>, usize, u16) {
            s.lens.get(&o).unwrap_or_else(|| {
                let ob = &an.objects[o as usize];
                (ob.literal_len, ob.literal_len.or(ob.known_prefix).unwrap_or(0), 0)
            })
        };
        // (with the distinct rows already joined: most yield states share their rows)
        let mut rows: rustc_hash::FxHashMap<(ObjId, Ns), (crate::domain::Row, rustc_hash::FxHashSet<crate::domain::Row>)> = rustc_hash::FxHashMap::default();
        for (o, ns) in fp.rows.iter().copied().filter(|(o, _)| old(*o)) {
            let (a, b) = (st.row(o, ns), pre.row(o, ns));
            let j = a.join(&b);
            rows.insert((o, ns), (j, [a, b].into_iter().collect()));
        }
        let mut slots: rustc_hash::FxHashMap<(ObjId, crate::state::SlotKey), Value> = rustc_hash::FxHashMap::default();
        let mut touched: Vec<ObjId> = Vec::new();
        for (o, sk) in fp.slots.iter().copied().filter(|(o, _)| old(*o)) {
            touched.push(o);
            slots.insert((o, sk), st.slots.get(&o, &sk).unwrap_or_default().join(&pre.slots.get(&o, &sk).unwrap_or_default()));
        }
        let mut elems: rustc_hash::FxHashMap<ObjId, KeyVal> = rustc_hash::FxHashMap::default();
        for o in fp.elems.iter().copied().filter(|o| old(*o)) {
            touched.push(o);
            elems.insert(o, crate::domain::kv_join(&st.elems.get(&o).unwrap_or(DATA), &pre.elems.get(&o).unwrap_or(DATA)));
        }
        touched.sort_unstable();
        touched.dedup();
        let mut lens: rustc_hash::FxHashMap<ObjId, (Option<usize>, usize, u16)> = touched.iter().map(|o| (*o, crate::state::join_len(&entry(self, st, *o), &entry(self, pre, *o)))).collect();
        let mut prev = pre;
        for y in ys {
            let base = std::mem::replace(&mut prev, y);
            for k in y.rows.diff_keys(&base.rows) {
                if let Some((r, seen)) = rows.get_mut(&k) {
                    let yr = y.row(k.0, k.1);
                    if !seen.contains(&yr) {
                        *r = r.join(&yr);
                        seen.insert(yr);
                    }
                }
            }
            for (o, sk) in y.slots.diff_keys(&base.slots) {
                if let Some(v) = slots.get_mut(&(o, sk)) {
                    *v = v.join(&y.slots.get(&o, &sk).unwrap_or_default());
                }
            }
            for o in y.elems.diff_keys(&base.elems) {
                if let Some(kv) = elems.get_mut(&o) {
                    *kv = crate::domain::kv_join(kv, &y.elems.get(&o).unwrap_or(DATA));
                }
            }
            for o in y.lens.diff_keys(&base.lens) {
                if let Some(l) = lens.get_mut(&o) {
                    *l = crate::state::join_len(l, &entry(self, y, o));
                }
            }
        }
        for ((o, ns), (r, _)) in rows {
            st.set_row(o, ns, r);
        }
        for ((o, sk), v) in slots {
            st.slots.set(&o, sk, v);
            if matches!(sk.0, Ns::Pos | Ns::Elem) {
                st.clear_last(o);
            }
        }
        for (o, kv) in elems {
            st.elems.insert(o, kv);
        }
        for (o, l) in lens {
            st.lens.insert(o, l);
        }
    }

    /// Does some running try's guard catch exceptions of this class?
    pub fn class_guarded(&self, c: &super::ExcNames) -> bool {
        self.guards.iter().any(|g| g.contains("*") || c.0.iter().any(|n| g.contains(n)) || (!c.1 && (g.contains("Exception") || g.contains("BaseException"))))
    }

    /// The running call may also raise these (None: something of unknown class).
    pub fn merge_esc(&mut self, cls: Option<Vec<super::ExcNames>>) {
        if let Some(e) = self.esc_cls.last_mut() {
            match (e.as_mut(), cls) {
                (Some(v), Some(cs)) => {
                    for c in cs {
                        if !v.contains(&c) {
                            v.push(c);
                        }
                    }
                }
                (Some(_), None) => *e = None,
                _ => {}
            }
        }
    }

    /// The class names of an exception value (one class; None: not known).
    pub fn exc_value_names(&self, v: &Value) -> Option<super::ExcNames> {
        if !v.kv.is_bot() || v.pts.is_empty() {
            return None;
        }
        let mut out: Option<super::ExcNames> = None;
        for o in v.pts.iter() {
            let names = if let Some(c) = self.instance_class(o) {
                self.exc_class_names(c)?
            } else if let Some(n) = self.exc_objs.get(&o) {
                super::builtin_exc_names(n)
            } else if self.kind(o) == Kind::External && self.class_info_opt(o).is_none() {
                // (a raised class: `raise KeyError`)
                super::builtin_exc_names(self.external_qual(o).rsplit('.').next().unwrap_or(""))
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
        let mro = self.mro(cls);
        let mut names: Vec<String> = Vec::new();
        let mut complete = true;
        for c in mro.iter() {
            let ci = self.class_info_opt(*c)?;
            names.push(ci.name.clone());
            let user: Vec<String> = ci.bases.iter().filter_map(|b| self.class_info_opt(*b).map(|x| x.name.clone())).collect();
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

    /// A builtin exception (`KeyError(key)`, …) as a value: open attributes, known `args`.
    pub fn exception_object(&mut self, cls_name: &str, args: Vec<Value>, node: &Node, state: &mut State) -> Value {
        let t = self.alloc_positional(Kind::Tuple, node, Some(args), Value::new(), DATA, state);
        let row = Row::with_source(crate::domain::Source::new(cls_name, crate::domain::SourceKind::Data));
        let oid = self.alloc_at(Kind::Namespace, node, state, &format!("{}@L{}", cls_name, node.line), Payload::None, Some(row), Ns::Attr, None, None);
        let strong = self.obj(oid).single;
        self.store_key(oid, Ns::Attr, &KeyVal::Lit(key_str_lit("args")), &t, strong, node, state);
        self.exc_objs.insert(oid, cls_name.to_string());
        Value::obj(oid)
    }

    /// `str(e)` / `repr(e)` of exception objects with known args.
    pub fn exc_text(&self, v: &Value, repr: bool, state: &State) -> Option<Value> {
        if v.pts.is_empty() || !v.kv.is_bot() {
            return None;
        }
        let mut out = Vec::new();
        for o in v.pts.iter() {
            let name = self.exc_objs.get(&o)?;
            let t = state.slot(o, Ns::Attr, key_str_lit("args"));
            let tid = t.single()?;
            let n = self.llen(tid, state)?;
            let mut parts = Vec::new();
            for i in 0..n {
                let a = state.slot(tid, Ns::Pos, intern::key_int(i as i64));
                parts.push(a.kv.as_lit().filter(|_| a.pts.is_empty())?);
            }
            let reprs: Vec<String> = parts.iter().map(|k| intern::key_repr(*k)).collect();
            let s = if repr {
                format!("{}({})", name, reprs.join(", "))
            } else if parts.len() == 1 {
                // (a KeyError shows its key's repr)
                if name == "KeyError" { reprs[0].clone() } else { intern::key_py_str(parts[0]) }
            } else if parts.is_empty() {
                String::new()
            } else if parts.len() == 1 {
                reprs[0].clone()
            } else {
                format!("({})", reprs.join(", "))
            };
            out.push(Value::lit(key_str_lit(&s)));
        }
        Some(crate::domain::join_values(out))
    }

    /// An exception named `name` is raised here (the rest of the path is dead).
    pub fn raise_here(&mut self, name: &str, state: &mut State) {
        self.may_raise_here(name, state);
        state.reachable = false;
    }

    /// An exception named `name` may be raised here: a handler that catches it starts from
    /// this state; otherwise the enclosing call raises.
    pub fn may_raise_here(&mut self, name: &str, state: &mut State) {
        if self.cur_exc_cls.is_none() {
            self.cur_exc_cls = Some(super::builtin_exc_names(name));
        }
        let caught = self.guards.iter().any(|g| g.contains("*") || g.contains("Exception") || g.contains("BaseException") || g.contains(name)) || self.cur_exc_cls.as_ref().is_some_and(|c| self.class_guarded(c));
        // a builtin exception a model raises: an object of its class (its args not known)
        if caught && self.cur_exc.is_none() && !self.raise_states.is_empty() {
            if let Some(nd) = self.exc_node {
                let n = name.rsplit('.').next().unwrap_or(name);
                if super::builtin_exc_names(n).1 && super::is_builtin_name(n) {
                    let e = self.exception_object(n, vec![], &nd, state);
                    if let Some(o) = e.pts.iter().next() {
                        let strong = self.obj(o).single;
                        self.store_key(o, Ns::Attr, &KeyVal::Lit(key_str_lit("args")), &Value::data("args"), strong, &nd, state);
                    }
                    self.cur_exc = Some(e);
                }
            }
        }
        let recorded = caught && self.record_raise_state(state);
        self.cur_exc_cls = None;
        if recorded {
            return;
        }
        if let Some(t) = self.raise_stack.last_mut() {
            *t = true;
        }
        let cls = self.cur_exc_cls.clone().unwrap_or_else(|| super::builtin_exc_names(name));
        let guarded = self.class_guarded(&cls);
        if let Some(Some(v)) = self.esc_cls.last_mut() {
            if !v.contains(&cls) {
                v.push(cls);
            }
        }
        // (the running try's handlers: only when one of them may catch it)
        if guarded {
            if let Some(m) = self.may_raise.last_mut() {
                *m = true;
            }
        }
    }

    /// The running try context may raise from here on (where exactly is not known): the
    /// requirements recorded so far are its handler's too.
    pub fn flag_may_raise(&mut self, state: &mut State) {
        let d = self.may_raise.len();
        if let Some(m) = self.may_raise.last_mut() {
            if !*m {
                self.flag_reqs.push((d, state.reqs.copy()));
            }
            *m = true;
        }
    }

    /// The call that just completed may raise: the enclosing call and try body learn it.
    pub fn note_raise(&mut self) {
        if let Some(e) = self.esc_cls.last_mut() {
            *e = None;
        }
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
            heap.push((oid, state.rows.get(&(oid, Ns::Item)), state.rows.get(&(oid, Ns::Attr)), state.slots.for_key(&oid), state.elems.get(&oid), self.obj(oid).single));
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
            self.set_llen(tid, state, if star_args.is_empty() { Some(rest_pos.len()) } else { None });
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
    /// The class a singledispatch implementation's first parameter is annotated with.
    pub fn sdispatch_annotation(&mut self, f: ObjId, method: bool, state: &mut State) -> Option<Value> {
        let fi = self.func_info(f);
        let super::FuncNode::Def(d) = &fi.node else { return None };
        let ps: Vec<&'static ast::ParameterWithDefault> = d.parameters.posonlyargs.iter().chain(d.parameters.args.iter()).collect();
        let p = ps.get(if method { 1 } else { 0 })?;
        let ann: &'static ast::Expr = p.parameter.annotation.as_deref()?;
        let mi = self.module_info(fi.module);
        self.frames.push(super::Frame::new(super::FrameKind::Module, mi.scopes.module(), fi.module, "sdispatch", true, "<ann>"));
        let v = self.eval(ann, state);
        self.frames.pop();
        Some(v)
    }

    /// The types a value may have, for dispatch: builtin type names / classes (None: unknown).
    fn dispatch_types(&self, v: &Value) -> Vec<(Option<Result<String, ObjId>>, Value)> {
        let mut out = Vec::new();
        for k in crate::domain::kv_literals(&v.kv).iter() {
            let t = if k == intern::key_none() {
                "NoneType".to_string()
            } else {
                match intern::key_get(k) {
                    intern::Key::Str(_) => "str".into(),
                    intern::Key::Int(_) => "int".into(),
                    intern::Key::Bool(_) => "bool".into(),
                    intern::Key::Float(_) => "float".into(),
                    intern::Key::Tuple(_) => "tuple".into(),
                    intern::Key::Bytes(_) => "bytes".into(),
                    _ => {
                        out.push((None, Value::lit(k)));
                        continue;
                    }
                }
            };
            out.push((Some(Ok(t)), Value::lit(k)));
        }
        if !v.kv.is_literal() && !v.kv.is_bot() {
            out.push((None, Value::kv_only(v.kv.clone())));
        }
        for o in v.pts.iter() {
            let t = match self.kind(o) {
                Kind::Dict if !self.counters.contains(&o) => Some(Ok("dict".to_string())),
                Kind::List if !self.deques.contains_key(&o) => Some(Ok("list".to_string())),
                Kind::Tuple => Some(Ok("tuple".to_string())),
                Kind::Set => Some(Ok("set".to_string())),
                Kind::Instance => self.instance_class(o).map(Err),
                _ => None,
            };
            out.push((t, Value::obj(o)));
        }
        out
    }

    /// Whether type `t` is (a subclass of) the registered type `reg`, and how far up (0: itself).
    fn dispatch_distance(&self, t: &Result<String, ObjId>, reg: &Value) -> Option<usize> {
        let mut best: Option<usize> = None;
        for r in reg.pts.iter() {
            let d = match (self.kind(r), t) {
                (Kind::External, Ok(name)) => {
                    let q = self.external_qual(r);
                    let rn = q.rsplit('.').next().unwrap_or("").to_string();
                    let chain: &[&str] = match name.as_str() {
                        "bool" => &["bool", "int", "object"],
                        "dict" => &["dict", "MutableMapping", "Mapping", "Collection", "Iterable", "object"],
                        "list" => &["list", "MutableSequence", "Sequence", "Collection", "Iterable", "object"],
                        "tuple" => &["tuple", "Sequence", "Collection", "Iterable", "object"],
                        "str" => &["str", "Sequence", "Collection", "Iterable", "object"],
                        "set" => &["set", "MutableSet", "Set", "AbstractSet", "Collection", "Iterable", "object"],
                        "int" => &["int", "Integral", "Real", "Number", "object"],
                        "float" => &["float", "Real", "Number", "object"],
                        _ => &[],
                    };
                    let pos = chain.iter().position(|c| *c == rn).or_else(|| if rn == name.as_str() { Some(0) } else { None });
                    pos
                }
                (Kind::External, Err(_)) => {
                    let q = self.external_qual(r);
                    if q.ends_with(".object") || q == "object" { Some(1000) } else { None }
                }
                (Kind::Class, Err(c)) => self.mro(*c).iter().position(|m| *m == r),
                _ => None,
            };
            if let Some(d) = d {
                best = Some(best.map(|b| b.min(d)).unwrap_or(d));
            }
        }
        best
    }

    #[allow(clippy::too_many_arguments)]
    fn sdispatch_call(&mut self, foid: ObjId, self_v: Option<Value>, args: Vec<Value>, kwargs: IndexMap<StrId, Value>, star_kwargs: Vec<Value>, state: &mut State, node: &Node, entry: bool) -> Value {
        let reg = self.sdispatch.get(&foid).cloned().unwrap_or_default();
        let mut groups: Vec<(ObjId, Value)> = Vec::new();
        let add = |f: ObjId, v: Value, groups: &mut Vec<(ObjId, Value)>| match groups.iter_mut().find(|(g, _)| *g == f) {
            Some((_, cur)) => *cur = cur.join(&v),
            None => groups.push((f, v)),
        };
        for (t, part) in self.dispatch_types(&args[0]) {
            match t {
                Some(t) => {
                    let mut best: Option<(usize, ObjId)> = None;
                    for (ty, f) in &reg {
                        if let Some(d) = self.dispatch_distance(&t, ty) {
                            if best.map(|(bd, _)| d < bd).unwrap_or(true) {
                                best = Some((d, *f));
                            }
                        }
                    }
                    add(best.map(|b| b.1).unwrap_or(foid), part, &mut groups);
                }
                // an unknown type: any implementation
                None => {
                    for (_, f) in &reg {
                        add(*f, part.clone(), &mut groups);
                    }
                    add(foid, part, &mut groups);
                }
            }
        }
        if groups.is_empty() {
            groups.push((foid, args[0].clone()));
        }
        let pre = state.copy_ref();
        let mut outs = Vec::new();
        let mut states: Vec<State> = Vec::new();
        let n = groups.len();
        for (f, part) in groups {
            let mut st = if n == 1 { std::mem::replace(state, State::bottom()) } else { pre.copy_ref() };
            let mut a = args.clone();
            a[0] = part;
            if f == foid {
                self.sd_bypass = Some(foid);
            }
            outs.push(self.call_user(f, self_v.clone(), a, kwargs.clone(), vec![], star_kwargs.clone(), &mut st, node, None, entry));
            self.sd_bypass = None;
            states.push(st);
        }
        let mut it = states.into_iter();
        let mut out = it.next().unwrap_or_else(State::bottom);
        for mut s2 in it {
            out = out.join(&mut s2);
        }
        state.replace_with(out);
        join_values(outs)
    }
}
