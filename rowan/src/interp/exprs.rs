//! Expressions (port of the `expr_*` methods, `truthiness`, `binop`, comprehensions).

use indexmap::IndexMap;
use ruff_python_ast as ast;

use crate::domain::{kv_join, kv_map, FnKind, KeyVal, KeysOf, Ns, ObjSet, RenameFn, Row, Value, DATA, BOT};
use crate::frontend::{self, Node};
use crate::intern::{self, key_float, key_int, key_str_lit, s, KeyId, STAR};
use crate::state::{Kind, State};

use super::funcs::Elem;
use super::{is_constant, names_in, Analyzer, ObjId, Payload, ViewKind, UNROLL_MAX};

/// Python's `s[lo:hi:step]` on characters.
/// `text[lo:hi:step]` (None: a zero step).
fn py_slice(cs: &[char], lo: Option<i64>, hi: Option<i64>, step: Option<i64>) -> Option<String> {
    let step = step.unwrap_or(1);
    (step != 0).then(|| slice_positions(cs.len() as i64, lo, hi, step).into_iter().map(|i| cs[i as usize]).collect())
}

impl Analyzer {
    /// Evaluate an expression (while a test is evaluated, each node's value is kept for its
    /// narrowing: `split`).
    pub fn eval(&mut self, node: &'static ast::Expr, state: &mut State) -> Value {
        let v = self.eval_inner(node, state);
        if let Some((depth, vals)) = &mut self.trace {
            if *depth == self.frames.len() {
                vals.insert(node as *const ast::Expr as usize, v.clone());
            }
        }
        v
    }

    fn eval_inner(&mut self, node: &'static ast::Expr, state: &mut State) -> Value {
        match node {
            ast::Expr::StringLiteral(sl) => Value::lit(key_str_lit(sl.value.to_str())),
            ast::Expr::NumberLiteral(n) => match &n.value {
                ast::Number::Int(i) => match i.as_i64() {
                    Some(v) => Value::lit(key_int(v)),
                    None => Value::data("bigint"),
                },
                ast::Number::Float(f) => Value::lit(key_float(*f)),
                ast::Number::Complex { .. } => Value::data("complex"),
            },
            ast::Expr::BooleanLiteral(b) => Value::lit(intern::key_bool(b.value)),
            ast::Expr::NoneLiteral(_) => self.none_v.clone(),
            ast::Expr::BytesLiteral(b) => Value::lit(intern::key(intern::Key::Bytes(b.value.bytes().collect()))),
            ast::Expr::EllipsisLiteral(_) => Value::lit(intern::key(intern::Key::Ellipsis)),
            ast::Expr::Name(n) => {
                let nd = frontend::node(node);
                let name = s(n.id.as_str());
                let v = self.load_name(name, &nd, state);
                if self.facts_on {
                    self.record_fact(&nd, name, &v, state);
                }
                v
            }
            ast::Expr::Attribute(a) => {
                if !state.narrowed.is_empty() {
                    if let Some(key) = super::chain_text(node) {
                        if let Some(hit) = state.narrowed.get(&s(&key)) {
                            return hit.clone();
                        }
                    }
                }
                let obj = self.eval(&a.value, state);
                let nd = frontend::node(node);
                self.load_attr(&obj, a.attr.id.as_str(), &nd, state, true)
            }
            ast::Expr::Subscript(sub) => {
                let obj = self.eval(&sub.value, state);
                // a generic pydantic model parametrised: `Page[User]`
                if let Some(c) = obj.single().filter(|c| self.kind(*c) == Kind::Class && crate::models::pydantic::is_validated(self, *c)) {
                    if let Some(v) = self.pyd_specialise(c, &sub.slice, state) {
                        return v;
                    }
                }
                {
                    let nd = frontend::node(node);
                    if let Some(v) = self.t_subscript(&obj, &sub.slice, &nd, state) {
                        return v;
                    }
                }
                let key = self.eval_key(&sub.slice, state);
                let bounds = Self::slice_label_bounds(&key.kv);
                if let Some(v) = bounds.and_then(|b| self.slice_positional(&obj, b, node, state)) {
                    return v;
                }
                // a known string sliced / indexed by known numbers (an index out of range raises
                // IndexError: on every string the value may be, or on some)
                let strs: Option<Vec<Vec<char>>> = crate::domain::kv_literals(&obj.kv).iter().map(|k| intern::key_as_str(k).map(|t| t.chars().collect())).collect();
                if let Some(strs) = strs.filter(|v| obj.pts.is_empty() && obj.kv.is_literal() && !v.is_empty()) {
                    let index = key.kv.as_lit().filter(|_| key.pts.is_empty()).and_then(intern::key_as_int);
                    let out: Option<intern::KeySet> = match (bounds, index) {
                        (Some((lo, hi, st)), _) => strs.iter().map(|cs| py_slice(cs, lo, hi, st).map(|t| key_str_lit(&t))).collect(),
                        (None, Some(i)) => {
                            let at = |cs: &Vec<char>| usize::try_from(if i < 0 { i + cs.len() as i64 } else { i }).ok().and_then(|j| cs.get(j).map(|c| key_str_lit(&c.to_string())));
                            let inside: intern::KeySet = strs.iter().filter_map(at).collect();
                            if inside.is_empty() {
                                self.raise_here("IndexError", state);
                                return Value::new();
                            } else if strs.iter().any(|cs| at(cs).is_none()) {
                                self.may_raise_here("IndexError", state);
                            }
                            Some(inside)
                        }
                        (None, None) => {
                            if !matches!(&*sub.slice, ast::Expr::Slice(_)) {
                                self.may_raise_here("IndexError", state);
                            }
                            None
                        }
                    };
                    if let Some(o) = out {
                        return Value::kv_only(KeyVal::lits(o));
                    }
                }
                let nd = frontend::node(node);
                // a key the name's object surely has (stored through the name): no failure
                if let (ast::Expr::Name(nm), Some(k)) = (&*sub.value, key.kv.as_lit().filter(|_| key.pts.is_empty())) {
                    if state.name_has_key(s(nm.id.as_str()), k) {
                        let v = self.load_item(&obj, Ns::Item, &key, &nd, state, false);
                        if let Some(o) = obj.pts.first() {
                            self.verdict(&nd, k, &crate::domain::Classification::with_note(crate::domain::Tier::Proven, "is always there"), o, Ns::Item, None, None);
                        }
                        return v;
                    }
                }
                self.load_item(&obj, Ns::Item, &key, &nd, state, true)
            }
            ast::Expr::Call(c) => {
                // (the tensor model reads `x.dim` as a method only when it is called)
                self.m.t_called_attr = frontend::node(&*c.func).addr;
                self.callee_nodes.insert(self.m.t_called_attr);
                let fv = self.eval(&c.func, state);
                let mut flag = false;
                if !self.tries.is_empty() {
                    let all_safe = fv.pts.iter().all(|o| match &self.obj(o).payload {
                        Payload::Method(q, _) if self.kind(o) == Kind::Method => super::safe_method(q) || super::SELF_REPORTING.iter().any(|p| q.starts_with(p)),
                        // user code: whether it raises is reported by its own analysis
                        // builtins that raise only on wrong types (len, str, sorted, …)
                        Payload::Str(q) if self.kind(o) == Kind::External => q.strip_prefix("builtins.").map(|n| super::SAFE_BUILTINS.contains(&n) || is_exception_name(n)).unwrap_or(false) || super::SELF_REPORTING_EXTERNALS.contains(&q.as_str())
                            // (making an exception object of a library class: `HTTPException(...)`)
                            || (q.rsplit('.').next().map(|n| n.ends_with("Error") || n.ends_with("Exception")).unwrap_or(false)),
                        _ => matches!(self.kind(o), Kind::Function | Kind::Bound | Kind::Class | Kind::Partial),
                    });
                    if !all_safe {
                        if super::debug_level() == 2 {
                            let what: Vec<String> = fv.pts.iter().map(|o| match &self.obj(o).payload { Payload::Method(q, _) | Payload::Str(q) => q.clone(), _ => self.obj(o).label.to_string() }).collect();
                            eprintln!("  call may raise at L{}: {}", frontend::node(node).line, what.join(", "));
                        }
                        flag = true;
                    }
                }
                let nd = frontend::node(node);
                let saved_node = self.exc_node.replace(nd);
                if let (Some(a @ (ast::Expr::ListComp(_) | ast::Expr::Generator(_))), Some(o)) = (c.arguments.args.first(), fv.single()) {
                    if fv.kv.is_bot() && self.kind(o) == Kind::External && matches!(self.external_qual(o).as_str(), "torch.cat" | "torch.concat" | "torch.concatenate" | "torch.stack") {
                        self.nonempty_comp = a as *const ast::Expr as usize;
                    }
                }
                let r = self.call_value(&fv, &nd, Some(c), state, None);
                self.exc_node = saved_node;
                // (flagged after the call: what the call requires of its inputs, it requires on
                // its raising paths too — a missing column is what makes `read_csv` raise)
                if flag {
                    self.flag_may_raise(state);
                }
                // `t.setdefault(k, …)` through a name: whichever object `t` is has k now
                if let ast::Expr::Attribute(a) = &*c.func {
                    if let (ast::Expr::Name(nm), "setdefault", Some(first)) = (&*a.value, a.attr.id.as_str(), c.arguments.args.first()) {
                        if let ast::Expr::StringLiteral(sl) = first {
                            state.add_name_key(s(nm.id.as_str()), key_str_lit(sl.value.to_str()));
                        }
                    }
                }
                r
            }
            ast::Expr::Dict(d) => {
                let nd = frontend::node(node);
                let oid = self.alloc_at(Kind::Dict, &nd, state, &format!("dict@L{}", nd.line), Payload::None, None, Ns::Item, None, None);
                for item in &d.items {
                    let val = self.eval(&item.value, state);
                    match &item.key {
                        None => self.copy_into(oid, Ns::Item, &val, state, true, &intern::KeySet::new()),
                        Some(k) => {
                            let kv = self.eval_key(k, state);
                            self.store_key(oid, Ns::Item, &kv.kv, &val, true, &nd, state);
                        }
                    }
                }
                Value::obj(oid)
            }
            ast::Expr::List(l) => self.container(node, Kind::List, &l.elts, state),
            ast::Expr::Tuple(t) => self.container(node, Kind::Tuple, &t.elts, state),
            ast::Expr::Set(st) => self.container(node, Kind::Set, &st.elts, state),
            ast::Expr::Lambda(l) => {
                let nd = frontend::node(node);
                self.make_function(super::FuncNode::Lambda(l), state, Some(&format!("lambda@L{}", nd.line)))
            }
            ast::Expr::Named(n) => {
                let v = self.eval(&n.value, state);
                self.bind_target(&n.target, &v, state);
                v
            }
            ast::Expr::If(i) => {
                let sp = self.split(&i.test, state.copy());
                let (mut a, mut b) = (sp.t, sp.f);
                // (a branch that raises gives no value)
                let va = if a.reachable { self.eval(&i.body, &mut a) } else { Value::new() };
                let vb = if b.reachable { self.eval(&i.orelse, &mut b) } else { Value::new() };
                let (va, vb) = (if a.reachable { va } else { Value::new() }, if b.reachable { vb } else { Value::new() });
                state.replace_with(a.join(&mut b));
                va.join(&vb)
            }
            ast::Expr::BoolOp(b) => {
                let (v, mut st, stops) = self.boolop(b, state.copy(), false);
                if let Some(mut s0) = stops {
                    st = st.join(&mut s0);
                }
                state.replace_with(st);
                v
            }
            ast::Expr::UnaryOp(u) => {
                let v = self.eval(&u.operand, state);
                if matches!(u.op, ast::UnaryOp::UAdd | ast::UnaryOp::USub) {
                    let nd = frontend::node(node);
                    if let Some(r) = crate::models::counter::unary(self, state, u.op == ast::UnaryOp::USub, &v, &nd) {
                        return r;
                    }
                }
                if u.op != ast::UnaryOp::Not {
                    if crate::models::torch::is_tensor(&v) {
                        return v;
                    }
                    if let Some(d) = crate::models::torch::dim_of_label(&v) {
                        return crate::models::torch::dim_value(if u.op == ast::UnaryOp::USub { crate::models::shape::scale(d, -1) } else { d });
                    }
                    if u.op == ast::UnaryOp::USub {
                        if let Some(r) = self.t_neg_named(&v, state) {
                            return r;
                        }
                    }
                }
                match u.op {
                    ast::UnaryOp::Not => match self.truth_of(&v, state, &frontend::node(node)) {
                        Some(b) => Value::lit(intern::key_bool(!b)),
                        None => Value::data("bool"),
                    },
                    ast::UnaryOp::USub => {
                        if let Some(k) = v.kv.as_lit().filter(|_| v.pts.is_empty()) {
                            if let Some(i) = intern::key_as_int(k) {
                                if !intern::with_key(k, |kk| matches!(kk, intern::Key::Bool(_))) {
                                    return Value::lit(key_int(-i));
                                }
                            }
                            if let Some(f) = intern::with_key(k, |kk| if let intern::Key::Float(b) = kk { Some(f64::from_bits(*b)) } else { None }) {
                                return Value::lit(key_float(-f));
                            }
                        }
                        if crate::models::pandas::is_seriesish(self, &v) {
                            return Value::with_kv(ObjSet::one(self.series), DATA);
                        }
                        Value::with_kv(v.pts.filter(|o| o == self.series), DATA)
                    }
                    _ if crate::models::pandas::is_seriesish(self, &v) => Value::with_kv(ObjSet::one(self.series), DATA),
                    _ => Value::with_kv(v.pts.filter(|o| o == self.series), DATA),
                }
            }
            ast::Expr::BinOp(b) => {
                let a = self.eval(&b.left, state);
                let c = self.eval(&b.right, state);
                let nd = frontend::node(node);
                let r = self.binop(b.op, &a, &c, &nd, state);
                // an unknown result is named by the expression (`1 + b`)
                if r.pts.is_empty() && matches!(r.kv, KeyVal::Data(l) if !intern::str_get(l).starts_with('\u{1}')) {
                    let text = frontend::text_at(&nd).split_whitespace().collect::<Vec<_>>().join(" ");
                    if !text.is_empty() && text.len() <= 60 {
                        // the expression names the value only inside this function (its names)
                        let scope = self.frame().func.unwrap_or(0);
                        return Value::data(&format!("expr:{}:{}", scope, text));
                    }
                }
                r
            }
            ast::Expr::Compare(c) => {
                // (each comparator is evaluated once, as Python does; a false link of a chain ends it)
                let nd = frontend::node(node);
                let mut left = self.eval(&c.left, state);
                let mut all_true = true;
                for (op, cmp) in c.ops.iter().zip(c.comparators.iter()) {
                    let b = self.eval(cmp, state);
                    let r = self.compare(*op, &left, &b, cmp, &nd, state);
                    if c.ops.len() == 1 {
                        return r;
                    }
                    match r.kv.as_lit().filter(|_| r.pts.is_empty()) {
                        Some(k) if k == intern::key_bool(false) => return r,
                        Some(k) if k == intern::key_bool(true) => {}
                        _ if crate::models::pandas::is_seriesish(self, &r) => return r,
                        _ => all_true = false,
                    }
                    left = b;
                }
                if all_true { Value::lit(intern::key_bool(true)) } else { Value::data("bool") }
            }
            ast::Expr::FString(f) => self.expr_fstring(f, node, state),
            ast::Expr::TString(_) => {
                let nd = frontend::node(node);
                Value::data(&format!("tstring@L{}", nd.line))
            }
            ast::Expr::Starred(st) => self.eval(&st.value, state),
            ast::Expr::Await(a) => self.eval(&a.value, state),
            ast::Expr::Yield(y) => {
                let v = match &y.value {
                    Some(v) => self.eval(v, state),
                    None => self.none_v.clone(),
                };
                // (the with body runs at every yield the generator reaches: one per path)
                if self.cm_hook.as_ref().map(|h| (!h.ran || (h.multi)) && self.frames.len() == h.depth + 1).unwrap_or(false) {
                    self.run_cm_body(v, state);
                    return self.none_v.clone();
                }
                // (a bare `yield` yields None)
                self.yield_value(v, state, (y as *const _ as usize, 0));
                // the value of the `yield` expression: what `send` passes, None when iterated
                if crate::frontend::any_send_call() {
                    Value::data("yield")
                } else {
                    self.none_v.clone()
                }
            }
            ast::Expr::YieldFrom(y) => {
                let v = self.eval(&y.value, state);
                let nd = frontend::node(&*y.value);
                let it = self.iterate(&v, &nd, state);
                match it.items {
                    // known items: yielded one by one, in order
                    Some(items) => {
                        for (i, item) in items.into_iter().enumerate() {
                            self.yield_value(item, state, (y as *const _ as usize, i + 1));
                        }
                    }
                    _ => {
                        // an unknown number of items: positions after this are unknown
                        if let Some(gid) = self.yield_value(it.elem.flat(), state, (y as *const _ as usize, 0)) {
                            self.set_llen(gid, state, None);
                            state.drop_obj_ns(gid, Ns::Pos);
                        }
                    }
                }
                // the value: the delegated generator's return value
                let rk = key_str_lit("\u{1}genret");
                if !v.pts.is_empty() && v.kv.is_bot() && v.pts.iter().all(|o| state.slot_opt(o, Ns::Attr, rk).is_some()) {
                    return crate::domain::join_values(v.pts.iter().map(|o| state.slot(o, Ns::Attr, rk)).collect::<Vec<_>>());
                }
                Value::data("yield")
            }
            ast::Expr::Slice(_) => self.eval_key(node, state),
            ast::Expr::DictComp(dc) => self.expr_dictcomp(dc, node, state),
            ast::Expr::ListComp(lc) => self.seq_comp(node, Kind::List, &lc.elt, &lc.generators, state),
            ast::Expr::SetComp(sc) => self.seq_comp(node, Kind::Set, &sc.elt, &sc.generators, state),
            ast::Expr::Generator(g) => {
                let v = self.seq_comp(node, Kind::List, &g.elt, &g.generators, state);
                if let Some(o) = v.single() {
                    self.obj_mut(o).iterator = true;
                }
                v
            }
            ast::Expr::IpyEscapeCommand(_) => {
                let nd = frontend::node(node);
                self.note(format!("unsupported expression IpyEscapeCommand at L{}", nd.line));
                Value::data(&format!("L{}", nd.line))
            }
        }
    }

