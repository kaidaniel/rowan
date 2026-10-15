//! Threads and executors: `threading.Thread(target=f, args=…, kwargs=…).start()` runs `f`
//! there (its exceptions stay in the thread); `ThreadPoolExecutor.submit(f, …)` runs `f` and
//! gives a Future whose `result()` is its return value (re-raising what `f` raised);
//! `executor.map(f, *its)` maps in input order; `as_completed` yields the futures in any order.

use indexmap::IndexMap;

use crate::domain::{join_values, Ns, ObjId, Row, Value};
use crate::frontend::Node;
use crate::intern::{self, key_str_lit, StrId};
use crate::interp::{Analyzer, Call, Payload};
use crate::state::{Kind, State};

fn obj(an: &mut Analyzer, st: &mut State, node: &Node, qual: &str, ctx: &str) -> ObjId {
    let oid = an.alloc_at(Kind::Namespace, node, st, qual, Payload::Method(qual.into(), Value::new()), Some(Row::new()), Ns::Attr, None, Some(ctx));
    an.heap.obj_mut(oid).payload = Payload::Method(qual.into(), Value::new());
    oid
}

fn slot_key(name: &str) -> intern::KeyId {
    key_str_lit(&format!("\u{1}{}", name))
}

/// The exceptions a call raised: (0 none / 1 maybe / 2 surely, their class names; empty: any).
pub struct Raised {
    pub level: u8,
    pub names: Vec<String>,
    /// the exception objects raised (empty: not known)
    pub exc: Value,
}

/// `f(*args, **kwargs)` with every exception caught (a thread, a future): its value, and
/// what it raised; the state after is the join of the normal end and the raising points.
pub fn call_catching(an: &mut Analyzer, f: &Value, args: Vec<Value>, kwargs: IndexMap<StrId, Value>, state: &mut State, node: &Node) -> (Value, Raised) {
    let mut pre = state.copy();
    an.enter_try(rustc_hash::FxHashSet::from_iter(["*".to_string()]), &mut pre);
    let outs = an.catching(|an| {
        let mut outs = Vec::new();
        for fo in f.pts.iter() {
            let v = match an.heap.kind(fo) {
                Kind::Function => an.call_user(fo, None, args.clone(), kwargs.clone(), vec![], vec![], state, node, None, false),
                _ => an.call_value(&Value::obj(fo), node, None, state, Some(args.clone())),
            };
            outs.push(v);
        }
        outs
    });
    let out = an.leave_try();
    let (flagged, raised_at, raised_v) = (out.any(), out.raised(), out.value());
    let died = !state.reachable;
    let raised = flagged || raised_at.is_some() || died;
    let level = if died && raised { 2 } else if raised { 1 } else { 0 };
    let mut names: Vec<String> = Vec::new();
    let mut any = flagged || raised_v.pts.is_empty();
    for o in raised_v.pts.iter() {
        let n = match an.heap.kind(o) {
            Kind::Instance => an.heap.instance_class(o).map(|c| an.heap.class_info(c).name.clone()),
            _ => an.heap.obj(o).label.split(|ch: char| !ch.is_alphanumeric() && ch != '_').next().map(|x| x.to_string()).filter(|x| !x.is_empty()),
        };
        match n {
            Some(n) if !names.contains(&n) => names.push(n),
            Some(_) => {}
            None => any = true,
        }
    }
    if any {
        names.clear();
    }
    match raised_at {
        Some(mut r) => {
            let after = if state.reachable { state.copy().join(&mut r) } else { r };
            state.replace_with(after);
        }
        None if died => {
            // (raised somewhere unknown: from the state before the call)
            state.replace_with(pre.copy());
        }
        None => {}
    }
    let exc = if any { Value::new() } else { raised_v.clone() };
    (join_values(outs), Raised { level, names, exc })
}

/// Re-raising what a call raised (`future.result()`).
pub fn reraise(an: &mut Analyzer, level: u8, names: &[String], state: &mut State) {
    if level == 0 {
        return;
    }
    let ns: Vec<String> = if names.is_empty() { vec!["Exception".into()] } else { names.to_vec() };
    if level == 2 && ns.len() == 1 {
        an.raise_here(&ns[0], state);
    } else {
        for n in &ns {
            an.may_raise_here(n, state);
        }
        if names.is_empty() {
            // (any exception: any handler may run)
            an.flag_may_raise(state);
        }
        if level == 2 {
            state.reachable = false;
        }
    }
}

