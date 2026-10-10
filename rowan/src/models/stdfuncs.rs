//! heapq (CPython's sift algorithm on comparable items), bisect, the `operator` functions as
//! callables (the binary operators, comparisons, item access) and `operator.methodcaller`.

use std::cmp::Ordering;

use crate::domain::{join_values, Ns, ObjId, Value};
use crate::intern::{self, KeyId};
use crate::interp::{Analyzer, Call};
use crate::state::{Kind, State};
use ruff_python_ast as ast;

/// Python's `<` order between two values when it is known (literals, tuples of them).
pub fn value_order(an: &Analyzer, st: &State, a: &Value, b: &Value) -> Option<Ordering> {
    let la = a.kv.as_lit().filter(|_| a.pts.is_empty());
    let lb = b.kv.as_lit().filter(|_| b.pts.is_empty());
    if let (Some(x), Some(y)) = (la, lb) {
        return key_order(x, y);
    }
    // tuples (objects or literals) compare item by item
    let items = |v: &Value| -> Option<Vec<Value>> {
        if let Some(k) = v.kv.as_lit().filter(|_| v.pts.is_empty()) {
            return match intern::key_get(k) {
                intern::Key::Tuple(ks) => Some(ks.into_iter().map(Value::lit).collect()),
                _ => None,
            };
        }
        let o = v.single().filter(|o| an.kind(*o) == Kind::Tuple && v.kv.is_bot())?;
        let n = an.llen(o, st)?;
        Some((0..n).map(|i| st.slot(o, Ns::Pos, intern::key_int(i as i64))).collect())
    };
    let (ia, ib) = (items(a)?, items(b)?);
    for (x, y) in ia.iter().zip(ib.iter()) {
        match value_order(an, st, x, y)? {
            Ordering::Equal => continue,
            o => return Some(o),
        }
    }
    Some(ia.len().cmp(&ib.len()))
}

fn key_order(x: KeyId, y: KeyId) -> Option<Ordering> {
    if let (intern::Key::Tuple(a), intern::Key::Tuple(b)) = (intern::key_get(x), intern::key_get(y)) {
        for (p, q) in a.iter().zip(b.iter()) {
            match key_order(*p, *q)? {
                Ordering::Equal => continue,
                o => return Some(o),
            }
        }
        return Some(a.len().cmp(&b.len()));
    }
    crate::interp::exprs::literal_order(x, y)
}

/// The items of a list with known positions.
fn list_items(an: &Analyzer, st: &State, o: ObjId) -> Option<Vec<Value>> {
    let n = an.llen(o, st)?;
    Some((0..n).map(|i| st.slot(o, Ns::Pos, intern::key_int(i as i64))).collect())
}

/// The list's items set to `vals` (positions known).
fn set_items(an: &mut Analyzer, st: &mut State, o: ObjId, vals: Vec<Value>) {
    st.drop_obj_ns(o, Ns::Pos);
    for (i, v) in vals.iter().enumerate() {
        st.set_slot(o, Ns::Pos, intern::key_int(i as i64), v.clone(), true);
    }
    let acc = join_values(vals.clone());
    st.set_slot(o, Ns::Elem, intern::STAR, Value::with_kv(acc.pts.clone(), crate::models::torch::elem_kv(&acc.kv)), false);
    let cur = st.elems.get(&o).unwrap_or_else(|| acc.kv.clone());
    st.elems.insert(o, crate::domain::kv_join(&cur, &acc.kv));
    an.set_llen(o, st, Some(vals.len()));
}

/// The list's positions are no longer known; `x` may be among its items.
fn blur(an: &mut Analyzer, st: &mut State, o: ObjId, x: Option<&Value>) {
    if let Some(x) = x {
        st.set_slot(o, Ns::Elem, intern::STAR, Value::with_kv(x.pts.clone(), crate::models::torch::elem_kv(&x.kv)), false);
        let cur = st.elems.get(&o).unwrap_or_else(|| x.kv.clone());
        st.elems.insert(o, crate::domain::kv_join(&cur, &x.kv));
    }
    an.set_llen(o, st, None);
    st.drop_obj_ns(o, Ns::Pos);
}