    /// At a `@contextmanager` generator's yield: the with body runs in the caller's frame with
    /// the target bound to the yielded value, then the generator goes on.
    fn run_cm_body(&mut self, v: Value, state: &mut State) {
        let Some(mut h) = self.cm_hook.take() else { return };
        let callee_fr = self.frames.pop().unwrap();
        let caller_env = match &h.env_before {
            Some(e) => e.clone(),
            None => {
                let e = self.env_stack.last().cloned().unwrap_or_default();
                h.env_before = Some(e.clone());
                e
            }
        };
        let callee_env = std::mem::replace(&mut state.env, caller_env);
        let callee_ret = state.ret.take();
        let callee_exits = state.exits.take();
        if let Some(t) = h.target {
            self.bind_target(t, &v, state);
        }
        self.cm_frames.push((self.frames.len() + 1, callee_env.clone(), self.env_stack.len()));
        match h.then.take() {
            Some(f) => f(self, v, state),
            None => self.exec_block(h.body, state),
        }
        h.body_died = !state.reachable && (!h.ran || h.body_died);
        // (the body's `return`s are the caller's: kept for when the with statement ends)
        if let Some(ex) = state.exits.take() {
            let mut ex = ex.copy();
            h.body_exits = Some(match h.body_exits.take() {
                Some(mut prev) => prev.join(&mut ex),
                None => ex,
            });
        }
        if let Some(r) = state.ret.take() {
            h.body_ret = Some(match h.body_ret.take() {
                Some(prev) => prev.join(&r),
                None => r,
            });
        }
        self.cm_frames.pop();
        let after = std::mem::replace(&mut state.env, callee_env);
        // (another yield's run: the caller's names after the with are either run's)
        let after = match h.env_after.take() {
            Some(prev) if h.ran => crate::state::join_env(&prev, &after),
            _ => after,
        };
        if let Some(top) = self.env_stack.last_mut() {
            *top = after.clone();
        }
        h.env_after = Some(after);
        h.ran = true;
        state.ret = callee_ret;
        state.exits = callee_exits;
        self.frames.push(callee_fr);
        self.cm_hook = Some(h);
    }

    /// After a yield whose consumer runs code before it asks for the next item: what the
    /// generator was given and has yielded may have changed (for the generator only: the
    /// consumer's own run of that code is analysed where it is).
    fn consumer_between(&mut self, gid: ObjId, state: &mut State) {
        let Some(changes) = self.gen_havoc.get(&gid).cloned().filter(|c| !c.is_empty()) else { return };
        let site = intern::s(&format!("consumer@{}", self.obj(gid).label.trim_start_matches("generator ")));
        let rid = self.any_object(state);
        // (an object that code changed in a way not one key: any of its facts, the objects
        // it holds aside: those it changed are listed too)
        let whole: Vec<ObjId> = changes.iter().filter(|c| c.1.is_none()).map(|c| c.0).collect();
        self.change_objects(&whole, rid, site, state);
        // (a key that code stored: any value, maybe stored)
        for (o, sk) in changes.iter().filter_map(|(o, k)| k.map(|k| (*o, k))) {
            let v = state.slot(o, sk.0, sk.1).join(&Value::obj(rid));
            state.set_slot(o, sk.0, sk.1, v, false);
            let r = state.row(o, sk.0).store(sk.1, false);
            state.set_row(o, sk.0, r);
        }
    }

    /// Code `site` may have changed these objects in any way (not the objects they hold): any
    /// key, any value (`rid`), any length.
    pub fn change_objects(&mut self, objs: &[ObjId], rid: ObjId, site: intern::StrId, state: &mut State) {
        for &o in objs {
            for ns in [Ns::Item, Ns::Attr] {
                if state.has_row(o, ns) {
                    let r = state.row(o, ns).mutate(site);
                    state.set_row(o, ns, r);
                }
            }
            for ((ns, k), v) in state.slots_of(o) {
                state.set_slot(o, ns, k, v.join(&Value::obj(rid)), false);
            }
            if state.elems.get(&o).is_some() {
                state.elems.insert(o, DATA);
            }
            self.set_llen(o, state, None);
        }
    }

    /// `site`: the yield (and the item of a `yield from`), which with the unrolled loop
    /// positions names the suspension point.
    fn yield_value(&mut self, v: Value, state: &mut State, site: (usize, usize)) -> Option<ObjId> {
        let foid = self.frame().func?;
        let ctx = self.frame().ctx.clone();
        let label = format!("generator {}", self.frame().label);
        let gid = self.alloc_raw(Kind::List, &format!("gen:{}", foid), &ctx, true, &label, Payload::None);
        if let Some(r) = self.gen_reads.iter_mut().rev().find(|r| r.0 == gid) {
            r.1 = true;
        }
        // a fresh generator list (no yield yet in this activation) starts empty
        let fresh = !state.has_slot(gid, Ns::Elem, STAR);
        // (lengths that were one of a few: one more each, after this item)
        let alts_before = if fresh { 0 } else { state.lens.get(&gid).filter(|l| l.0.is_none()).map(|l| l.2).unwrap_or(0) };
        state.set_slot(gid, Ns::Elem, STAR, v.clone(), false);
        let cur = state.elems.get(&gid).unwrap_or_else(|| v.kv.clone());
        state.elems.insert(gid, kv_join(&cur, &v.kv));
        // straight-line yields keep their order; in a branch too (a length per path, joined
        // to the shorter known prefix, like appends)
        let in_branch = self.cond_depth > 0;
        let positional = self.frame().single && self.frame().loop_depth == 0 && self.obj(gid).single;
        let ll = if fresh { Some(0) } else { self.llen(gid, state) };
        match (positional, ll) {
            (true, Some(n)) if n < 16 => {
                state.set_slot(gid, Ns::Pos, key_int(n as i64), v, true);
                self.set_llen(gid, state, Some(n + 1));
                // (the state at this item: what a consumer that took it sees of the effects)
                // (in a branch: another path may have its own item n — the consumer then
                // takes the generator's joined state there)
                let conflicted = self.gen_pos_conflict.contains(&(gid, n));
                let mut conflict_now = false;
                let ps = self.gen_pos_states.entry(gid).or_default();
                while ps.len() <= n {
                    ps.push(None);
                }
                ps[n] = match (in_branch, ps[n].take(), conflicted) {
                    (_, _, true) => None,
                    (true, Some(_), false) => {
                        conflict_now = true;
                        None
                    }
                    _ => Some(state.copy()),
                };
                if conflict_now {
                    self.gen_pos_conflict.insert((gid, n));
                }
            }
            // (the yields so far keep their positions: a known prefix)
            (_, Some(n)) if n > 0 => {
                self.set_prefix(gid, state, n);
            }
            _ if self.prefix(gid, state).is_some() => {}
            _ => {
                self.set_llen(gid, state, None);
                state.drop_obj_ns(gid, Ns::Pos);
            }
        }
        if alts_before != 0 {
            if let Some((None, p, 0)) = state.lens.get(&gid) {
                let shifted = (alts_before as u32) << 1;
                if shifted <= 0xFFFF {
                    state.lens.insert(gid, (None, p, shifted as u16));
                }
            }
        }
        const MAX_YIELD_STATES: usize = 4096;
        let key = (site.0, site.1, self.frame().unroll.clone());
        // (in a loop's fixpoint a later round's state at a suspension point covers the
        // earlier rounds', as the loop's exit does)
        let in_fixpoint = self.frame().loop_depth > 0;
        let here = state.copy();
        let ys = self.gen_yield_states.entry(gid).or_default();
        if let Some(slot) = ys.get_mut(&key).filter(|_| in_fixpoint) {
            *slot = here;
        } else if ys.len() < MAX_YIELD_STATES {
            ys.push(key, here);
        } else if let Some(last) = ys.list.last_mut() {
            // (past the cap: the last one stands for the rest too)
            let mut h = here;
            last.1 = last.1.join(&mut h);
        }
        self.consumer_between(gid, state);
        Some(gid)
    }

    pub fn eval_key(&mut self, node: &'static ast::Expr, state: &mut State) -> Value {
        match node {
            ast::Expr::Tuple(t) => {
                let parts: Vec<Value> = t.elts.iter().map(|e| self.eval(e, state)).collect();
                if parts.iter().all(|p| p.kv.as_lit().is_some()) {
                    return Value::lit(intern::key_tuple(parts.iter().map(|p| p.kv.as_lit().unwrap()).collect()));
                }
                let nd = frontend::node(node);
                let tid = self.alloc_at(Kind::Tuple, &nd, state, "tuple", Payload::None, None, Ns::Item, None, None);
                self.set_llen(tid, state, Some(parts.len()));
                for (i, p) in parts.iter().enumerate() {
                    state.set_slot(tid, Ns::Pos, key_int(i as i64), p.clone(), true);
                }
                Value::obj(tid)
            }
            ast::Expr::Slice(sl) => {
                let nd = frontend::node(node);
                // known bounds are kept in the label (`slice[1:3:1]@L5`; a missing bound is empty)
                let mut b: Vec<String> = Vec::new();
                for e in [&sl.lower, &sl.upper, &sl.step] {
                    match e.as_deref() {
                        None => b.push(String::new()),
                        Some(e) => match self.int_value(e, state) {
                            Some(i) => b.push(i.to_string()),
                            None => return Value::data(&format!("slice@L{}", nd.line)),
                        },
                    }
                }
                Value::data(&format!("slice[{}]@L{}", b.join(":"), nd.line))
            }
            _ => {
                let v = self.eval(node, state);
                self.keyify_in(&v, state)
            }
        }
    }

