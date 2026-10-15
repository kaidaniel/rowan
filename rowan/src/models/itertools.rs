//! itertools (and `operator` callbacks in it): on known items each function gives its exact
//! result — an iterator (a one-shot list) with positions, its callbacks (`key=`, predicates,
//! `func`) called per item with their own contexts — else an iterator of unknown length with
//! the right elements. `count`, `cycle` and `repeat(x)` are infinite iterator objects
//! (`stdobj.infinite`) that `zip`, `islice` and `zip_longest` expand to the length needed.

use crate::domain::{join_values, Ns, ObjId, Row, Value};
use crate::frontend::Node;
use crate::intern;
use crate::interp::{Analyzer, Call, Payload};
use crate::state::{Kind, State};

/// Results beyond this many positions: unknown positions.
const MAX_ITEMS: usize = 256;

#[derive(Clone, Debug)]
pub enum Infinite {
    Count(Value, Value),
    Cycle(Vec<Value>),
    Repeat(Value),
}

fn sub_node(node: &Node, i: usize, salt: u32) -> Node {
    let mut n = *node;
    n.col = node.col.wrapping_add(salt + i as u32);
    n
}

/// A one-shot iterator over `vals` (positions known) or with element `elem` (unknown count).
pub fn it_list(an: &mut Analyzer, vals: Option<Vec<Value>>, elem: Value, node: &Node, st: &mut State, ctx: &str) -> Value {
    let lid = an.alloc_at(Kind::List, node, st, &format!("iter@L{}", node.line), Payload::None, None, Ns::Item, None, Some(ctx));
    let single = an.heap.obj(lid).single;
    let acc = match &vals {
        Some(vs) => join_values(vs.clone()),
        None => elem,
    };
    st.set_slot(lid, Ns::Elem, intern::STAR, Value::with_kv(acc.pts.clone(), crate::models::torch::elem_kv(&acc.kv)), single);
    st.elems.insert(lid, acc.kv.clone());
    match vals {
        Some(vs) if single && vs.len() <= MAX_ITEMS => {
            for (i, v) in vs.iter().enumerate() {
                st.set_slot(lid, Ns::Pos, intern::key_int(i as i64), v.clone(), true);
            }
            an.heap.set_llen(lid, st, Some(vs.len()));
        }
        _ => an.heap.set_llen(lid, st, None),
    }
    an.heap.set_iterator(lid);
    Value::obj(lid)
}

/// A tuple with these items.
pub fn it_tuple(an: &mut Analyzer, vals: Vec<Value>, node: &Node, st: &mut State, ctx: &str) -> Value {
    let tid = an.alloc_at(Kind::Tuple, node, st, &format!("tuple@L{}", node.line), Payload::None, None, Ns::Item, None, Some(ctx));
    let acc = join_values(vals.clone());
    st.set_slot(tid, Ns::Elem, intern::STAR, Value::with_kv(acc.pts.clone(), crate::models::torch::elem_kv(&acc.kv)), true);
    st.elems.insert(tid, acc.kv.clone());
    for (i, v) in vals.iter().enumerate() {
        st.set_slot(tid, Ns::Pos, intern::key_int(i as i64), v.clone(), true);
    }
    an.heap.set_llen(tid, st, Some(vals.len()));
    Value::obj(tid)
}

/// The infinite iterator a value is (one of them), if it is one.
pub fn infinite_of(an: &Analyzer, v: &Value) -> Option<Infinite> {
    let o = v.single()?;
    if an.heap.kind(o) != Kind::Namespace || !matches!(&an.heap.obj(o).payload, Payload::Method(q, _) if q == "stdobj.infinite") {
        return None;
    }
    an.m.infinites.get(&o).cloned()
}

/// The next `n` items of an iterator whose items are known that far: an infinite one, or
/// a one-shot iterator (a generator) from its cursor within its known positions (fewer at
/// its end).
pub fn take_known(an: &mut Analyzer, v: &Value, n: usize, st: &State) -> Option<Vec<Value>> {
    if let Some(inf) = infinite_of(an, v) {
        return infinite_take(an, &inf, n);
    }
    let o = v.single().filter(|o| v.kv.is_bot() && an.heap.obj(*o).iterator && an.heap.obj(*o).single && an.heap.kind(*o) == Kind::List)?;
    let c = an.cursor_of(o, st)?;
    let end = match an.heap.llen(o, st) {
        Some(len) => (c + n).min(len),
        None if c + n <= an.heap.prefix(o, st).unwrap_or(0) => c + n,
        None => return None,
    };
    Some((c..end).map(|i| st.slot(o, Ns::Pos, intern::key_int(i as i64))).collect())
}