// CPython's heapq._siftdown / _siftup, with a known order (None: an item pair not comparable)
fn siftdown(an: &Analyzer, st: &State, h: &mut [Value], startpos: usize, mut pos: usize) -> Option<()> {
    let newitem = h[pos].clone();
    while pos > startpos {
        let parentpos = (pos - 1) >> 1;
        let parent = h[parentpos].clone();
        if value_order(an, st, &newitem, &parent)? == Ordering::Less {
            h[pos] = parent;
            pos = parentpos;
            continue;
        }
        break;
    }
    h[pos] = newitem;
    Some(())
}

fn siftup(an: &Analyzer, st: &State, h: &mut [Value], mut pos: usize) -> Option<()> {
    let endpos = h.len();
    let startpos = pos;
    let newitem = h[pos].clone();
    let mut childpos = 2 * pos + 1;
    while childpos < endpos {
        let rightpos = childpos + 1;
        if rightpos < endpos && value_order(an, st, &h[childpos], &h[rightpos])? != Ordering::Less {
            childpos = rightpos;
        }
        h[pos] = h[childpos].clone();
        pos = childpos;
        childpos = 2 * pos + 1;
    }
    h[pos] = newitem;
    siftdown(an, st, h, startpos, pos)
}

fn lit_int(v: &Value) -> Option<i64> {
    v.kv.as_lit().filter(|_| v.pts.is_empty()).and_then(intern::key_as_int)
}

pub fn m_heapq(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = c.qual.rsplit('.').next().unwrap_or("").to_string();
    let heap = c.args.first().cloned().unwrap_or_default();
    let h = heap.single().filter(|o| an.kind(*o) == Kind::List && an.obj(*o).single);
    let elem_of = |an: &mut Analyzer, st: &mut State, v: &Value| an.iterate(v, &c.node, st).elem.one_or_empty();
    match name.as_str() {
        "heappush" => {
            let x = c.args.get(1).cloned().unwrap_or_default();
            match h.and_then(|o| list_items(an, st, o).map(|its| (o, its))) {
                Some((o, mut its)) => {
                    its.push(x.clone());
                    let n = its.len();
                    match siftdown(an, st, &mut its, 0, n - 1) {
                        Some(()) => set_items(an, st, o, its),
                        None => blur(an, st, o, Some(&x)),
                    }
                }
                None => {
                    for o in heap.pts.iter() {
                        blur(an, st, o, Some(&x));
                    }
                }
            }
            an.none_v.clone()
        }
        "heappop" | "heapreplace" | "heappushpop" => {
            let x = c.args.get(1).cloned();
            if let Some((o, mut its)) = h.and_then(|o| list_items(an, st, o).map(|its| (o, its))) {
                let res: Option<Value> = match name.as_str() {
                    "heappop" => {
                        if its.is_empty() {
                            an.raise_here("IndexError", st);
                            return Value::new();
                        }
                        let last = its.pop().unwrap();
                        if its.is_empty() {
                            Some(last)
                        } else {
                            let top = std::mem::replace(&mut its[0], last);
                            siftup(an, st, &mut its, 0).map(|_| top)
                        }
                    }
                    "heapreplace" => {
                        if its.is_empty() {
                            an.raise_here("IndexError", st);
                            return Value::new();
                        }
                        let top = std::mem::replace(&mut its[0], x.clone().unwrap_or_default());
                        siftup(an, st, &mut its, 0).map(|_| top)
                    }
                    _ => {
                        let item = x.clone().unwrap_or_default();
                        if !its.is_empty() {
                            match value_order(an, st, &its[0], &item) {
                                Some(Ordering::Less) => {
                                    let top = std::mem::replace(&mut its[0], item);
                                    siftup(an, st, &mut its, 0).map(|_| top)
                                }
                                Some(_) => Some(item),
                                None => None,
                            }
                        } else {
                            Some(item)
                        }
                    }
                };
                if let Some(r) = res {
                    set_items(an, st, o, its);
                    return r;
                }
            }
            // not known: one of the items (or the pushed one)
            let mut r = elem_of(an, st, &heap);
            if let Some(x) = &x {
                r = r.join(x);
            }
            for o in heap.pts.iter() {
                blur(an, st, o, x.as_ref());
            }
            an.may_raise_here("IndexError", st);
            r
        }
        "heapify" => {
            if let Some((o, mut its)) = h.and_then(|o| list_items(an, st, o).map(|its| (o, its))) {
                let n = its.len();
                let mut ok = true;
                for i in (0..n / 2).rev() {
                    if siftup(an, st, &mut its, i).is_none() {
                        ok = false;
                        break;
                    }
                }
                if ok {
                    set_items(an, st, o, its);
                    return an.none_v.clone();
                }
            }
            for o in heap.pts.iter() {
                blur(an, st, o, None);
            }
            an.none_v.clone()
        }
        "nlargest" | "nsmallest" => {
            // `sorted(iterable, key=key, reverse=nlargest)[:n]`
            let n = c.args.first().and_then(lit_int);
            let src = c.args.get(1).cloned().unwrap_or_default();
            let key = c.kw("key");
            let it = an.iterate(&src, &c.node, st);
            if let (Some(n), Some(items)) = (n, it.items.clone()) {
                let mut keyed: Vec<(Value, Value)> = Vec::new();
                for (k, x) in items.iter().enumerate() {
                    let mut nd = c.node;
                    nd.col = c.node.col.wrapping_add(1_500_000 + k as u32);
                    let kv = match &key {
                        Some(f) => an.call_value(f, &nd, None, st, Some(vec![x.clone()])),
                        None => x.clone(),
                    };
                    keyed.push((kv, x.clone()));
                }
                let mut ok = true;
                let mut idx: Vec<usize> = (0..keyed.len()).collect();
                // (a stable sort on the keys; every pair must compare)
                for i in 0..keyed.len() {
                    for j in i + 1..keyed.len() {
                        if value_order(an, st, &keyed[i].0, &keyed[j].0).is_none() {
                            ok = false;
                        }
                    }
                }
                if ok {
                    let rev = name == "nlargest";
                    idx.sort_by(|&a, &b| {
                        let o = value_order(an, st, &keyed[a].0, &keyed[b].0).unwrap_or(Ordering::Equal);
                        if rev { o.reverse() } else { o }
                    });
                    let vals: Vec<Value> = idx.into_iter().take(n.max(0) as usize).map(|i| keyed[i].1.clone()).collect();
                    let l = an.it_list(Some(vals), Value::new(), &c.node, st, "heapn");
                    return plain_list(an, l);
                }
            }
            if let Some(f) = &key {
                let e = it.elem.one_or_empty();
                an.call_value(f, &c.node, None, st, Some(vec![e]));
            }
            let e = it.elem.one_or_empty();
            let l = an.it_list(None, e, &c.node, st, "heapn");
            plain_list(an, l)
        }
        "merge" => {
            let mut all: Vec<Value> = Vec::new();
            let mut elem = Value::new();
            for s in &c.args {
                let it = an.iterate(s, &c.node, st);
                elem = elem.join(&it.elem.one_or_empty());
                if let Some(its) = it.items {
                    all.extend(its);
                }
            }
            an.it_list(None, elem, &c.node, st, "heapmerge")
        }
        _ => Value::data(&format!("heapq.{}", name)),
    }
}