    fn container(&mut self, node: &'static ast::Expr, kind: Kind, elts: &'static [ast::Expr], state: &mut State) -> Value {
        let nd = frontend::node(node);
        let oid = self.alloc_at(kind, &nd, state, &format!("{}@L{}", kind.name(), nd.line), Payload::None, None, Ns::Item, None, None);
        let single = self.obj(oid).single;
        let mut vals: Vec<Value> = Vec::new();
        let mut literal = true;
        let mut size_parts: Option<Vec<Result<Value, Value>>> = if self.torch_on && elts.iter().any(|e| matches!(e, ast::Expr::Starred(_))) { Some(Vec::new()) } else { None };
        for e in elts {
            if let ast::Expr::Starred(st) = e {
                literal = false;
                let inner = self.eval(&st.value, state);
                if let Some(p) = size_parts.as_mut() {
                    p.push(Err(inner.clone()));
                }
                let en = frontend::node(e);
                let it = self.iterate(&inner, &en, state);
                vals.push(self.elem_value(&it.elem, &en, "star"));
            } else {
                let v = self.eval(e, state);
                if let Some(p) = size_parts.as_mut() {
                    p.push(Ok(v.clone()));
                }
                vals.push(v);
            }
        }
        // `(*x.shape[:-1], n)`: a size
        if let Some(parts) = size_parts {
            if let Some(v) = self.t_size_from_parts(&parts, &nd, state) {
                return v;
            }
        }
        let mut acc = Value::new();
        let mut kvacc: Option<KeyVal> = None;
        for v in &vals {
            acc = acc.join(&Value::with_kv(v.pts.clone(), DATA));
            kvacc = Some(match kvacc {
                None => v.kv.clone(),
                Some(k) => kv_join(&k, &v.kv),
            });
        }
        state.set_slot(oid, Ns::Elem, STAR, Value::with_kv(acc.pts, crate::models::torch::elem_kv(&acc.kv)), single);
        if !vals.is_empty() {
            let kvacc = kvacc.unwrap();
            let nv = match state.elems.get(&oid) {
                Some(cur) if !single => kv_join(&cur, &kvacc),
                _ => kvacc,
            };
            state.elems.insert(oid, nv);
        } else {
            state.elems.insert(oid, KeyVal::Lits(intern::KeySet::new()));
        }
        if literal && (single || self.llen(oid, state) == Some(vals.len()) || !state.has_slot(oid, Ns::Elem, STAR) || state.slot(oid, Ns::Elem, STAR).pts.is_empty()) {
            // positions are structural: the same literal always has the same length, so a
            // weak (loop) allocation keeps per-position slots, joined across allocations
            self.set_llen(oid, state, Some(vals.len()));
            for (i, v) in vals.iter().enumerate() {
                state.set_slot(oid, Ns::Pos, key_int(i as i64), v.clone(), single);
            }
        } else {
            self.set_llen(oid, state, None);
            state.drop_obj_ns(oid, Ns::Pos);
        }
        if kind == Kind::Set {
            self.normalize_set(oid, state);
        }
        Value::obj(oid)
    }

    /// `kv_map` that also records a narrowing event when the key is a parameter: the renamed
    /// key is only known once the parameter is bound to a literal (at the call site).
    pub fn rename_kv(&mut self, kv: &KeyVal, f: &RenameFn) -> KeyVal {
        kv_map(kv, f)
    }

    /// `xs[lo:hi]` with constant bounds on one list/tuple with known positions: a container
    /// with the selected positions.
    /// The single known int an expression evaluates to.
    pub fn int_value(&mut self, e: &'static ast::Expr, state: &mut State) -> Option<i64> {
        let v = self.eval(e, state);
        if !v.pts.is_empty() {
            return None;
        }
        intern::key_as_int(v.kv.as_lit()?)
    }

    /// `xs[lo:hi:step]` of one list / tuple of known length: its positions.
    fn slice_positional(&mut self, obj: &Value, (lo, hi, step): (Option<i64>, Option<i64>, Option<i64>), node: &'static ast::Expr, state: &mut State) -> Option<Value> {
        let step = step.unwrap_or(1);
        let oid = obj.single().filter(|o| step != 0 && matches!(self.kind(*o), Kind::List | Kind::Tuple))?;
        let n = self.llen(oid, state)? as i64;
        let vals: Vec<Value> = slice_positions(n, lo, hi, step).iter().map(|i| state.slot(oid, Ns::Pos, key_int(*i))).collect();
        let kv = state.elems.get(&oid).unwrap_or(DATA);
        let kind = self.kind(oid);
        Some(self.alloc_positional(kind, &frontend::node(node), Some(vals), Value::new(), kv, state))
    }

    /// The label of a slice with known bounds (`slice[1:3:]@L5`): its bounds (None: omitted).
    pub fn slice_label_bounds(kv: &KeyVal) -> Option<(Option<i64>, Option<i64>, Option<i64>)> {
        let KeyVal::Data(d) = kv else { return None };
        let t = intern::str_get(*d);
        let inner = t.strip_prefix("slice[")?.split(']').next()?.to_string();
        let parts: Vec<&str> = inner.split(':').collect();
        if parts.len() != 3 {
            return None;
        }
        let p = |x: &str| -> Option<Option<i64>> { if x.is_empty() { Some(None) } else { x.parse::<i64>().ok().map(Some) } };
        Some((p(parts[0])?, p(parts[1])?, p(parts[2])?))
    }

    /// A fresh list / tuple at `nd`: with `vals` its positions are known (while the allocation
    /// is single), otherwise its elements are `elem`; `kv` is its element key-value.
    pub fn alloc_positional(&mut self, kind: Kind, nd: &Node, vals: Option<Vec<Value>>, elem: Value, kv: KeyVal, state: &mut State) -> Value {
        self.alloc_positional_sfx(kind, nd, "", vals, elem, kv, state)
    }

    /// `alloc_positional` at the node's site plus `sfx` (several objects one model call makes
    /// at one node, e.g. the pairs of `most_common`: `#0`, `#1`, …).
    #[allow(clippy::too_many_arguments)]
    pub fn alloc_positional_sfx(&mut self, kind: Kind, nd: &Node, sfx: &str, vals: Option<Vec<Value>>, elem: Value, kv: KeyVal, state: &mut State) -> Value {
        let site = format!("{}{}", crate::interp::site_string(nd), sfx);
        let nid = self.alloc(kind, &site, state, &format!("{}@L{}", kind.name(), nd.line), Payload::None, None, Ns::Item, None, None);
        let single = self.obj(nid).single;
        let acc = match &vals {
            Some(vs) => crate::domain::join_values(vs.clone()),
            None => elem,
        };
        state.set_slot(nid, Ns::Elem, STAR, Value::with_kv(acc.pts, crate::models::torch::elem_kv(&acc.kv)), single);
        state.elems.insert(nid, kv);
        match vals {
            Some(vs) if single => {
                self.set_llen(nid, state, Some(vs.len()));
                for (i, v) in vs.iter().enumerate() {
                    state.set_slot(nid, Ns::Pos, key_int(i as i64), v.clone(), true);
                }
            }
            _ => self.set_llen(nid, state, None),
        }
        Value::obj(nid)
    }

    /// `a op b` for one comparison operator (`right`: b as written).
    fn compare(&mut self, op: ast::CmpOp, a: &Value, b: &Value, right: &'static ast::Expr, nd: &Node, state: &mut State) -> Value {
        let lit = |t: bool| Value::lit(intern::key_bool(t));
        // a tensor is never None
        if matches!(op, ast::CmpOp::Is | ast::CmpOp::IsNot) && matches!(right, ast::Expr::NoneLiteral(_)) && crate::models::torch::is_tensor(a) && a.pts.is_empty() {
            return lit(op == ast::CmpOp::IsNot);
        }
        let ordering = !matches!(op, ast::CmpOp::In | ast::CmpOp::NotIn | ast::CmpOp::Is | ast::CmpOp::IsNot);
        if ordering && crate::models::torch::is_tensor(a) {
            return self.t_compare(a, b, nd, state);
        }
        // a named unknown against a number / tensor
        if ordering && self.torch_on && a.kv.is_bot() && a.pts.len() == 1 && a.pts.iter().all(|o| self.kind(o) == Kind::Unknown) {
            if let Some(r) = self.t_compare_named(a, b, nd, state) {
                return r;
            }
            return self.compare_const(op, a, b, right, state).map(lit).unwrap_or_else(|| Value::data("bool"));
        }
        if ordering {
            // library values (paths, Decimal, datetime); instances: their comparison methods
            // (`__lt__`, reflected, @total_ordering); tuples item by item
            if let Some(r) = crate::models::library_compare(self, op, a, b) {
                return r;
            }
            if let Some(r) = self.rich_compare(op, a, b, nd, state) {
                return r;
            }
            if let Some(o) = self.tuple_order(a, b, state) {
                use std::cmp::Ordering::*;
                return lit(match op {
                    ast::CmpOp::Lt => o == Less,
                    ast::CmpOp::LtE => o != Greater,
                    ast::CmpOp::Gt => o == Greater,
                    ast::CmpOp::GtE => o != Less,
                    ast::CmpOp::Eq => o == Equal,
                    _ => o != Equal,
                });
            }
        }
        if let Some(r) = self.compare_const(op, a, b, right, state) {
            return lit(r);
        }
        if crate::models::pandas::is_seriesish(self, a) || crate::models::pandas::is_seriesish(self, b) {
            return Value::with_kv(ObjSet::one(self.series), DATA);
        }
        // (tuples holding parameters: decided per call site)
        let both = a.pts.union(&b.pts);
        if both.iter().any(|o| self.kind(o) == Kind::Frame) {
            return Value::with_kv(both.filter(|o| self.kind(o) == Kind::Frame), DATA);
        }
        Value::data("bool")
    }

