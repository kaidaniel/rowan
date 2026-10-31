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

/// A call of a `functools.lru_cache` / `cache` function (`recv`: a method's receiver): with
/// known arguments, the first call's result is the result of every later call with the same
/// arguments while the cache keeps it (an unbounded one always: `functools.cache`,
/// `lru_cache(maxsize=None)`; a bounded one may have dropped it: the call may run again). With
/// arguments not known (a receiver, `*args`, unknowns) the call may be a hit on any result
/// cached so far (then it does not run) or a miss.
pub(crate) fn memo_call(an: &mut Analyzer, oid: ObjId, recv: Option<Value>, a: Args, state: &mut State, node: &Node) -> Value {
    let lit = |v: &Value| v.kv.as_lit().filter(|_| v.pts.is_empty());
    let pos: Option<Vec<KeyId>> = a.pos.iter().map(lit).collect();
    let kws: Option<Vec<(StrId, KeyId)>> = a.kw.iter().map(|(k, v)| lit(v).map(|x| (*k, x))).collect();
    // the cache is a hidden dict in the (flow-sensitive) state: on paths where the
    // first call happened the result is reused, elsewhere the call runs
    let key = pos.zip(kws).filter(|_| recv.is_none() && a.star.is_empty() && a.star_kw.is_empty()).map(|(p, k)| {
        let mut parts = p;
        for (n, v) in k {
            parts.push(intern::key_tuple(vec![key_str_lit(&intern::str_get(n)), v]));
        }
        intern::key_tuple(parts)
    });
    let memo = an.heap.alloc_raw(Kind::Dict, &format!("memo:{}", oid), "", true, "lru_cache", Payload::None);
    let row = state.row(memo, Ns::Item);
    let bounded = state.slot_opt(memo, Ns::Attr, key_str_lit("\u{1}bounded")).is_some();
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
    // (what calls with arguments not known cached: under any key)
    let star = state.slot_opt(memo, Ns::Item, intern::STAR);
    let Some(mk) = key else {
        let cached = an.all_values(memo, Ns::Item, state);
        let may_hit = !row.present.is_empty() || star.is_some();
        let pre = may_hit.then(|| state.copy());
        bump(state, "hits", false);
        bump(state, "misses", false);
        // (in one context for every such call: what it makes is one site's objects, as the
        // results a cache keeps are some calls' — not one more object per call site)
        let r = an.call_user_in(oid, recv, a, state, node, &format!("memo:{}", oid));
        let out = if may_hit { r.join(&cached) } else { r.clone() };
        if let Some(mut pre) = pre {
            let j = state.join(&mut pre);
            state.replace_with(j);
        }
        let v = star.map_or(r.clone(), |s| s.join(&r));
        state.set_slot(memo, Ns::Item, intern::STAR, v, false);
        return out;
    };
    if row.definite.contains(mk) && !bounded {
        bump(state, "hits", true);
        return state.slot(memo, Ns::Item, mk);
    }
    // (a hit on some paths, a bounded cache's hit that may have dropped the result, or a
    // call with arguments not known that may have cached it: a hit or a miss, the call's
    // effects only on the miss)
    let hit = row.present.contains(mk).then(|| state.slot(memo, Ns::Item, mk));
    let pre = (hit.is_some() || star.is_some()).then(|| state.copy());
    if hit.is_some() || star.is_some() {
        bump(state, "hits", false);
        bump(state, "misses", false);
    } else {
        bump(state, "misses", true);
    }
    let r = an.call_user(oid, None, a, state, node, false);
    let out = [hit, star].into_iter().flatten().fold(r, |acc, v| acc.join(&v));
    if let Some(mut pre) = pre {
        let j = state.join(&mut pre);
        state.replace_with(j);
    }
    let row = state.row(memo, Ns::Item).store(mk, true);
    state.set_row(memo, Ns::Item, row);
    state.set_slot(memo, Ns::Item, mk, out.clone(), true);
    out
}

/// A functools decorator (`singledispatch` and `.register`, `lru_cache` / `cache`, `wraps`)
/// on the function `fobj` (decorated so far: `fv`): applied; false for any other decorator.
pub(crate) fn functools_decorator(an: &mut Analyzer, q: &str, d: &'static ast::Decorator, fobj: Option<ObjId>, fv: &mut Value, state: &mut State) -> bool {
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
        // (`lru_cache` keeps 128 results unless told `maxsize=None`: it may drop one)
        let unbounded = q == "functools.cache" || matches!(&d.expression, ast::Expr::Call(c) if c.arguments.keywords.iter().any(|k| k.arg.as_ref().is_some_and(|a| a.as_str() == "maxsize") && k.value.is_none_literal_expr()) || matches!(c.arguments.args.first(), Some(e) if e.is_none_literal_expr()));
        *fv = memo_wrapper(an, fv, unbounded, &crate::frontend::node(d), state);
        return true;
    }
    // `@functools.wraps(fn)`: the wrapper takes fn's name, qualname, doc, __wrapped__, and
    // the attributes fn was given (its `__dict__`)
    if q == "functools.wraps" {
        if let (ast::Expr::Call(cl), Some(f)) = (&d.expression, fobj) {
            if let Some(a0) = cl.arguments.args.first() {
                let wv = an.eval(a0, state);
                wrap_like(an, state, f, wv);
            }
        }
        return true;
    }
    false
}