/// (nlargest / nsmallest return lists: not one-shot iterators)
fn plain_list(an: &mut Analyzer, v: Value) -> Value {
    if let Some(o) = v.single() {
        an.obj_mut(o).iterator = false;
    }
    v
}

pub fn m_bisect(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = c.qual.rsplit('.').next().unwrap_or("").to_string();
    let a = c.args.first().cloned().unwrap_or_default();
    let x = c.args.get(1).cloned().unwrap_or_default();
    let key = c.kw("key");
    let right = !name.ends_with("_left");
    let insort = name.starts_with("insort");
    // a parameter's value: decided at the call site
    for v in [&x, &a] {
        if let Some(p) = v.kv.as_param() {
            an.record_event("concrete", p);
        }
    }
    let o = a.single().filter(|o| matches!(an.kind(*o), Kind::List | Kind::Tuple));
    let items = o.and_then(|o| list_items(an, st, o));
    if let Some(its) = items {
        let lo = c.arg(2, Some("lo")).and_then(|v| lit_int(&v)).unwrap_or(0).max(0) as usize;
        let hi = c.arg(3, Some("hi")).and_then(|v| lit_int(&v)).map(|h| h.max(0) as usize).unwrap_or(its.len()).min(its.len());
        // key= applies to the items (not to x, except for insort, which applies it to x too)
        let xk = match (&key, insort) {
            (Some(f), true) => an.call_value(f, &c.node, None, st, Some(vec![x.clone()])),
            _ => x.clone(),
        };
        let mut keys: Vec<Value> = Vec::new();
        for (k, it) in its.iter().enumerate() {
            let mut nd = c.node;
            nd.col = c.node.col.wrapping_add(1_600_000 + k as u32);
            keys.push(match &key {
                Some(f) => an.call_value(f, &nd, None, st, Some(vec![it.clone()])),
                None => it.clone(),
            });
        }
        let (mut l, mut h) = (lo, hi);
        let mut ok = true;
        while l < h {
            let mid = (l + h) / 2;
            match value_order(an, st, &xk, &keys[mid]) {
                Some(o) => {
                    let go_left = if right { o == Ordering::Less } else { o != Ordering::Greater };
                    if go_left {
                        h = mid;
                    } else {
                        l = mid + 1;
                    }
                }
                None => {
                    ok = false;
                    break;
                }
            }
        }
        if ok {
            if insort {
                let mut its = its;
                its.insert(l, x.clone());
                if let Some(o) = o {
                    set_items(an, st, o, its);
                }
                return an.none_v.clone();
            }
            return Value::lit(intern::key_int(l as i64));
        }
    }
    if insort {
        for o in a.pts.iter() {
            blur(an, st, o, Some(&x));
        }
        return an.none_v.clone();
    }
    Value::data("bisect index")
}

