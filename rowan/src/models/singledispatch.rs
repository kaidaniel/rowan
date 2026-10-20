//! functools: `singledispatch` (a call dispatches on its first argument's type to the
//! implementation registered for the nearest class) and `lru_cache` / `cache` (`memo_call`).

use indexmap::IndexMap;
use ruff_python_ast as ast;

use crate::domain::{join_values, Ns, ObjId, Value};
use crate::frontend::Node;
use crate::intern::{self, key_str_lit, KeyId, StrId};
use crate::interp::{Analyzer, Payload};
use crate::state::{Kind, State};
use crate::interp::Args;

/// The class a singledispatch implementation's first parameter is annotated with.
pub fn sdispatch_annotation(an: &mut Analyzer, f: ObjId, method: bool, state: &mut State) -> Option<Value> {
    let fi = an.heap.func_info(f);
    let crate::interp::FuncNode::Def(d) = &fi.node else { return None };
    let ps: Vec<&'static ast::ParameterWithDefault> = d.parameters.posonlyargs.iter().chain(d.parameters.args.iter()).collect();
    let p = ps.get(if method { 1 } else { 0 })?;
    let ann: &'static ast::Expr = p.parameter.annotation.as_deref()?;
    let v = an.in_module(fi.module, "sdispatch", "<ann>", |an| an.eval(ann, state));
    Some(v)
}

/// The types a value may have, for dispatch: builtin type names / classes (None: unknown).
fn dispatch_types(an: &Analyzer, v: &Value) -> Vec<(Option<Result<String, ObjId>>, Value)> {
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
        let t = match an.heap.kind(o) {
            Kind::Dict if !an.m.counters.contains(&o) => Some(Ok("dict".to_string())),
            Kind::List if !an.m.deques.contains_key(&o) => Some(Ok("list".to_string())),
            Kind::Tuple => Some(Ok("tuple".to_string())),
            Kind::Set => Some(Ok("set".to_string())),
            Kind::Instance => an.heap.instance_class(o).map(Err),
            _ => None,
        };
        out.push((t, Value::obj(o)));
    }
    out
}

/// Whether type `t` is (a subclass of) the registered type `reg`, and how far up (0: itself).
fn dispatch_distance(an: &Analyzer, t: &Result<String, ObjId>, reg: &Value) -> Option<usize> {
    let mut best: Option<usize> = None;
    for r in reg.pts.iter() {
        let d = match (an.heap.kind(r), t) {
            (Kind::External, Ok(name)) => {
                let q = an.heap.external_qual(r);
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
                let q = an.heap.external_qual(r);
                if q.ends_with(".object") || q == "object" { Some(1000) } else { None }
            }
            (Kind::Class, Err(c)) => an.heap.mro(*c).iter().position(|m| *m == r),
            _ => None,
        };
        if let Some(d) = d {
            best = Some(best.map(|b| b.min(d)).unwrap_or(d));
        }
    }
    best
}