/// The first `n` items of an infinite iterator.
pub fn infinite_take(_an: &mut Analyzer, inf: &Infinite, n: usize) -> Option<Vec<Value>> {
    if n > MAX_ITEMS {
        return None;
    }
    match inf {
        Infinite::Count(start, step) => {
            let s = start.kv.as_lit().filter(|_| start.pts.is_empty()).and_then(intern::key_as_int);
            let d = step.kv.as_lit().filter(|_| step.pts.is_empty()).and_then(intern::key_as_int);
            match (s, d) {
                (Some(s), Some(d)) => Some((0..n as i64).map(|i| Value::lit(intern::key_int(s + i * d))).collect()),
                _ => Some(vec![Value::data("count"); n]),
            }
        }
        Infinite::Cycle(items) if !items.is_empty() => Some((0..n).map(|i| items[i % items.len()].clone()).collect()),
        Infinite::Cycle(_) => Some(vec![]),
        Infinite::Repeat(v) => Some(vec![v.clone(); n]),
    }
}

/// The element an infinite iterator yields (for a loop over it).
pub fn infinite_elem(_an: &Analyzer, inf: &Infinite) -> Value {
    match inf {
        Infinite::Count(..) => Value::data("count"),
        Infinite::Cycle(items) => join_values(items.clone()),
        Infinite::Repeat(v) => v.clone(),
    }
}

fn new_infinite(an: &mut Analyzer, inf: Infinite, node: &Node, st: &mut State) -> Value {
    let oid = an.alloc_at(Kind::Namespace, node, st, "itertools iterator", Payload::Method("stdobj.infinite".into(), Value::new()), Some(Row::new()), Ns::Attr, None, None);
    an.heap.set_payload(oid, Payload::Method("stdobj.infinite".into(), Value::new()));
    an.m.infinites.insert(oid, inf);
    Value::obj(oid)
}

/// `f(*args)` for a callback (a function, `operator.add`, a builtin…).
fn it_call(an: &mut Analyzer, f: &Value, args: Vec<Value>, node: &Node, st: &mut State) -> Value {
    // `operator.add` / `mul` / … on two values: the binary operator
    if args.len() == 2 && f.pts.len() == 1 {
        let o = f.pts.first().unwrap();
        if an.heap.kind(o) == Kind::External {
            let q = an.heap.external_qual(o);
            let op = match q.as_str() {
                "operator.add" | "operator.concat" => Some(ruff_python_ast::Operator::Add),
                "operator.sub" => Some(ruff_python_ast::Operator::Sub),
                "operator.mul" => Some(ruff_python_ast::Operator::Mult),
                "operator.floordiv" => Some(ruff_python_ast::Operator::FloorDiv),
                "operator.truediv" => Some(ruff_python_ast::Operator::Div),
                "operator.mod" => Some(ruff_python_ast::Operator::Mod),
                "operator.pow" => Some(ruff_python_ast::Operator::Pow),
                "operator.or_" => Some(ruff_python_ast::Operator::BitOr),
                "operator.and_" => Some(ruff_python_ast::Operator::BitAnd),
                _ => None,
            };
            if let Some(op) = op {
                return an.binop(op, &args[0], &args[1], node, st);
            }
        }
    }
    an.call_value(f, node, None, st, Some(args))
}

fn items(an: &mut Analyzer, v: &Value, node: &Node, st: &mut State) -> (Option<Vec<Value>>, Value) {
    let it = an.iterate(v, node, st);
    let e = an.elem_value(&it.elem, node, "el");
    (it.items, e)
}