/// `threading.Thread(group=None, target=None, name=None, args=(), kwargs=None, daemon=None)`;
/// `threading.Timer(interval, function, args=None, kwargs=None)` is a thread running `function`.
pub fn thread_new(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let o = obj(an, st, &c.node, "stdobj.thread", &format!("th{}", c.site));
    let timer = c.qual == "threading.Timer";
    let (target, args, kwargs) = if timer { (c.arg(1, Some("function")), c.arg(2, Some("args")), c.arg(3, Some("kwargs"))) } else { (c.arg(1, Some("target")), c.arg(3, Some("args")), c.arg(4, Some("kwargs"))) };
    let target = target.unwrap_or_else(|| an.none_v.clone());
    let (args, kwargs) = (args.unwrap_or_default(), kwargs.unwrap_or_default());
    for (k, v) in [("target", target), ("args", args), ("kwargs", kwargs), ("timer", Value::lit(intern::key_bool(timer)))] {
        st.set_slot(o, Ns::Attr, slot_key(k), v, true);
    }
    let name = if timer { None } else { c.arg(2, Some("name")) };
    let daemon = if timer { None } else { c.arg(5, Some("daemon")) };
    for (k, v) in [("name", name.unwrap_or_else(|| Value::data("thread name"))), ("daemon", daemon.unwrap_or_else(|| Value::lit(intern::key_bool(false))))] {
        let kk = key_str_lit(k);
        st.set_slot(o, Ns::Attr, kk, v, true);
        st.set_row(o, Ns::Attr, st.row(o, Ns::Attr).store(kk, true));
    }
    Value::obj(o)
}

/// The positional and keyword arguments a `args=` tuple and a `kwargs=` dict give.
fn call_args(an: &mut Analyzer, st: &mut State, node: &Node, args: &Value, kwargs: &Value) -> (Vec<Value>, IndexMap<StrId, Value>) {
    let mut a = Vec::new();
    if !args.empty() {
        let it = an.iterate(args, node, st);
        match it.items {
            Some(items) => a = items,
            None => a.push(an.elem_value(&it.elem, node, "el")),
        }
    }
    let mut kw = IndexMap::new();
    for d in kwargs.pts.iter().filter(|o| an.heap.kind(*o) == Kind::Dict).collect::<Vec<_>>() {
        let row = st.row(d, Ns::Item);
        for k in row.present.iter() {
            if let Some(name) = intern::key_as_str(k) {
                kw.insert(intern::s(&name), st.slot(d, Ns::Item, k));
            }
        }
    }
    (a, kw)
}

/// `thread.start()` / `thread.run()`: the target runs (with every exception caught).
fn thread_start(an: &mut Analyzer, st: &mut State, c: &Call, o: ObjId) {
    let target = st.slot(o, Ns::Attr, slot_key("target"));
    if target.pts.is_empty() {
        return;
    }
    let args = st.slot(o, Ns::Attr, slot_key("args"));
    let kwargs = st.slot(o, Ns::Attr, slot_key("kwargs"));
    let (a, kw) = call_args(an, st, &c.node, &args, &kwargs);
    // (a timer's function runs unless it is cancelled first)
    let mut cancelled = (st.slot(o, Ns::Attr, slot_key("timer")).kv.as_lit() != Some(intern::key_bool(false))).then(|| st.copy());
    call_catching(an, &target, a, kw, st, &c.node);
    if let Some(c) = cancelled.as_mut() {
        let j = st.join(c);
        st.replace_with(j);
    }
}