/// `lru_cache(...)(fv)` / `cache(fv)`: a new function object (CPython's `_lru_cache_wrapper`)
/// that runs `fv`'s code through its cache (`memo_call`), with `fv`'s name, attributes and
/// `__wrapped__`; of what is not one function of the program, what a call that does not run gives.
fn memo_wrapper(an: &mut Analyzer, fv: &Value, unbounded: bool, node: &Node, state: &mut State) -> Value {
    let Some(f) = fv.single().filter(|f| fv.kv.is_bot() && an.heap.kind(*f) == Kind::Function) else {
        return an.unknown_call("functools.lru_cache", None, Args::new(std::slice::from_ref(fv), &IndexMap::new(), &[], &[]), state, node);
    };
    let fi = (*an.heap.func_info(f)).clone();
    fi.is_memo.set(true);
    let label = format!("lru_cache of {}", fi.qualname);
    let w = an.alloc_at(Kind::Function, node, state, &label, Payload::Func(std::rc::Rc::new(fi.clone())), Some(crate::domain::Row::new()), Ns::Attr, None, Some(&format!("memo{}", f)));
    an.heap.obj_mut(w).payload = Payload::Func(std::rc::Rc::new(fi));
    // the (empty) cache exists from the decoration on
    let memo = an.heap.alloc_raw(Kind::Dict, &format!("memo:{}", w), "", true, "lru_cache", Payload::None);
    state.set_row(memo, Ns::Item, crate::domain::Row::new());
    if !unbounded {
        state.set_slot(memo, Ns::Attr, key_str_lit("\u{1}bounded"), Value::lit(intern::key_none()), true);
    }
    wrap_like(an, state, w, fv.clone());
    Value::obj(w)
}

/// Wrapper `f` takes `wv`'s name, qualname, doc, `__wrapped__` and the attributes it was given
/// (its `__dict__`): `functools.wraps(wv)(f)`, `update_wrapper(f, wv)`.
fn wrap_like(an: &mut Analyzer, state: &mut State, f: ObjId, wv: Value) {
    let strong = wv.pts.len() == 1;
    for w in wv.pts.iter().filter(|w| an.heap.kind(*w) == Kind::Function) {
        // (its keys too: there where they are there on w)
        let wrow = state.row(w, Ns::Attr);
        let mut row = state.row(f, Ns::Attr);
        for (k, v) in state.obj_slots(w, Ns::Attr) {
            row = row.store(k, strong && wrow.definite.contains(k));
            state.set_slot(f, Ns::Attr, k, v, strong);
        }
        state.set_row(f, Ns::Attr, row);
    }
    // (a wrapper of what rowan does not know, a function a call that does not run gave back:
    // it has whatever attributes that has)
    if !wv.kv.is_bot() || wv.pts.iter().any(|w| an.heap.kind(w) != Kind::Function) {
        let r = state.row(f, Ns::Attr).mutate(intern::s("functools.wraps"));
        state.set_row(f, Ns::Attr, r);
    }
    an.m.wrapped.insert(f, wv);
}

/// `functools.update_wrapper(wrapper, wrapped)`: the wrapper, with what `wraps` gives it
/// (`wrap_like`). In a followed package's code the result is an unknown that changed nothing
/// (the wrapper still gets wrapped's attributes): CPython's changes only the wrapper, and
/// the decorators' wrappers given back, run where they are called, would cost the package's
/// allowance. A wrapper that is not one of the program's functions: an unknown call.
pub fn m_update_wrapper(an: &mut Analyzer, st: &mut State, c: &crate::interp::Call) -> Value {
    let in_library = an.module_of_idx(c.node.module).is_some_and(|m| an.heap.module_info(m).library);
    match (c.args.first(), c.args.get(1)) {
        (Some(w), Some(f)) if c.kwargs.is_empty() && c.star_args.is_empty() && !w.pts.is_empty() && w.kv.is_bot() && w.pts.iter().all(|o| an.heap.kind(o) == Kind::Function) => {
            for o in w.pts.iter() {
                wrap_like(an, st, o, f.clone());
            }
            match in_library {
                true => an.unknown_call(&c.qual, None, crate::interp::Args::pos(&[]), st, &c.node),
                false => w.clone(),
            }
        }
        _ => an.unknown_call(&c.qual, c.recv.as_ref(), c.arguments(), st, &c.node),
    }
}

/// `functools.cache(f)`, `lru_cache(f)`: the wrapper (`memo_wrapper`); `lru_cache(maxsize=…)`,
/// `lru_cache(None)`: the decorator that makes it (`functools.lru_cache.decorating`, its
/// receiver whether the cache is bounded).
pub fn m_lru_cache(an: &mut Analyzer, st: &mut State, c: &crate::interp::Call) -> Value {
    let lit = |v: &Value| v.pts.is_empty() && v.kv.is_literal();
    match (c.qual.as_str(), c.args.first()) {
        ("functools.cache", Some(f)) => memo_wrapper(an, f, true, &c.node, st),
        ("functools.lru_cache", Some(f)) if !lit(f) && c.kwargs.is_empty() => memo_wrapper(an, f, false, &c.node, st),
        ("functools.lru_cache", first) => {
            let size = first.or_else(|| c.kwargs.get(&intern::s("maxsize")));
            let unbounded = size.is_some_and(|v| v.pts.is_empty() && v.kv.as_lit() == Some(intern::key_none()));
            an.method_value("functools.lru_cache.decorating", &Value::lit(intern::key_bool(unbounded)))
        }
        ("functools.lru_cache.decorating", Some(f)) => {
            let unbounded = c.recv.as_ref().and_then(|r| r.kv.as_lit()) == Some(intern::key_bool(true));
            memo_wrapper(an, f, unbounded, &c.node, st)
        }
        _ => an.unknown_call(&c.qual, c.recv.as_ref(), c.arguments(), st, &c.node),
    }
}