/// `itertools.<f>(…)`.
pub fn m_itertools(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = c.qual.rsplit('.').next().unwrap_or("").to_string();
    let node = c.node;
    let ctx = format!("it{}{}", name, c.site);
    let arg = |i: usize, kw: &str| c.arg(i, Some(kw));
    match name.as_str() {
        "count" => {
            let start = arg(0, "start").unwrap_or_else(|| Value::lit(intern::key_int(0)));
            let step = arg(1, "step").unwrap_or_else(|| Value::lit(intern::key_int(1)));
            new_infinite(an, Infinite::Count(start, step), &node, st)
        }
        "cycle" => {
            let Some(x) = arg(0, "iterable") else { return Value::data("cycle") };
            match items(an, &x, &node, st) {
                (Some(its), _) => new_infinite(an, Infinite::Cycle(its), &node, st),
                (None, e) => it_list(an, None, e, &node, st, &ctx),
            }
        }
        "repeat" => {
            let x = arg(0, "object").unwrap_or_default();
            match arg(1, "times") {
                None => new_infinite(an, Infinite::Repeat(x), &node, st),
                Some(t) => match t.kv.as_lit().filter(|_| t.pts.is_empty()).and_then(intern::key_as_int) {
                    Some(n) => it_list(an, Some(vec![x; n.max(0) as usize]), Value::new(), &node, st, &ctx),
                    None => it_list(an, None, x, &node, st, &ctx),
                },
            }
        }
        "islice" => {
            let Some(src) = arg(0, "iterable") else { return Value::data("islice") };
            let lit = |v: Option<Value>| -> Option<Option<i64>> {
                match v {
                    None => Some(None),
                    Some(v) if v.pts.is_empty() && v.kv.as_lit() == Some(intern::key_none()) => Some(None),
                    Some(v) => v.kv.as_lit().filter(|_| v.pts.is_empty()).and_then(intern::key_as_int).map(Some),
                }
            };
            let (start, stop, step) = if c.args.len() <= 2 { (Some(None), lit(c.args.get(1).cloned()), Some(None)) } else { (lit(c.args.get(1).cloned()), lit(c.args.get(2).cloned()), lit(c.args.get(3).cloned())) };
            let (Some(start), Some(stop), Some(step)) = (start, stop, step) else {
                let e = items(an, &src, &node, st).1;
                return it_list(an, None, e, &node, st, &ctx);
            };
            let (s0, st1) = (start.unwrap_or(0).max(0) as usize, step.unwrap_or(1).max(1) as usize);
            let its = match infinite_of(an, &src) {
                Some(inf) => match stop {
                    Some(e) => infinite_take(an, &inf, e.max(0) as usize),
                    None => None,
                },
                None => items(an, &src, &node, st).0.or_else(|| stop.and_then(|e| take_known(an, &src, e.max(0) as usize, st))),
            };
            match its {
                Some(its) => {
                    let end = stop.map(|e| (e.max(0) as usize).min(its.len())).unwrap_or(its.len());
                    let sel: Vec<Value> = (s0..end).step_by(st1).map(|i| its[i].clone()).collect();
                    it_list(an, Some(sel), Value::new(), &node, st, &ctx)
                }
                None => {
                    let e = match infinite_of(an, &src) {
                        Some(inf) => infinite_elem(an, &inf),
                        None => items(an, &src, &node, st).1,
                    };
                    it_list(an, None, e, &node, st, &ctx)
                }
            }
        }
        "chain" | "from_iterable" => {
            let srcs: Vec<Value> = if name == "from_iterable" {
                match c.args.first().map(|a| items(an, a, &node, st)) {
                    Some((Some(its), _)) => its,
                    Some((None, e)) => {
                        let inner = items(an, &e, &node, st).1;
                        return it_list(an, None, inner, &node, st, &ctx);
                    }
                    None => vec![],
                }
            } else {
                c.args.clone()
            };
            let mut all: Option<Vec<Value>> = Some(Vec::new());
            let mut elem = Value::new();
            for s in &srcs {
                let (its, e) = items(an, s, &node, st);
                elem = elem.join(&e);
                match (its, all.as_mut()) {
                    (Some(its), Some(a)) => a.extend(its),
                    _ => all = None,
                }
            }
            it_list(an, all, elem, &node, st, &ctx)
        }
        "product" | "permutations" | "combinations" | "combinations_with_replacement" => {
            let repeat = c.kw("repeat").and_then(|v| v.kv.as_lit().filter(|_| v.pts.is_empty()).and_then(intern::key_as_int)).unwrap_or(1).max(0) as usize;
            let srcs: Vec<Value> = if name == "product" { c.args.clone() } else { c.args.first().cloned().into_iter().collect() };
            let mut cols: Option<Vec<Vec<Value>>> = Some(Vec::new());
            let mut elem = Value::new();
            for s in &srcs {
                let (its, e) = items(an, s, &node, st);
                elem = elem.join(&e);
                match (its, cols.as_mut()) {
                    (Some(its), Some(cs)) => cs.push(its),
                    _ => cols = None,
                }
            }
            let r = c.arg(1, Some("r")).and_then(|v| v.kv.as_lit().filter(|_| v.pts.is_empty()).and_then(intern::key_as_int)).map(|x| x.max(0) as usize);
            let Some(cols) = cols else {
                // unknown inputs: tuples of their elements
                let t = it_tuple(an, vec![elem.clone()], &node, st, &format!("{}t", ctx));
                if let Some(o) = t.single() {
                    an.heap.set_llen(o, st, None);
                }
                return it_list(an, None, t, &node, st, &ctx);
            };
            let idx_tuples: Vec<Vec<(usize, usize)>> = match name.as_str() {
                "product" => {
                    let mut cs: Vec<usize> = Vec::new();
                    for _ in 0..repeat {
                        for (j, col) in cols.iter().enumerate() {
                            let _ = col;
                            cs.push(j);
                        }
                    }
                    let mut out: Vec<Vec<(usize, usize)>> = vec![vec![]];
                    for j in cs {
                        let mut next = Vec::new();
                        for t in &out {
                            for i in 0..cols[j].len() {
                                let mut t2 = t.clone();
                                t2.push((j, i));
                                next.push(t2);
                            }
                        }
                        out = next;
                        if out.len() > MAX_ITEMS {
                            break;
                        }
                    }
                    out
                }
                _ => {
                    let n = cols.first().map(|c0| c0.len()).unwrap_or(0);
                    let k = r.unwrap_or(n);
                    let mut out: Vec<Vec<usize>> = Vec::new();
                    fn rec(n: usize, k: usize, kind: &str, cur: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
                        if out.len() > MAX_ITEMS {
                            return;
                        }
                        if cur.len() == k {
                            out.push(cur.clone());
                            return;
                        }
                        let from = match kind {
                            "combinations" => cur.last().map(|x| x + 1).unwrap_or(0),
                            "combinations_with_replacement" => cur.last().copied().unwrap_or(0),
                            _ => 0,
                        };
                        for i in from..n {
                            if kind == "permutations" && cur.contains(&i) {
                                continue;
                            }
                            cur.push(i);
                            rec(n, k, kind, cur, out);
                            cur.pop();
                        }
                    }
                    if k <= n || name == "combinations_with_replacement" {
                        rec(n, k, &name, &mut Vec::new(), &mut out);
                    }
                    out.into_iter().map(|t| t.into_iter().map(|i| (0, i)).collect()).collect()
                }
            };
            if idx_tuples.len() > MAX_ITEMS {
                let t = it_tuple(an, vec![elem.clone()], &node, st, &format!("{}t", ctx));
                if let Some(o) = t.single() {
                    an.heap.set_llen(o, st, None);
                }
                return it_list(an, None, t, &node, st, &ctx);
            }
            let mut tuples = Vec::new();
            for (k, t) in idx_tuples.iter().enumerate() {
                let vals: Vec<Value> = t.iter().map(|(j, i)| cols[*j][*i].clone()).collect();
                tuples.push(it_tuple(an, vals, &node, st, &format!("{}t{}", ctx, k)));
            }
            it_list(an, Some(tuples), Value::new(), &node, st, &ctx)
        }
        "accumulate" => {
            let Some(src) = arg(0, "iterable") else { return Value::data("accumulate") };
            let func = arg(1, "func").filter(|f| !(f.pts.is_empty() && f.kv.as_lit() == Some(intern::key_none())));
            let initial = c.kw("initial");
            let (its, e) = items(an, &src, &node, st);
            match its {
                Some(its) => {
                    let mut out = Vec::new();
                    let mut acc: Option<Value> = initial.clone();
                    if let Some(i) = &initial {
                        out.push(i.clone());
                    }
                    for (k, x) in its.into_iter().enumerate() {
                        let nv = match &acc {
                            None => x,
                            Some(a) => {
                                let nd = sub_node(&node, k, 1_100_000);
                                match &func {
                                    Some(f) => it_call(an, f, vec![a.clone(), x], &nd, st),
                                    None => an.binop(ruff_python_ast::Operator::Add, a, &x, &nd, st),
                                }
                            }
                        };
                        out.push(nv.clone());
                        acc = Some(nv);
                    }
                    it_list(an, Some(out), Value::new(), &node, st, &ctx)
                }
                None => {
                    let r = match &func {
                        Some(f) => it_call(an, f, vec![e.clone(), e.clone()], &node, st),
                        None => an.binop(ruff_python_ast::Operator::Add, &e, &e, &node, st),
                    };
                    let el = e.join(&r).join(&initial.unwrap_or_default());
                    it_list(an, None, el, &node, st, &ctx)
                }
            }
        }
        "starmap" => {
            let (Some(f), Some(src)) = (arg(0, "function"), arg(1, "iterable")) else { return Value::data("starmap") };
            let (its, e) = items(an, &src, &node, st);
            match its {
                Some(its) => {
                    let mut out = Vec::new();
                    for (k, t) in its.into_iter().enumerate() {
                        let nd = sub_node(&node, k, 1_200_000);
                        let args = match items(an, &t, &nd, st).0 {
                            Some(a) => a,
                            None => vec![items(an, &t, &nd, st).1],
                        };
                        out.push(it_call(an, &f, args, &nd, st));
                    }
                    it_list(an, Some(out), Value::new(), &node, st, &ctx)
                }
                None => {
                    let args = match items(an, &e, &node, st).0 {
                        Some(a) => a,
                        None => vec![items(an, &e, &node, st).1],
                    };
                    let r = it_call(an, &f, args, &node, st);
                    it_list(an, None, r, &node, st, &ctx)
                }
            }
        }
        "zip_longest" => {
            let fill = c.kw("fillvalue").unwrap_or_else(|| an.none_v.clone());
            let mut cols: Option<Vec<Vec<Value>>> = Some(Vec::new());
            let mut elems: Vec<Value> = Vec::new();
            for s in &c.args {
                let (its, e) = items(an, s, &node, st);
                elems.push(e.join(&fill));
                match (its, cols.as_mut()) {
                    (Some(its), Some(cs)) => cs.push(its),
                    _ => cols = None,
                }
            }
            match cols {
                Some(cols) => {
                    let n = cols.iter().map(|c0| c0.len()).max().unwrap_or(0);
                    let mut tuples = Vec::new();
                    for k in 0..n {
                        let vals: Vec<Value> = cols.iter().map(|c0| c0.get(k).cloned().unwrap_or_else(|| fill.clone())).collect();
                        tuples.push(it_tuple(an, vals, &node, st, &format!("{}t{}", ctx, k)));
                    }
                    it_list(an, Some(tuples), Value::new(), &node, st, &ctx)
                }
                None => {
                    let t = it_tuple(an, elems, &node, st, &format!("{}t", ctx));
                    it_list(an, None, t, &node, st, &ctx)
                }
            }
        }
        "takewhile" | "dropwhile" | "filterfalse" => {
            let (Some(pred), Some(src)) = (arg(0, "predicate"), arg(1, "iterable")) else { return Value::data(&name) };
            let (its, e) = items(an, &src, &node, st);
            match its {
                Some(its) => {
                    let mut out = Vec::new();
                    let mut dropping = true;
                    let mut known = true;
                    for (k, x) in its.into_iter().enumerate() {
                        let nd = sub_node(&node, k, 1_300_000);
                        if name == "dropwhile" && !dropping {
                            out.push(x);
                            continue;
                        }
                        let r = it_call(an, &pred, vec![x.clone()], &nd, st);
                        let t = an.truth_of(&r, st, &nd);
                        match (name.as_str(), t) {
                            ("takewhile", Some(true)) => out.push(x),
                            ("takewhile", Some(false)) => break,
                            ("dropwhile", Some(true)) => {}
                            ("dropwhile", Some(false)) => {
                                dropping = false;
                                out.push(x);
                            }
                            ("filterfalse", Some(false)) => out.push(x),
                            ("filterfalse", Some(true)) => {}
                            _ => {
                                known = false;
                                break;
                            }
                        }
                    }
                    if known {
                        it_list(an, Some(out), Value::new(), &node, st, &ctx)
                    } else {
                        let el = items(an, &src, &node, st).1;
                        it_list(an, None, el, &node, st, &ctx)
                    }
                }
                None => {
                    it_call(an, &pred, vec![e.clone()], &node, st);
                    it_list(an, None, e, &node, st, &ctx)
                }
            }
        }
        "compress" => {
            let (Some(data), Some(sel)) = (arg(0, "data"), arg(1, "selectors")) else { return Value::data("compress") };
            let (di, de) = items(an, &data, &node, st);
            let (si, _) = items(an, &sel, &node, st);
            if let (Some(di), Some(si)) = (di, si) {
                let mut out = Vec::new();
                let mut known = true;
                for (x, s) in di.into_iter().zip(si.into_iter()) {
                    match an.truth_of(&s, st, &node) {
                        Some(true) => out.push(x),
                        Some(false) => {}
                        None => known = false,
                    }
                }
                if known {
                    return it_list(an, Some(out), Value::new(), &node, st, &ctx);
                }
            }
            it_list(an, None, de, &node, st, &ctx)
        }
        "tee" => {
            let Some(src) = arg(0, "iterable") else { return Value::data("tee") };
            let n = arg(1, "n").and_then(|v| v.kv.as_lit().filter(|_| v.pts.is_empty()).and_then(intern::key_as_int)).unwrap_or(2).max(0) as usize;
            let (its, e) = items(an, &src, &node, st);
            let copies: Vec<Value> = (0..n).map(|k| it_list(an, its.clone(), e.clone(), &node, st, &format!("{}c{}", ctx, k))).collect();
            it_tuple(an, copies, &node, st, &format!("{}t", ctx))
        }
        "pairwise" => {
            let Some(src) = arg(0, "iterable") else { return Value::data("pairwise") };
            let (its, e) = items(an, &src, &node, st);
            match its {
                Some(its) => {
                    let mut tuples = Vec::new();
                    for k in 1..its.len() {
                        tuples.push(it_tuple(an, vec![its[k - 1].clone(), its[k].clone()], &node, st, &format!("{}t{}", ctx, k)));
                    }
                    it_list(an, Some(tuples), Value::new(), &node, st, &ctx)
                }
                None => {
                    let t = it_tuple(an, vec![e.clone(), e], &node, st, &format!("{}t", ctx));
                    it_list(an, None, t, &node, st, &ctx)
                }
            }
        }
        "batched" => {
            let Some(src) = arg(0, "iterable") else { return Value::data("batched") };
            let n = arg(1, "n").and_then(|v| v.kv.as_lit().filter(|_| v.pts.is_empty()).and_then(intern::key_as_int));
            let (its, e) = items(an, &src, &node, st);
            match (its, n) {
                (Some(its), Some(n)) if n > 0 => {
                    let n = n as usize;
                    let mut tuples = Vec::new();
                    for (k, chunk) in its.chunks(n).enumerate() {
                        tuples.push(it_tuple(an, chunk.to_vec(), &node, st, &format!("{}t{}", ctx, k)));
                    }
                    it_list(an, Some(tuples), Value::new(), &node, st, &ctx)
                }
                _ => {
                    let t = it_tuple(an, vec![e], &node, st, &format!("{}t", ctx));
                    if let Some(o) = t.single() {
                        an.heap.set_llen(o, st, None);
                    }
                    it_list(an, None, t, &node, st, &ctx)
                }
            }
        }
        "groupby" => {
            let Some(src) = arg(0, "iterable") else { return Value::data("groupby") };
            let key = arg(1, "key").filter(|f| !(f.pts.is_empty() && f.kv.as_lit() == Some(intern::key_none())));
            let (its, e) = items(an, &src, &node, st);
            let key_of = |an: &mut Analyzer, st: &mut State, x: &Value, nd: &Node| -> Value {
                match &key {
                    Some(f) => it_call(an, f, vec![x.clone()], nd, st),
                    None => x.clone(),
                }
            };
            if let Some(its) = its {
                // consecutive runs of one key (keys known literals)
                let mut groups: Vec<(intern::KeyId, Value, Vec<Value>)> = Vec::new();
                let mut known = true;
                for (k, x) in its.iter().enumerate() {
                    let nd = sub_node(&node, k, 1_400_000);
                    let kv = key_of(an, st, x, &nd);
                    let Some(kk) = kv.kv.as_lit().filter(|_| kv.pts.is_empty()) else {
                        known = false;
                        break;
                    };
                    match groups.last_mut() {
                        Some((g, _, members)) if *g == kk => members.push(x.clone()),
                        _ => groups.push((kk, kv.clone(), vec![x.clone()])),
                    }
                }
                if known {
                    let mut pairs = Vec::new();
                    for (k, (_, kv, members)) in groups.into_iter().enumerate() {
                        let g = it_list(an, Some(members), Value::new(), &node, st, &format!("{}g{}", ctx, k));
                        pairs.push(it_tuple(an, vec![kv, g], &node, st, &format!("{}p{}", ctx, k)));
                    }
                    return it_list(an, Some(pairs), Value::new(), &node, st, &ctx);
                }
            }
            let kv = key_of(an, st, &e, &node);
            let g = it_list(an, None, e, &node, st, &format!("{}g", ctx));
            let p = it_tuple(an, vec![kv, g], &node, st, &format!("{}p", ctx));
            it_list(an, None, p, &node, st, &ctx)
        }
        _ => Value::data(&format!("itertools.{}", name)),
    }
}