/// `ThreadPoolExecutor(max_workers=…, thread_name_prefix=…, initializer=…, initargs=…)` /
/// `ProcessPoolExecutor(…)`: a thread pool's initializer runs in its workers (with every
/// exception caught; a process pool's runs in other processes).
pub fn executor_new(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let threads = !c.qual.ends_with("ProcessPoolExecutor");
    if let Some(f) = c.arg(2, Some("initializer")).filter(|f| threads && !f.pts.is_empty()) {
        let args = c.arg(3, Some("initargs")).unwrap_or_default();
        let (a, kw) = call_args(an, st, &c.node, &args, &Value::new());
        // (when a worker starts: perhaps never)
        let mut none = st.copy();
        call_catching(an, &f, a, kw, st, &c.node);
        let j = st.join(&mut none);
        st.replace_with(j);
    }
    Value::obj(obj(an, st, &c.node, "stdobj.executor", &format!("ex{}", c.site)))
}

fn future_of(an: &mut Analyzer, st: &mut State, node: &Node, ctx: &str, v: Value, r: &Raised) -> Value {
    let o = obj(an, st, node, "stdobj.future", ctx);
    st.set_slot(o, Ns::Attr, slot_key("result"), v, true);
    st.set_slot(o, Ns::Attr, slot_key("raised"), Value::lit(intern::key_int(r.level as i64)), true);
    // what exception() gives: None, the raised exception, or (not known) either
    let none = an.none_v.clone();
    let exc = match r.level {
        0 => none,
        2 if !r.exc.empty() => r.exc.clone(),
        2 => Value::data("exception"),
        _ if !r.exc.empty() => r.exc.join(&none),
        _ => none.join(&Value::data("exception")),
    };
    st.set_slot(o, Ns::Attr, slot_key("exc"), exc, true);
    an.m.future_excs.insert(o, r.names.clone());
    Value::obj(o)
}

pub fn m_conc(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let q = c.qual.as_str();
    let name = q.rsplit('.').next().unwrap_or("");
    let recv = c.recv.clone().unwrap_or_default();
    let objs: Vec<ObjId> = recv.pts.iter().collect();
    if q.starts_with("stdobj.thread.") {
        match name {
            "start" | "run" => {
                for o in objs {
                    thread_start(an, st, c, o);
                }
                return an.none_v.clone();
            }
            "join" => return an.none_v.clone(),
            "is_alive" => return Value::data("alive"),
            _ => return Value::data(name),
        }
    }
    if q.starts_with("stdobj.executor.") {
        match name {
            "__enter__" => return recv,
            "__exit__" | "shutdown" => return an.none_v.clone(),
            "submit" => {
                let Some(f) = c.args.first().cloned() else { return Value::data("future") };
                let args: Vec<Value> = c.args.iter().skip(1).cloned().collect();
                let (v, r) = call_catching(an, &f, args, c.kwargs.clone(), st, &c.node);
                return future_of(an, st, &c.node, &format!("fut{}", c.site), v, &r);
            }
            "map" => {
                let Some(f) = c.args.first().cloned() else { return Value::data("map") };
                let its: Vec<Value> = c.args.iter().skip(1).cloned().collect();
                let iters: Vec<crate::interp::funcs::Iteration> = its.iter().map(|x| an.iterate(x, &c.node, st)).collect();
                let all_known = iters.iter().all(|i| i.items.is_some());
                let mut results = Vec::new();
                let mut worst = Raised { level: 0, names: vec![], exc: Value::new() };
                if all_known && !iters.is_empty() {
                    let n = iters.iter().map(|i| i.items.as_ref().unwrap().len()).min().unwrap_or(0);
                    for k in 0..n {
                        let args: Vec<Value> = iters.iter().map(|i| i.items.as_ref().unwrap()[k].clone()).collect();
                        // (each call its own context: the results stay distinct objects)
                        let mut nd = c.node;
                        nd.col = c.node.col.wrapping_add(2_000_000 + k as u32);
                        let (v, r) = call_catching(an, &f, args, IndexMap::new(), st, &nd);
                        if r.level > 0 {
                            worst.level = worst.level.max(1);
                            worst.names.extend(r.names);
                        }
                        results.push(v);
                    }
                } else {
                    let args: Vec<Value> = iters.iter().map(|i| an.elem_value(&i.elem, &c.node, "el")).collect();
                    let (v, r) = call_catching(an, &f, args, IndexMap::new(), st, &c.node);
                    worst.level = r.level.min(1);
                    worst.names = r.names;
                    results.push(v);
                }
                // (an exception comes out when its result is reached: here, approximately)
                reraise(an, worst.level, &worst.names, st);
                let lid = an.alloc_at(Kind::List, &c.node, st, &format!("map@L{}", c.line), Payload::None, None, Ns::Item, None, Some(&format!("exmap{}", c.site)));
                let acc = join_values(results.clone());
                st.set_slot(lid, Ns::Elem, intern::STAR, Value::with_kv(acc.pts.clone(), crate::models::torch::elem_kv(&acc.kv)), true);
                st.elems.insert(lid, acc.kv.clone());
                if all_known && an.heap.obj(lid).single {
                    for (i, v) in results.iter().enumerate() {
                        st.set_slot(lid, Ns::Pos, intern::key_int(i as i64), v.clone(), true);
                    }
                    an.heap.set_llen(lid, st, Some(results.len()));
                } else {
                    an.heap.set_llen(lid, st, None);
                }
                an.heap.obj_mut(lid).iterator = true;
                return Value::obj(lid);
            }
            _ => return Value::data(name),
        }
    }
    if q.starts_with("stdobj.future.") {
        match name {
            "result" => {
                let mut outs = Vec::new();
                for o in objs {
                    let lvl = st.slot(o, Ns::Attr, slot_key("raised")).kv.as_lit().and_then(intern::key_as_int).unwrap_or(1) as u8;
                    let names = an.m.future_excs.get(&o).cloned().unwrap_or_default();
                    outs.push(st.slot(o, Ns::Attr, slot_key("result")));
                    reraise(an, lvl, &names, st);
                }
                return join_values(outs);
            }
            "done" => return Value::lit(intern::key_bool(true)),
            "cancel" | "cancelled" | "running" => return Value::lit(intern::key_bool(false)),
            "exception" => return join_values(objs.iter().map(|o| st.slot(*o, Ns::Attr, slot_key("exc"))).collect::<Vec<_>>()),
            "add_done_callback" => {
                if let Some(f) = c.args.first().cloned() {
                    call_catching(an, &f, vec![recv.clone()], IndexMap::new(), st, &c.node);
                }
                return an.none_v.clone();
            }
            _ => return Value::data(name),
        }
    }
    Value::data(name)
}