    /// `a is None`, `a is not None`, `a == lit`, `a != lit` when the outcome is decidable
    /// from the value's literals: a value with only non-placeholder objects is never None.
    fn compare_const(&mut self, op: ast::CmpOp, a: &Value, b: &Value, right: &'static ast::Expr, state: &mut State) -> Option<bool> {
        let none = intern::key_none();
        let lits = crate::domain::kv_literals(&a.kv);
        let has_ph = a.pts.iter().any(|o| self.kind(o) == Kind::Unknown);
        let known_lits = a.kv.is_literal() || a.kv.is_bot();
        // an unknown length against 0 (or a negative number): `len(xs) >= 0` holds
        if let (true, KeyVal::Data(d), ast::Expr::NumberLiteral(nl)) = (a.pts.is_empty(), &a.kv, right) {
            if &*intern::str_get(*d) == "len" {
                if let ast::Number::Int(i) = &nl.value {
                    if let Some(n) = i.as_i64() {
                        match op {
                            ast::CmpOp::GtE if n <= 0 => return Some(true),
                            ast::CmpOp::Gt if n < 0 => return Some(true),
                            ast::CmpOp::Lt if n <= 0 => return Some(false),
                            ast::CmpOp::LtE if n < 0 => return Some(false),
                            ast::CmpOp::Eq | ast::CmpOp::NotEq if n < 0 => return Some(op == ast::CmpOp::NotEq),
                            _ => {}
                        }
                    }
                }
            }
        }
        match op {
            ast::CmpOp::Is | ast::CmpOp::IsNot if matches!(right, ast::Expr::NoneLiteral(_)) => {
                let is_none = if a.pts.is_empty() && a.kv.is_literal() && lits.len() == 1 && lits.contains(none) {
                    true
                } else if !has_ph && known_lits && !lits.contains(none) && (!a.pts.is_empty() || !lits.is_empty()) {
                    false
                } else {
                    // undecidable here: decided per call site when the operand is concrete there
                    return None;
                };
                Some(if op == ast::CmpOp::Is { is_none } else { !is_none })
            }
            // identity of objects: distinct abstract objects are distinct objects; one strong
            // object is itself
            ast::CmpOp::Is | ast::CmpOp::IsNot if !a.pts.is_empty() && a.kv.is_bot() => {
                if b.pts.is_empty() || !b.kv.is_bot() {
                    return None;
                }
                // builtin classes (`type(x) is dict`): the same class exactly when the same name
                let builtin_q = |an: &Analyzer, v: &Value| -> Option<Vec<String>> { v.pts.iter().map(|o| if an.kind(o) == Kind::External { Some(an.external_qual(o)).filter(|q| q.starts_with("builtins.")) } else { None }).collect() };
                if let (Some(qa), Some(qb)) = (builtin_q(self, &a), builtin_q(self, &b)) {
                    if qb.len() == 1 {
                        let same_all = qa.iter().all(|q| *q == qb[0]);
                        let diff_all = qa.iter().all(|q| *q != qb[0]);
                        if same_all || diff_all {
                            let same = same_all;
                            return Some(if op == ast::CmpOp::Is { same } else { !same });
                        }
                    }
                }
                let opaque = |an: &Analyzer, v: &Value| v.pts.iter().any(|o| matches!(an.kind(o), Kind::Unknown | Kind::External | Kind::Class | Kind::Module | Kind::Function));
                if opaque(self, &a) || opaque(self, &b) {
                    return None;
                }
                let same = if a.pts.intersection(&b.pts).is_empty() {
                    false
                } else if a.pts.len() == 1 && a.pts == b.pts && self.obj(a.pts.first().unwrap()).single {
                    true
                } else {
                    return None;
                };
                Some(if op == ast::CmpOp::Is { same } else { !same })
            }
            ast::CmpOp::In | ast::CmpOp::NotIn => {
                let decide = |found: bool| Some(if op == ast::CmpOp::In { found } else { !found });
                // members of a Flag: bits; else a member is its key
                if (!self.member_key.is_empty() || !self.identity_objs.borrow().is_empty()) && !a.pts.is_empty() {
                    if let Some(r) = self.flag_contains(a, &b, state) {
                        return decide(r);
                    }
                    let ka = self.keyify_in(a, state);
                    if ka.pts.is_empty() {
                        return self.compare_const(op, &ka, b, right, state);
                    }
                }
                if !a.pts.is_empty() || !a.kv.is_literal() || lits.is_empty() {
                    return None;
                }
                // a range with known bounds: membership of a known int
                if let (Some(Payload::Range(lo, hi, step)), [k]) = (b.single().map(|o| &self.obj(o).payload), &lits.iter().collect::<Vec<_>>()[..]) {
                    let x = intern::key_as_int(*k)?;
                    let inside = if *step > 0 { x >= *lo && x < *hi } else { x <= *lo && x > *hi };
                    return decide(inside && (x - lo) % step == 0);
                }
                // literals in a positional container of known literals: decided when every one
                // is there, or none is (by Python's equality: `1.0 in [1, 2]`)
                if let Some(o) = b.single().filter(|o| matches!(self.kind(*o), Kind::List | Kind::Tuple | Kind::Set)) {
                    let n = self.llen(o, state)?;
                    let items: Vec<KeyId> = (0..n).map(|i| {
                        let sv = state.slot(o, Ns::Pos, key_int(i as i64));
                        // (Enum members / identity objects as their keys)
                        let sv = if sv.pts.is_empty() { sv } else { self.keyify_in(&sv, state) };
                        sv.kv.as_lit().filter(|_| sv.pts.is_empty())
                    }).collect::<Option<_>>()?;
                    let eqs: Vec<bool> = lits.iter().map(|k| items.iter().map(|x| lit_eq(*x, k)).collect::<Option<Vec<bool>>>().map(|v| v.contains(&true))).collect::<Option<_>>()?;
                    let hits = eqs.iter().filter(|b| **b).count();
                    return if hits == lits.len() || hits == 0 { decide(hits > 0) } else { None };
                }
                // a literal key in records whose rows decide it
                if lits.len() != 1 {
                    return None;
                }
                let k = lits.iter().next().unwrap();
                // a substring test on known strings
                if let (Some(needle), Some(hay)) = (intern::key_as_str(k), b.kv.as_lit().filter(|_| b.pts.is_empty()).and_then(intern::key_as_str)) {
                    return decide(hay.contains(&*needle));
                }
                if let Some(o) = b.single() {
                    match self.obj(o).payload.clone() {
                        // a view of a dict: its keys are the dict's (live); its values are decided
                        // when they are all known literals
                        Payload::View(ViewKind::Keys | ViewKind::Proxy, inner) => {
                            return self.compare_const(op, a, &inner, right, state);
                        }
                        Payload::View(ViewKind::Values, inner) => {
                            let d = inner.single().filter(|d| self.kind(*d) == Kind::Dict)?;
                            let items = self.literal_dict_items(d, state)?;
                            let vals: Vec<KeyId> = items.iter().map(|(_, v)| v.kv.as_lit().filter(|_| v.pts.is_empty())).collect::<Option<_>>()?;
                            return decide(vals.contains(&k));
                        }
                        _ => {}
                    }
                }
                let recs = self.record_objects(&b);
                if b.pts.is_empty() || recs.len() != b.pts.len() {
                    return None;
                }
                let rows: Vec<Row> = recs.iter().map(|o| state.row(*o, Ns::Item)).collect();
                if recs.iter().all(|o| matches!(self.kind(*o), Kind::Dict | Kind::Frame)) {
                    if rows.iter().all(|r| r.definite.contains(k)) {
                        return decide(true);
                    }
                    if rows.iter().all(|r| r.lacks(k)) {
                        return decide(false);
                    }
                    return None;
                }
                // removed (`del`, `pop`) or excluded on every path, whatever the container
                rows.iter().all(|r| r.absent.contains(k)).then_some(op != ast::CmpOp::In)
            }
            ast::CmpOp::Eq | ast::CmpOp::NotEq => {
                // Enum members: their keys (a mixed-in one: its value)
                let (a, b, lits) = if self.member_key.is_empty() {
                    (a.clone(), b.clone(), lits)
                } else {
                    let ka = self.member_base(&self.keyify(a));
                    let kb = self.member_base(&self.keyify(&b));
                    let l = crate::domain::kv_literals(&ka.kv);
                    (ka, kb, l)
                };
                let bl = crate::domain::kv_literals(&b.kv);
                // a builtin dict / list / tuple / set never equals a str / number / bool / None
                let container = |an: &Analyzer, v: &Value| !v.pts.is_empty() && v.pts.iter().all(|o| matches!(an.kind(o), Kind::Dict | Kind::List | Kind::Tuple | Kind::Set) && !crate::models::pandas::is_series_obj(an, o)) && matches!(v.kv, KeyVal::Bot);
                let scalar = |v: &Value| v.pts.is_empty() && v.kv.is_literal() && !crate::domain::kv_literals(&v.kv).iter().any(|k| intern::with_key(k, |kk| matches!(kk, intern::Key::Tuple(_))));
                if (container(self, &a) && scalar(&b)) || (container(self, &b) && scalar(&a)) {
                    return Some(op == ast::CmpOp::NotEq);
                }
                // (by Python's equality: `1 == 1.0 == True`; several literals decide when every
                // pair agrees)
                let decided = if a.pts.is_empty() && b.pts.is_empty() && a.kv.is_literal() && b.kv.is_literal() {
                    let eqs: Option<Vec<bool>> = lits.iter().flat_map(|x| bl.iter().map(move |y| lit_eq(x, y))).collect();
                    eqs.filter(|e| !e.is_empty() && e.iter().all(|x| *x == e[0])).map(|e| e[0])
                } else {
                    None
                };
                let Some(eq) = decided else {
                    return None;
                };
                Some(if op == ast::CmpOp::Eq { eq } else { !eq })
            }
            ast::CmpOp::Lt | ast::CmpOp::LtE | ast::CmpOp::Gt | ast::CmpOp::GtE => {
                // two single literals of one kind (numbers, or strings) are ordered here;
                // anything else is decided per call site when it is concrete there
                let bl = crate::domain::kv_literals(&b.kv);
                if !a.pts.is_empty() || !b.pts.is_empty() || !a.kv.is_literal() || !b.kv.is_literal() || lits.len() != 1 || bl.len() != 1 {
                    return None;
                }
                let x = lits.iter().next().unwrap();
                let y = bl.iter().next().unwrap();
                let ord = literal_order(x, y)?;
                Some(match op {
                    ast::CmpOp::Lt => ord == std::cmp::Ordering::Less,
                    ast::CmpOp::LtE => ord != std::cmp::Ordering::Greater,
                    ast::CmpOp::Gt => ord == std::cmp::Ordering::Greater,
                    _ => ord != std::cmp::Ordering::Less,
                })
            }
            _ => None,
        }
    }

    /// True/False when a value is definitely truthy/falsy, else None.
    /// The members of a key view / set / frozenset when they are all known: a keys view of a
    /// closed dict, a set built from known literals.
    pub fn known_members(&self, v: &Value, state: &State) -> Option<intern::KeySet> {
        let o = v.single()?;
        match (self.kind(o), &self.obj(o).payload) {
            (Kind::View, Payload::View(ViewKind::Keys | ViewKind::Proxy, inner)) => {
                let d = inner.single().filter(|d| self.kind(*d) == Kind::Dict)?;
                let row = state.row(d, Ns::Item);
                (row.closed() && row.present == row.definite).then(|| row.present.clone())
            }
            (Kind::Set | Kind::List | Kind::Tuple, _) => {
                let n = self.llen(o, state)?;
                let ks: Option<Vec<KeyId>> = (0..n).map(|i| {
                    let s = state.slot(o, Ns::Pos, key_int(i as i64));
                    s.kv.as_lit().filter(|_| s.pts.is_empty())
                }).collect();
                ks.map(|k| k.into_iter().collect())
            }
            _ => None,
        }
    }

    /// A set keeps known positions only for distinct literal members, sorted (Python's
    /// iteration order is not known; membership and size are): anything else forgets them.
    pub fn normalize_set(&mut self, oid: u32, state: &mut State) {
        if self.kind(oid) != Kind::Set {
            return;
        }
        let Some(n) = self.llen(oid, state) else { return };
        let mut ks: Vec<KeyId> = Vec::with_capacity(n);
        for i in 0..n {
            let v = state.slot(oid, Ns::Pos, key_int(i as i64));
            // an object hashing by value / identity: its key
            let v = if !v.pts.is_empty() && v.kv.is_bot() && v.pts.len() == 1 {
                let kv = self.keyify_in(&v, state);
                if kv.pts.is_empty() { kv } else { v }
            } else {
                v
            };
            match (v.pts.is_empty(), v.kv.as_lit()) {
                (true, Some(k)) => ks.push(k),
                _ => {
                    self.set_llen(oid, state, None);
                    state.drop_obj_ns(oid, Ns::Pos);
                    return;
                }
            }
        }
        self.set_members(oid, intern::KeySet::from_iter(ks), state);
    }

    /// Set `oid`'s members to exactly `ks` (literal keys).
    pub fn set_members(&mut self, oid: u32, ks: intern::KeySet, state: &mut State) {
        state.drop_obj_ns(oid, Ns::Pos);
        let sorted = intern::sorted_by_str(ks.iter());
        for (i, k) in sorted.iter().enumerate() {
            state.set_slot(oid, Ns::Pos, key_int(i as i64), Value::lit(*k), true);
        }
        self.set_llen(oid, state, Some(sorted.len()));
        state.set_slot(oid, Ns::Elem, STAR, Value::kv_only(crate::models::torch::elem_kv(&KeyVal::lits(ks.clone()))), true);
        state.elems.insert(oid, if ks.is_empty() { KeyVal::Lits(intern::KeySet::new()) } else { KeyVal::lits(ks) });
    }

    /// A new set with exactly the literal members `ks`.
    pub fn literal_set(&mut self, nd: &Node, ks: intern::KeySet, state: &mut State) -> Value {
        let vals: Vec<Value> = intern::sorted_by_str(ks.iter()).into_iter().map(Value::lit).collect();
        let v = self.alloc_positional(Kind::Set, nd, Some(vals), Value::new(), if ks.is_empty() { KeyVal::Lits(intern::KeySet::new()) } else { KeyVal::lits(ks) }, state);
        if let Some(o) = v.single() {
            self.normalize_set(o, state);
        }
        v
    }

    /// The known positions of a list / tuple that can be updated in place (one strong object
    /// outside loops), else None.
    pub fn known_positions(&self, oid: u32, state: &State) -> Option<Vec<Value>> {
        if !matches!(self.kind(oid), Kind::List | Kind::Tuple) || !self.obj(oid).single || self.frame().loop_depth > 0 {
            return None;
        }
        let n = self.llen(oid, state)?;
        Some((0..n).map(|i| state.slot(oid, Ns::Pos, key_int(i as i64))).collect())
    }

    /// The positions of `oid` become `vals`.
    pub fn set_positions(&mut self, oid: u32, vals: Vec<Value>, state: &mut State) {
        let old = self.llen(oid, state).unwrap_or(0);
        for i in vals.len()..old {
            state.pop_slot(oid, Ns::Pos, key_int(i as i64));
        }
        for (i, v) in vals.iter().enumerate() {
            state.set_slot(oid, Ns::Pos, key_int(i as i64), v.clone(), true);
        }
        let acc = crate::domain::join_values(vals.clone());
        state.set_slot(oid, Ns::Elem, STAR, Value::with_kv(acc.pts.clone(), crate::models::torch::elem_kv(&acc.kv)), true);
        self.set_llen(oid, state, Some(vals.len()));
    }

    /// A one-shot iterator iterated to its end: later iterations see nothing.
    /// A lazy iterator consumed (to the end, or partly: `full` false): what it takes from its
    /// sources.
    pub fn consume_lazy(&mut self, v: &Value, full: bool, state: &mut State) {
        for lo in v.pts.iter() {
            let Some(srcs) = self.lazy_src.get(&lo).cloned() else { continue };
            // (consumed once on a path: a one-shot iterator itself)
            let done = key_str_lit("\u{1}consumed");
            if state.has_slot(lo, Ns::Attr, done) {
                continue;
            }
            state.set_slot(lo, Ns::Attr, done, Value::lit(intern::key_bool(true)), true);
            for (src, n) in srcs {
                match (full, n, self.cursor_of(src, state)) {
                    (true, Some(n), Some(c)) => {
                        let end = match self.llen(src, state) {
                            Some(len) => (c + n).min(len),
                            None => c + n,
                        };
                        self.set_cursor(src, Some(end), state);
                    }
                    (true, None, _) => {
                        self.exhaust(&Value::obj(src), state);
                    }
                    _ => {
                        self.set_cursor(src, None, state);
                    }
                }
            }
        }
    }

    /// Move a one-shot iterator's cursor (None: not known). A generator with tracked effects
    /// gets the facts of the objects it changes as they are after that many items — while
    /// they still are what was installed last (the consumer did not change them meanwhile).
    pub fn set_cursor(&mut self, o: u32, c: Option<usize>, state: &mut State) {
        self.set_cursor_ex(o, c, false, state)
    }

