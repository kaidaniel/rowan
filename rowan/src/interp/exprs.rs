//! Expressions (port of the `expr_*` methods, `truthiness`, `binop`, comprehensions).

use indexmap::IndexMap;
use ruff_python_ast as ast;

use crate::domain::{kv_join, kv_map, FnKind, KeyVal, KeysOf, Ns, ObjSet, RenameFn, Row, Value, DATA, BOT};
use crate::frontend::{self, Node};
use crate::intern::{self, key_float, key_int, key_str_lit, s, KeyId, STAR};
use crate::state::{Kind, State};

use super::funcs::Elem;
use super::{is_constant, names_in, Analyzer, Payload, ViewKind, SAFE_METHODS, UNROLL_MAX};

/// Python's `s[lo:hi:step]` on characters.
fn py_slice(cs: &[char], lo: Option<i64>, hi: Option<i64>, step: Option<i64>) -> Option<String> {
    let n = cs.len() as i64;
    let step = step.unwrap_or(1);
    if step == 0 {
        return None;
    }
    let norm = |x: i64| if x < 0 { x + n } else { x };
    let mut out = String::new();
    if step > 0 {
        let lo = lo.map(norm).unwrap_or(0).clamp(0, n);
        let hi = hi.map(norm).unwrap_or(n).clamp(0, n);
        let mut i = lo;
        while i < hi {
            out.push(cs[i as usize]);
            i += step;
        }
    } else {
        let lo = lo.map(norm).unwrap_or(n - 1).clamp(-1, n - 1);
        let hi = hi.map(norm).unwrap_or(-1).clamp(-1, n - 1);
        let mut i = lo;
        while i > hi {
            out.push(cs[i as usize]);
            i += step;
        }
    }
    Some(out)
}