/// `concurrent.futures.as_completed(fs)`: the futures, in any order.
pub fn as_completed(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(fs) = c.args.first() else { return Value::data("as_completed") };
    let it = an.iterate(fs, &c.node, st);
    // (a dict of futures: its keys, the future objects)
    let e = match &it.items {
        Some(items) => join_values(items.iter().map(|x| an.unkey(x)).collect::<Vec<_>>()),
        None => an.elem_value(&it.elem, &c.node, "el"),
    };
    let lid = an.alloc_at(Kind::List, &c.node, st, &format!("as_completed@L{}", c.line), Payload::None, None, Ns::Item, None, Some("asc"));
    st.set_slot(lid, Ns::Elem, intern::STAR, e.clone(), true);
    st.elems.insert(lid, e.kv.clone());
    an.heap.set_llen(lid, st, None);
    an.heap.obj_mut(lid).iterator = true;
    Value::obj(lid)
}

/// `t.start()` of a `threading.Thread` subclass: its `run()` runs (with every exception
/// caught); `t.join()` waits.
fn thread_sub(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    if c.qual == "threadsub.start" {
        let recv = c.recv.clone().unwrap_or_default();
        let mut pre = st.copy();
        an.enter_try(rustc_hash::FxHashSet::from_iter(["*".to_string()]), &mut pre);
        an.catching(|an| an.call_method_with(&recv, "run", vec![], &c.node, st));
        an.leave_try();
    }
    an.none_v.clone()
}

pub fn register(r: &mut dyn FnMut(&[&'static str], crate::models::Model)) {
    r(&["threadsub.start", "threadsub.join"], thread_sub);
    r(&["threading.Thread", "threading.Timer"], thread_new);
    r(&["concurrent.futures.ThreadPoolExecutor", "concurrent.futures.ProcessPoolExecutor", "concurrent.futures.thread.ThreadPoolExecutor"], executor_new);
    r(&["concurrent.futures.as_completed"], as_completed);
}