    /// `set_cursor`; `at_end`: used up (its length may not be known).
    pub fn set_cursor_ex(&mut self, o: u32, c: Option<usize>, at_end: bool, state: &mut State) {
        let cv = match c {
            Some(n) => Value::lit(key_int(n as i64)),
            None => Value::data(if at_end { "cursor-end" } else { "cursor" }),
        };
        state.set_slot(o, Ns::Attr, key_str_lit("__cursor"), cv, true);
        let Some(ge) = self.gen_eff.get(&o).cloned() else { return };
        let tk = key_str_lit("\u{1}effpos");
        let tag = state.slot_opt(o, Ns::Attr, tk).and_then(|v| v.kv.as_lit().filter(|_| v.pts.is_empty())).and_then(intern::key_as_str).map(|s| s.to_string());
        let expected: Option<&State> = match tag.as_deref() {
            Some("pre") => Some(&ge.pre),
            Some("join") => Some(&ge.join),
            Some("end") => ge.end.as_ref(),
            Some(t) => t.strip_prefix("at").and_then(|k| k.parse::<usize>().ok()).and_then(|k| ge.at.get(k)).and_then(|x| x.as_ref()),
            None => None,
        };
        let Some(expected) = expected else { return };
        if !gen_facts_equal(state, expected, &ge) {
            state.set_slot(o, Ns::Attr, tk, Value::data("effpos"), true);
            return;
        }
        let ended = at_end || matches!((c, self.llen(o, state)), (Some(k), Some(n)) if k >= n);
        let (target, ntag): (&State, String) = match c {
            Some(0) => (&ge.pre, "pre".into()),
            _ if ended && ge.end.is_some() => (ge.end.as_ref().unwrap(), "end".into()),
            Some(k) => match ge.at.get(k - 1).and_then(|x| x.as_ref()) {
                Some(s) => (s, format!("at{}", k - 1)),
                None => (&ge.join, "join".into()),
            },
            None => (&ge.join, "join".into()),
        };
        gen_copy_facts(state, target, &ge);
        state.set_slot(o, Ns::Attr, tk, Value::lit(key_str_lit(&ntag)), true);
    }

    /// How many items a one-shot iterator has given (None: not known).
    pub fn cursor_of(&self, o: u32, state: &State) -> Option<usize> {
        match state.slot_opt(o, Ns::Attr, key_str_lit("__cursor")) {
            None => Some(0),
            Some(v) => v.kv.as_lit().filter(|_| v.pts.is_empty()).and_then(intern::key_as_int).map(|i| i.max(0) as usize),
        }
    }

    pub fn exhaust(&mut self, v: &Value, state: &mut State) {
        self.consume_lazy(v, true, state);
        if let Some(o) = v.single() {
            if self.obj(o).iterator && self.obj(o).single {
                match self.llen(o, state) {
                    Some(n) => self.set_cursor(o, Some(n), state),
                    // (used up, its length not known: at its end all the same)
                    None => self.set_cursor_ex(o, None, true, state),
                }
            }
        }
    }

    /// An instance whose class (or a base) defines `__bool__` or `__len__`.
    /// An instance of a class deriving from a class rowan does not see (a builtin, a library
    /// class): the base may define what the class body does not.
    pub fn external_based(&self, oid: u32) -> bool {
        self.instance_class(oid).is_some_and(|c| self.mro(c).iter().any(|x| self.class_info_opt(*x).is_some_and(|ci| ci.open_base || ci.lib.dict_base)))
    }

    pub fn has_truth_dunder(&self, oid: u32) -> bool {
        match self.instance_class(oid) {
            Some(c) => self.class_lookup(c, "__bool__").is_some() || self.class_lookup(c, "__len__").is_some(),
            None => false,
        }
    }

    /// `truthiness`, with instances whose class defines `__bool__` / `__len__` decided by
    /// calling it (control-flow tests).
    pub fn truth_of(&mut self, v: &Value, state: &mut State, node: &Node) -> Option<bool> {
        if !v.pts.is_empty() && v.pts.iter().any(|o| self.kind(o) == Kind::Instance && self.has_truth_dunder(o)) {
            let mut verdicts: Vec<bool> = Vec::new();
            for o in v.pts.iter() {
                if self.kind(o) != Kind::Instance || !self.has_truth_dunder(o) {
                    verdicts.push(self.truthiness(&Value::obj(o), state)?);
                    continue;
                }
                let cls = self.instance_class(o).unwrap();
                let name = if self.class_lookup(cls, "__bool__").is_some() { "__bool__" } else { "__len__" };
                let r = self.call_method_with(&Value::obj(o), name, Vec::new(), node, state)?;
                verdicts.push(self.truthiness(&r, state)?);
            }
            verdicts.dedup();
            return if verdicts.len() == 1 { Some(verdicts[0]) } else { None };
        }
        self.truthiness(v, state)
    }

    pub fn truthiness(&mut self, v: &Value, state: &State) -> Option<bool> {
        if v.pts.is_empty() && v.kv.is_literal() {
            let lits = crate::domain::kv_literals(&v.kv);
            if !lits.is_empty() {
                // (an Enum member, an identity object: true)
                let truthy = |k: KeyId| crate::models::pyvalues::truthy(k).unwrap_or(true);
                let mut it = lits.iter();
                let first = truthy(it.next().unwrap());
                if it.all(|k| truthy(k) == first) {
                    return Some(first);
                }
                return None;
            }
        }
        if !v.kv.is_bot() || v.pts.is_empty() {
            return None;
        }
        let mut verdicts: Vec<bool> = Vec::new();
        for oid in v.pts.iter() {
            match self.kind(oid) {
                Kind::List | Kind::Tuple | Kind::Set if self.llen(oid, state).is_some() => {
                    verdicts.push(self.llen(oid, state).unwrap() > 0);
                }
                Kind::Dict => {
                    let row = state.row(oid, Ns::Item);
                    if !row.definite.is_empty() {
                        verdicts.push(true);
                    } else if row.closed() && row.present.is_empty() {
                        verdicts.push(false);
                    } else {
                        return None;
                    }
                }
                Kind::List | Kind::Tuple | Kind::Set => match self.llen(oid, state) {
                    None => return None,
                    Some(n) => verdicts.push(n > 0),
                },
                // (a class deriving from a builtin / library class — `class C(list)` — may have
                // its `__len__` / `__bool__`)
                Kind::Instance if self.has_truth_dunder(oid) || self.external_based(oid) => return None,
                Kind::Instance | Kind::Function | Kind::Bound | Kind::Class | Kind::Module | Kind::Namespace => verdicts.push(true),
                _ => return None,
            }
        }
        verdicts.dedup();
        if verdicts.len() == 1 {
            Some(verdicts[0])
        } else {
            None
        }
    }

    pub fn binop(&mut self, op: ast::Operator, a: &Value, b: &Value, node: &Node, state: &mut State) -> Value {
        if let Some(v) = self.binop_dunder(op, a, b, node, state) {
            return v;
        }
        if op == ast::Operator::Div {
            if let Some(v) = crate::models::pathtext::path_div(self, state, a, b, node) {
                return v;
            }
        }
        if let Some(v) = self.flag_binop(op, a, b, node, state) {
            return v;
        }
        if let Some(v) = self.t_binop(op, a, b, node, state) {
            return v;
        }
        // Counter arithmetic
        if let Some(ch) = match op {
            ast::Operator::Add => Some('+'),
            ast::Operator::Sub => Some('-'),
            ast::Operator::BitAnd => Some('&'),
            ast::Operator::BitOr => Some('|'),
            _ => None,
        } {
            if let Some(v) = crate::models::counter::binop(self, state, ch, a, b, node) {
                return v;
            }
        }
        let both = a.pts.union(&b.pts);
        // arithmetic on a parameter (or a field of one): the result is decided per call site
        // set operations on key views / sets with known members: a set of the result
        if matches!(op, ast::Operator::BitAnd | ast::Operator::BitOr | ast::Operator::Sub | ast::Operator::BitXor) {
            if let (Some(x), Some(y)) = (self.known_members(a, state), self.known_members(b, state)) {
                let r = match op {
                    ast::Operator::BitAnd => x.intersection(&y),
                    ast::Operator::BitOr => x.union(&y),
                    ast::Operator::Sub => x.difference(&y),
                    _ => x.difference(&y).union(&y.difference(&x)),
                };
                return self.literal_set(node, r, state);
            }
        }
        if op == ast::Operator::BitOr {
            // a parameter's `|` (a dict or a set): decided per call site
            let recs: Vec<u32> = both.iter().filter(|o| self.kind(*o) == Kind::Dict).collect();
            if !recs.is_empty() {
                let oid = self.alloc_at(Kind::Dict, node, state, &format!("dict@L{}", node.line), Payload::None, None, Ns::Item, None, None);
                self.copy_into(oid, Ns::Item, a, state, true, &intern::KeySet::new());
                self.copy_into(oid, Ns::Item, b, state, true, &intern::KeySet::new());
                return Value::obj(oid);
            }
        }
        if op == ast::Operator::Add {
            let la = a.kv.as_lit();
            let lb = b.kv.as_lit();
            // (a known text before or after one of several texts, or keys)
            match (la.and_then(intern::key_as_str), lb.and_then(intern::key_as_str)) {
                (Some(x), _) if matches!(b.kv, KeyVal::Lits(_) | KeyVal::KeysOf(_)) => return Value::kv_only(self.affix(&b.kv, &x, "")),
                (_, Some(y)) if matches!(a.kv, KeyVal::Lits(_) | KeyVal::KeysOf(_)) => return Value::kv_only(self.affix(&a.kv, "", &y)),
                _ => {}
            }
            // two lists with known positions: the concatenation's positions
            if let (Some(x), Some(y)) = (a.single(), b.single()) {
                if matches!((self.kind(x), self.kind(y)), (Kind::List, Kind::List) | (Kind::Tuple, Kind::Tuple)) {
                    let (px, py) = (self.llen(x, state).map(|n| (0..n).map(|i| state.slot(x, Ns::Pos, key_int(i as i64))).collect::<Vec<_>>()), self.llen(y, state).map(|n| (0..n).map(|i| state.slot(y, Ns::Pos, key_int(i as i64))).collect::<Vec<_>>()));
                    if let (Some(mut px), Some(py)) = (px, py) {
                        px.extend(py);
                        let kv = kv_join(&state.elems.get(&x).unwrap_or(DATA), &state.elems.get(&y).unwrap_or(DATA));
                        return self.alloc_positional(self.kind(x), node, Some(px), Value::new(), kv, state);
                    }
                }
            }
            let lists: Vec<u32> = both.iter().filter(|o| self.kind(*o) == Kind::List).collect();
            if !lists.is_empty() {
                let oid = self.alloc_at(Kind::List, node, state, &format!("list@L{}", node.line), Payload::None, None, Ns::Item, None, None);
                let mut acc = Value::new();
                let mut kv: Option<KeyVal> = None;
                for o in lists {
                    acc = acc.join(&state.slot(o, Ns::Elem, STAR));
                    let e = state.elems.get(&o).unwrap_or(DATA);
                    kv = Some(match kv {
                        None => e,
                        Some(k) => kv_join(&k, &e),
                    });
                }
                state.set_slot(oid, Ns::Elem, STAR, acc, true);
                state.elems.insert(oid, kv.unwrap_or(DATA));
                return Value::obj(oid);
            }
        }
        if op == ast::Operator::Mult {
            // a list with known positions times a known count
            let (seq, cnt) = if a.pts.len() == 1 { (a, b) } else { (b, a) };
            if let (Some(x), Some(n)) = (seq.single(), cnt.kv.as_lit().filter(|_| cnt.pts.is_empty()).and_then(intern::key_as_int)) {
                if matches!(self.kind(x), Kind::List | Kind::Tuple) {
                    if let Some(len) = self.llen(x, state) {
                        if len * (n.max(0) as usize) <= 64 {
                            let px: Vec<Value> = (0..len).map(|i| state.slot(x, Ns::Pos, key_int(i as i64))).collect();
                            let mut out: Vec<Value> = Vec::new();
                            for _ in 0..n.max(0) {
                                out.extend(px.iter().cloned());
                            }
                            let kv = state.elems.get(&x).unwrap_or(DATA);
                            return self.alloc_positional(self.kind(x), node, Some(out), Value::new(), kv, state);
                        }
                    }
                }
            }
        }
        // two literals: exactly as Python computes it
        if let (Some(x), Some(y), true, true) = (a.kv.as_lit(), b.kv.as_lit(), a.pts.is_empty(), b.pts.is_empty()) {
            let name = match op {
                ast::Operator::Add => "add",
                ast::Operator::Sub => "sub",
                ast::Operator::Mult => "mul",
                ast::Operator::Div => "truediv",
                ast::Operator::FloorDiv => "floordiv",
                ast::Operator::Mod => "mod",
                ast::Operator::Pow => "pow",
                ast::Operator::LShift => "lshift",
                ast::Operator::RShift => "rshift",
                ast::Operator::BitOr => "or_",
                ast::Operator::BitXor => "xor",
                ast::Operator::BitAnd => "and_",
                _ => "",
            };
            if !name.is_empty() {
                if let Some(r) = crate::models::pyvalues::operator_call(name, &[x, y]) {
                    let is_tuple = intern::with_key(r, |k| matches!(k, intern::Key::Tuple(_)));
                    if !is_tuple {
                        return Value::lit(r);
                    }
                }
            }
        }
        if op == ast::Operator::Mod {
            // `"%(name)s" % mapping`: each named field is a read of the mapping
            if let Some(sx) = a.kv.as_lit().filter(|_| a.pts.is_empty()).and_then(intern::key_as_str) {
                let recs = self.record_objects(b);
                if !recs.is_empty() && sx.contains("%(") {
                    let mut rest: &str = &sx;
                    while let Some(i) = rest.find("%(") {
                        let after = &rest[i + 2..];
                        let Some(j) = after.find(')') else { break };
                        let name = after[..j].to_string();
                        self.load_item(b, Ns::Item, &Value::lit(key_str_lit(&name)), node, state, true);
                        rest = &after[j..];
                    }
                    // a closed mapping of literals: the text
                    if let ([m], false) = (&recs[..], false) {
                        let row = state.row(*m, Ns::Item);
                        if !row.open() && row.present == row.definite {
                            let entries: Option<Vec<(String, KeyId)>> = row.present.iter().map(|k| {
                                let v = state.slot(*m, Ns::Item, k);
                                Some((intern::key_as_str(k)?.to_string(), v.kv.as_lit().filter(|_| v.pts.is_empty())?))
                            }).collect();
                            if let Some(es) = entries {
                                if let Some(s0) = crate::models::pyvalues::percent_format(&sx, crate::models::pyvalues::PercentArgs::Mapping(&es)) {
                                    return Value::lit(key_str_lit(&s0));
                                }
                            }
                        }
                    }
                    return Value::data(&format!("fmt@L{}", node.line));
                }
            }
            if let Some(x) = a.kv.as_lit() {
                if let Some(sx) = intern::key_as_str(x) {
                    // arguments that are each one of a few literals: every combination, as
                    // Python formats it
                    if let Some(rs) = format_percent_alternatives(&sx, b, state, self) {
                        return Value::kv_only(KeyVal::lits(crate::intern::KeySet::from_iter(rs.iter().map(|r| key_str_lit(r)))));
                    }
                    // "a%sb%s" with one argument not a literal: the literal text around it (a
                    // key built from a parameter keeps its prefix and suffix)
                    let sx2 = sx.to_string();
                    let n = sx2.matches("%s").count();
                    if n >= 1 && !sx2.replace("%s", "").contains('%') {
                        let segs: Vec<String> = sx2.split("%s").map(|s| s.to_string()).collect();
                        let args: Vec<KeyVal> = if n == 1 {
                            vec![b.kv.clone()]
                        } else {
                            match b.single() {
                                Some(t) if matches!(self.kind(t), Kind::Tuple) && self.llen(t, state) == Some(n) => (0..n).map(|i| state.slot(t, Ns::Pos, key_int(i as i64)).kv).collect(),
                                _ => Vec::new(),
                            }
                        };
                        let lit = |i: usize| args[i].as_lit().map(intern::key_py_str);
                        if let [h] = (0..args.len()).filter(|i| lit(*i).is_none()).collect::<Vec<_>>()[..] {
                            if args.len() == n {
                                let pre: String = (0..h).map(|i| format!("{}{}", segs[i], lit(i).unwrap())).collect::<String>() + &segs[h];
                                let post: String = segs[h + 1].clone() + &(h + 1..n).map(|i| format!("{}{}", lit(i).unwrap(), segs[i + 1])).collect::<String>();
                                return Value::kv_only(self.affix(&args[h], &pre, &post));
                            }
                        }
                    }
                }
            }
        }
        if crate::models::pandas::is_seriesish(self, &a) || crate::models::pandas::is_seriesish(self, &b) {
            return Value::with_kv(ObjSet::one(self.series), DATA);
        }
        let frames = both.filter(|o| self.kind(o) == Kind::Frame);
        if !frames.is_empty() {
            return Value::with_kv(frames, DATA);
        }
        Value::data(&format!("L{}", node.line))
    }