pub(crate) fn sdispatch_call(an: &mut Analyzer, foid: ObjId, self_v: Option<Value>, args: Vec<Value>, kwargs: IndexMap<StrId, Value>, star_kwargs: Vec<Value>, state: &mut State, node: &Node, entry: bool) -> Value {
    let reg = an.m.sdispatch.get(&foid).cloned().unwrap_or_default();
    let mut groups: Vec<(ObjId, Value)> = Vec::new();
    let add = |f: ObjId, v: Value, groups: &mut Vec<(ObjId, Value)>| match groups.iter_mut().find(|(g, _)| *g == f) {
        Some((_, cur)) => *cur = cur.join(&v),
        None => groups.push((f, v)),
    };
    for (t, part) in dispatch_types(an, &args[0]) {
        match t {
            Some(t) => {
                let mut best: Option<(usize, ObjId)> = None;
                for (ty, f) in &reg {
                    if let Some(d) = dispatch_distance(an, &t, ty) {
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
    let pre = state.copy();
    let mut outs = Vec::new();
    let mut states: Vec<State> = Vec::new();
    let n = groups.len();
    for (f, part) in groups {
        let mut st = if n == 1 { std::mem::replace(state, State::bottom()) } else { pre.copy() };
        let mut a = args.clone();
        a[0] = part;
        if f == foid {
            an.m.sd_bypass = Some(foid);
        }
        outs.push(an.call_user(f, self_v.clone(), Args::new(&a, &kwargs, &[], &star_kwargs), &mut st, node, entry));
        an.m.sd_bypass = None;
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

/// A call of a `functools.lru_cache` / `cache` function: with known arguments, the first
/// call's result is the result of every later call with the same arguments.
pub(crate) fn memo_call(an: &mut Analyzer, oid: ObjId, args: &[Value], kwargs: &IndexMap<StrId, Value>, state: &mut State, node: &Node) -> Value {
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
        return an.call_user(oid, None, Args::new(&args, &kwargs, &[], &[]), state, node, false);
    };
    let memo = an.heap.alloc_raw(Kind::Dict, &format!("memo:{}", oid), "", true, "lru_cache", Payload::None);
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
    let r = an.call_user(oid, None, Args::new(&args, &kwargs, &[], &[]), state, node, false);
    let out = if row.present.contains(mk) { r.join(&state.slot(memo, Ns::Item, mk)) } else { r };
    let row = state.row(memo, Ns::Item).store(mk, true);
    state.set_row(memo, Ns::Item, row);
    state.set_slot(memo, Ns::Item, mk, out.clone(), true);
    out
}

/// A functools decorator (`singledispatch` and `.register`, `lru_cache` / `cache`, `wraps`)
/// on the function `fobj` (its `fi`): applied; false for any other decorator.
pub(crate) fn functools_decorator(an: &mut Analyzer, q: &str, d: &'static ast::Decorator, fobj: Option<ObjId>, fi: Option<&crate::interp::FuncInfo>, state: &mut State) -> bool {
    // `@singledispatch`: implementations registered on it are chosen by the type of the
    // first argument
    if matches!(q, "functools.singledispatch" | "functools.singledispatchmethod") {
        if let Some(f) = fobj.filter(|o| an.heap.kind(*o) == Kind::Function) {
            an.m.sdispatch.insert(f, Vec::new());
            if q.ends_with("method") {
                an.m.sdispatch_method.insert(f);
            }
        }
        return true;
    }
    if q == "sdispatch.register" {
        let dv = match &d.expression {
            ast::Expr::Call(c) => an.eval(&c.func, state),
            other => an.eval(other, state),
        };
        let base = dv.pts.iter().find_map(|m| match &an.heap.obj(m).payload {
            Payload::Method(_, b) => b.single(),
            _ => None,
        });
        let impl_f = fobj.filter(|o| an.heap.kind(*o) == Kind::Function);
        if let (Some(base), Some(impl_f)) = (base, impl_f) {
            // the type: `@f.register(T)`, else the first parameter's annotation
            let ty = match &d.expression {
                ast::Expr::Call(c) if !c.arguments.args.is_empty() => Some(an.eval(&c.arguments.args[0], state)),
                _ => sdispatch_annotation(an, impl_f, an.m.sdispatch_method.contains(&base), state),
            };
            if let (Some(ty), Some(reg)) = (ty, an.m.sdispatch.get_mut(&base)) {
                reg.push((ty, impl_f));
            }
        }
        return true;
    }
    if matches!(q, "functools.lru_cache" | "functools.cache") {
        if let (Some(fi), Some(f)) = (&fi, fobj) {
            fi.is_memo.set(true);
            // the (empty) cache exists from the definition on, on every path
            let memo = an.heap.alloc_raw(Kind::Dict, &format!("memo:{}", f), "", true, "lru_cache", Payload::None);
            state.set_row(memo, Ns::Item, crate::domain::Row::new());
        }
        return true;
    }
    // `@functools.wraps(fn)`: the wrapper takes fn's name, qualname, doc, __wrapped__, and
    // the attributes fn was given (its `__dict__`)
    if q == "functools.wraps" {
        if let (ast::Expr::Call(cl), Some(f)) = (&d.expression, fobj) {
            if let Some(a0) = cl.arguments.args.first() {
                let wv = an.eval(a0, state);
                let strong = wv.pts.len() == 1;
                for w in wv.pts.iter().filter(|w| an.heap.kind(*w) == Kind::Function) {
                    for (k, v) in state.obj_slots(w, Ns::Attr) {
                        state.set_slot(f, Ns::Attr, k, v, strong);
                    }
                }
                an.m.wrapped.insert(f, wv);
            }
        }
        return true;
    }
    false
}
