//! Expressions: the dispatch by kind (`eval`), names, attributes, subscripts and slices, calls,
//! displays, yields. Operators are in `ops.rs`, comparisons and truth in `compare.rs`,
//! comprehensions in `comps.rs`.

use ruff_python_ast as ast;

use crate::domain::{join_values, kv_join, kv_literals, kv_map, DATA, KeyVal, Ns, Value};
use crate::frontend::{self, Node};
use crate::intern::{self, key_float, key_int, key_str_lit, s, KeyId, STAR};
use crate::source::RenameFn;
use crate::state::{Kind, State};

use super::{Analyzer, Payload, ViewKind};

impl Analyzer {
    /// Evaluate an expression (while a test is evaluated, each node's value is kept for its
    /// narrowing: `split`).
    pub fn eval(&mut self, node: &'static ast::Expr, state: &mut State) -> Value {
        let v = self.eval_inner(node, state);
        if let Some((depth, vals)) = &mut self.mode.trace {
            if *depth == self.cx.frames.len() {
                vals.insert(node as *const ast::Expr as usize, v.clone());
            }
        }
        v
    }

    /// The value of an expression, by its kind (literals, names, operators, calls, displays,
    /// comprehensions, …).
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
                if self.findings.facts_on {
                    self.record_fact(&nd, name, &v, state);
                }
                v
            }
            ast::Expr::Attribute(a) => {
                if !state.narrowed.is_empty() {
                    if let Some(key) = crate::syntax::chain_text(node) {
                        if let Some(hit) = state.narrowed.get(&s(&key)) {
                            return hit.clone();
                        }
                    }
                }
                let obj = self.eval(&a.value, state);
                let nd = frontend::node(node);
                self.load_attr(&obj, a.attr.id.as_str(), &nd, state, true)
            }
            ast::Expr::Subscript(sub) => self.expr_subscript(sub, node, state),
            ast::Expr::Call(c) => self.expr_call(c, node, state),
            ast::Expr::Dict(d) => self.expr_dict(d, node, state),
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
            ast::Expr::UnaryOp(u) => self.expr_unaryop(u, node, state),
            ast::Expr::BinOp(b) => self.expr_binop(b, node, state),
            ast::Expr::Compare(c) => self.expr_compare(c, node, state),
            ast::Expr::FString(f) => self.expr_fstring(f, node, state),
            ast::Expr::TString(_) => {
                let nd = frontend::node(node);
                Value::data(&format!("tstring@L{}", nd.line))
            }
            ast::Expr::Starred(st) => self.eval(&st.value, state),
            ast::Expr::Await(a) => self.eval(&a.value, state),
            ast::Expr::Yield(y) => self.expr_yield(y, state),
            ast::Expr::YieldFrom(y) => self.expr_yield_from(y, state),
            ast::Expr::Slice(_) => self.eval_key(node, state),
            ast::Expr::DictComp(dc) => self.expr_dictcomp(dc, node, state),
            ast::Expr::ListComp(lc) => self.seq_comp(node, Kind::List, &lc.elt, &lc.generators, state),
            ast::Expr::SetComp(sc) => self.seq_comp(node, Kind::Set, &sc.elt, &sc.generators, state),
            ast::Expr::Generator(g) => {
                let v = self.seq_comp(node, Kind::List, &g.elt, &g.generators, state);
                if let Some(o) = v.single() {
                    self.heap.obj_mut(o).iterator = true;
                }
                v
            }
            ast::Expr::IpyEscapeCommand(_) => {
                let nd = frontend::node(node);
                self.findings.note(format!("unsupported expression IpyEscapeCommand at L{}", nd.line));
                Value::data(&format!("L{}", nd.line))
            }
        }
    }

    /// A dict display (`**m` unpacked into it).
    fn expr_dict(&mut self, d: &'static ast::ExprDict, node: &'static ast::Expr, state: &mut State) -> Value {
        let nd = frontend::node(node);
        let oid = self.alloc_at(Kind::Dict, &nd, state, &format!("dict@L{}", nd.line), Payload::None, None, Ns::Item, None, None);
        for item in &d.items {
            let val = self.eval(&item.value, state);
            match &item.key {
                None => self.copy_into(oid, Ns::Item, &val, state, true, &intern::KeySet::new()),
                Some(k) => {
                    let kv = self.eval_key(k, state);
                    self.check_hashable(&kv, state);
                    self.store_key(oid, Ns::Item, &kv.kv, &val, true, &nd, state);
                }
            }
        }
        Value::obj(oid)
    }

    /// `yield from it`: every item of `it` is yielded; the value is the sub-generator's return.
    fn expr_yield_from(&mut self, y: &'static ast::ExprYieldFrom, state: &mut State) -> Value {
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
                    self.heap.set_llen(gid, state, None);
                    state.drop_obj_ns(gid, Ns::Pos);
                }
            }
        }
        // the value: the delegated generator's return value
        let rk = key_str_lit("\u{1}genret");
        if !v.pts.is_empty() && v.kv.is_bot() && v.pts.iter().all(|o| state.slot_opt(o, Ns::Attr, rk).is_some()) {
            return join_values(v.pts.iter().map(|o| state.slot(o, Ns::Attr, rk)).collect::<Vec<_>>());
        }
        Value::data("yield")
    }

    /// `yield v`: the item joins the generator's items; the state here is a suspension point.
    fn expr_yield(&mut self, y: &'static ast::ExprYield, state: &mut State) -> Value {
        let v = match &y.value {
            Some(v) => self.eval(v, state),
            None => self.none_v.clone(),
        };
        // (the with body runs at every yield the generator reaches: one per path)
        if self.gens.cm_hook.as_ref().map(|h| (!h.ran || (h.multi)) && self.cx.frames.len() == h.depth + 1).unwrap_or(false) {
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

    /// A call: the callee, its arguments (`*` / `**` unpacked), `call_value`.
    fn expr_call(&mut self, c: &'static ast::ExprCall, node: &'static ast::Expr, state: &mut State) -> Value {
        if let Some(v) = crate::models::pandas::unknown_drop(self, c, &frontend::node(node), state) {
            return v;
        }
        // (the tensor model reads `x.dim` as a method only when it is called)
        self.m.t_called_attr = frontend::node(&*c.func).addr;
        self.mode.callee_nodes.insert(self.m.t_called_attr);
        let fv = self.eval(&c.func, state);
        let mut flag = false;
        if !self.unwind.tries.is_empty() {
            let all_safe = fv.pts.iter().all(|o| match &self.heap.obj(o).payload {
                Payload::Method(q, _) if self.heap.kind(o) == Kind::Method => super::safe_method(q) || super::SELF_REPORTING.iter().any(|p| q.starts_with(p)),
                // user code: whether it raises is reported by its own analysis
                // builtins that raise only on wrong types (len, str, sorted, …)
                Payload::Str(q) if self.heap.kind(o) == Kind::External => q.strip_prefix("builtins.").map(|n| super::SAFE_BUILTINS.contains(&n) || is_exception_name(n)).unwrap_or(false) || super::SELF_REPORTING_EXTERNALS.contains(&q.as_str())
                    // (making an exception object of a library class: `HTTPException(...)`)
                    || (q.rsplit('.').next().map(|n| n.ends_with("Error") || n.ends_with("Exception")).unwrap_or(false)),
                _ => matches!(self.heap.kind(o), Kind::Function | Kind::Bound | Kind::Class | Kind::Partial),
            });
            if !all_safe {
                if super::debug_level() == 2 {
                    let what: Vec<String> = fv.pts.iter().map(|o| match &self.heap.obj(o).payload { Payload::Method(q, _) | Payload::Str(q) => q.clone(), _ => self.heap.obj(o).label.to_string() }).collect();
                    eprintln!("  call may raise at L{}: {}", frontend::node(node).line, what.join(", "));
                }
                flag = true;
            }
        }
        let nd = frontend::node(node);
        let saved_node = self.unwind.exc_node.replace(nd);
        if let (Some(a @ (ast::Expr::ListComp(_) | ast::Expr::Generator(_))), Some(o)) = (c.arguments.args.first(), fv.single()) {
            if fv.kv.is_bot() && self.heap.kind(o) == Kind::External && matches!(self.heap.external_qual(o).as_str(), "torch.cat" | "torch.concat" | "torch.concatenate" | "torch.stack") {
                self.mode.nonempty_comp = a as *const ast::Expr as usize;
            }
        }
        let r = self.call_value(&fv, &nd, Some(c), state, None);
        self.unwind.exc_node = saved_node;
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

    /// `obj[key]`: an item read (a tensor's indexing goes to the torch model).
    fn expr_subscript(&mut self, sub: &'static ast::ExprSubscript, node: &'static ast::Expr, state: &mut State) -> Value {
        if let Some(v) = crate::models::pandas::unknown_loc(self, sub, &frontend::node(node), state) {
            return v;
        }
        let obj = self.eval(&sub.value, state);
        // a generic pydantic model parametrised: `Page[User]`
        if let Some(c) = obj.single().filter(|c| self.heap.kind(*c) == Kind::Class && crate::models::pydantic::is_validated(self, *c)) {
            if let Some(v) = crate::models::pydantic::pyd_specialise(self, c, &sub.slice, state) {
                return v;
            }
        }
        {
            let nd = frontend::node(node);
            if let Some(v) = crate::models::torch::t_subscript(self, &obj, &sub.slice, &nd, state) {
                return v;
            }
        }
        let key = self.eval_key(&sub.slice, state);
        if let Some(v) = crate::models::pandas::unknown_selection(self, &obj, sub, &key, &frontend::node(node), state) {
            return v;
        }
        let bounds = Self::slice_label_bounds(&key.kv);
        if let Some(v) = bounds.and_then(|b| self.slice_positional(&obj, b, node, state)) {
            return v;
        }
        // a known string sliced / indexed by known numbers (an index out of range raises
        // IndexError: on every string the value may be, or on some)
        let strs: Option<Vec<Vec<char>>> = kv_literals(&obj.kv).iter().map(|k| intern::key_as_str(k).map(|t| t.chars().collect())).collect();
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
        // a text not known indexed or sliced: a str (an index of bytes: an int), the index perhaps
        // out of range, the key perhaps of a wrong type
        if let (true, Some(t @ ("str" | "bytes"))) = (obj.pts.is_empty() && !obj.kv.is_literal() && !self.m.torch_on, self.value_type(&obj)) {
            let slice = matches!(&*sub.slice, ast::Expr::Slice(_));
            if !slice {
                self.may_raise_here("IndexError", state);
                if !matches!(self.value_type(&key), Some("int" | "bool")) {
                    self.may_raise_here("TypeError", state);
                }
            }
            return Value::data(&crate::domain::typed_label(if t == "bytes" && !slice { "int" } else { t }, ""));
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

    /// A subscript's key: a tuple of literals is one literal key, else a tuple object; a slice
    /// an unknown labelled with its known bounds (`slice[1:3:1]@L5`); else the expression.
    pub fn eval_key(&mut self, node: &'static ast::Expr, state: &mut State) -> Value {
        match node {
            ast::Expr::Tuple(t) => {
                let parts: Vec<Value> = t.elts.iter().map(|e| self.eval(e, state)).collect();
                if parts.iter().all(|p| p.pts.is_empty() && p.kv.as_lit().is_some()) {
                    return Value::lit(intern::key_tuple(parts.iter().map(|p| p.kv.as_lit().unwrap()).collect()));
                }
                let nd = frontend::node(node);
                let tid = self.alloc_at(Kind::Tuple, &nd, state, "tuple", Payload::None, None, Ns::Item, None, None);
                self.heap.set_llen(tid, state, Some(parts.len()));
                for (i, p) in parts.iter().enumerate() {
                    state.set_slot(tid, Ns::Pos, key_int(i as i64), p.clone(), true);
                }
                Value::obj(tid)
            }
            ast::Expr::Slice(sl) => {
                let nd = frontend::node(node);
                // known bounds are kept in the label (`slice[1:3:1]@L5`; a missing bound is empty;
                // `slice-prefix`: from the start by one, to a bound not known)
                let mut b: Vec<String> = Vec::new();
                for e in [&sl.lower, &sl.upper, &sl.step] {
                    match e.as_deref() {
                        None => b.push(String::new()),
                        Some(e) => match self.int_value(e, state) {
                            Some(i) => b.push(i.to_string()),
                            None => {
                                let prefix = matches!(b.first().map(|x| x.as_str()), Some("" | "0")) && b.len() == 1;
                                let step1 = sl.step.as_deref().is_none_or(|s| self.int_value(s, state) == Some(1));
                                return Value::data(&format!("slice{}@L{}", if prefix && step1 { "-prefix" } else { "" }, nd.line));
                            }
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

    /// A list, tuple or set display: a container of its elements, by position when no starred
    /// element makes their positions unknown.
    fn container(&mut self, node: &'static ast::Expr, kind: Kind, elts: &'static [ast::Expr], state: &mut State) -> Value {
        let nd = frontend::node(node);
        let oid = self.alloc_at(kind, &nd, state, &format!("{}@L{}", kind.name(), nd.line), Payload::None, None, Ns::Item, None, None);
        let single = self.heap.obj(oid).single;
        let mut vals: Vec<Value> = Vec::new();
        let mut literal = true;
        let mut size_parts: Option<Vec<Result<Value, Value>>> = if self.m.torch_on && elts.iter().any(|e| matches!(e, ast::Expr::Starred(_))) { Some(Vec::new()) } else { None };
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
                if kind == Kind::Set {
                    self.check_hashable(&v, state);
                }
                vals.push(v);
            }
        }
        // `(*x.shape[:-1], n)`: a size
        if let Some(parts) = size_parts {
            if let Some(v) = crate::models::torch::t_size_from_parts(self, &parts, &nd, state) {
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
        if literal && (single || self.heap.llen(oid, state) == Some(vals.len()) || !state.has_slot(oid, Ns::Elem, STAR) || state.slot(oid, Ns::Elem, STAR).pts.is_empty()) {
            // positions are structural: the same literal always has the same length, so a
            // weak (loop) allocation keeps per-position slots, joined across allocations
            self.heap.set_llen(oid, state, Some(vals.len()));
            for (i, v) in vals.iter().enumerate() {
                state.set_slot(oid, Ns::Pos, key_int(i as i64), v.clone(), single);
            }
        } else {
            self.heap.set_llen(oid, state, None);
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
    pub(in crate::interp) fn int_value(&mut self, e: &'static ast::Expr, state: &mut State) -> Option<i64> {
        let v = self.eval(e, state);
        if !v.pts.is_empty() {
            return None;
        }
        intern::key_as_int(v.kv.as_lit()?)
    }

    /// `xs[lo:hi:step]` of one list / tuple of known length: its positions.
    fn slice_positional(&mut self, obj: &Value, (lo, hi, step): (Option<i64>, Option<i64>, Option<i64>), node: &'static ast::Expr, state: &mut State) -> Option<Value> {
        let step = step.unwrap_or(1);
        let oid = obj.single().filter(|o| step != 0 && matches!(self.heap.kind(*o), Kind::List | Kind::Tuple))?;
        let n = self.heap.llen(oid, state)? as i64;
        let vals: Vec<Value> = slice_positions(n, lo, hi, step).iter().map(|i| state.slot(oid, Ns::Pos, key_int(*i))).collect();
        let kv = state.elems.get(&oid).unwrap_or(DATA);
        let kind = self.heap.kind(oid);
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
        let single = self.heap.obj(nid).single;
        let acc = match &vals {
            Some(vs) => join_values(vs.clone()),
            None => elem,
        };
        state.set_slot(nid, Ns::Elem, STAR, Value::with_kv(acc.pts, crate::models::torch::elem_kv(&acc.kv)), single);
        state.elems.insert(nid, kv);
        match vals {
            Some(vs) if single => {
                self.heap.set_llen(nid, state, Some(vs.len()));
                for (i, v) in vs.iter().enumerate() {
                    state.set_slot(nid, Ns::Pos, key_int(i as i64), v.clone(), true);
                }
            }
            _ => self.heap.set_llen(nid, state, None),
        }
        Value::obj(nid)
    }

    /// True/False when a value is definitely truthy/falsy, else None.
    /// The members of a key view / set / frozenset when they are all known: a keys view of a
    /// closed dict, a set built from known literals.
    pub fn known_members(&self, v: &Value, state: &State) -> Option<intern::KeySet> {
        let o = v.single()?;
        match (self.heap.kind(o), &self.heap.obj(o).payload) {
            (Kind::View, Payload::View(ViewKind::Keys | ViewKind::Proxy, inner)) => {
                let d = inner.single().filter(|d| self.heap.kind(*d) == Kind::Dict)?;
                let row = state.row(d, Ns::Item);
                (row.closed() && row.present == row.definite).then(|| row.present.clone())
            }
            (Kind::Set | Kind::List | Kind::Tuple, _) => {
                let n = self.heap.llen(o, state)?;
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
        if self.heap.kind(oid) != Kind::Set {
            return;
        }
        let Some(n) = self.heap.llen(oid, state) else { return };
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
                    self.heap.set_llen(oid, state, None);
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
        self.heap.set_llen(oid, state, Some(sorted.len()));
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
        if !matches!(self.heap.kind(oid), Kind::List | Kind::Tuple) || !self.heap.obj(oid).single || self.frame().loop_depth > 0 {
            return None;
        }
        let n = self.heap.llen(oid, state)?;
        Some((0..n).map(|i| state.slot(oid, Ns::Pos, key_int(i as i64))).collect())
    }

    /// The positions of `oid` become `vals`, and its elements exactly their values (both the
    /// objects in the element slot and the scalars in `state.elems`).
    pub fn set_positions(&mut self, oid: u32, vals: Vec<Value>, state: &mut State) {
        let old = self.heap.llen(oid, state).unwrap_or(0);
        for i in vals.len()..old {
            state.pop_slot(oid, Ns::Pos, key_int(i as i64));
        }
        for (i, v) in vals.iter().enumerate() {
            state.set_slot(oid, Ns::Pos, key_int(i as i64), v.clone(), true);
        }
        if vals.is_empty() {
            state.drop_obj_ns(oid, Ns::Elem);
            state.elems.insert(oid, KeyVal::Lits(intern::KeySet::new()));
        } else {
            let acc = join_values(vals.clone());
            state.set_slot(oid, Ns::Elem, STAR, Value::with_kv(acc.pts.clone(), crate::models::torch::elem_kv(&acc.kv)), true);
            state.elems.insert(oid, acc.kv);
        }
        self.heap.set_llen(oid, state, Some(vals.len()));
    }

    /// One more value among the elements of a list / tuple / set (an append, a store at a
    /// position not known): its objects join the element slot, its scalar part the
    /// elements' (`state.elems`; none recorded there: not known, which a join with another
    /// path's elements must not lose).
    pub fn add_elem(&mut self, oid: u32, v: &Value, state: &mut State) {
        state.set_slot(oid, Ns::Elem, STAR, Value::with_kv(v.pts.clone(), crate::models::torch::elem_kv(&v.kv)), false);
        let cur = state.elems.get(&oid).unwrap_or(DATA);
        state.elems.insert(oid, kv_join(&cur, &v.kv));
    }
}

/// builtins whose calls do not raise for the values this analysis sees
/// A builtin exception class: constructing one does not raise.
pub fn is_exception_name(n: &str) -> bool {
    n.ends_with("Error") || n.ends_with("Exception") || n.ends_with("Warning") || matches!(n, "StopIteration" | "KeyboardInterrupt" | "SystemExit" | "GeneratorExit")
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

}

/// Python's `s[lo:hi:step]` on characters.
/// `text[lo:hi:step]` (None: a zero step).
fn py_slice(cs: &[char], lo: Option<i64>, hi: Option<i64>, step: Option<i64>) -> Option<String> {
    let step = step.unwrap_or(1);
    (step != 0).then(|| slice_positions(cs.len() as i64, lo, hi, step).into_iter().map(|i| cs[i as usize]).collect())
}