    /// The text `pre` + v + `post` for a value v that is one of a few texts or some keys:
    /// literals as their `str`, the rest renamed (a key made from a parameter keeps the text
    /// around it).
    fn affix(&mut self, kv: &KeyVal, pre: &str, post: &str) -> KeyVal {
        let to_str = |k: KeyId| if intern::key_is_str(k) { k } else { key_str_lit(&intern::key_py_str(k)) };
        let mut kv = match kv {
            KeyVal::Lit(k) => KeyVal::Lit(to_str(*k)),
            KeyVal::Lits(ks) => KeyVal::Lits(ks.iter().map(to_str).collect()),
            kv => kv.clone(),
        };
        for (f, t) in [(FnKind::Prefix, pre), (FnKind::Suffix, post)] {
            if !t.is_empty() {
                kv = self.rename_kv(&kv, &RenameFn::with_arg(f, key_str_lit(t)));
            }
        }
        kv
    }

    fn expr_fstring(&mut self, f: &'static ast::ExprFString, node: &'static ast::Expr, state: &mut State) -> Value {
        // parts: Some(Ok(str)) literal, Some(Err(value)) hole, None opaque hole
        enum Part {
            Lit(String),
            Hole(Value),
            Opaque,
        }
        let mut parts: Vec<Part> = Vec::new();
        for p in f.value.iter() {
            match p {
                ast::FStringPart::Literal(sl) => parts.push(Part::Lit(sl.value.to_string())),
                ast::FStringPart::FString(fs) => {
                    for el in fs.elements.iter() {
                        match el {
                            ast::InterpolatedStringElement::Literal(l) => parts.push(Part::Lit(l.value.to_string())),
                            ast::InterpolatedStringElement::Interpolation(i) => {
                                let v = self.eval(&i.expression, state);
                                // the spec's text (`{n:03d}`, `{x:>{w}}`); None: a part of it is not known
                                let spec_text: Option<Option<String>> = i.format_spec.as_ref().map(|spec| {
                                    let mut text = String::new();
                                    let mut literal = true;
                                    for se in spec.elements.iter() {
                                        match se {
                                            ast::InterpolatedStringElement::Literal(l) => text.push_str(&l.value),
                                            ast::InterpolatedStringElement::Interpolation(si) => {
                                                let sv = self.eval(&si.expression, state);
                                                match sv.kv.as_lit().filter(|_| sv.pts.is_empty() && si.conversion == ast::ConversionFlag::None && si.format_spec.is_none()) {
                                                    Some(k) => text.push_str(&intern::key_py_str(k)),
                                                    None => literal = false,
                                                }
                                            }
                                        }
                                    }
                                    Some(text).filter(|_| literal)
                                });
                                // an object of the program's classes: its `__format__` / `__str__` /
                                // `__repr__` makes the text
                                let nd = frontend::node(node);
                                let ran = match i.conversion {
                                    ast::ConversionFlag::None => {
                                        let spec_v = match &spec_text {
                                            None => Value::lit_str(""),
                                            Some(Some(t)) => Value::lit_str(t),
                                            Some(None) => Value::data("spec"),
                                        };
                                        crate::models::format_dunder(self, &v, spec_v, &nd, state)
                                    }
                                    ast::ConversionFlag::Str => self.run_dunder(&v, &["__str__", "__repr__"], vec![], &nd, state),
                                    _ => self.run_dunder(&v, &["__repr__"], vec![], &nd, state),
                                };
                                if let Some((t, rest)) = ran {
                                    let whole = rest.pts.is_empty() && rest.kv.is_bot() && (i.conversion == ast::ConversionFlag::None || spec_text.is_none());
                                    parts.push(if whole { Part::Hole(t) } else { Part::Opaque });
                                    continue;
                                }
                                // `{x!r}` / `{x!s}` / `{x!a}`: the converted text
                                let v = match i.conversion {
                                    ast::ConversionFlag::None => self.enum_text(&v, state).unwrap_or(v),
                                    conv => {
                                        let name = match conv {
                                            ast::ConversionFlag::Repr => "repr",
                                            ast::ConversionFlag::Ascii => "ascii",
                                            _ => "str",
                                        };
                                        let v = if name == "str" { self.enum_text(&v, state).unwrap_or(v) } else { v };
                                        match v.kv.as_lit().filter(|_| v.pts.is_empty()).and_then(|k| crate::models::pyvalues::builtin(name, &[k], &[])) {
                                            Some(t) => Value::lit(t),
                                            None => {
                                                Value::data("conv")
                                            }
                                        }
                                    }
                                };
                                match spec_text {
                                    // a spec applied to a known value
                                    Some(Some(t)) => match v.kv.as_lit().filter(|_| v.pts.is_empty()).and_then(|k| format_spec_literal(k, &t)) {
                                        Some(s) => parts.push(Part::Lit(s)),
                                        None => parts.push(Part::Opaque),
                                    },
                                    Some(None) => parts.push(Part::Opaque),
                                    None => parts.push(Part::Hole(v)),
                                }
                            }
                        }
                    }
                }
            }
        }
        let holes: Vec<usize> = parts.iter().enumerate().filter(|(_, p)| !matches!(p, Part::Lit(_))).map(|(i, _)| i).collect();
        if holes.is_empty() {
            let s: String = parts.iter().map(|p| if let Part::Lit(l) = p { l.as_str() } else { "" }).collect();
            return Value::lit(key_str_lit(&s));
        }
        if holes.len() == 1 {
            if let Part::Hole(v) = &parts[holes[0]] {
                let i = holes[0];
                let pre: String = parts[..i].iter().map(|p| if let Part::Lit(l) = p { l.as_str() } else { "" }).collect();
                let post: String = parts[i + 1..].iter().map(|p| if let Part::Lit(l) = p { l.as_str() } else { "" }).collect();
                let kv = self.affix(&v.kv, &pre, &post);
                // (objects in the hole, an unknown str among them: other texts too, one unknown)
                if !v.pts.is_empty() && v.kv.is_literal() {
                    let nd = frontend::node(node);
                    let site = format!("fstring@L{}", nd.line);
                    let o = self.unknown_source(&site, crate::domain::SourceKind::Data, state, &site, "", Ns::Item, vec![]);
                    return Value::with_kv(ObjSet::one(o), kv);
                }
                return Value::kv_only(kv);
            }
        }
        // several holes: known literals concatenate; a parameter among them is decided per
        // call site
        let mut text = String::new();
        let mut all_lit = true;
        for p in &parts {
            match p {
                Part::Lit(l) => text.push_str(l),
                Part::Hole(v) => {
                    match v.kv.as_lit().filter(|_| v.pts.is_empty()) {
                        Some(k) => text.push_str(&intern::key_py_str(k)),
                        None => all_lit = false,
                    }
                }
                Part::Opaque => all_lit = false,
            }
        }
        if all_lit {
            return Value::lit(key_str_lit(&text));
        }
        let nd = frontend::node(node);
        Value::data(&format!("fstring@L{}", nd.line))
    }

    // comprehensions ---------------------------------------------------------
    /// Run a comprehension's generators (`limit`: the most known items a generator unrolls).
    fn comp(&mut self, gens: &'static [ast::Comprehension], limit: usize, state: &mut State, body: &mut dyn FnMut(&mut Analyzer, &mut State), own: ObjId) {
        self.with_gen_restart(state, true, &mut |an, st| an.comp_scoped(gens, limit, st, body, own));
    }

    fn comp_scoped(&mut self, gens: &'static [ast::Comprehension], limit: usize, state: &mut State, body: &mut dyn FnMut(&mut Analyzer, &mut State), own: ObjId) {
        let mut names: Vec<intern::StrId> = gens.iter().flat_map(|g| names_in(&g.target)).map(|n| s(&n)).collect();
        names.sort_unstable();
        names.dedup();
        let outer: Vec<Option<Value>> = names.iter().map(|n| state.env.get(n).cloned()).collect();
        self.frame_mut().comp_names.push(names.clone());
        self.comp_cells.push(Default::default());
        self.comp_gen(gens, 0, limit, state, body, own);
        self.comp_cells.pop();
        self.frame_mut().comp_names.pop();
        // (the comprehension's variables are its own: the enclosing names are as before)
        for (n, v) in names.into_iter().zip(outer) {
            match v {
                Some(v) => {
                    state.env.insert(n, v);
                }
                None => {
                    state.env.shift_remove(&n);
                }
            }
        }
    }