impl Analyzer {
    pub fn eval(&mut self, node: &'static ast::Expr, state: &mut State) -> Value {
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
                if let ast::Expr::Slice(sl) = &*sub.slice {
                    if let Some(v) = self.slice_positional(&obj, sl, node, state) {
                        return v;
                    }
                }
                // a known string sliced / indexed by known numbers
                if obj.pts.is_empty() && obj.kv.is_literal() && !super::disabled("strslice") {
                    let strs: Option<Vec<std::sync::Arc<str>>> = crate::domain::kv_literals(&obj.kv).iter().map(intern::key_as_str).collect();
                    if let Some(strs) = strs.filter(|v| !v.is_empty()) {
                        let mut out: Option<intern::KeySet> = Some(intern::KeySet::new());
                        match &*sub.slice {
                            ast::Expr::Slice(sl) => {
                                let mut b: Vec<Option<i64>> = Vec::new();
                                for e in [&sl.lower, &sl.upper, &sl.step] {
                                    match e.as_deref() {
                                        None => b.push(None),
                                        Some(e) => match self.int_value(e, state) {
                                            Some(i) => b.push(Some(i)),
                                            None => {
                                                out = None;
                                                break;
                                            }
                                        },
                                    }
                                }
                                if let Some(o) = out.as_mut() {
                                    for st0 in &strs {
                                        let cs: Vec<char> = st0.chars().collect();
                                        match py_slice(&cs, b[0], b[1], b[2]) {
                                            Some(t) => *o = o.insert(key_str_lit(&t)),
                                            None => {
                                                out = None;
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                            idx => {
                                let iv = self.eval(idx, state);
                                match iv.kv.as_lit().filter(|_| iv.pts.is_empty()).and_then(intern::key_as_int) {
                                    Some(i) => {
                                        for st0 in &strs {
                                            let cs: Vec<char> = st0.chars().collect();
                                            let j = if i < 0 { i + cs.len() as i64 } else { i };
                                            if j < 0 || j as usize >= cs.len() {
                                                out = None;
                                                break;
                                            }
                                            if let Some(o) = out.as_mut() {
                                                *o = o.insert(key_str_lit(&cs[j as usize].to_string()));
                                            }
                                        }
                                    }
                                    None => out = None,
                                }
                            }
                        }
                        if let Some(o) = out {
                            return Value::kv_only(KeyVal::lits(o));
                        }
                    }
                }
                let key = self.eval_key(&sub.slice, state);
                let nd = frontend::node(node);
                // a key the name's object surely has (stored through the name): no failure
                if let (ast::Expr::Name(nm), Some(k)) = (&*sub.value, key.kv.as_lit().filter(|_| key.pts.is_empty())) {
                    if state.name_has_key(s(nm.id.as_str()), k) {
                        let v = self.load_item(&obj, Ns::Item, &key, &nd, state, false);
                        if let Some(o) = obj.pts.first() {
                            self.verdict(&nd, k, &crate::domain::Classification::with_note(crate::domain::Tier::Proven, "stored through this name"), o, Ns::Item, None, None);
                        }
                        return v;
                    }
                }
                self.load_item(&obj, Ns::Item, &key, &nd, state, true)
            }
            ast::Expr::Call(c) => {
                // (the tensor model reads `x.dim` as a method only when it is called)
                self.t_called_attr = frontend::node(&*c.func).addr;
                let fv = self.eval(&c.func, state);
                if !self.may_raise.is_empty() {
                    let all_safe = fv.pts.iter().all(|o| match &self.obj(o).payload {
                        Payload::Method(q, _) if self.kind(o) == Kind::Method => SAFE_METHODS.contains(&q.as_str()) || SELF_REPORTING.iter().any(|p| q.starts_with(p)),
                        // user code: whether it raises is reported by its own analysis
                        // builtins that raise only on wrong types (len, str, sorted, …)
                        Payload::Str(q) if self.kind(o) == Kind::External => q.strip_prefix("builtins.").map(|n| SAFE_BUILTINS.contains(&n) || is_exception_name(n)).unwrap_or(false) || SELF_REPORTING_EXTERNALS.contains(&q.as_str()),
                        _ => matches!(self.kind(o), Kind::Function | Kind::Bound | Kind::Class | Kind::Partial),
                    });
                    if !all_safe {
                        *self.may_raise.last_mut().unwrap() = true;
                    }
                }
                let nd = frontend::node(node);
                let r = self.call_value(&fv, &nd, Some(c), state, None);
                // `t.setdefault(k, …)` through a name: whichever object `t` is has k now
                if let ast::Expr::Attribute(a) = &*c.func {
                    if let (ast::Expr::Name(nm), "setdefault", Some(first)) = (&*a.value, a.attr.id.as_str(), c.arguments.args.first()) {
                        if let ast::Expr::StringLiteral(sl) = first {
                            if !super::disabled("namekeys") {
                                state.add_name_key(s(nm.id.as_str()), key_str_lit(sl.value.to_str()));
                            }
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
                let tv = self.eval(&i.test, state);
                // a decided test: one branch only
                if !super::disabled("ifexp") {
                    let nd = frontend::node(node);
                    match self.truth_of(&tv, state, &nd) {
                        Some(true) => return self.eval(&i.body, state),
                        Some(false) => return self.eval(&i.orelse, state),
                        None => {}
                    }
                }
                let mut a = state.copy();
                let mut b = state.copy();
                self.narrow(&i.test, true, &mut a);
                self.narrow(&i.test, false, &mut b);
                let va = if a.reachable { self.eval(&i.body, &mut a) } else { Value::new() };
                let vb = if b.reachable { self.eval(&i.orelse, &mut b) } else { Value::new() };
                let j = a.join(&mut b);
                state.replace_with(j);
                va.join(&vb)
            }
            ast::Expr::BoolOp(b) => self.expr_boolop(b, state),
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
                        return crate::models::torch::dim_value(if u.op == ast::UnaryOp::USub { crate::shape::scale(d, -1) } else { d });
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
                        if let Some(k) = v.kv.as_lit() {
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
                if r.pts.is_empty() && matches!(r.kv, KeyVal::Data(l) if !intern::str_get(l).starts_with('\u{1}')) && !super::disabled("exprsym") {
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
                let a = self.eval(&c.left, state);
                // a tensor is never None
                if c.ops.len() == 1 && matches!(c.ops[0], ast::CmpOp::Is | ast::CmpOp::IsNot) && matches!(c.comparators[0], ast::Expr::NoneLiteral(_)) && crate::models::torch::is_tensor(&a) && a.pts.is_empty() {
                    return Value::lit(intern::key_bool(c.ops[0] == ast::CmpOp::IsNot));
                }
                if c.ops.len() == 1 && matches!(c.ops[0], ast::CmpOp::Lt | ast::CmpOp::LtE | ast::CmpOp::Gt | ast::CmpOp::GtE | ast::CmpOp::Eq | ast::CmpOp::NotEq) {
                    if crate::models::torch::is_tensor(&a) {
                        let b = self.eval(&c.comparators[0], state);
                        let nd = frontend::node(node);
                        return self.t_compare(&a, &b, &nd, state);
                    }
                    // a named unknown against a number / tensor (simple right operands only:
                    // the interpreter evaluates them again otherwise)
                    if self.torch_on && a.kv.is_bot() && a.pts.len() == 1 && a.pts.iter().all(|o| matches!(self.kind(o), crate::state::Kind::Ph | crate::state::Kind::Unknown)) {
                        // (the right operand is evaluated once here)
                        let b = self.eval(&c.comparators[0], state);
                        let nd = frontend::node(node);
                        if let Some(r) = self.t_compare_named(&a, &b, &nd, state) {
                            return r;
                        }
                        if let Some(r) = self.compare_values(c.ops[0], &a, &b, &c.comparators[0], state) {
                            return Value::lit(intern::key_bool(r));
                        }
                        return Value::data("bool");
                    }
                }
                if c.ops.len() == 1 {
                    if let Some(b) = self.compare_const(c.ops[0], &a, &c.comparators[0], state) {
                        return Value::lit(intern::key_bool(b));
                    }
                } else if !super::disabled("chaincmp") {
                    // `a < b <= c`: each link on the operands' values; a false link decides
                    // the chain (the rest is not evaluated), all true links make it true
                    let mut left = a.clone();
                    let mut all_known = true;
                    for (op, cmp) in c.ops.iter().zip(c.comparators.iter()) {
                        let b = self.eval(cmp, state);
                        match self.compare_values(*op, &left, &b, cmp, state) {
                            Some(false) => return Value::lit(intern::key_bool(false)),
                            Some(true) => {}
                            None => all_known = false,
                        }
                        left = b;
                    }
                    if all_known {
                        return Value::lit(intern::key_bool(true));
                    }
                }
                for cmp in c.comparators.iter() {
                    let b = self.eval(cmp, state);
                    if crate::models::pandas::is_seriesish(self, &a) || crate::models::pandas::is_seriesish(self, &b) {
                        return Value::with_kv(ObjSet::one(self.series), DATA);
                    }
                    let both = a.pts.union(&b.pts);
                    if both.iter().any(|o| self.kind(o) == Kind::Frame) {
                        return Value::with_kv(both.filter(|o| self.kind(o) == Kind::Frame), DATA);
                    }
                }
                Value::data("bool")
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
                if self.cm_hook.as_ref().map(|h| !h.ran && self.frames.len() == h.depth + 1).unwrap_or(false) {
                    self.run_cm_body(v, state);
                    return self.none_v.clone();
                }
                if y.value.is_some() {
                    self.yield_value(v, state);
                }
                Value::data("yield")
            }
            ast::Expr::YieldFrom(y) => {
                let v = self.eval(&y.value, state);
                let nd = frontend::node(&*y.value);
                let it = self.iterate(&v, &nd, state);
                match it.items {
                    // known items: yielded one by one, in order
                    Some(items) if !super::disabled("yieldfrom") => {
                        for item in items {
                            self.yield_value(item, state);
                        }
                    }
                    _ => {
                        self.yield_value(it.elem.flat(), state);
                        // an unknown number of items: positions after this are unknown
                        if let Some(foid) = self.frame().func {
                            if let Some(gid) = self.func_info(foid).generator.get() {
                                self.set_llen(gid, state, None);
                                state.drop_obj_ns(gid, Ns::Pos);
                            }
                        }
                    }
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
        let caller_env = self.env_stack.last().cloned().unwrap_or_default();
        let callee_env = std::mem::replace(&mut state.env, caller_env);
        let callee_ret = state.ret.take();
        let callee_exits = state.exits.take();
        if let Some(t) = h.target {
            self.bind_target(t, &v, state);
        }
        self.cm_frames.push((self.frames.len() + 1, callee_env.clone()));
        match h.then.take() {
            Some(f) => f(self, v, state),
            None => self.exec_block(h.body, state),
        }
        self.cm_frames.pop();
        let after = std::mem::replace(&mut state.env, callee_env);
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

    fn yield_value(&mut self, v: Value, state: &mut State) {
        let Some(foid) = self.frame().func else { return };
        let ctx = self.frame().ctx.clone();
        let label = format!("generator {}", self.frame().label);
        let gid = self.alloc_raw(Kind::List, &format!("gen:{}", foid), &ctx, true, &label, Payload::None);
        // a fresh generator list (no yield yet in this activation) starts empty
        let fresh = !state.has_slot(gid, Ns::Elem, STAR);
        state.set_slot(gid, Ns::Elem, STAR, v.clone(), false);
        let cur = state.elems.get(&gid).unwrap_or_else(|| v.kv.clone());
        state.elems.insert(gid, kv_join(&cur, &v.kv));
        // straight-line yields keep their order
        let positional = self.frame().single && self.frame().loop_depth == 0 && self.cond_depth == 0 && self.obj(gid).single;
        let ll = if fresh { Some(0) } else { self.llen(gid, state) };
        match (positional, ll) {
            (true, Some(n)) if n < 16 => {
                state.set_slot(gid, Ns::Pos, key_int(n as i64), v, true);
                self.set_llen(gid, state, Some(n + 1));
            }
            _ => {
                self.set_llen(gid, state, None);
                state.drop_obj_ns(gid, Ns::Pos);
            }
        }
        let fi = self.func_info(foid);
        if fi.generator.get().is_none() {
            fi.generator.set(Some(gid));
        }
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
                self.keyify(&v)
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
                vals.push(it.elem.one_or_empty());
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

    fn expr_boolop(&mut self, b: &'static ast::ExprBoolOp, state: &mut State) -> Value {
        let mut acc = self.eval(&b.values[0], state);
        let is_and = b.op == ast::BoolOp::And;
        let mut pre = state.copy();
        let mut st = state.copy();
        self.cond_depth += 1;
        self.branch_depth += 1;
        let mut prev: &'static ast::Expr = &b.values[0];
        for v in b.values[1..].iter() {
            let truth = self.truthiness(&acc, &st);
            if (!is_and && truth == Some(true)) || (is_and && truth == Some(false)) {
                break;
            }
            self.narrow(prev, is_and, &mut st);
            if !st.reachable {
                break;
            }
            let rhs = self.eval(v, &mut st);
            if (!is_and && truth == Some(false)) || (is_and && truth == Some(true)) {
                acc = rhs;
            } else {
                acc = acc.join(&rhs);
            }
            prev = v;
        }
        self.branch_depth -= 1;
        self.cond_depth -= 1;
        if st.reachable {
            let j = pre.join(&mut st);
            state.replace_with(j);
        } else {
            state.replace_with(pre);
        }
        acc
    }

    /// `kv_map` that also records a narrowing event when the key is a parameter: the renamed
    /// key is only known once the parameter is bound to a literal (at the call site).
    pub fn rename_kv(&mut self, kv: &KeyVal, f: &RenameFn) -> KeyVal {
        if let KeyVal::Param(ph) = kv {
            self.record_event("narrow", *ph);
        }
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
        let k = v.kv.as_lit()?;
        intern::with_key(k, |kk| match kk {
            intern::Key::Int(i) => Some(*i),
            _ => None,
        })
    }

    fn slice_positional(&mut self, obj: &Value, sl: &'static ast::ExprSlice, node: &'static ast::Expr, state: &mut State) -> Option<Value> {
        if let Some(step_e) = sl.step.as_deref() {
            // a known step (positive or negative) with known or default bounds
            let step = self.int_value(step_e, state)?;
            if step == 0 {
                return None;
            }
            let oid = obj.single()?;
            let kind = self.kind(oid);
            if !matches!(kind, Kind::List | Kind::Tuple) {
                return None;
            }
            let n = self.llen(oid, state)? as i64;
            let lo = match sl.lower.as_deref() { None => None, Some(e) => Some(self.int_value(e, state)?) };
            let hi = match sl.upper.as_deref() { None => None, Some(e) => Some(self.int_value(e, state)?) };
            let idx = slice_positions(n, lo, hi, step);
            let vals: Vec<Value> = idx.iter().map(|i| state.slot(oid, Ns::Pos, key_int(*i))).collect();
            let kv = state.elems.get(&oid).unwrap_or(DATA);
            let nd = frontend::node(node);
            return Some(self.alloc_positional(kind, &nd, Some(vals), Value::new(), kv, state));
        }
        fn bound(e: &Option<Box<ast::Expr>>) -> Option<Option<i64>> {
            match e {
                None => Some(None),
                Some(x) => match &**x {
                    ast::Expr::NumberLiteral(n) => match &n.value {
                        ast::Number::Int(i) => i.as_i64().map(Some),
                        _ => None,
                    },
                    ast::Expr::UnaryOp(u) if u.op == ast::UnaryOp::USub => match &*u.operand {
                        ast::Expr::NumberLiteral(n) => match &n.value {
                            ast::Number::Int(i) => i.as_i64().map(|i| Some(-i)),
                            _ => None,
                        },
                        _ => None,
                    },
                    _ => None,
                },
            }
        }
        // a bound that is not written as a number may still hold one known value
        let lo = match bound(&sl.lower) {
            Some(b) => b,
            None => Some(self.int_value(sl.lower.as_deref()?, state)?),
        };
        let hi = match bound(&sl.upper) {
            Some(b) => b,
            None => Some(self.int_value(sl.upper.as_deref()?, state)?),
        };
        let oid = obj.single()?;
        let kind = self.kind(oid);
        if !matches!(kind, Kind::List | Kind::Tuple) {
            return None;
        }
        let n = self.llen(oid, state)? as i64;
        let norm = |b: Option<i64>, dflt: i64| -> i64 {
            let b = b.unwrap_or(dflt);
            let b = if b < 0 { n + b } else { b };
            b.clamp(0, n)
        };
        let lo = norm(lo, 0);
        let hi = norm(hi, n);
        let nd = frontend::node(node);
        let vals: Vec<Value> = (lo..hi.max(lo)).map(|i| state.slot(oid, Ns::Pos, key_int(i))).collect();
        let kv = state.elems.get(&oid).unwrap_or(DATA);
        Some(self.alloc_positional(kind, &nd, Some(vals), Value::new(), kv, state))
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
        let nid = self.alloc_at(kind, nd, state, &format!("{}@L{}", kind.name(), nd.line), Payload::None, None, Ns::Item, None, None);
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

    /// `compare_const` on an already evaluated right operand `b` (written as `right`).
    fn compare_values(&mut self, op: ast::CmpOp, a: &Value, b: &Value, right: &'static ast::Expr, state: &mut State) -> Option<bool> {
        self.cmp_right = Some(b.clone());
        let r = self.compare_const(op, a, right, state);
        self.cmp_right = None;
        r
    }

    fn eval_right(&mut self, right: &'static ast::Expr, state: &mut State) -> Value {
        match self.cmp_right.take() {
            Some(v) => v,
            None => self.eval(right, state),
        }
    }

    /// `a is None`, `a is not None`, `a == lit`, `a != lit` when the outcome is decidable
    /// from the value's literals: a value with only non-placeholder objects is never None.
    fn compare_const(&mut self, op: ast::CmpOp, a: &Value, right: &'static ast::Expr, state: &mut State) -> Option<bool> {
        let none = intern::key_none();
        let lits = crate::domain::kv_literals(&a.kv);
        let has_ph = a.pts.iter().any(|o| self.kind(o) == Kind::Ph || self.kind(o) == Kind::Unknown);
        let known_lits = a.kv.is_literal() || a.kv.is_bot();
        match op {
            ast::CmpOp::Is | ast::CmpOp::IsNot if matches!(right, ast::Expr::NoneLiteral(_)) => {
                let is_none = if a.pts.is_empty() && a.kv.is_literal() && lits.len() == 1 && lits.contains(none) {
                    true
                } else if !has_ph && known_lits && !lits.contains(none) && (!a.pts.is_empty() || !lits.is_empty()) {
                    false
                } else {
                    // undecidable here: decided per call site when the operand is concrete there
                    for o in a.pts.iter() {
                        if self.kind(o) == Kind::Ph {
                            self.record_event("truth", o);
                        }
                    }
                    if let Some(ph) = a.kv.as_param() {
                        self.record_event("truth", ph);
                    }
                    return None;
                };
                Some(if op == ast::CmpOp::Is { is_none } else { !is_none })
            }
            // identity of objects: distinct abstract objects are distinct objects; one strong
            // object is itself
            ast::CmpOp::Is | ast::CmpOp::IsNot if !super::disabled("objis") && !a.pts.is_empty() && a.kv.is_bot() => {
                let b = self.eval_right(right, state);
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
                let opaque = |an: &Analyzer, v: &Value| v.pts.iter().any(|o| matches!(an.kind(o), Kind::Ph | Kind::Unknown | Kind::External | Kind::Class | Kind::Module | Kind::Function));
                if opaque(self, &a) || opaque(self, &b) {
                    for o in a.pts.iter().chain(b.pts.iter()) {
                        if self.kind(o) == Kind::Ph {
                            self.record_event("truth", o);
                        }
                    }
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
                if let Some(ph) = a.kv.as_param() {
                    self.record_event("narrow", ph);
                }
                // members of a Flag: bits; else a member is its key
                if !self.member_key.is_empty() && !a.pts.is_empty() {
                    let b = self.eval_right(right, state);
                    if let Some(r) = self.flag_contains(a, &b, state) {
                        return Some(if op == ast::CmpOp::In { r } else { !r });
                    }
                    self.cmp_right = Some(b);
                    let ka = self.keyify(a);
                    if ka.pts.is_empty() {
                        return self.compare_const(op, &ka, right, state);
                    }
                }
                // a literal key in records whose rows decide it
                if a.pts.is_empty() && a.kv.is_literal() && lits.len() == 1 {
                    let k = lits.iter().next().unwrap();
                    let b = self.eval_right(right, state);
                    // a substring test on known strings
                    if let (Some(needle), Some(hay)) = (intern::key_as_str(k), b.kv.as_lit().filter(|_| b.pts.is_empty()).and_then(intern::key_as_str)) {
                        let found = hay.contains(&*needle);
                        return Some(if op == ast::CmpOp::In { found } else { !found });
                    }
                    if b.pts.is_empty() {
                        if let Some(ph) = b.kv.as_param() {
                            self.record_event("narrow", ph);
                        }
                    }
                    // membership in a placeholder record: decided per call site
                    for o in b.pts.iter() {
                        if super::disabled("inph") {
                            break;
                        }
                        if self.kind(o) == Kind::Ph {
                            self.record_event("narrow", o);
                        } else if let Some(src) = self.obj(o).copied_from {
                            self.record_event("narrow", src);
                        }
                    }
                    // a range with known bounds: membership of a known int
                    if let Some(o) = b.single() {
                        if let (Payload::Range(lo, hi, step), Some(x)) = (&self.obj(o).payload, intern::key_as_int(k)) {
                            let (lo, hi, step) = (*lo, *hi, *step);
                            let inside = if step > 0 { x >= lo && x < hi } else { x <= lo && x > hi };
                            let member = inside && (x - lo) % step == 0;
                            return Some(if op == ast::CmpOp::In { member } else { !member });
                        }
                    }
                    // a positional container of known literals: membership is decided
                    if let Some(o) = b.single() {
                        if matches!(self.kind(o), Kind::List | Kind::Tuple | Kind::Set) {
                            if let Some(n) = self.llen(o, state) {
                                let mut all_lit = true;
                                let mut found = false;
                                for i in 0..n {
                                    let sv = state.slot(o, Ns::Pos, key_int(i as i64));
                                    match sv.kv.as_lit().filter(|_| sv.pts.is_empty()) {
                                        Some(x) if x == k => found = true,
                                        Some(_) => {}
                                        None => {
                                            all_lit = false;
                                            break;
                                        }
                                    }
                                }
                                if all_lit {
                                    return Some(if op == ast::CmpOp::In { found } else { !found });
                                }
                            }
                        }
                    }
                    // a view of a dict: its keys are the dict's (live); its values are decided
                    // when they are all known literals
                    if let Some(o) = b.single() {
                        if let Payload::View(vk, inner) = self.obj(o).payload.clone() {
                            match vk {
                                ViewKind::Keys => {
                                    let b2 = inner;
                                    let recs = self.record_objects(&b2);
                                    if !b2.pts.is_empty() && recs.len() == b2.pts.len() && recs.iter().all(|o| matches!(self.kind(*o), Kind::Dict | Kind::Frame)) {
                                        let rows: Vec<Row> = recs.iter().map(|o| state.row(*o, Ns::Item)).collect();
                                        if rows.iter().all(|r| r.definite.contains(k)) {
                                            return Some(op == ast::CmpOp::In);
                                        }
                                        if rows.iter().all(|r| r.closed() && !r.present.contains(k)) {
                                            return Some(op != ast::CmpOp::In);
                                        }
                                    }
                                    return None;
                                }
                                ViewKind::Values => {
                                    if let Some(d) = inner.single().filter(|d| self.kind(*d) == Kind::Dict) {
                                        if let Some(items) = self.literal_dict_items(d, state) {
                                            let mut found = false;
                                            for (_, v) in &items {
                                                match v.kv.as_lit().filter(|_| v.pts.is_empty()) {
                                                    Some(x) if x == k => found = true,
                                                    Some(_) => {}
                                                    None => return None,
                                                }
                                            }
                                            return Some(if op == ast::CmpOp::In { found } else { !found });
                                        }
                                    }
                                    return None;
                                }
                                _ => {}
                            }
                        }
                    }
                    let recs = self.record_objects(&b);
                    if !b.pts.is_empty() && recs.len() == b.pts.len() && recs.iter().all(|o| matches!(self.kind(*o), Kind::Dict | Kind::Frame)) {
                        let rows: Vec<Row> = recs.iter().map(|o| state.row(*o, Ns::Item)).collect();
                        let member = if rows.iter().all(|r| r.definite.contains(k)) {
                            true
                        } else if rows.iter().all(|r| r.closed() && !r.present.contains(k)) {
                            false
                        } else {
                            return None;
                        };
                        return Some(if op == ast::CmpOp::In { member } else { !member });
                    }
                }
                None
            }
            ast::CmpOp::Eq | ast::CmpOp::NotEq => {
                let b = self.eval_right(right, state);
                // Enum members: their keys (a mixed-in one: its value)
                let (a, b, lits) = if self.member_key.is_empty() {
                    (a.clone(), b, lits)
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
                if !crate::interp::disabled("containereq") && ((container(self, &a) && scalar(&b)) || (container(self, &b) && scalar(&a))) {
                    return Some(op == ast::CmpOp::NotEq);
                }
                if !a.pts.is_empty() || !b.pts.is_empty() || !a.kv.is_literal() || !b.kv.is_literal() || lits.len() != 1 || bl.len() != 1 {
                    for v in [&a, &b] {
                        for o in v.pts.iter() {
                            if self.kind(o) == Kind::Ph {
                                self.record_event("truth", o);
                            }
                        }
                        if let Some(ph) = v.kv.as_param() {
                            self.record_event("truth", ph);
                        }
                    }
                    return None;
                }
                let eq = lits == bl;
                Some(if op == ast::CmpOp::Eq { eq } else { !eq })
            }
            ast::CmpOp::Lt | ast::CmpOp::LtE | ast::CmpOp::Gt | ast::CmpOp::GtE => {
                // two single literals of one kind (numbers, or strings) are ordered here;
                // anything else is decided per call site when it is concrete there
                let b = self.eval_right(right, state);
                let bl = crate::domain::kv_literals(&b.kv);
                if !a.pts.is_empty() || !b.pts.is_empty() || !a.kv.is_literal() || !b.kv.is_literal() || lits.len() != 1 || bl.len() != 1 {
                    for v in [&a, &b] {
                        for o in v.pts.iter() {
                            if self.kind(o) == Kind::Ph {
                                self.record_event("truth", o);
                            }
                        }
                        if let Some(ph) = v.kv.as_param() {
                            self.record_event("truth", ph);
                        }
                    }
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
            (Kind::View, Payload::View(ViewKind::Keys, inner)) => {
                let d = inner.single().filter(|d| self.kind(*d) == Kind::Dict)?;
                let row = state.row(d, Ns::Item);
                (row.closed() && row.present == row.definite && !row.symbolic()).then(|| row.present.clone())
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
        if self.kind(oid) != Kind::Set || crate::interp::disabled("setmembers") {
            return;
        }
        let Some(n) = self.llen(oid, state) else { return };
        let mut ks: Vec<KeyId> = Vec::with_capacity(n);
        for i in 0..n {
            let v = state.slot(oid, Ns::Pos, key_int(i as i64));
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
    pub fn exhaust(&mut self, v: &Value, state: &mut State) {
        if super::disabled("exhaust") {
            return;
        }
        if let Some(o) = v.single() {
            if self.obj(o).iterator && self.obj(o).single {
                if let Some(n) = self.llen(o, state) {
                    state.set_slot(o, Ns::Attr, key_str_lit("__cursor"), Value::lit(key_int(n as i64)), true);
                }
            }
        }
    }

    /// An instance whose class (or a base) defines `__bool__` or `__len__`.
    pub fn has_truth_dunder(&self, oid: u32) -> bool {
        match self.instance_class(oid) {
            Some(c) => self.class_lookup(c, "__bool__").is_some() || self.class_lookup(c, "__len__").is_some(),
            None => false,
        }
    }

    /// `truthiness`, with instances whose class defines `__bool__` / `__len__` decided by
    /// calling it (control-flow tests).
    pub fn truth_of(&mut self, v: &Value, state: &mut State, node: &Node) -> Option<bool> {
        if !v.pts.is_empty() && v.pts.iter().any(|o| self.kind(o) == Kind::Instance && self.has_truth_dunder(o)) && !super::disabled("truthdunder") {
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
                let truthy = |k: KeyId| {
                    intern::with_key(k, |kk| match kk {
                        intern::Key::Str(s) => !s.is_empty(),
                        intern::Key::Int(i) => *i != 0,
                        intern::Key::Float(b) => f64::from_bits(*b) != 0.0,
                        intern::Key::Bool(b) => *b,
                        intern::Key::None => false,
                        intern::Key::Tuple(t) => !t.is_empty(),
                        intern::Key::Bytes(b) => !b.is_empty(),
                        intern::Key::Ellipsis => true,
                        intern::Key::Inv(..) => true,
                        intern::Key::Member(..) => true,
                    })
                };
                let mut it = lits.iter();
                let first = truthy(it.next().unwrap());
                if it.all(|k| truthy(k) == first) {
                    return Some(first);
                }
                return None;
            }
        }
        if let Some(ph) = v.kv.as_param() {
            self.record_event("truth", ph);
            return None;
        }
        if !v.kv.is_bot() || v.pts.is_empty() {
            return None;
        }
        let mut verdicts: Vec<bool> = Vec::new();
        for oid in v.pts.iter() {
            match self.kind(oid) {
                Kind::Ph => {
                    self.record_event("truth", oid);
                    return None;
                }
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
                Kind::Instance if self.has_truth_dunder(oid) => return None,
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
        if matches!(op, ast::Operator::Add | ast::Operator::Sub | ast::Operator::Mult | ast::Operator::FloorDiv | ast::Operator::Div | ast::Operator::Mod) {
            for v in [a, b] {
                if let Some(ph) = v.kv.as_param() {
                    self.record_event("narrow", ph);
                }
            }
        }
        // set operations on key views / sets with known members: a set of the result
        if matches!(op, ast::Operator::BitAnd | ast::Operator::BitOr | ast::Operator::Sub | ast::Operator::BitXor) && !super::disabled("keysetops") {
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
            for o in both.iter() {
                if self.kind(o) == Kind::Ph {
                    self.record_event("concrete", o);
                }
            }
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
            if let (Some(x), Some(y)) = (la, lb) {
                if let (Some(sx), Some(sy)) = (intern::key_as_str(x), intern::key_as_str(y)) {
                    return Value::lit(key_str_lit(&format!("{}{}", sx, sy)));
                }
            }
            if let Some(x) = la {
                if intern::key_is_str(x) && matches!(b.kv, KeyVal::Lits(_) | KeyVal::KeysOf(_) | KeyVal::Param(_)) {
                    return Value::kv_only(self.rename_kv(&b.kv, &RenameFn::with_arg(FnKind::Prefix, x)));
                }
            }
            if let Some(y) = lb {
                if intern::key_is_str(y) && matches!(a.kv, KeyVal::Lits(_) | KeyVal::KeysOf(_) | KeyVal::Param(_)) {
                    return Value::kv_only(self.rename_kv(&a.kv, &RenameFn::with_arg(FnKind::Suffix, y)));
                }
            }
            if let (Some(x), Some(y)) = (la, lb) {
                if let Some(r) = num_binop(x, y, |p, q| p + q, |p, q| p + q) {
                    return Value::lit(r);
                }
            }
            // two lists with known positions: the concatenation's positions
            if let (Some(x), Some(y)) = (a.single(), b.single()) {
                if matches!((self.kind(x), self.kind(y)), (Kind::List, Kind::List) | (Kind::Tuple, Kind::Tuple)) && !super::disabled("listcat") {
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
        if op == ast::Operator::Mult && !super::disabled("listcat") {
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
            if !name.is_empty() && !super::disabled("pure") && !self.torch_on {
                if let Some(r) = crate::models::pyvalues::operator_call(name, &[x, y]) {
                    let is_tuple = intern::with_key(r, |k| matches!(k, intern::Key::Tuple(_)));
                    if !is_tuple {
                        return Value::lit(r);
                    }
                }
            }
        }
        if op == ast::Operator::Sub {
            if let (Some(x), Some(y)) = (a.kv.as_lit(), b.kv.as_lit()) {
                if let Some(r) = num_binop(x, y, |p, q| p - q, |p, q| p - q) {
                    return Value::lit(r);
                }
            }
        }
        if let (Some(x), Some(y)) = (a.kv.as_lit(), b.kv.as_lit()) {
            let nonzero = intern::with_key(y, |k| !matches!(k, intern::Key::Int(0)));
            let r = match op {
                ast::Operator::Mult => num_binop(x, y, |p, q| p.wrapping_mul(q), |p, q| p * q),
                ast::Operator::FloorDiv if nonzero => num_binop(x, y, |p, q| p.div_euclid(q), |p, q| (p / q).floor()),
                ast::Operator::Mod if nonzero && !intern::key_is_str(x) => num_binop(x, y, |p, q| p.rem_euclid(q), |p, q| p - (p / q).floor() * q),
                // true division: a float (of the exact quotient, also for two ints)
                ast::Operator::Div if nonzero => {
                    let f = |k: KeyId| -> Option<f64> {
                        match intern::key_get(k) {
                            intern::Key::Int(i) => Some(i as f64),
                            intern::Key::Float(b) => Some(f64::from_bits(b)),
                            intern::Key::Bool(b) => Some(b as i64 as f64),
                            _ => None,
                        }
                    };
                    match (f(x), f(y)) {
                        (Some(p), Some(q)) if q != 0.0 => Some(key_float(p / q)),
                        _ => None,
                    }
                }
                _ => None,
            };
            if let Some(r) = r {
                return Value::lit(r);
            }
        }
        if op == ast::Operator::Mod {
            // `"%(name)s" % mapping`: each named field is a read of the mapping
            if let Some(sx) = a.kv.as_lit().filter(|_| a.pts.is_empty()).and_then(intern::key_as_str) {
                let recs = self.record_objects(b);
                if !recs.is_empty() && sx.contains("%(") && !super::disabled("pctmap") {
                    let mut rest: &str = &sx;
                    while let Some(i) = rest.find("%(") {
                        let after = &rest[i + 2..];
                        let Some(j) = after.find(')') else { break };
                        let name = after[..j].to_string();
                        self.load_item(b, Ns::Item, &Value::lit(key_str_lit(&name)), node, state, true);
                        rest = &after[j..];
                    }
                    // a closed mapping of literals: the text
                    if let ([m], false) = (&recs[..], super::disabled("pure")) {
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
                    // the formatted text depends on the arguments: a parameter among them is
                    // decided per call site
                    let mut arg_kvs: Vec<KeyVal> = vec![b.kv.clone()];
                    if let Some(tp) = b.single() {
                        if matches!(self.kind(tp), Kind::Tuple) {
                            if let Some(n) = self.llen(tp, state) {
                                arg_kvs = (0..n).map(|i| state.slot(tp, Ns::Pos, key_int(i as i64)).kv).collect();
                            }
                        }
                    }
                    for kv in &arg_kvs {
                        if let Some(ph) = kv.as_param() {
                            self.record_event("narrow", ph);
                        }
                    }
                    // "%s ... %d" with the arguments: literal segments around at most one
                    // non-literal argument (a parameter: decided per call site); width and
                    // precision specs are applied when every argument is a literal
                    if let Some(r) = format_percent_literal(&sx, b, state, self) {
                        return Value::lit(key_str_lit(&r));
                    }
                    let sx2 = sx.replace("%d", "%s");
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
                        if args.len() == n {
                            for a in &args {
                                if let Some(ph) = a.as_param() {
                                    self.record_event("narrow", ph);
                                }
                            }
                            let to_str = |kv: &KeyVal| -> KeyVal {
                                match kv.as_lit() {
                                    Some(k) if !intern::key_is_str(k) => KeyVal::Lit(key_str_lit(&intern::key_py_str(k))),
                                    _ => kv.clone(),
                                }
                            };
                            let args: Vec<KeyVal> = args.iter().map(to_str).collect();
                            let nonlit: Vec<usize> = (0..n).filter(|i| args[*i].as_lit().is_none()).collect();
                            if nonlit.len() <= 1 {
                                let hole = nonlit.first().copied();
                                // literal text before and after the hole (or the whole string)
                                let text = |from: usize, to: usize| -> String {
                                    let mut out = String::new();
                                    for i in from..to {
                                        out.push_str(&segs[i]);
                                        if i < n {
                                            out.push_str(&intern::key_py_str(args[i].as_lit().unwrap()));
                                        }
                                    }
                                    out
                                };
                                match hole {
                                    None => return Value::lit(key_str_lit(&text(0, n + 1))),
                                    Some(h) => {
                                        let pre = format!("{}{}", text(0, h), segs[h]);
                                        let post = {
                                            let mut s = String::new();
                                            for i in (h + 1)..(n + 1) {
                                                if i <= n && i > h + 1 - 1 && i - 1 > h {
                                                    s.push_str(&intern::key_py_str(args[i - 1].as_lit().unwrap()));
                                                }
                                                s.push_str(&segs[i]);
                                            }
                                            s
                                        };
                                        let mut kv = args[h].clone();
                                        if !pre.is_empty() {
                                            kv = self.rename_kv(&kv, &RenameFn::with_arg(FnKind::Prefix, key_str_lit(&pre)));
                                        }
                                        if !post.is_empty() {
                                            kv = self.rename_kv(&kv, &RenameFn::with_arg(FnKind::Suffix, key_str_lit(&post)));
                                        }
                                        return Value::kv_only(kv);
                                    }
                                }
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
                                if i.conversion != ast::ConversionFlag::None {
                                    parts.push(Part::Opaque);
                                } else if let Some(spec) = &i.format_spec {
                                    // a literal spec applied to a known value (`{n:03d}`)
                                    let mut spec_text = String::new();
                                    let mut literal_spec = true;
                                    for se in spec.elements.iter() {
                                        match se {
                                            ast::InterpolatedStringElement::Literal(l) => spec_text.push_str(&l.value),
                                            _ => literal_spec = false,
                                        }
                                    }
                                    let v = self.eval(&i.expression, state);
                                    if let Some(ph) = v.kv.as_param() {
                                        self.record_event("narrow", ph);
                                    }
                                    match (literal_spec, v.kv.as_lit().filter(|_| v.pts.is_empty())) {
                                        (true, Some(k)) => match format_spec_literal(k, &spec_text) {
                                            Some(s) => parts.push(Part::Lit(s)),
                                            None => parts.push(Part::Opaque),
                                        },
                                        _ => parts.push(Part::Opaque),
                                    }
                                } else {
                                    let v = self.eval(&i.expression, state);
                                    let v = self.enum_text(&v, state).unwrap_or(v);
                                    parts.push(Part::Hole(v));
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
                let mut kv = v.kv.clone();
                if let Some(k) = kv.as_lit() {
                    if !intern::key_is_str(k) {
                        kv = KeyVal::Lit(key_str_lit(&intern::key_py_str(k)));
                    }
                }
                if !pre.is_empty() {
                    kv = self.rename_kv(&kv, &RenameFn::with_arg(FnKind::Prefix, key_str_lit(&pre)));
                }
                if !post.is_empty() {
                    kv = self.rename_kv(&kv, &RenameFn::with_arg(FnKind::Suffix, key_str_lit(&post)));
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
                    if let Some(ph) = v.kv.as_param() {
                        self.record_event("narrow", ph);
                    }
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
    fn comp(&mut self, gens: &'static [ast::Comprehension], state: &mut State, body: &mut dyn FnMut(&mut Analyzer, &mut State)) {
        self.frame_mut().overlays.push(IndexMap::new());
        self.comp_cells.push(Default::default());
        self.comp_gen(gens, 0, state, body);
        self.comp_cells.pop();
        self.frame_mut().overlays.pop();
    }

    fn comp_gen(&mut self, gens: &'static [ast::Comprehension], i: usize, state: &mut State, body: &mut dyn FnMut(&mut Analyzer, &mut State)) {
        if i == gens.len() {
            body(self, state);
            return;
        }
        let g = &gens[i];
        let itv = self.eval(&g.iter, state);
        let exhaust_after = itv.clone();
        let gn = frontend::node(&g.iter);
        let it = self.iterate(&itv, &gn, state);
        let understood = g.ifs.iter().all(|c| cond_is_exclusion(c, &g.target));
        if let Some(items) = &it.items {
            if items.len() <= UNROLL_MAX {
                // known items: each is its own straight-line iteration; a filter decided
                // for the item keeps it straight-line (positions survive), an undecided one
                // makes the iteration conditional
                self.comp_known.push(true);
                for (idx, item) in items.iter().enumerate() {
                    let mut s2 = state.copy();
                    self.frame_mut().overlays.push(IndexMap::new());
                    self.frame_mut().unroll.push(idx);
                    let decided = self.comp_run_one(g, gens, i, &Elem::One(item.clone()), &mut s2, body, understood);
                    self.frame_mut().unroll.pop();
                    self.frame_mut().overlays.pop();
                    if s2.reachable {
                        if understood || decided {
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
        self.frame_mut().overlays.push(IndexMap::new());
        self.comp_run_one(g, gens, i, &elem, &mut s2, body, understood);
        self.frame_mut().overlays.pop();
        self.comp_known.pop();
        if it.exhaustive && understood && s2.reachable {
            state.replace_with(s2);
        } else {
            let mut joined = state.join(&mut s2);
            let ev = elem.value_part();
            self.promote_elem_reqs(&mut joined, &s2, &ev, state);
            state.replace_with(joined);
        }
    }

    /// Runs one iteration of generator `i`; returns whether every filter was decided (then
    /// the iteration was straight-line code).
    fn comp_run_one(&mut self, g: &'static ast::Comprehension, gens: &'static [ast::Comprehension], i: usize, it: &Elem, st: &mut State, body: &mut dyn FnMut(&mut Analyzer, &mut State), understood: bool) -> bool {
        self.comp_bind(&g.target, it, st);
        let mut decided = true;
        for cond in &g.ifs {
            let cv = self.eval(cond, st);
            match self.truth_of(&cv, st, &frontend::node(cond)) {
                Some(true) => {}
                Some(false) => {
                    st.reachable = false;
                    return decided;
                }
                None => {
                    decided = false;
                    self.narrow(cond, true, st);
                    if !st.reachable {
                        return decided;
                    }
                }
            }
        }
        let conditional = !decided && !understood;
        if conditional {
            self.cond_depth += 1;
        }
        self.comp_gen(gens, i + 1, st, body);
        if conditional {
            self.cond_depth -= 1;
        }
        decided
    }

    fn comp_bind(&mut self, target: &'static ast::Expr, it: &Elem, state: &mut State) {
        let names = names_in(target);
        {
            let ov = self.frame_mut().overlays.last_mut().unwrap();
            for n in names {
                ov.entry(s(&n)).or_insert_with(Value::new);
            }
        }
        self.bind_iter_target(target, it, state);
        if !self.comp_cells.is_empty() && !super::disabled("compcell") {
            // the variable's cell holds its latest value
            let ctx = self.frame().ctx.clone();
            let addr = target as *const ast::Expr as usize;
            for n in names_in(target) {
                let sid = s(&n);
                let cid = self.alloc_raw(Kind::Cell, &format!("compcell:{}:{}", addr, n), &ctx, true, &format!("cell {}", n), Payload::None);
                let v = self.frame().overlays.iter().rev().find_map(|ov| ov.get(&sid).cloned()).unwrap_or_default();
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
        self.comp(&dc.generators, state, &mut body);
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
        let mut body = |an: &mut Analyzer, st: &mut State| {
            let v = an.eval(elt, st);
            if positional && an.comp_known.iter().all(|k| *k) && an.cond_depth == base_depth && n < UNROLL_MAX {
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
        let mark = self.comp_known.len();
        self.comp(gens, state, &mut body);
        self.comp_known.truncate(mark);
        if positional {
            self.set_llen(oid, state, Some(n));
        } else {
            self.set_llen(oid, state, None);
            state.drop_obj_ns(oid, Ns::Pos);
        }
        if kind == Kind::Set {
            self.normalize_set(oid, state);
        }
        Value::obj(oid)
    }
}

/// Ordering of two literals of one kind: numbers (ints, floats, bools) or strings.
pub fn literal_order(x: KeyId, y: KeyId) -> Option<std::cmp::Ordering> {
    let nx = intern::with_key(x, num_of);
    let ny = intern::with_key(y, num_of);
    if let (Some(a), Some(b)) = (nx, ny) {
        return match (a, b) {
            (Num::I(p), Num::I(q)) => Some(p.cmp(&q)),
            (p, q) => p.f().partial_cmp(&q.f()),
        };
    }
    if let (Some(a), Some(b)) = (intern::key_as_str(x), intern::key_as_str(y)) {
        return Some(a.cmp(&b));
    }
    None
}

fn num_binop(x: KeyId, y: KeyId, fi: impl Fn(i64, i64) -> i64, ff: impl Fn(f64, f64) -> f64) -> Option<KeyId> {
    let nx = intern::with_key(x, num_of);
    let ny = intern::with_key(y, num_of);
    match (nx?, ny?) {
        (Num::I(a), Num::I(b)) => Some(key_int(fi(a, b))),
        (a, b) => Some(key_float(ff(a.f(), b.f()))),
    }
}

#[derive(Clone, Copy)]
enum Num {
    I(i64),
    F(f64),
}
impl Num {
    fn f(self) -> f64 {
        match self {
            Num::I(i) => i as f64,
            Num::F(f) => f,
        }
    }
}

fn num_of(k: &intern::Key) -> Option<Num> {
    match k {
        intern::Key::Int(i) => Some(Num::I(*i)),
        intern::Key::Float(b) => Some(Num::F(f64::from_bits(*b))),
        _ => None,
    }
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

/// `"%03d-%s|%.2f" % args` with literal arguments only: the formatted string.
fn format_percent_literal(fmt: &str, b: &Value, state: &State, an: &Analyzer) -> Option<String> {
    // every argument a literal: exactly as Python formats it
    if !super::disabled("pure") {
        let args: Option<Vec<KeyId>> = if b.pts.is_empty() {
            b.kv.as_lit().map(|k| vec![k])
        } else {
            b.single().filter(|o| an.kind(*o) == Kind::Tuple).and_then(|o| {
                let n = an.llen(o, state)?;
                (0..n).map(|i| {
                    let x = state.slot(o, Ns::Pos, key_int(i as i64));
                    x.kv.as_lit().filter(|_| x.pts.is_empty())
                }).collect()
            })
        };
        if let Some(args) = args {
            if let Some(s0) = crate::models::pyvalues::percent_format(fmt, crate::models::pyvalues::PercentArgs::Positional(&args)) {
                return Some(s0);
            }
        }
    }
    // collect the conversion specs
    let bytes: Vec<char> = fmt.chars().collect();
    let mut specs: Vec<(usize, usize, String, char)> = Vec::new(); // (start, end, flags+width+prec, conv)
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == '%' {
            let start = i;
            i += 1;
            if i < bytes.len() && bytes[i] == '%' {
                i += 1;
                continue;
            }
            let mut spec = String::new();
            while i < bytes.len() && (bytes[i].is_ascii_digit() || matches!(bytes[i], '-' | '+' | ' ' | '0' | '.' | '#')) {
                spec.push(bytes[i]);
                i += 1;
            }
            if i >= bytes.len() {
                return None;
            }
            let conv = bytes[i];
            if !matches!(conv, 'd' | 'i' | 's' | 'f' | 'r' | 'x') {
                return None;
            }
            specs.push((start, i + 1, spec, conv));
            i += 1;
        } else {
            i += 1;
        }
    }
    if specs.is_empty() || specs.iter().all(|s| s.2.is_empty()) {
        return None; // the plain path handles these (it also records events)
    }
    let n = specs.len();
    let args: Vec<KeyId> = if n == 1 {
        vec![b.kv.as_lit().filter(|_| b.pts.is_empty())?]
    } else {
        let t = b.single()?;
        if !matches!(an.kind(t), Kind::Tuple) || an.llen(t, state) != Some(n) {
            return None;
        }
        let mut v = Vec::new();
        for i in 0..n {
            let s = state.slot(t, Ns::Pos, key_int(i as i64));
            v.push(s.kv.as_lit().filter(|_| s.pts.is_empty())?);
        }
        v
    };
    let mut out = String::new();
    let mut pos = 0;
    for (k, (start, end, spec, conv)) in specs.iter().enumerate() {
        out.push_str(&bytes[pos..*start].iter().collect::<String>().replace("%%", "%"));
        let zero = spec.starts_with('0');
        let left = spec.starts_with('-');
        let (wpart, prec) = match spec.split_once('.') {
            Some((w, p)) => (w.trim_start_matches(['-', '0', '+', ' ', '#']).to_string(), p.parse::<usize>().ok()),
            None => (spec.trim_start_matches(['-', '0', '+', ' ', '#']).to_string(), None),
        };
        let width: usize = wpart.parse().unwrap_or(0);
        let text = intern::with_key(args[k], |kk| match (conv, kk) {
            ('d' | 'i', intern::Key::Int(v)) => Some(v.to_string()),
            ('d' | 'i', intern::Key::Bool(v)) => Some((*v as i64).to_string()),
            ('d' | 'i', intern::Key::Float(v)) => Some((f64::from_bits(*v).trunc() as i64).to_string()),
            ('f', intern::Key::Int(v)) => Some(format!("{:.*}", prec.unwrap_or(6), *v as f64)),
            ('f', intern::Key::Float(v)) => Some(format!("{:.*}", prec.unwrap_or(6), f64::from_bits(*v))),
            ('x', intern::Key::Int(v)) => Some(format!("{:x}", v)),
            ('s' | 'r', _) => Some(intern::key_py_str(args[k])),
            _ => None,
        })?;
        let padded = if text.chars().count() >= width {
            text
        } else if left {
            format!("{:<w$}", text, w = width)
        } else if zero && matches!(conv, 'd' | 'i' | 'f' | 'x') {
            let neg = text.starts_with('-');
            let body = text.trim_start_matches('-');
            let fill = width - text.chars().count();
            format!("{}{}{}", if neg { "-" } else { "" }, "0".repeat(fill), body)
        } else {
            format!("{:>w$}", text, w = width)
        };
        out.push_str(&padded);
        pos = *end;
    }
    out.push_str(&bytes[pos..].iter().collect::<String>().replace("%%", "%"));
    Some(out)
}

/// builtins whose calls do not raise for the values this analysis sees
/// A builtin exception class: constructing one does not raise.
pub fn is_exception_name(n: &str) -> bool {
    n.ends_with("Error") || n.ends_with("Exception") || n.ends_with("Warning") || matches!(n, "StopIteration" | "KeyboardInterrupt" | "SystemExit" | "GeneratorExit")
}

const SAFE_BUILTINS: &[&str] = &["len", "str", "repr", "bool", "int", "float", "sorted", "reversed", "list", "dict", "set", "tuple", "frozenset", "min", "max", "sum", "abs", "round", "isinstance", "issubclass", "hasattr", "enumerate", "zip", "range", "print", "id", "hash", "type", "callable", "any", "all", "map", "filter", "format", "iter"];

/// `format(value, spec)` for a known literal and a plain spec: `[[fill]align][0][width][.prec][type]`.
pub(crate) fn format_spec_literal(k: KeyId, spec: &str) -> Option<String> {
    if !super::disabled("pure") {
        if let Some(s0) = crate::models::pyvalues::format_value(k, spec) {
            return Some(s0);
        }
    }
    format_spec_literal_old(k, spec)
}

fn format_spec_literal_old(k: KeyId, spec: &str) -> Option<String> {
    let chars: Vec<char> = spec.chars().collect();
    let mut i = 0;
    let mut fill = ' ';
    let mut align: Option<char> = None;
    if chars.len() >= 2 && matches!(chars[1], '<' | '>' | '^') {
        fill = chars[0];
        align = Some(chars[1]);
        i = 2;
    } else if !chars.is_empty() && matches!(chars[0], '<' | '>' | '^') {
        align = Some(chars[0]);
        i = 1;
    }
    let mut zero = false;
    if i < chars.len() && chars[i] == '0' {
        zero = true;
        i += 1;
    }
    let mut width = 0usize;
    while i < chars.len() && chars[i].is_ascii_digit() {
        width = width * 10 + chars[i].to_digit(10).unwrap() as usize;
        i += 1;
    }
    let mut prec: Option<usize> = None;
    if i < chars.len() && chars[i] == '.' {
        i += 1;
        let mut p = 0usize;
        let mut any = false;
        while i < chars.len() && chars[i].is_ascii_digit() {
            p = p * 10 + chars[i].to_digit(10).unwrap() as usize;
            i += 1;
            any = true;
        }
        if !any {
            return None;
        }
        prec = Some(p);
    }
    let ty = if i < chars.len() { chars[i] } else { ' ' };
    if i + 1 < chars.len() {
        return None;
    }
    let text = intern::with_key(k, |kk| match (ty, kk) {
        (' ' | 's', intern::Key::Str(s)) => Some(match prec {
            Some(p) => s.chars().take(p).collect(),
            None => s.to_string(),
        }),
        (' ' | 'd', intern::Key::Int(v)) => Some(v.to_string()),
        ('d', intern::Key::Bool(b)) => Some((*b as i64).to_string()),
        (' ' | 'f', intern::Key::Float(b)) => Some(format!("{:.*}", prec.unwrap_or(6), f64::from_bits(*b))),
        ('f', intern::Key::Int(v)) => Some(format!("{:.*}", prec.unwrap_or(6), *v as f64)),
        ('x', intern::Key::Int(v)) => Some(format!("{:x}", v)),
        (' ', intern::Key::Bool(b)) => Some(if *b { "True".into() } else { "False".into() }),
        _ => None,
    })?;
    let numeric = intern::with_key(k, |kk| matches!(kk, intern::Key::Int(_) | intern::Key::Float(_) | intern::Key::Bool(_)));
    let n = text.chars().count();
    if n >= width {
        return Some(text);
    }
    let pad = width - n;
    Some(match align {
        Some('<') => format!("{}{}", text, fill.to_string().repeat(pad)),
        Some('^') => format!("{}{}{}", fill.to_string().repeat(pad / 2), text, fill.to_string().repeat(pad - pad / 2)),
        Some('>') => format!("{}{}", fill.to_string().repeat(pad), text),
        None if zero && numeric => {
            let neg = text.starts_with('-');
            format!("{}{}{}", if neg { "-" } else { "" }, "0".repeat(pad), text.trim_start_matches('-'))
        }
        None if numeric => format!("{}{}", " ".repeat(pad), text),
        None => format!("{}{}", text, " ".repeat(pad)),
        Some(_) => return None,
    })
}

impl Analyzer {
    /// `a op b` where every object `a` (or else `b`) may be is an instance of a class defining
    /// `__op__` (`__rop__`): that method's result. None: not decided this way.
    pub fn binop_dunder(&mut self, op: ast::Operator, a: &Value, b: &Value, node: &Node, state: &mut State) -> Option<Value> {
        use ast::Operator as O;
        if crate::interp::disabled("dunderops") {
            return None;
        }
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

/// Models that say themselves where they raise (raise_here / may_raise_here): their calls are
/// not "may raise anything".
/// Library constructors whose models report their raises.
const SELF_REPORTING_EXTERNALS: &[&str] = &["io.StringIO", "csv.writer", "csv.DictWriter", "csv.reader", "csv.DictReader"];

const SELF_REPORTING: &[&str] = &["pydantic.", "fieldinfo.", "pydverr.", "typeadapter.", "fastapi.client.", "fastapi.response.", "fastapi.request.", "stdobj.", "fastapi.bg.", "fastapi.app.", "fastapi.router.", "fastapi.state."];

/// The positions `xs[lo:hi:step]` selects from a sequence of length `n` (`slice.indices`).
pub fn slice_positions(n: i64, lo: Option<i64>, hi: Option<i64>, step: i64) -> Vec<i64> {
    let norm = |b: i64| if b < 0 { b + n } else { b };
    if step > 0 {
        let s = lo.map(norm).unwrap_or(0).clamp(0, n);
        let e = hi.map(norm).unwrap_or(n).clamp(0, n);
        (s..e).step_by(step as usize).collect()
    } else {
        let s = lo.map(norm).unwrap_or(n - 1).clamp(-1, n - 1);
        let e = hi.map(norm).unwrap_or(-1).clamp(-1, n - 1);
        let mut v = Vec::new();
        let mut i = s;
        while i > e {
            v.push(i);
            i += step;
        }
        v
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