/// `operator.add(a, b)` etc. called as functions (by `reduce`, `map`, …): the operators.
pub fn m_operator_fn(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = c.qual.rsplit('.').next().unwrap_or("").to_string();
    if let Some(v) = crate::models::pure::try_pure(an, st, c) {
        return v;
    }
    let a = c.args.first().cloned().unwrap_or_default();
    let b = c.args.get(1).cloned().unwrap_or_default();
    let op = match name.as_str() {
        "add" | "concat" | "iadd" => Some(ast::Operator::Add),
        "sub" | "isub" => Some(ast::Operator::Sub),
        "mul" | "imul" => Some(ast::Operator::Mult),
        "truediv" => Some(ast::Operator::Div),
        "floordiv" => Some(ast::Operator::FloorDiv),
        "mod" => Some(ast::Operator::Mod),
        "pow" => Some(ast::Operator::Pow),
        "or_" | "ior" => Some(ast::Operator::BitOr),
        "and_" | "iand" => Some(ast::Operator::BitAnd),
        "xor" => Some(ast::Operator::BitXor),
        "lshift" => Some(ast::Operator::LShift),
        "rshift" => Some(ast::Operator::RShift),
        _ => None,
    };
    if let Some(op) = op {
        if c.args.len() == 2 {
            return an.binop(op, &a, &b, &c.node, st);
        }
    }
    match name.as_str() {
        "getitem" if c.args.len() == 2 => an.load_item(&a, Ns::Item, &b, &c.node, st, true),
        "contains" if c.args.len() == 2 => Value::data("bool"),
        _ => crate::models::noeffect(an, st, c),
    }
}

/// `operator.methodcaller(name, *args, **kwargs)`: `lambda obj: obj.name(*args, **kwargs)`, as a
/// small generated module whose globals hold the arguments.
pub fn m_methodcaller(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(mname) = c.args.first().and_then(|a| a.kv.as_lit().filter(|_| a.pts.is_empty())).and_then(intern::key_as_str) else {
        return an.unknown_call(&c.qual, None, &c.args, &c.kwargs, &c.star_args, &c.star_kwargs, st, &c.node);
    };
    if !mname.chars().all(|ch| ch.is_alphanumeric() || ch == '_') || !c.star_args.is_empty() || !c.star_kwargs.is_empty() {
        return an.unknown_call(&c.qual, None, &c.args, &c.kwargs, &c.star_args, &c.star_kwargs, st, &c.node);
    }
    let pos: Vec<String> = (1..c.args.len()).map(|i| format!("_mc_a{}", i)).collect();
    let kws: Vec<(String, String)> = c.kwargs.keys().enumerate().map(|(i, k)| (intern::str_get(*k).to_string(), format!("_mc_k{}", i))).collect();
    let mut call_args: Vec<String> = pos.clone();
    call_args.extend(kws.iter().map(|(k, g)| format!("{}={}", k, g)));
    let modname = format!("<methodcaller {} {}>", mname, c.site);
    let src = format!("def caller(obj):\n    return obj.{}({})\n", mname, call_args.join(", "));
    let moid = match an.modules.get(&modname) {
        Some(&m) => m,
        None => {
            let Ok((idx, tree)) = crate::frontend::parse(&modname, None, src) else { return Value::data("methodcaller") };
            an.make_module_pub(&modname, None, idx, tree)
        }
    };
    // the arguments of this call, then the function
    for (i, v) in c.args.iter().enumerate().skip(1) {
        st.set_slot(moid, Ns::Attr, intern::key_str_lit(&format!("_mc_a{}", i)), v.clone(), true);
    }
    for ((_, g), v) in kws.iter().zip(c.kwargs.values()) {
        st.set_slot(moid, Ns::Attr, intern::key_str_lit(g), v.clone(), true);
    }
    an.run_module(moid, st);
    st.slot(moid, Ns::Attr, intern::key_str_lit("caller"))
}

