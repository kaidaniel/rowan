//! Expressions (port of the `expr_*` methods, `truthiness`, `binop`, comprehensions).

use indexmap::IndexMap;
use ruff_python_ast as ast;

use crate::domain::{kv_join, kv_map, FnKind, KeyVal, KeysOf, Ns, ObjSet, RenameFn, Row, Value, DATA, BOT};
use crate::frontend::{self, Node};
use crate::intern::{self, key_float, key_int, key_str_lit, s, KeyId, STAR};
use crate::state::{Kind, State};

use super::funcs::Elem;
use super::{is_constant, names_in, Analyzer, Payload, SAFE_METHODS, UNROLL_MAX};

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
                if let ast::Expr::Slice(sl) = &*sub.slice {
                    if let Some(v) = self.slice_positional(&obj, sl, node, state) {
                        return v;
                    }
                }
                let key = self.eval_key(&sub.slice, state);
                let nd = frontend::node(node);
                self.load_item(&obj, Ns::Item, &key, &nd, state, true)
            }
            ast::Expr::Call(c) => {
                let fv = self.eval(&c.func, state);
                if !self.may_raise.is_empty() {
                    let all_safe = fv.pts.iter().all(|o| match &self.obj(o).payload {
                        Payload::Method(q, _) if self.kind(o) == Kind::Method => SAFE_METHODS.contains(&q.as_str()),
                        // user code: whether it raises is reported by its own analysis
                        _ => matches!(self.kind(o), Kind::Function | Kind::Bound | Kind::Class | Kind::Partial),
                    });
                    if !all_safe {
                        *self.may_raise.last_mut().unwrap() = true;
                    }
                }
                let nd = frontend::node(node);
                self.call_value(&fv, &nd, Some(c), state, None)
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
                    match self.truthiness(&tv, state) {
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
                match u.op {
                    ast::UnaryOp::Not => match self.truthiness(&v, state) {
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
                        Value::with_kv(v.pts.filter(|o| o == self.series), DATA)
                    }
                    _ => Value::with_kv(v.pts.filter(|o| o == self.series), DATA),
                }
            }
            ast::Expr::BinOp(b) => {
                let a = self.eval(&b.left, state);
                let c = self.eval(&b.right, state);
                let nd = frontend::node(node);
                self.binop(b.op, &a, &c, &nd, state)
            }
            ast::Expr::Compare(c) => {
                let a = self.eval(&c.left, state);
                if c.ops.len() == 1 {
                    if let Some(b) = self.compare_const(c.ops[0], &a, &c.comparators[0], state) {
                        return Value::lit(intern::key_bool(b));
                    }
                }
                for cmp in c.comparators.iter() {
                    let b = self.eval(cmp, state);
                    if a.pts.contains(self.series) || b.pts.contains(self.series) {
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
                if let Some(v) = &y.value {
                    let v = self.eval(v, state);
                    self.yield_value(v, state);
                }
                Value::data("yield")
            }
            ast::Expr::YieldFrom(y) => {
                let v = self.eval(&y.value, state);
                let nd = frontend::node(&*y.value);
                let v = self.iterate(&v, &nd, state).elem.flat();
                self.yield_value(v, state);
                Value::data("yield")
            }
            ast::Expr::Slice(_) => Value::data("slice"),
            ast::Expr::DictComp(dc) => self.expr_dictcomp(dc, node, state),
            ast::Expr::ListComp(lc) => self.seq_comp(node, Kind::List, &lc.elt, &lc.generators, state),
            ast::Expr::SetComp(sc) => self.seq_comp(node, Kind::Set, &sc.elt, &sc.generators, state),
            ast::Expr::Generator(g) => self.seq_comp(node, Kind::List, &g.elt, &g.generators, state),
            ast::Expr::IpyEscapeCommand(_) => {
                let nd = frontend::node(node);
                self.note(format!("unsupported expression IpyEscapeCommand at L{}", nd.line));
                Value::data(&format!("L{}", nd.line))
            }
        }
    }

    fn yield_value(&mut self, v: Value, state: &mut State) {
        let Some(foid) = self.frame().func else { return };
        let ctx = self.frame().ctx.clone();
        let label = format!("generator {}", self.frame().label);
        let gid = self.alloc_raw(Kind::List, &format!("gen:{}", foid), &ctx, true, &label, Payload::None);
        state.set_slot(gid, Ns::Elem, STAR, v.clone(), false);
        let cur = state.elems.get(&gid).unwrap_or_else(|| v.kv.clone());
        state.elems.insert(gid, kv_join(&cur, &v.kv));
        // straight-line yields keep their order
        let positional = self.frame().single && self.frame().loop_depth == 0 && self.cond_depth == 0 && self.obj(gid).single;
        match (positional, self.obj(gid).literal_len) {
            (true, Some(n)) if n < 16 => {
                state.set_slot(gid, Ns::Pos, key_int(n as i64), v, true);
                self.obj_mut(gid).literal_len = Some(n + 1);
            }
            _ => {
                self.obj_mut(gid).literal_len = None;
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
                self.obj_mut(tid).literal_len = Some(parts.len());
                for (i, p) in parts.iter().enumerate() {
                    state.set_slot(tid, Ns::Pos, key_int(i as i64), p.clone(), true);
                }
                Value::obj(tid)
            }
            ast::Expr::Slice(_) => {
                let nd = frontend::node(node);
                Value::data(&format!("slice@L{}", nd.line))
            }
            _ => self.eval(node, state),
        }
    }

    fn container(&mut self, node: &'static ast::Expr, kind: Kind, elts: &'static [ast::Expr], state: &mut State) -> Value {
        let nd = frontend::node(node);
        let oid = self.alloc_at(kind, &nd, state, &format!("{}@L{}", kind.name(), nd.line), Payload::None, None, Ns::Item, None, None);
        let single = self.obj(oid).single;
        let mut vals: Vec<Value> = Vec::new();
        let mut literal = true;
        for e in elts {
            if let ast::Expr::Starred(st) = e {
                literal = false;
                let inner = self.eval(&st.value, state);
                let en = frontend::node(e);
                let it = self.iterate(&inner, &en, state);
                vals.push(it.elem.one_or_empty());
            } else {
                vals.push(self.eval(e, state));
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
        state.set_slot(oid, Ns::Elem, STAR, Value::with_kv(acc.pts, DATA), single);
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
        if literal && (single || self.obj(oid).literal_len == Some(vals.len()) || !state.has_slot(oid, Ns::Elem, STAR) || state.slot(oid, Ns::Elem, STAR).pts.is_empty()) {
            // positions are structural: the same literal always has the same length, so a
            // weak (loop) allocation keeps per-position slots, joined across allocations
            self.obj_mut(oid).literal_len = Some(vals.len());
            for (i, v) in vals.iter().enumerate() {
                state.set_slot(oid, Ns::Pos, key_int(i as i64), v.clone(), single);
            }
        } else {
            self.obj_mut(oid).literal_len = None;
            state.drop_obj_ns(oid, Ns::Pos);
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
    fn slice_positional(&mut self, obj: &Value, sl: &'static ast::ExprSlice, node: &'static ast::Expr, state: &mut State) -> Option<Value> {
        if sl.step.is_some() {
            return None;
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
        let lo = bound(&sl.lower)?;
        let hi = bound(&sl.upper)?;
        let oid = obj.single()?;
        let kind = self.kind(oid);
        if !matches!(kind, Kind::List | Kind::Tuple) {
            return None;
        }
        let n = self.obj(oid).literal_len? as i64;
        let norm = |b: Option<i64>, dflt: i64| -> i64 {
            let b = b.unwrap_or(dflt);
            let b = if b < 0 { n + b } else { b };
            b.clamp(0, n)
        };
        let lo = norm(lo, 0);
        let hi = norm(hi, n);
        let nd = frontend::node(node);
        let nid = self.alloc_at(kind, &nd, state, &format!("{}@L{}", kind.name(), nd.line), Payload::None, None, Ns::Item, None, None);
        let vals: Vec<Value> = (lo..hi.max(lo)).map(|i| state.slot(oid, Ns::Pos, key_int(i))).collect();
        let single = self.obj(nid).single;
        let acc = crate::domain::join_values(vals.clone());
        state.set_slot(nid, Ns::Elem, STAR, Value::with_kv(acc.pts, DATA), single);
        let kv = state.elems.get(&oid).unwrap_or(DATA);
        state.elems.insert(nid, kv);
        if single {
            self.obj_mut(nid).literal_len = Some(vals.len());
            for (i, v) in vals.iter().enumerate() {
                state.set_slot(nid, Ns::Pos, key_int(i as i64), v.clone(), true);
            }
        } else {
            self.obj_mut(nid).literal_len = None;
        }
        Some(Value::obj(nid))
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
            ast::CmpOp::In | ast::CmpOp::NotIn => {
                if let Some(ph) = a.kv.as_param() {
                    self.record_event("narrow", ph);
                }
                // a literal key in records whose rows decide it
                if a.pts.is_empty() && a.kv.is_literal() && lits.len() == 1 {
                    let k = lits.iter().next().unwrap();
                    let b = self.eval(right, state);
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
                    let recs = self.record_objects(&b);
                    if !b.pts.is_empty() && recs.len() == b.pts.len() && recs.iter().all(|o| matches!(self.kind(*o), Kind::Dict)) {
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
                let b = self.eval(right, state);
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
                let eq = lits == bl;
                Some(if op == ast::CmpOp::Eq { eq } else { !eq })
            }
            _ => None,
        }
    }

    /// True/False when a value is definitely truthy/falsy, else None.
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
                Kind::List | Kind::Tuple | Kind::Set => match self.obj(oid).literal_len {
                    None => return None,
                    Some(n) => verdicts.push(n > 0),
                },
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
        let both = a.pts.union(&b.pts);
        if op == ast::Operator::BitOr {
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
        if op == ast::Operator::Sub {
            if let (Some(x), Some(y)) = (a.kv.as_lit(), b.kv.as_lit()) {
                if let Some(r) = num_binop(x, y, |p, q| p - q, |p, q| p - q) {
                    return Value::lit(r);
                }
            }
        }
        if op == ast::Operator::Mod {
            if let Some(x) = a.kv.as_lit() {
                if let Some(sx) = intern::key_as_str(x) {
                    // "%s ... %d" with the arguments: literal segments around at most one
                    // non-literal argument (a parameter: decided per call site)
                    let sx2 = sx.replace("%d", "%s");
                    let n = sx2.matches("%s").count();
                    if n >= 1 && !sx2.replace("%s", "").contains('%') {
                        let segs: Vec<String> = sx2.split("%s").map(|s| s.to_string()).collect();
                        let args: Vec<KeyVal> = if n == 1 {
                            vec![b.kv.clone()]
                        } else {
                            match b.single() {
                                Some(t) if matches!(self.kind(t), Kind::Tuple) && self.obj(t).literal_len == Some(n) => (0..n).map(|i| state.slot(t, Ns::Pos, key_int(i as i64)).kv).collect(),
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
        if a.pts.contains(self.series) || b.pts.contains(self.series) {
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
                                if i.format_spec.is_some() || i.conversion != ast::ConversionFlag::None {
                                    parts.push(Part::Opaque);
                                } else {
                                    let v = self.eval(&i.expression, state);
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
        let nd = frontend::node(node);
        Value::data(&format!("fstring@L{}", nd.line))
    }

    // comprehensions ---------------------------------------------------------
    fn comp(&mut self, gens: &'static [ast::Comprehension], state: &mut State, body: &mut dyn FnMut(&mut Analyzer, &mut State)) {
        self.frame_mut().overlays.push(IndexMap::new());
        self.comp_gen(gens, 0, state, body);
        self.frame_mut().overlays.pop();
    }

    fn comp_gen(&mut self, gens: &'static [ast::Comprehension], i: usize, state: &mut State, body: &mut dyn FnMut(&mut Analyzer, &mut State)) {
        if i == gens.len() {
            body(self, state);
            return;
        }
        let g = &gens[i];
        let itv = self.eval(&g.iter, state);
        let gn = frontend::node(&g.iter);
        let it = self.iterate(&itv, &gn, state);
        let understood = g.ifs.iter().all(|c| cond_is_exclusion(c, &g.target));
        if let Some(items) = &it.items {
            if items.len() <= UNROLL_MAX {
                if !understood {
                    self.cond_depth += 1;
                }
                for (idx, item) in items.iter().enumerate() {
                    let mut s2 = state.copy();
                    self.frame_mut().overlays.push(IndexMap::new());
                    self.frame_mut().unroll.push(idx);
                    self.comp_run_one(g, gens, i, &Elem::One(item.clone()), &mut s2, body);
                    self.frame_mut().unroll.pop();
                    self.frame_mut().overlays.pop();
                    if s2.reachable {
                        if understood {
                            state.replace_with(s2);
                        } else {
                            let j = state.join(&mut s2);
                            state.replace_with(j);
                        }
                    }
                }
                if !understood {
                    self.cond_depth -= 1;
                }
                return;
            }
        }
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
        if !understood {
            self.cond_depth += 1;
        }
        self.comp_run_one(g, gens, i, &elem, &mut s2, body);
        if !understood {
            self.cond_depth -= 1;
        }
        self.frame_mut().overlays.pop();
        if it.exhaustive && understood && s2.reachable {
            state.replace_with(s2);
        } else {
            let mut joined = state.join(&mut s2);
            let ev = elem.value_part();
            self.promote_elem_reqs(&mut joined, &s2, &ev, state);
            state.replace_with(joined);
        }
    }

    fn comp_run_one(&mut self, g: &'static ast::Comprehension, gens: &'static [ast::Comprehension], i: usize, it: &Elem, st: &mut State, body: &mut dyn FnMut(&mut Analyzer, &mut State)) {
        self.comp_bind(&g.target, it, st);
        for cond in &g.ifs {
            self.eval(cond, st);
            self.narrow(cond, true, st);
            if !st.reachable {
                return;
            }
        }
        self.comp_gen(gens, i + 1, st, body);
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
        self.obj_mut(oid).literal_len = None;
        let mut body = |an: &mut Analyzer, st: &mut State| {
            let v = an.eval(elt, st);
            st.set_slot(oid, Ns::Elem, STAR, Value::with_kv(v.pts.clone(), DATA), false);
            let cur = st.elems.get(&oid).unwrap_or_else(|| v.kv.clone());
            st.elems.insert(oid, kv_join(&cur, &v.kv));
            let _ = an;
        };
        if state.elems.get(&oid).is_none() {
            state.elems.insert(oid, BOT);
        }
        self.comp(gens, state, &mut body);
        Value::obj(oid)
    }
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