    fn comp_gen(&mut self, gens: &'static [ast::Comprehension], i: usize, limit: usize, state: &mut State, body: &mut dyn FnMut(&mut Analyzer, &mut State), own: ObjId) {
        if i == gens.len() {
            body(self, state);
            return;
        }
        let g = &gens[i];
        let nonempty = std::mem::take(&mut self.nonempty_gen);
        let itv = self.eval(&g.iter, state);
        let exhaust_after = itv.clone();
        let gn = frontend::node(&g.iter);
        self.force_gen(&itv, state, true);
        let it = self.iterate(&itv, &gn, state);
        let understood = g.ifs.iter().all(|c| cond_is_exclusion(c, &g.target));
        if let Some(items) = &it.items {
            if items.len() <= limit {
                // known items: each is its own straight-line iteration; a filter decided
                // for the item keeps it straight-line (positions survive), an undecided one
                // makes the iteration conditional
                self.comp_known.push(true);
                for (idx, item) in items.iter().enumerate() {
                    let mut s2 = state.copy();
                    self.frame_mut().unroll.push(idx);
                    let decided = self.comp_run_one(g, gens, i, limit, &Elem::One(item.clone()), &mut s2, body, understood, own);
                    self.frame_mut().unroll.pop();
                    if s2.reachable {
                        // (an undecided filter may skip this item: joined with skipping it)
                        if decided {
                            state.replace_with(s2);
                        } else {
                            let j = state.join(&mut s2);
                            state.replace_with(j);
                        }
                    }
                }
                self.comp_known.pop();
                self.exhaust(&exhaust_after, state);
                return;
            }
        }
        self.comp_known.push(false);
        let depth = self.cond_depth;
        let mut elem = it.elem.clone();
        if it.exhaustive && understood {
            match &elem {
                Elem::One(v) => {
                    if let KeyVal::KeysOf(ko) = &v.kv {
                        elem = Elem::One(Value::with_kv(v.pts.clone(), KeyVal::KeysOf(KeysOf { exhaustive_depth: depth, ..ko.clone() })));
                    }
                }
                Elem::Pair(k0, v1) => {
                    if let KeyVal::KeysOf(ko) = &k0.kv {
                        elem = Elem::Pair(Value::with_kv(k0.pts.clone(), KeyVal::KeysOf(KeysOf { exhaustive_depth: depth, ..ko.clone() })), v1.clone());
                    }
                }
            }
        }
        let mut s2 = state.copy();
        let mark0 = self.serial;
        let vmark = self.verdicts.len();
        self.comp_run_one(g, gens, i, limit, &elem, &mut s2, body, understood, own);
        if it.exhaustive && understood && s2.reachable {
            self.comp_known.pop();
            state.replace_with(s2);
            return;
        }
        // (an iteration that changed nothing an older object or a name holds, but the result
        // it builds: the next one starts from the same state, one pass is the fixpoint)
        // (the element itself narrowed by a read is not a change the next element sees)
        let elem_objs = elem.flat().pts;
        let older = |o: ObjId| self.objects[o as usize].born < mark0 && o != own && !elem_objs.contains(o) && !self.obj(o).iter_elem;
        let fp = s2.footprint(state);
        // (the comprehension's own variables aside)
        let own_var = |n: &intern::StrId| self.frame().comp_names.iter().any(|ns| ns.contains(n));
        let same_env = s2.env.iter().all(|(n, v)| own_var(n) || state.env.get(n) == Some(v)) && state.env.keys().all(|n| own_var(n) || s2.env.contains_key(n));
        let quiet = same_env && fp.rows.iter().all(|(o, _)| !older(*o)) && fp.slots.iter().all(|(o, _)| !older(*o)) && fp.elems.iter().all(|o| !older(*o)) && s2.lens.diff_keys(&state.lens).iter().all(|o| !older(*o));
        if quiet {
            self.comp_known.pop();
            let mut joined = if nonempty { s2.copy() } else { state.join(&mut s2) };
            let ev = elem.value_part();
            self.promote_elem_reqs(&mut joined, &s2, &ev, state);
            state.replace_with(joined);
            return;
        }
        // items of unknown number: what one iteration did, the next one sees too (a walrus
        // accumulator, a `pop` from a dict): rounds to a fixpoint, as a `for` loop has (whose
        // verdicts supersede the first pass's)
        self.drop_verdicts_since(vmark);
        let (mut head, _, s2) = self.loop_fixpoint(state, &mut |an, body_s| {
            an.comp_run_one(g, gens, i, limit, &elem, body_s, body, understood, own);
        });
        self.comp_known.pop();
        // (at least one iteration: the states after one, from the fixpoint)
        if nonempty && self.loop_converged && s2.reachable {
            head = s2.copy();
        }
        let ev = elem.value_part();
        self.promote_elem_reqs(&mut head, &s2, &ev, state);
        state.replace_with(head);
    }

    /// Runs one iteration of generator `i`; returns whether every filter was decided (then
    /// the iteration was straight-line code).
    fn comp_run_one(&mut self, g: &'static ast::Comprehension, gens: &'static [ast::Comprehension], i: usize, limit: usize, it: &Elem, st: &mut State, body: &mut dyn FnMut(&mut Analyzer, &mut State), understood: bool, own: ObjId) -> bool {
        self.comp_bind(&g.target, it, st);
        let mut decided = true;
        for cond in &g.ifs {
            let sp = self.split(cond, st.copy());
            decided &= !sp.f.reachable;
            st.replace_with(sp.t);
            if !st.reachable {
                return decided;
            }
        }
        let conditional = !decided && !understood;
        if conditional {
            self.cond_depth += 1;
        }
        self.comp_gen(gens, i + 1, limit, st, body, own);
        if conditional {
            self.cond_depth -= 1;
        }
        decided
    }

    fn comp_bind(&mut self, target: &'static ast::Expr, it: &Elem, state: &mut State) {
        self.bind_iter_target(target, it, state);
        if !self.comp_cells.is_empty() {
            // the variable's cell holds its latest value
            let ctx = self.frame().ctx.clone();
            let addr = target as *const ast::Expr as usize;
            for n in names_in(target) {
                let sid = s(&n);
                let cid = self.alloc_raw(Kind::Cell, &format!("compcell:{}:{}", addr, n), &ctx, true, &format!("cell {}", n), Payload::None);
                let v = state.env.get(&sid).cloned().unwrap_or_default();
                state.set_slot(cid, Ns::Cell, super::funcs::cell_key(), v, true);
                self.comp_cells.last_mut().unwrap().insert(sid, cid);
            }
        }
    }

    fn expr_dictcomp(&mut self, dc: &'static ast::ExprDictComp, node: &'static ast::Expr, state: &mut State) -> Value {
        let nd = frontend::node(node);
        let oid = self.alloc_at(Kind::Dict, &nd, state, &format!("dictcomp@L{}", nd.line), Payload::None, None, Ns::Item, None, None);
        let base_depth = self.cond_depth;
        let key_e: &'static ast::Expr = &dc.key;
        let val_e: &'static ast::Expr = &dc.value;
        let mut body = |an: &mut Analyzer, st: &mut State| {
            let k = an.eval_key(key_e, st);
            let v = an.eval(val_e, st);
            let strong = an.obj(oid).single && an.cond_depth == base_depth;
            an.store_key(oid, Ns::Item, &k.kv, &v, strong, &nd, st);
        };
        let limit = comp_unroll_limit(&[key_e, val_e]);
        self.comp(&dc.generators, limit, state, &mut body, oid);
        Value::obj(oid)
    }

    fn seq_comp(&mut self, node: &'static ast::Expr, kind: Kind, elt: &'static ast::Expr, gens: &'static [ast::Comprehension], state: &mut State) -> Value {
        let nd = frontend::node(node);
        let oid = self.alloc_at(kind, &nd, state, &format!("{}comp@L{}", kind.name(), nd.line), Payload::None, None, Ns::Item, None, None);
        self.set_llen(oid, state, None);
        state.drop_obj_ns(oid, Ns::Pos);
        let base_depth = self.cond_depth;
        let strong = self.obj(oid).single && matches!(kind, Kind::List | Kind::Tuple | Kind::Set);
        // positions: every item known and every filter decided (straight-line iterations)
        let mut n: usize = 0;
        let mut positional = strong;
        let limit = comp_unroll_limit(&[elt]);
        // (a set's positions are its distinct members: a literal seen before adds none)
        let mut seen: Vec<KeyId> = Vec::new();
        let mut body = |an: &mut Analyzer, st: &mut State| {
            let v = an.eval(elt, st);
            let dup = kind == Kind::Set && v.pts.is_empty() && v.kv.as_lit().map(|k| seen.contains(&k) || {
                seen.push(k);
                false
            }) == Some(true);
            let straight = positional && an.comp_known.iter().all(|k| *k) && an.cond_depth == base_depth;
            if straight && dup {
                // (already a member)
            } else if straight && n < limit {
                st.set_slot(oid, Ns::Pos, key_int(n as i64), v.clone(), true);
                n += 1;
            } else {
                positional = false;
            }
            st.set_slot(oid, Ns::Elem, STAR, Value::with_kv(v.pts.clone(), crate::models::torch::elem_kv(&v.kv)), false);
            let cur = st.elems.get(&oid).unwrap_or_else(|| v.kv.clone());
            st.elems.insert(oid, kv_join(&cur, &v.kv));
        };
        if state.elems.get(&oid).is_none() {
            state.elems.insert(oid, BOT);
        }
        // (a source list with a first element, read before iterating it)
        let src_nonempty = strong && kind != Kind::Set && gens.len() == 1 && gens[0].ifs.is_empty() && !gens[0].is_async && match &gens[0].iter {
            ast::Expr::Name(src) => {
                let sv = self.load_name(s(src.id.as_str()), &Node::none(), state);
                sv.single().filter(|o| matches!(self.kind(*o), Kind::List | Kind::Tuple) && sv.kv.is_bot()).map(|so| self.prefix(so, state).is_some()).unwrap_or(false)
            }
            _ => false,
        };
        let mark = self.comp_known.len();
        // (the tensors `torch.cat` joins: at least one iteration ran, or it raised)
        if std::mem::take(&mut self.nonempty_comp) == node as *const ast::Expr as usize && gens.len() == 1 && gens[0].ifs.is_empty() {
            self.nonempty_gen = true;
        }
        self.comp(gens, limit, state, &mut body, oid);
        self.comp_known.truncate(mark);
        if positional {
            self.set_llen(oid, state, Some(n));
        } else {
            self.set_llen(oid, state, None);
            state.drop_obj_ns(oid, Ns::Pos);
            // `[f(x) for x in items]` over a list with a first element (`if items:`): the
            // result has one too (one of the body's values)
            if src_nonempty {
                let e = state.slot(oid, Ns::Elem, STAR);
                let ekv = state.elems.get(&oid).unwrap_or(crate::domain::DATA);
                if !(e.pts.is_empty() && ekv.is_bot()) {
                    state.set_slot(oid, Ns::Pos, key_int(0), Value::with_kv(e.pts, ekv), true);
                    state.lens.insert(oid, (None, 1, 0));
                }
            }
        }
        if kind == Kind::Set {
            self.normalize_set(oid, state);
        }
        Value::obj(oid)
    }
}

/// How many known items a comprehension's generators unroll: up to 64 (as many literals as a
/// value keeps) when what it makes of each item calls nothing (`{w: i for i, w in
/// enumerate(words)}`: each word a sure key), else UNROLL_MAX.
fn comp_unroll_limit(parts: &[&ast::Expr]) -> usize {
    let costly = |e: &ast::Expr| matches!(e, ast::Expr::Call(_) | ast::Expr::Lambda(_) | ast::Expr::ListComp(_) | ast::Expr::SetComp(_) | ast::Expr::DictComp(_) | ast::Expr::Generator(_) | ast::Expr::Await(_) | ast::Expr::Yield(_) | ast::Expr::YieldFrom(_) | ast::Expr::Named(_));
    if parts.iter().any(|p| ruff_python_ast::helpers::any_over_expr(p, &costly)) {
        UNROLL_MAX
    } else {
        64
    }
}

/// Ordering of two literals of one kind: numbers (ints, floats, bools) or strings.
/// How Python orders two literals (None: they do not compare).
pub fn literal_order(x: KeyId, y: KeyId) -> Option<std::cmp::Ordering> {
    crate::models::pyvalues::py_cmp(x, y)
}

/// `k != "x"`, `k not in (...)`, `k in (...)`, and conjunctions of those, on the key target.
pub fn cond_is_exclusion(cond: &ast::Expr, target: &ast::Expr) -> bool {
    let tnames = names_in(target);
    match cond {
        ast::Expr::BoolOp(b) if b.op == ast::BoolOp::And => b.values.iter().all(|c| cond_is_exclusion(c, target)),
        ast::Expr::UnaryOp(u) if u.op == ast::UnaryOp::Not => cond_is_exclusion(&u.operand, target),
        ast::Expr::Compare(c) if c.ops.len() == 1 => {
            let ast::Expr::Name(l) = &*c.left else { return false };
            if !tnames.iter().any(|n| n == l.id.as_str()) {
                return false;
            }
            let op = c.ops[0];
            let rhs = &c.comparators[0];
            let lit = is_constant(rhs)
                || match rhs {
                    ast::Expr::Tuple(t) => t.elts.iter().all(is_constant),
                    ast::Expr::List(l) => l.elts.iter().all(is_constant),
                    ast::Expr::Set(s) => s.elts.iter().all(is_constant),
                    _ => false,
                };
            if lit && matches!(op, ast::CmpOp::NotEq | ast::CmpOp::Eq | ast::CmpOp::In | ast::CmpOp::NotIn) {
                return true;
            }
            if matches!(op, ast::CmpOp::NotIn | ast::CmpOp::In) && matches!(rhs, ast::Expr::Name(_)) {
                return true;
            }
            false
        }
        _ => false,
    }
}

/// `fmt % args` when each argument is one of a few literals: the texts of every combination
/// (at most 64), exactly as Python formats them; None when an argument is not literal or a
/// combination does not format.
fn format_percent_alternatives(fmt: &str, b: &Value, state: &State, an: &Analyzer) -> Option<Vec<String>> {
    let lits = |v: &Value| -> Option<Vec<KeyId>> { if v.pts.is_empty() && v.kv.is_literal() { Some(crate::domain::kv_literals(&v.kv).iter().collect()) } else { None } };
    let alts: Vec<Vec<KeyId>> = if b.pts.is_empty() {
        vec![lits(b)?]
    } else {
        let o = b.single().filter(|o| an.kind(*o) == Kind::Tuple)?;
        let n = an.llen(o, state)?;
        (0..n).map(|i| lits(&state.slot(o, Ns::Pos, key_int(i as i64)))).collect::<Option<Vec<_>>>()?
    };
    let total: usize = alts.iter().map(|a| a.len()).product();
    if total == 0 || total > 64 {
        return None;
    }
    let mut out: Vec<String> = Vec::new();
    let mut idx = vec![0usize; alts.len()];
    loop {
        let args: Vec<KeyId> = alts.iter().zip(&idx).map(|(a, i)| a[*i]).collect();
        let s = crate::models::pyvalues::percent_format(fmt, crate::models::pyvalues::PercentArgs::Positional(&args))?;
        if !out.contains(&s) {
            out.push(s);
        }
        let mut p = 0;
        while p < idx.len() {
            idx[p] += 1;
            if idx[p] < alts[p].len() {
                break;
            }
            idx[p] = 0;
            p += 1;
        }
        if p == idx.len() {
            break;
        }
    }
    Some(out)
}

/// builtins whose calls do not raise for the values this analysis sees
/// A builtin exception class: constructing one does not raise.
pub fn is_exception_name(n: &str) -> bool {
    n.ends_with("Error") || n.ends_with("Exception") || n.ends_with("Warning") || matches!(n, "StopIteration" | "KeyboardInterrupt" | "SystemExit" | "GeneratorExit")
}