pub fn register(r: &mut dyn FnMut(&[&'static str], crate::models::Model)) {
    r(
        &[
            "itertools.count", "itertools.cycle", "itertools.repeat", "itertools.islice", "itertools.chain", "itertools.chain.from_iterable",
            "itertools.product", "itertools.permutations", "itertools.combinations", "itertools.combinations_with_replacement",
            "itertools.accumulate", "itertools.starmap", "itertools.zip_longest", "itertools.takewhile", "itertools.dropwhile",
            "itertools.filterfalse", "itertools.compress", "itertools.tee", "itertools.pairwise", "itertools.batched", "itertools.groupby",
        ],
        m_itertools,
    );
}

/// What a model call does to the one-shot iterators it is given: a full consumer uses
/// them up; a lazy wrapper (zip, islice, map, …) takes from them when it is consumed
/// itself; anything else leaves their position unknown.
pub(crate) fn after_model_consume(an: &mut Analyzer, call: &Call, r: &Value, state: &mut State) {
    // (one-shot iterators, and lazy iterators over them)
    let its: Vec<(usize, u32)> = call.args.iter().chain(call.star_args.iter()).enumerate().filter_map(|(i, a)| a.single().filter(|o| a.kv.is_bot() && ((an.heap.obj(*o).iterator && an.heap.obj(*o).single && matches!(an.heap.kind(*o), Kind::List)) || an.m.lazy_src.contains_key(o))).map(|o| (i, o))).collect();
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
            an.exhaust(&Value::obj(o), state);
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
            an.m.lazy_src.insert(ro, vec![(its[0].1, Some(n.max(0) as usize))]);
            return;
        }
    }
    if q == "builtins.zip" {
        if let (Some(ro), Some(pulls)) = (robj, zip_pulls(an, call, state)) {
            an.m.lazy_src.insert(ro, pulls);
            return;
        }
    }
    if LAZY_ALL.contains(&q) {
        if let Some(ro) = robj {
            an.m.lazy_src.insert(ro, its.iter().map(|(_, o)| (*o, None)).collect());
            return;
        }
    }
    for (_, o) in its {
        if an.m.lazy_src.contains_key(&o) {
            an.consume_lazy(&Value::obj(o), false, state);
        } else {
            an.set_cursor(o, None, state);
        }
    }
}

/// The items `zip(...)` takes from each one-shot iterator argument when it is consumed:
/// rounds = the shortest length; in the last round the arguments before the first that
/// ran out each gave one more.
pub(crate) fn zip_pulls(an: &mut Analyzer, call: &Call, state: &mut State) -> Option<Vec<(ObjId, Option<usize>)>> {
    if !call.star_args.is_empty() || call.kwargs.keys().any(|k| &*intern::str_get(*k) != "strict") {
        return None;
    }
    // (exact remaining length, or a lower bound)
    let mut lens: Vec<(Option<usize>, usize, Option<u32>)> = Vec::new();
    for a in &call.args {
        let one = a.single().filter(|o| a.kv.is_bot() && an.heap.obj(*o).iterator && an.heap.obj(*o).single && matches!(an.heap.kind(*o), Kind::List));
        match one {
            Some(o) => {
                let c = an.cursor_of(o, state)?;
                match an.heap.llen(o, state) {
                    Some(n) => lens.push((Some(n.saturating_sub(c)), n.saturating_sub(c), Some(o))),
                    None => lens.push((None, an.heap.prefix(o, state).unwrap_or(0).saturating_sub(c), Some(o))),
                }
            }
            None => {
                let it = an.iterate(a, &call.node, state);
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