/// Two values equal as Python compares them, when known.
fn value_eq(an: &Analyzer, st: &State, a: &Value, b: &Value) -> Option<bool> {
    if a.pts.is_empty() && b.pts.is_empty() {
        let (x, y) = (a.kv.as_lit()?, b.kv.as_lit()?);
        if x == y {
            return Some(true);
        }
        return match key_order(x, y) {
            Some(o) => Some(o == Ordering::Equal),
            // (different kinds of literal: str vs int, None vs …)
            None => Some(false),
        };
    }
    if a.pts.len() == 1 && a.pts == b.pts && a.kv.is_bot() && b.kv.is_bot() && an.obj(a.pts.first().unwrap()).single {
        return Some(true);
    }
    let _ = st;
    None
}

/// `xs.index(x)` / `xs.count(x)` on a list / tuple with known items.
pub fn m_seq_index(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let count = c.qual.ends_with("count");
    let recv = c.recv.clone().unwrap_or_default();
    let x = c.args.first().cloned().unwrap_or_default();
    if let Some(p) = x.kv.as_param() {
        an.record_event("concrete", p);
    }
    if let Some(o) = recv.single().filter(|o| matches!(an.kind(*o), Kind::List | Kind::Tuple)) {
        if let Some(its) = list_items(an, st, o) {
            let eqs: Vec<Option<bool>> = its.iter().map(|it| value_eq(an, st, it, &x)).collect();
            if count {
                if eqs.iter().all(|e| e.is_some()) {
                    return Value::lit(intern::key_int(eqs.iter().filter(|e| **e == Some(true)).count() as i64));
                }
            } else {
                for (i, e) in eqs.iter().enumerate() {
                    match e {
                        Some(true) => return Value::lit(intern::key_int(i as i64)),
                        Some(false) => continue,
                        None => break,
                    }
                }
                if eqs.iter().all(|e| *e == Some(false)) {
                    an.raise_here("ValueError", st);
                    return Value::new();
                }
            }
        }
    }
    if !count {
        an.may_raise_here("ValueError", st);
    }
    Value::data(if count { "count" } else { "index" })
}

pub fn register(r: &mut dyn FnMut(&[&'static str], crate::models::Model)) {
    r(&["list.index", "list.count", "tuple.index", "tuple.count"], m_seq_index);
    r(&["heapq.heappush", "heapq.heappop", "heapq.heapify", "heapq.heapreplace", "heapq.heappushpop", "heapq.nlargest", "heapq.nsmallest", "heapq.merge"], m_heapq);
    r(&["bisect.bisect", "bisect.bisect_left", "bisect.bisect_right", "bisect.insort", "bisect.insort_left", "bisect.insort_right"], m_bisect);
    r(
        &[
            "operator.add", "operator.sub", "operator.mul", "operator.truediv", "operator.floordiv", "operator.mod", "operator.pow", "operator.or_",
            "operator.and_", "operator.xor", "operator.lshift", "operator.rshift", "operator.concat", "operator.getitem", "operator.contains",
            "operator.iadd", "operator.isub", "operator.imul", "operator.ior", "operator.iand",
        ],
        m_operator_fn,
    );
    r(&["operator.methodcaller"], m_methodcaller);
}