/// `format(value, spec)` for a known literal and a plain spec: `[[fill]align][0][width][.prec][type]`.
pub(crate) fn format_spec_literal(k: KeyId, spec: &str) -> Option<String> {
    crate::models::pyvalues::format_value(k, spec)
}


impl Analyzer {
    /// `a op b` where every object `a` (or else `b`) may be is an instance of a class defining
    /// `__op__` (`__rop__`): that method's result. None: not decided this way.
    pub fn binop_dunder(&mut self, op: ast::Operator, a: &Value, b: &Value, node: &Node, state: &mut State) -> Option<Value> {
        use ast::Operator as O;
        let name = match op {
            O::Add => "add",
            O::Sub => "sub",
            O::Mult => "mul",
            O::MatMult => "matmul",
            O::Div => "truediv",
            O::FloorDiv => "floordiv",
            O::Mod => "mod",
            O::Pow => "pow",
            O::BitAnd => "and",
            O::BitOr => "or",
            O::BitXor => "xor",
            O::LShift => "lshift",
            O::RShift => "rshift",
        };
        let methods = |an: &Analyzer, v: &Value, dunder: &str| -> Option<Vec<(u32, u32)>> {
            if v.pts.is_empty() || !v.kv.is_bot() {
                return None;
            }
            let mut out = Vec::new();
            for o in v.pts.iter() {
                if an.kind(o) != Kind::Instance {
                    return None;
                }
                let cls = an.instance_class(o)?;
                let owner = an.class_lookup(cls, dunder)?;
                let f = an.cls_slots.get(&(owner, Ns::Attr, key_str_lit(dunder)))?.single()?;
                if an.kind(f) != Kind::Function {
                    return None;
                }
                out.push((o, f));
            }
            Some(out)
        };
        let (calls, other) = match methods(self, a, &format!("__{}__", name)) {
            Some(m) => (m, b.clone()),
            None => (methods(self, b, &format!("__r{}__", name))?, a.clone()),
        };
        let mut outs = Vec::new();
        for (o, f) in calls {
            outs.push(self.call_user(f, Some(Value::obj(o)), vec![other.clone()], IndexMap::new(), vec![], vec![], state, node, None, false));
        }
        Some(crate::domain::join_values(outs))
    }
}



/// The positions `xs[lo:hi:step]` selects from a sequence of length `n` (`slice.indices`).
pub fn slice_positions(n: i64, lo: Option<i64>, hi: Option<i64>, step: i64) -> Vec<i64> {
    let (s, e, step) = slice_indices(n, lo, hi, step);
    if step > 0 {
        (s..e).step_by(step as usize).collect()
    } else {
        (e + 1..=s).rev().step_by(step.unsigned_abs() as usize).collect()
    }
}

/// `slice(lo, hi, step).indices(n)`.
pub fn slice_indices(n: i64, lo: Option<i64>, hi: Option<i64>, step: i64) -> (i64, i64, i64) {
    let adj = |b: Option<i64>, dflt: i64, lower: i64, upper: i64| -> i64 {
        match b {
            None => dflt,
            Some(b) => {
                let b = if b < 0 { b + n } else { b };
                b.clamp(lower, upper)
            }
        }
    };
    if step > 0 {
        (adj(lo, 0, 0, n), adj(hi, n, 0, n), step)
    } else {
        (adj(lo, n - 1, -1, n - 1), adj(hi, -1, -1, n - 1), step)
    }
}

impl Analyzer {
    /// The comparison method `name` an instance's (user) class defines.
    fn cmp_dunder(&self, o: ObjId, name: &str) -> Option<ObjId> {
        if self.kind(o) != Kind::Instance {
            return None;
        }
        let cls = self.instance_class(o)?;
        let owner = self.class_lookup(cls, name)?;
        let fv = self.cls_slots.get(&(owner, Ns::Attr, key_str_lit(name)))?;
        fv.pts.iter().find(|f| self.kind(*f) == Kind::Function)
    }

    /// `a OP b` where an operand is an instance with comparison methods: the method, else the
    /// other's reflected one, else what `@functools.total_ordering` derives, else identity for
    /// `==` / `!=` (None: not such a comparison).
    pub fn rich_compare(&mut self, op: ast::CmpOp, a: &Value, b: &Value, node: &Node, state: &mut State) -> Option<Value> {
        // (one of several values, some of them instances of the program's classes: each
        // compared on its own path)
        let user = |an: &Analyzer, o: ObjId| an.kind(o) == Kind::Instance && an.instance_class(o).is_some();
        if self.split_dispatch(a, &user) {
            return Some(self.per_receiver(a, state, &mut |an, p, st| an.rich_compare(op, p, b, node, st).unwrap_or_else(|| Value::data("bool"))));
        }
        if self.split_dispatch(b, &user) {
            return Some(self.per_receiver(b, state, &mut |an, p, st| an.rich_compare(op, a, p, node, st).unwrap_or_else(|| Value::data("bool"))));
        }
        let inst = |an: &Analyzer, v: &Value| v.single().filter(|o| an.kind(*o) == Kind::Instance && v.kv.is_bot());
        let (ai, bi) = (inst(self, a), inst(self, b));
        if ai.is_none() && bi.is_none() {
            return None;
        }
        let (name, refl) = match op {
            ast::CmpOp::Lt => ("__lt__", "__gt__"),
            ast::CmpOp::LtE => ("__le__", "__ge__"),
            ast::CmpOp::Gt => ("__gt__", "__lt__"),
            ast::CmpOp::GtE => ("__ge__", "__le__"),
            ast::CmpOp::Eq => ("__eq__", "__eq__"),
            ast::CmpOp::NotEq => ("__ne__", "__ne__"),
            _ => return None,
        };
        let call = |an: &mut Analyzer, f: ObjId, recv: &Value, other: &Value, state: &mut State| an.call_user(f, Some(recv.clone()), vec![other.clone()], IndexMap::new(), vec![], vec![], state, node, None, false);
        if let Some(f) = ai.and_then(|o| self.cmp_dunder(o, name)) {
            return Some(call(self, f, a, b, state));
        }
        if let Some(f) = bi.and_then(|o| self.cmp_dunder(o, refl)) {
            return Some(call(self, f, b, a, state));
        }
        let lit = |b: Option<bool>| match b {
            Some(x) => Value::lit(intern::key_bool(x)),
            None => Value::data("bool"),
        };
        // `!=` from `__eq__`
        if op == ast::CmpOp::NotEq {
            if let Some(f) = ai.and_then(|o| self.cmp_dunder(o, "__eq__")).or_else(|| bi.and_then(|o| self.cmp_dunder(o, "__eq__"))) {
                let (x, y) = if ai.and_then(|o| self.cmp_dunder(o, "__eq__")).is_some() { (a, b) } else { (b, a) };
                let r = call(self, f, x, y, state);
                return Some(lit(self.truth_of(&r, state, node).map(|t| !t)));
            }
        }
        // @total_ordering: one ordering method and __eq__ give the others
        let ordered = |an: &Analyzer, o: Option<ObjId>| o.and_then(|o| an.instance_class(o)).map(|c| an.mro(c).iter().any(|m| an.total_ordering.contains(m))).unwrap_or(false);
        if matches!(op, ast::CmpOp::Lt | ast::CmpOp::LtE | ast::CmpOp::Gt | ast::CmpOp::GtE) && ordered(self, ai) {
            if let Some(f) = ai.and_then(|o| self.cmp_dunder(o, "__lt__")) {
                let lt = call(self, f, a, b, state);
                let lt = self.truth_of(&lt, state, node);
                let eqv = self.rich_compare(ast::CmpOp::Eq, a, b, node, state).unwrap_or_else(|| Value::data("bool"));
                let eq = self.truth_of(&eqv, state, node);
                let r = match op {
                    ast::CmpOp::LtE => match (lt, eq) {
                        (Some(true), _) | (_, Some(true)) => Some(true),
                        (Some(false), Some(false)) => Some(false),
                        _ => None,
                    },
                    ast::CmpOp::Gt => match (lt, eq) {
                        (Some(true), _) | (_, Some(true)) => Some(false),
                        (Some(false), Some(false)) => Some(true),
                        _ => None,
                    },
                    ast::CmpOp::GtE => lt.map(|t| !t),
                    _ => lt,
                };
                return Some(lit(r));
            }
        }
        if matches!(op, ast::CmpOp::Lt | ast::CmpOp::LtE | ast::CmpOp::Gt | ast::CmpOp::GtE) {
            let decide = |o: std::cmp::Ordering| match op {
                ast::CmpOp::Lt => o.is_lt(),
                ast::CmpOp::LtE => o.is_le(),
                ast::CmpOp::Gt => o.is_gt(),
                _ => o.is_ge(),
            };
            // a member of an Enum mixing in int / str (IntEnum, StrEnum): its value
            let mixed = |an: &Analyzer, o: Option<ObjId>| -> Option<Value> {
                let o = o?;
                let c = an.instance_class(o)?;
                if !an.enum_kind.get(&c).map(|k| k.mixed).unwrap_or(false) {
                    return None;
                }
                let v = state.slot(o, Ns::Attr, key_str_lit("value"));
                v.kv.as_lit().filter(|_| v.pts.is_empty()).map(|_| v)
            };
            let (ma, mb) = (mixed(self, ai), mixed(self, bi));
            if ma.is_some() || mb.is_some() {
                let x = ma.unwrap_or_else(|| a.clone());
                let y = mb.unwrap_or_else(|| b.clone());
                if let Some(o) = crate::models::stdfuncs::value_order(self, state, &x, &y) {
                    return Some(lit(Some(decide(o))));
                }
            }
            // `@dataclass(order=True)`: two instances of the class compare their fields
            if let Some(o) = ai.zip(bi).and_then(|(x, y)| self.dataclass_order(x, y, state)) {
                return Some(lit(Some(decide(o))));
            }
        }
        // no __eq__: identity
        if matches!(op, ast::CmpOp::Eq | ast::CmpOp::NotEq) {
            let no_eq =|an: &Analyzer, o: Option<ObjId>| o.map(|o| an.cmp_dunder(o, "__eq__").is_none() && an.instance_class(o).map(|c| !an.class_info(c).open_base && an.class_info(c).dataclass_fields.borrow().is_none()).unwrap_or(false)).unwrap_or(true);
            if let (Some(x), Some(y)) = (ai, bi) {
                if no_eq(self, ai) && no_eq(self, bi) {
                    let same = if x == y && self.obj(x).single { Some(true) } else if x != y { Some(false) } else { None };
                    return Some(lit(same.map(|s| if op == ast::CmpOp::Eq { s } else { !s })));
                }
            }
        }
        None
    }

    /// The order of two tuples (objects with known items, or literal tuples) item by item.
    fn tuple_order(&self, a: &Value, b: &Value, state: &State) -> Option<std::cmp::Ordering> {
        let tupleish = |an: &Analyzer, v: &Value| {
            (v.pts.is_empty() && v.kv.as_lit().map(|k| intern::with_key(k, |kk| matches!(kk, intern::Key::Tuple(_)))).unwrap_or(false))
                || (v.single().map(|o| an.kind(o) == Kind::Tuple).unwrap_or(false) && v.kv.is_bot())
        };
        if !tupleish(self, a) || !tupleish(self, b) {
            return None;
        }
        crate::models::stdfuncs::value_order(self, state, a, b)
    }

    /// `a < b` for sorting / max / min when the items are instances ordering themselves.
    pub fn instance_less(&mut self, a: &Value, b: &Value, node: &Node, state: &mut State) -> Option<bool> {
        let r = self.rich_compare(ast::CmpOp::Lt, a, b, node, state)?;
        self.truth_of(&r, state, node)
    }
}

/// Do two states agree on the facts a generator's effects concern?
fn gen_facts_equal(a: &State, b: &State, ge: &crate::interp::GenEffects) -> bool {
    ge.rows.iter().all(|k| a.rows.get(k) == b.rows.get(k))
        && ge.slots.iter().all(|(o, sk)| a.slots.get(o, sk) == b.slots.get(o, sk))
        && ge.elems.iter().all(|o| a.elems.get(o) == b.elems.get(o))
        && ge.lens.iter().all(|o| a.lens.get(o) == b.lens.get(o))
}

/// Install `src`'s facts about a generator's affected objects into `dst`.
pub fn gen_copy_facts(dst: &mut State, src: &State, ge: &crate::interp::GenEffects) {
    for k in &ge.rows {
        match src.rows.get(k) {
            Some(r) => dst.rows.insert(*k, r),
            None => {
                dst.rows.remove(k);
            }
        }
    }
    for (o, sk) in &ge.slots {
        match src.slots.get(o, sk) {
            Some(v) => dst.slots.set(o, *sk, v),
            None => {
                dst.slots.remove(o, sk);
            }
        }
    }
    for o in &ge.elems {
        match src.elems.get(o) {
            Some(v) => dst.elems.insert(*o, v),
            None => {
                dst.elems.remove(o);
            }
        }
    }
    for o in &ge.lens {
        match src.lens.get(o) {
            Some(v) => dst.lens.insert(*o, v),
            None => {
                dst.lens.remove(o);
            }
        }
    }
    for (o, sk) in &ge.slots {
        if matches!(sk.0, Ns::Pos | Ns::Elem) {
            dst.clear_last(*o);
        }
    }
}

/// Python's `==` on two literals (`1 == 1.0 == True`); Enum members and identity-hashed
/// objects compare by identity (their keys), as the engine keys them.
fn lit_eq(a: KeyId, b: KeyId) -> Option<bool> {
    let by_id = |k: KeyId| intern::with_key(k, |kk| matches!(kk, intern::Key::Member(..) | intern::Key::Inv(..)));
    if a == b || by_id(a) || by_id(b) {
        return Some(a == b);
    }
    crate::models::pyvalues::py_eq(a, b)
}
