//! Operators and text: `a op b` and `op a` on known values (CPython-exact on literals), the
//! program's operator dunders, f-strings and `%` formatting.

use ruff_python_ast as ast;

use crate::domain::{join_values, key_type, kv_exclude, kv_join, kv_literals, BOT, label_name, label_type, typed_label, DATA, KeyVal, Ns, ObjId, ObjSet, TYPE_NAMES, Value};
use crate::frontend::{self, Node};
use crate::intern::{self, key_float, key_int, key_str_lit, KeyId, STAR};
use crate::source::{FnKind, RenameFn};
use crate::state::{Kind, State};

use super::{Analyzer, Args, Payload};

/// A part of an f-string: literal text, a hole of known value, or a hole whose text is not
/// known.
enum Part {
    Lit(String),
    Hole(Value),
    Opaque,
}

impl Part {
    /// This part, a hole only when its value passes `keep` (else opaque).
    fn clone_if(&self, keep: impl Fn(&Value) -> bool) -> Part {
        match self {
            Part::Hole(v) if !keep(v) => Part::Opaque,
            Part::Hole(v) => Part::Hole(v.clone()),
            Part::Lit(l) => Part::Lit(l.clone()),
            Part::Opaque => Part::Opaque,
        }
    }
}

impl Analyzer {
    /// Literals other than None beside objects (`"dev" if c else ["p", "q"]`): a value whose
    /// parts Python treats as different types. (Beside an unknown object, which may be the
    /// literals too, no: what is done with it covers them. None beside objects: `binop`,
    /// `load_item`, `load_attr`, calls raise for it.)
    pub(in crate::interp) fn mixed(&self, v: &Value) -> bool {
        !v.pts.is_empty() && v.kv.is_literal() && kv_literals(&v.kv).iter().any(|k| k != intern::key_none()) && v.pts.iter().all(|o| self.heap.kind(o) != Kind::Unknown)
    }

    /// The literals a value may be beside what is not known of it: an unknown's (`8080 | ?`),
    /// those beside unknown objects (`8080` of a slot an unknown update may have changed).
    pub(in crate::interp) fn literals_beside(&self, v: &Value) -> intern::KeySet {
        if v.kv.is_literal() && !v.pts.is_empty() && v.pts.iter().all(|o| self.heap.kind(o) == Kind::Unknown) {
            kv_literals(&v.kv)
        } else {
            v.kv.hint()
        }
    }

    /// An operation on values of which one is `mixed`: on its objects and on its literals, each
    /// on a copy of the state; the results and the states joined. None: no such value.
    pub(in crate::interp) fn split_mixed(&mut self, vs: &[&Value], state: &mut State, f: &mut dyn FnMut(&mut Analyzer, &[Value], &mut State) -> Value) -> Option<Value> {
        let i = vs.iter().position(|v| self.mixed(v))?;
        self.run_parts(vs, i, vec![Value::objs(vs[i].pts.clone()), Value::kv_only(vs[i].kv.clone())], state, f)
    }

    /// `split_mixed`, and also on an unknown that may be some literals (`8080 | ?`, literals
    /// beside an unknown object; against values without objects): on them and on the unknown,
    /// which the result keeps as literals it may be (`8081 | port + 1`).
    pub(in crate::interp) fn split_hinted(&mut self, vs: &[&Value], state: &mut State, f: &mut dyn FnMut(&mut Analyzer, &[Value], &mut State) -> Value) -> Option<Value> {
        if let Some(r) = self.split_mixed(vs, state, f) {
            return Some(r);
        }
        // (only against values without objects: the literals beside objects would run their
        // dunders once more, and beside unknown objects those decide which dunders run)
        let scalars = |i: usize| vs.iter().enumerate().all(|(j, w)| j == i || w.pts.is_empty());
        let i = (0..vs.len()).find(|&i| !self.literals_beside(vs[i]).is_empty() && scalars(i))?;
        let v = vs[i];
        let parts = if v.kv.is_literal() {
            vec![Value::objs(v.pts.clone()), Value::kv_only(v.kv.clone())]
        } else {
            // (None among them: the rest is not None, `kv_not_none`)
            let rest = if v.kv.hint().contains(intern::key_none()) { crate::domain::kv_not_none(&v.kv).unhinted() } else { v.kv.unhinted() };
            vec![Value::with_kv(v.pts.clone(), rest), Value::kv_only(KeyVal::lits(v.kv.hint()))]
        };
        self.run_parts(vs, i, parts, state, f)
    }

    /// `f` run with argument `i` replaced by each of `parts`, on a copy of the state each; the
    /// results and the states joined.
    fn run_parts(&mut self, vs: &[&Value], i: usize, parts: Vec<Value>, state: &mut State, f: &mut dyn FnMut(&mut Analyzer, &[Value], &mut State) -> Value) -> Option<Value> {
        let mut out = Value::new();
        let mut acc: Option<State> = None;
        for part in parts {
            let mut st = state.copy();
            let mut args: Vec<Value> = vs.iter().map(|v| (*v).clone()).collect();
            args[i] = part;
            let r = f(self, &args, &mut st);
            if st.reachable {
                out = out.join(&r);
            }
            acc = Some(match acc {
                None => st,
                Some(mut a) => a.join(&mut st),
            });
        }
        state.replace_with(acc.unwrap());
        Some(out)
    }

    /// `a op b` (`binop`).
    pub(super) fn expr_binop(&mut self, b: &'static ast::ExprBinOp, node: &'static ast::Expr, state: &mut State) -> Value {
        let a = self.eval(&b.left, state);
        let c = self.eval(&b.right, state);
        let nd = frontend::node(node);
        let r = self.binop(b.op, &a, &c, &nd, state);
        self.named_by_expr(r, &nd)
    }

    /// A call's or unary operator's unknown result of no name and type (`d.copy()` of an
    /// unknown `d`, `-n`) named by the expression, as a binary operator's is.
    pub(super) fn named_call(&self, r: Value, nd: &Node) -> Value {
        let KeyVal::Data(l, _) = &r.kv else { return r };
        let l = intern::str_get(*l);
        // (a label with a type, a name or a model's meaning is kept)
        let unnamed = l.is_empty() || &*l == "copy" || (l.starts_with('L') && l[1..].chars().all(|c| c.is_ascii_digit()));
        // (the tensor model reads a named unknown as one that may be a tensor)
        if !r.pts.is_empty() || !unnamed || self.m.torch_on {
            return r;
        }
        self.named_by_expr(r, nd)
    }

    /// An operator's unknown result named by the expression (`1 + b`, `-n`), of its type when
    /// known.
    fn named_by_expr(&self, r: Value, nd: &Node) -> Value {
        if let (true, KeyVal::Data(l, _)) = (r.pts.is_empty(), &r.kv) {
            let (l, h) = (intern::str_get(*l), r.kv.hint());
            let text = frontend::text_at(nd).split_whitespace().collect::<Vec<_>>().join(" ");
            if !l.starts_with('\u{1}') && !text.is_empty() && text.len() <= 60 {
                // the expression names the value only inside this function (its names)
                let scope = self.frame().func.unwrap_or(0);
                let name = format!("expr:{}:{}", scope, text);
                let label = match label_type(&l) {
                    Some(t) => typed_label(t, &name),
                    None => name,
                };
                return Value::kv_only(KeyVal::Data(intern::s(&label), h));
            }
        }
        r
    }

    /// The builtin type every part of `v` has (`domain::TYPE_NAMES`): its literals', its typed
    /// unknowns' (a label's, an annotated parameter's); None when a part's is not known or
    /// they differ.
    pub fn value_type(&self, v: &Value) -> Option<&'static str> {
        let mut ty: Option<&'static str> = None;
        let mut add = |t: Option<&'static str>| -> Option<()> {
            let t = t?;
            if ty.is_some_and(|u| u != t) {
                return None;
            }
            ty = Some(t);
            Some(())
        };
        for o in v.pts.iter() {
            add(self.heap.obj(o).ty)?;
        }
        match &v.kv {
            KeyVal::Bot => {}
            KeyVal::Data(l, _) => add(label_type(&intern::str_get(*l)))?,
            KeyVal::Lit(_) | KeyVal::Lits(_) => {
                for k in kv_literals(&v.kv).iter() {
                    add(TYPE_NAMES.iter().copied().find(|t| key_type(k, t)))?;
                }
            }
            KeyVal::KeysOf(_) => return None,
        }
        ty
    }

    /// `v`, an unknown of type `ty` when it is an unknown scalar (named as it is, unless `named`
    /// is false: its name is what it was made from, not what it is).
    pub fn as_type(&self, v: Value, ty: Option<&'static str>, named: bool) -> Value {
        let (Some(t), true, KeyVal::Data(l, _)) = (ty, v.pts.is_empty() && !self.m.torch_on, &v.kv) else { return v };
        let l = intern::str_get(*l);
        let name = label_name(&l);
        // (labels that carry more than a name — a JSON text, a shape, a pattern — are the
        // models': left as they are; `len` is kept, comparisons read it)
        let plain = name.is_empty() || name.starts_with("expr:") || matches!(name, "str" | "int" | "bool" | "len") || (name.starts_with('L') && name[1..].chars().all(|c| c.is_ascii_digit()));
        if !plain || label_type(&l) == Some(t) {
            return v;
        }
        let keep = (named && !matches!(name, "str" | "int" | "bool")) || name == "len";
        // (the literals it may be: those of the type)
        let hint = v.kv.hint().iter().filter(|k| key_type(*k, t)).collect();
        Value::kv_only(KeyVal::Data(intern::s(&typed_label(t, if keep { name } else { "" })), hint))
    }

    /// `-x`, `+x`, `~x`, `not x`.
    pub(super) fn expr_unaryop(&mut self, u: &'static ast::ExprUnaryOp, node: &'static ast::Expr, state: &mut State) -> Value {
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
                if let Some(r) = crate::models::torch::t_neg_named(self, &v, state) {
                    return r;
                }
            }
        }
        // (`-x` of an int is an int; `not x` a bool)
        let ty = match (u.op, self.value_type(&v)) {
            (ast::UnaryOp::Not, _) => Some("bool"),
            (ast::UnaryOp::Invert, Some("int" | "bool")) => Some("int"),
            (ast::UnaryOp::USub | ast::UnaryOp::UAdd, Some("int" | "bool")) => Some("int"),
            (ast::UnaryOp::USub | ast::UnaryOp::UAdd, Some("float")) => Some("float"),
            _ => None,
        };
        // (None, a str, a builtin container: no `-`, `+`, `~` — TypeError)
        let invert = u.op == ast::UnaryOp::Invert;
        if u.op != ast::UnaryOp::Not && self.builtin_operand(&v) {
            let bad = |k: KeyId| intern::with_key(k, |k| matches!(k, intern::Key::None | intern::Key::Str(_) | intern::Key::Bytes(_) | intern::Key::Tuple(_)) || (invert && matches!(k, intern::Key::Float(_))));
            let lits = kv_literals(&v.kv);
            let (some, all) = (!v.pts.is_empty() || lits.iter().any(bad), lits.iter().all(bad));
            if some && all {
                self.raise_here("TypeError", state);
                return Value::new();
            } else if some {
                self.may_raise_here("TypeError", state);
            }
        }
        // (an unknown's operator is code rowan cannot see)
        if u.op != ast::UnaryOp::Not && !self.unwind.tries.is_empty() && self.untyped_operand(&v) && !crate::models::pandas::is_seriesish(self, &v) {
            self.flag_may_raise(state);
        }
        // (the program's instances whose class lacks the operator's dunder: TypeError)
        let dunder = match u.op {
            ast::UnaryOp::USub => "__neg__",
            ast::UnaryOp::UAdd => "__pos__",
            _ => "__invert__",
        };
        let v = if u.op == ast::UnaryOp::Not { v } else {
            match self.without_dunders(&v, &[dunder], state) {
                Some(v) => v,
                None => return Value::new(),
            }
        };
        let r = self.unary_untyped(u, v, node, state);
        let r = self.as_type(r, ty, false);
        if u.op == ast::UnaryOp::Not {
            return r;
        }
        self.named_call(r, &frontend::node(node))
    }

    /// `op v` (`not`, `-`, `+`, `~`) before the result's type is applied.
    fn unary_untyped(&mut self, u: &'static ast::ExprUnaryOp, v: Value, node: &'static ast::Expr, state: &mut State) -> Value {
        // (the program's instances: their `__neg__` / `__pos__` / `__invert__`)
        let dunder = match u.op {
            ast::UnaryOp::USub => Some("__neg__"),
            ast::UnaryOp::UAdd => Some("__pos__"),
            ast::UnaryOp::Invert => Some("__invert__"),
            ast::UnaryOp::Not => None,
        };
        if let Some((r, rest)) = dunder.and_then(|d| self.run_dunder(&v, &[d], Vec::new(), &frontend::node(node), state)) {
            return if rest.empty() { r } else { r.join(&self.unary_untyped(u, rest, node, state)) };
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
                    return Value::with_kv(ObjSet::one(self.m.series), DATA);
                }
                Value::with_kv(v.pts.filter(|o| o == self.m.series), DATA)
            }
            _ if crate::models::pandas::is_seriesish(self, &v) => Value::with_kv(ObjSet::one(self.m.series), DATA),
            _ => Value::with_kv(v.pts.filter(|o| o == self.m.series), DATA),
        }
    }

    /// Is `v` partly of no type known here: an unknown or library object (not one of a builtin
    /// scalar type), an unknown scalar of no type? Its operators are code rowan cannot see.
    pub(in crate::interp) fn untyped_operand(&self, v: &Value) -> bool {
        let untyped_label = matches!(&v.kv, KeyVal::Data(l, _) if crate::domain::label_base(&intern::str_get(*l)).is_none());
        untyped_label || v.pts.iter().any(|o| matches!(self.heap.kind(o), Kind::Unknown | Kind::External) && self.heap.obj(o).ty.is_none())
    }

    /// Is `v` literals and builtin containers only (no unknown, no instance whose methods an
    /// operator may run)?
    pub(in crate::interp) fn builtin_operand(&self, v: &Value) -> bool {
        !matches!(v.kv, KeyVal::Data(_, _) | KeyVal::KeysOf(_)) && !crate::models::pandas::is_seriesish(self, v) && v.pts.iter().all(|o| matches!(self.heap.kind(o), Kind::List | Kind::Dict | Kind::Tuple | Kind::Set | Kind::View))
    }

    /// `t op= v`, t's value being `cur`: Python changes t's object in place where its type
    /// has an in-place operator: `xs += ys` on a list (`list.extend`), `d |= e` on a dict
    /// (`dict.update`), `s |= t` (`&=`, `-=`, `^=`) on a set, the program's instances whose
    /// class defines `__iop__`. Each of those runs on a path of its own, and every other part
    /// of `cur` (an unknown object, a tuple, `9` of `[1] | 9`) gets `t op v` on another (None's
    /// raise there); `store` binds t on each path. Sending the whole value through `binop`
    /// when only some of it can change in place would make a new object, and an alias of the
    /// old one would miss the change. Returns the result, or None (nothing changes in place:
    /// `t = t op v`).
    pub fn binop_in_place(&mut self, op: ast::Operator, cur: &Value, rhs: &Value, node: &Node, state: &mut State, store: &dyn Fn(&mut Analyzer, &Value, &mut State)) -> Option<Value> {
        let in_place = |kind: Kind| match (op, kind) {
            (ast::Operator::Add, Kind::List) => Some("list.extend"),
            (ast::Operator::BitOr, Kind::Dict) => Some("dict.update"),
            (ast::Operator::BitOr, Kind::Set) => Some("set.update"),
            (ast::Operator::BitAnd, Kind::Set) => Some("set.intersection_update"),
            (ast::Operator::Sub, Kind::Set) => Some("set.difference_update"),
            (ast::Operator::BitXor, Kind::Set) => Some("set.symmetric_difference_update"),
            _ => None,
        };
        let idunder = format!("__i{}__", dunder_stem(op));
        let (dunder_calls, after) = self.method_calls(cur, &[idunder.as_str()]);
        let mut groups: Vec<(&'static str, Vec<ObjId>)> = Vec::new();
        let mut others: Vec<ObjId> = Vec::new();
        for o in after.pts.iter() {
            match in_place(self.heap.kind(o)) {
                Some(m) => match groups.iter_mut().find(|(g, _)| *g == m) {
                    Some((_, os)) => os.push(o),
                    None => groups.push((m, vec![o])),
                },
                None => others.push(o),
            }
        }
        if groups.is_empty() && dunder_calls.is_empty() {
            return None;
        }
        let base = state.copy();
        let mut paths = Vec::new();
        let mut result = Value::new();
        let rest = Value::with_kv(ObjSet::from_iter(others), cur.kv.clone());
        if !rest.pts.is_empty() || !rest.kv.is_bot() {
            let mut alt = base.copy();
            store(self, &rest, &mut alt);
            let r = self.binop(op, &rest, rhs, node, &mut alt);
            store(self, &r, &mut alt);
            result = result.join(&r);
            paths.push(alt);
        }
        if !dunder_calls.is_empty() {
            // (a method that returns NotImplemented: Python goes on with `t op v`)
            let objs = Value::objs(cur.pts.filter(|o| !after.pts.contains(o)));
            let mut path = base.copy();
            store(self, &objs, &mut path);
            if let Some((r, _)) = self.run_dunder(&objs, &[idunder.as_str()], vec![rhs.clone()], node, &mut path) {
                let ni: Vec<ObjId> = r.pts.iter().filter(|o| self.heap.kind(*o) == Kind::External && self.heap.external_qual(*o) == "builtins.NotImplemented").collect();
                let mut r = Value::with_kv(r.pts.filter(|o| !ni.contains(&o)), r.kv.clone());
                if !ni.is_empty() {
                    r = r.join(&self.binop(op, &objs, rhs, node, &mut path));
                }
                store(self, &r, &mut path);
                result = result.join(&r);
                paths.push(path);
            }
        }
        for (m, os) in groups {
            let mut path = base.copy();
            let objs = Value::objs(ObjSet::from_iter(os));
            let mv = self.method_value(m, &objs);
            self.call_value(&mv, node, None, &mut path, Some(vec![rhs.clone()]));
            store(self, &objs, &mut path);
            result = result.join(&objs);
            paths.push(path);
        }
        let mut paths = paths.into_iter();
        let Some(first) = paths.next() else {
            state.reachable = false;
            return Some(result);
        };
        *state = first;
        for mut p in paths {
            *state = state.join(&mut p);
        }
        Some(result)
    }

    /// `a op b`: each part on its own (literals beside objects), CPython's result on known
    /// literals, the program's operator dunders, the models' (tensors, frames, …), else an
    /// unknown of the type the operands' types give (`int + int`: `int(?)`).
    pub fn binop(&mut self, op: ast::Operator, a: &Value, b: &Value, node: &Node, state: &mut State) -> Value {
        // None with a literal or a builtin container: no operator takes it (TypeError); the
        // rest of its value goes on
        for (x, y, first) in [(a, b, true), (b, a, false)] {
            if x.kv.is_literal() && kv_literals(&x.kv).contains(intern::key_none()) && self.builtin_operand(y) {
                let rest = Value::with_kv(x.pts.clone(), kv_exclude(&x.kv, &intern::KeySet::one(intern::key_none())).unwrap_or(BOT));
                if rest.pts.is_empty() && rest.kv.is_bot() {
                    self.raise_here("TypeError", state);
                    return Value::new();
                }
                self.may_raise_here("TypeError", state);
                return if first { self.binop(op, &rest, y, node, state) } else { self.binop(op, y, &rest, node, state) };
            }
        }
        if let Some(r) = self.split_hinted(&[a, b], state, &mut |an, vs, st| an.binop(op, &vs[0], &vs[1], node, st)) {
            return r;
        }
        let scalar = |v: &Value| v.pts.iter().all(|o| matches!(self.heap.obj(o).ty, Some("int" | "float" | "bool" | "str" | "bytes")));
        if scalar(a) && scalar(b) && self.scalar_binop_raises(op, a, b, state) {
            return Value::new();
        }
        if self.seq_binop_raises(op, a, b) {
            self.raise_here("TypeError", state);
            return Value::new();
        }
        let r = self.binop_untyped(op, a, b, node, state);
        let ty = binop_type(op, self.value_type(a), self.value_type(b));
        self.as_type(r, ty, true)
    }

    /// Does `a op b` surely raise TypeError for a builtin list or tuple operand: an operator
    /// sequences do not have (`[1] | [2]`, `xs - ys`) with another builtin value, or `+` of a
    /// list and a tuple.
    fn seq_binop_raises(&self, op: ast::Operator, a: &Value, b: &Value) -> bool {
        use ast::Operator::*;
        let kind = |v: &Value| -> Option<Kind> {
            let k = self.heap.kind(v.pts.iter().next()?);
            (v.kv.is_bot() && matches!(k, Kind::List | Kind::Tuple) && v.pts.iter().all(|o| self.heap.kind(o) == k && !self.heap.obj(o).iterator)).then_some(k)
        };
        // (literals other than str, which `%` formats with a sequence)
        let lits = |v: &Value| v.pts.is_empty() && v.kv.is_literal() && !kv_literals(&v.kv).iter().any(intern::key_is_str);
        let builtin = |v: &Value| kind(v).is_some() || lits(v) || (v.kv.is_bot() && !v.pts.is_empty() && v.pts.iter().all(|o| matches!(self.heap.kind(o), Kind::Dict | Kind::Set)));
        match op {
            Add => matches!((kind(a), kind(b)), (Some(x), Some(y)) if x != y),
            BitOr | BitAnd | BitXor | Sub | Div | FloorDiv | Pow | LShift | RShift | MatMult | Mod => (kind(a).is_some() && builtin(b)) || (kind(b).is_some() && (kind(a).is_some() || lits(a))),
            _ => false,
        }
    }

    /// `a op b` before the result's type is applied: the program's dunders and the models'
    /// (`binop_by_models`), operators on a parameter (decided per call site), set operations on
    /// known members, CPython's result on two literals, `%` formatting, pandas results, else an
    /// unknown.
    fn binop_untyped(&mut self, op: ast::Operator, a: &Value, b: &Value, node: &Node, state: &mut State) -> Value {
        if let Some(v) = self.binop_by_models(op, a, b, node, state) {
            return v;
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
            let recs: Vec<u32> = both.iter().filter(|o| self.heap.kind(*o) == Kind::Dict).collect();
            if !recs.is_empty() {
                let oid = self.alloc_at(Kind::Dict, node, state, &format!("dict@L{}", node.line), Payload::None, None, Ns::Item, None, None);
                self.copy_into(oid, Ns::Item, a, state, true, &intern::KeySet::new());
                self.copy_into(oid, Ns::Item, b, state, true, &intern::KeySet::new());
                return self.with_rest(op, a, b, Kind::Dict, Value::obj(oid), node, state);
            }
        }
        if op == ast::Operator::Add {
            if let Some(v) = self.binop_add_seq(a, b, &both, node, state) {
                return v;
            }
        }
        if op == ast::Operator::Mult {
            if let Some(v) = self.binop_mult_seq(a, b, node, state) {
                return v;
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
            // (a text's formatting: what it may raise, its unknown objects' dunders too, is
            // `percent_raises`'s)
            let text = a.pts.is_empty() && self.value_type(a) == Some("str");
            if text && self.percent_raises(a, b, state) {
                return Value::new();
            }
            if let Some(v) = self.binop_percent(a, b, node, state) {
                return v;
            }
            if text && !crate::models::pandas::is_seriesish(self, b) {
                return Value::data(&format!("L{}", node.line));
            }
        }
        if crate::models::pandas::is_seriesish(self, &a) || crate::models::pandas::is_seriesish(self, &b) {
            return Value::with_kv(ObjSet::one(self.m.series), DATA);
        }
        let frames = both.filter(|o| self.heap.kind(o) == Kind::Frame);
        if !frames.is_empty() {
            return Value::with_kv(frames, DATA);
        }
        // (no dunder or model took the objects)
        self.objects_op_raises(&both, state);
        Value::data(&format!("L{}", node.line))
    }

    /// An operator on objects `both` no dunder or model decided: unknown objects' operators are
    /// unknown code (any raise), the others' a TypeError.
    fn objects_op_raises(&mut self, both: &ObjSet, state: &mut State) {
        // (an unknown of a builtin type, an annotated parameter's: a scalar's, `scalar_binop_raises`)
        let both = both.filter(|o| !matches!(self.heap.obj(o).ty, Some("int" | "float" | "bool" | "str" | "bytes")));
        if both.is_empty() || self.unwind.tries.is_empty() {
            return;
        }
        if both.iter().any(|o| matches!(self.heap.kind(o), Kind::Unknown | Kind::External)) {
            self.flag_may_raise(state);
        } else {
            self.may_raise_here("TypeError", state);
        }
    }

    /// What `a op b` of scalars may raise (true: surely): a TypeError where their types do not
    /// surely fit, a ZeroDivisionError for `/`, `//`, `%` by what may be zero (surely: numeric
    /// literals by 0), for `**` a ZeroDivisionError (`0 ** -1`) or an OverflowError, a
    /// ValueError for a shift by what may be negative. Two literals Python computes raise
    /// nothing; `text % args` is formatting's (`percent_raises`). A model's number (a tensor
    /// dim: an int; a dim's quotient: a float) is one; its other labels (tensors) are its own.
    fn scalar_binop_raises(&mut self, op: ast::Operator, a: &Value, b: &Value, state: &mut State) -> bool {
        use ast::Operator::*;
        let model_ty = |v: &Value| -> Option<Option<&'static str>> {
            let KeyVal::Data(l, _) = v.kv else { return None };
            let l = intern::str_get(l);
            let l = l.strip_prefix('\u{1}')?;
            Some(match l.as_bytes().first() {
                Some(b'D') => Some("int"),
                Some(b'Q') => Some("float"),
                // (any number)
                _ if l == "N" => Some("number"),
                _ => None,
            })
        };
        if self.unwind.tries.is_empty() || model_ty(a) == Some(None) || model_ty(b) == Some(None) {
            return false;
        }
        if let (Some(x), Some(y)) = (a.kv.as_lit(), b.kv.as_lit()) {
            let name = match op { Add => "add", Sub => "sub", Mult => "mul", Div => "truediv", FloorDiv => "floordiv", Mod => "mod", Pow => "pow", LShift => "lshift", RShift => "rshift", BitOr => "or_", BitXor => "xor", BitAnd => "and_", MatMult => "" };
            if crate::models::pyvalues::operator_call(name, &[x, y]).is_some() {
                return false;
            }
        }
        let (ta, tb) = (model_ty(a).flatten().or(self.value_type(a)), model_ty(b).flatten().or(self.value_type(b)));
        if op == Mod && (matches!(ta, Some("str" | "bytes")) || a.kv.as_lit().and_then(intern::key_as_str).is_some()) {
            return false;
        }
        let num = |t: Option<&str>| matches!(t, Some("int" | "bool" | "float" | "number"));
        // ("number": int or float, of the type either gives)
        let fit = |t: Option<&'static str>| if t == Some("number") { Some("float") } else { t };
        if binop_type(op, fit(ta), fit(tb)).is_none() && !(op == Pow && num(ta) && num(tb)) {
            self.may_raise_here("TypeError", state);
        }
        let zero = |k: KeyId| intern::with_key(k, |k| matches!(k, intern::Key::Int(0) | intern::Key::Bool(false)) || matches!(k, intern::Key::Float(f) if f64::from_bits(*f) == 0.0));
        let unknown = |v: &Value| !v.pts.is_empty() || matches!(v.kv, KeyVal::Data(_, _) | KeyVal::KeysOf(_));
        let (la, lb) = (kv_literals(&a.kv), kv_literals(&b.kv));
        match op {
            Div | FloorDiv | Mod if !matches!(ta, Some("str" | "bytes")) => {
                // (a number, known or of a numeric type, by zeros)
                let number = (!unknown(a) && !la.is_empty() && la.iter().all(intern::key_is_number)) || (a.pts.is_empty() && num(ta));
                if number && !unknown(b) && !lb.is_empty() && lb.iter().all(zero) {
                    self.raise_here("ZeroDivisionError", state);
                    return true;
                }
                if unknown(b) || lb.iter().any(zero) {
                    self.may_raise_here("ZeroDivisionError", state);
                }
            }
            Pow if unknown(a) || unknown(b) || (!la.is_empty() && !lb.is_empty()) => {
                self.may_raise_here("ZeroDivisionError", state);
                self.may_raise_here("OverflowError", state);
            }
            LShift | RShift if unknown(b) || lb.iter().any(|k| intern::key_as_int(k).is_some_and(|n| n < 0)) => {
                self.may_raise_here("ValueError", state);
            }
            _ => {}
        }
        false
    }

    /// `a op b` where the program's dunders or a library model decide it: `__add__` & co., a
    /// path's `/`, Flag members, tensors, Counters.
    fn binop_by_models(&mut self, op: ast::Operator, a: &Value, b: &Value, node: &Node, state: &mut State) -> Option<Value> {
        if let Some(v) = self.binop_dunder(op, a, b, node, state) {
            return Some(v);
        }
        if op == ast::Operator::Div {
            if let Some(v) = crate::models::pathtext::path_div(self, state, a, b, node) {
                return Some(v);
            }
        }
        if let Some(v) = crate::models::enums::flag_binop(self, op, a, b, node, state) {
            return Some(v);
        }
        if let Some(v) = crate::models::torch::t_binop(self, op, a, b, node, state) {
            return Some(v);
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
                return Some(v);
            }
        }
        None
    }

    /// `text % args`: `%(name)s` fields read the mapping; literal arguments format as Python does;
    /// one unknown argument keeps the text around it.
    fn binop_percent(&mut self, a: &Value, b: &Value, node: &Node, state: &mut State) -> Option<Value> {
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
                // (the values it formats may be the program's instances)
                self.library_dunders(std::slice::from_ref(b), state, node);
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
                                return Some(Value::lit(key_str_lit(&s0)));
                            }
                        }
                    }
                }
                return Some(Value::data(&format!("fmt@L{}", node.line)));
            }
        }
        let Some(sx) = a.kv.as_lit().filter(|_| a.pts.is_empty()).and_then(intern::key_as_str) else {
            // (a text not known: it may format the instances it is given)
            self.library_dunders(std::slice::from_ref(b), state, node);
            return None;
        };
        let (sx, args) = self.percent_args(&sx, b, node, state)?;
        // arguments that are each one of a few literals: every combination, as Python formats it
        if let Some(rs) = format_percent_alternatives(&sx, &args) {
            return Some(Value::kv_only(KeyVal::lits(crate::intern::KeySet::from_iter(rs.iter().map(|r| key_str_lit(r))))));
        }
        // "a%sb%s" with one argument not a literal: the literal text around it (a key built
        // from a parameter keeps its prefix and suffix)
        let n = sx.matches("%s").count();
        if n >= 1 && n == args.len() && !sx.replace("%s", "").contains('%') {
            let segs: Vec<String> = sx.split("%s").map(|s| s.to_string()).collect();
            // (an object's text is not known: an unknown)
            let args: Vec<KeyVal> = args.into_iter().map(|v| if v.pts.is_empty() { v.kv } else { DATA }).collect();
            let lit = |i: usize| args[i].as_lit().map(intern::key_py_str);
            if let [h] = (0..args.len()).filter(|i| lit(*i).is_none()).collect::<Vec<_>>()[..] {
                let pre: String = (0..h).map(|i| format!("{}{}", segs[i], lit(i).unwrap())).collect::<String>() + &segs[h];
                let post: String = segs[h + 1].clone() + &(h + 1..n).map(|i| format!("{}{}", lit(i).unwrap(), segs[i + 1])).collect::<String>();
                return Some(Value::kv_only(self.affix(&args[h], &pre, &post)));
            }
        }
        None
    }

    /// What `text % b` may raise (true: surely), for each of the texts `a` may be: a
    /// ValueError for a conversion Python does not have, a TypeError for arguments that do not
    /// fill the conversions (a tuple's items, else b alone: a mapping may fill none) or one a
    /// numeric conversion or `%c` does not take; unknown objects' formatting is unknown code.
    fn percent_raises(&mut self, a: &Value, b: &Value, state: &mut State) -> bool {
        if self.unwind.tries.is_empty() {
            return false;
        }
        if b.pts.iter().any(|o| matches!(self.heap.kind(o), Kind::Unknown | Kind::External)) {
            self.flag_may_raise(state);
            return false;
        }
        let texts: Vec<String> = if a.kv.is_literal() { kv_literals(&a.kv).iter().filter_map(|k| intern::key_as_str(k).map(|s| s.to_string())).collect() } else { vec![] };
        if texts.is_empty() {
            // (a text not known: any conversion)
            self.may_raise_here("TypeError", state);
            self.may_raise_here("ValueError", state);
            return false;
        }
        let outcomes: Vec<Option<(&'static str, bool)>> = texts.iter().map(|t| self.percent_outcome(t, b, state)).collect();
        let mut classes: Vec<&'static str> = outcomes.iter().flatten().map(|o| o.0).collect();
        classes.dedup();
        for c in &classes {
            self.may_raise_here(c, state);
        }
        let sure = outcomes.iter().all(|o| o.is_some_and(|o| o.1));
        if sure {
            state.reachable = false;
        }
        sure
    }

    /// What formatting `text % b` may raise: its class and whether surely (None: nothing).
    fn percent_outcome(&self, text: &str, b: &Value, state: &State) -> Option<(&'static str, bool)> {
        let Some(convs) = percent_conversions(text) else {
            // (`%(name)s` of a dict: its fields are reads of it, `binop_percent`'s; `*` widths,
            // an incomplete format)
            let dict = !b.pts.is_empty() && b.kv.is_bot() && b.pts.iter().all(|o| self.heap.kind(o) == Kind::Dict);
            return (!(dict && text.contains("%(") && !text.contains('*'))).then_some(("TypeError", false));
        };
        if convs.iter().any(|(_, c)| !"diouxXeEfFgGcrsa".contains(*c)) {
            return Some(("ValueError", true));
        }
        let n = convs.len();
        let tuple_lit = kv_literals(&b.kv).iter().any(|k| intern::with_key(k, |k| matches!(k, intern::Key::Tuple(_))));
        let tuples = b.pts.iter().filter(|o| self.heap.kind(*o) == Kind::Tuple).count();
        let args: Vec<Value> = match b.single().filter(|_| tuples == 1 && b.kv.is_bot()) {
            Some(t) => {
                let Some(len) = self.heap.llen(t, state) else { return Some(("TypeError", false)) };
                if len != n {
                    return Some(("TypeError", true));
                }
                (0..len).map(|i| state.slot(t, Ns::Pos, key_int(i as i64))).collect()
            }
            // (one of several tuples, or a value that may be one)
            None if tuples > 0 || tuple_lit || (b.pts.is_empty() && matches!(b.kv, KeyVal::Data(..)) && self.value_type(b).is_none()) => return Some(("TypeError", false)),
            None => {
                // (b alone: a mapping — a dict, a list, an instance — may fill no conversion)
                let mapping = b.pts.iter().any(|o| matches!(self.heap.kind(o), Kind::Dict | Kind::List | Kind::Instance | Kind::Namespace));
                if n != 1 {
                    return (!(n == 0 && mapping && b.kv.is_bot())).then_some(("TypeError", n >= 2 || !mapping && b.pts.is_empty()));
                }
                vec![b.clone()]
            }
        };
        // each argument against its conversion: surely if one surely fails
        let mut out = None;
        for ((_, c), v) in convs.iter().zip(&args) {
            match self.percent_arg_fails(*c, v) {
                Some(true) => return Some(("TypeError", true)),
                Some(false) => out = Some(("TypeError", false)),
                None => {}
            }
        }
        out
    }

    /// Whether conversion `c` fails on `v` (true: surely; None: it does not): numeric ones take
    /// numbers (`%x`, `%o`: ints), `%c` an int or one character; `%s`, `%r`, `%a` anything.
    fn percent_arg_fails(&self, c: char, v: &Value) -> Option<bool> {
        if matches!(c, 's' | 'r' | 'a') {
            return None;
        }
        let int_only = matches!(c, 'x' | 'X' | 'o' | 'c');
        // (each part: Some(true) fails, Some(false) may, None takes it)
        let mut parts: Vec<Option<bool>> = v.pts.iter().map(|o| match self.heap.obj(o).ty {
            Some("int" | "bool") => (c == 'c').then_some(false),
            Some("float") => Some(int_only),
            Some("str") if c == 'c' => Some(false),
            Some(_) => Some(true),
            None if matches!(self.heap.kind(o), Kind::Dict | Kind::List | Kind::Set | Kind::Tuple) => Some(true),
            None => Some(false),
        }).collect();
        match &v.kv {
            KeyVal::Bot => {}
            kv if kv.is_literal() => parts.extend(kv_literals(kv).iter().map(|k| intern::with_key(k, |k| match k {
                intern::Key::Int(i) if c == 'c' => (!(0..=0x10FFFF).contains(i)).then_some(true),
                intern::Key::Str(s) if c == 'c' => (s.chars().count() != 1).then_some(true),
                intern::Key::Int(_) | intern::Key::Bool(_) => None,
                intern::Key::Float(_) => int_only.then_some(true),
                intern::Key::Str(_) | intern::Key::Bytes(_) | intern::Key::None => Some(true),
                _ => Some(false),
            }))),
            _ => parts.push(match self.value_type(v) {
                Some("int" | "bool") => (c == 'c').then_some(false),
                Some("float") => Some(int_only),
                Some("str") if c == 'c' => Some(false),
                Some(_) => Some(true),
                None => Some(false),
            }),
        }
        if !parts.is_empty() && parts.iter().all(|p| *p == Some(true)) {
            return Some(true);
        }
        parts.iter().any(|p| p.is_some()).then_some(false)
    }

    /// The arguments `text % b` formats, in order (b's items when it is a tuple of known
    /// length, else b), the program's instances among those formatted by `%s` / `%r` / `%a` as
    /// their `__str__` / `__repr__` make them (those dunders run here); the dunders other
    /// conversions may run on instances (`%d`: `__index__`) as calls that do not run; with the
    /// text, those converted as `%s` (their text). None: a tuple whose length is not known.
    fn percent_args(&mut self, text: &str, b: &Value, node: &Node, state: &mut State) -> Option<(String, Vec<Value>)> {
        let args: Vec<Value> = match b.single().filter(|t| self.heap.kind(*t) == Kind::Tuple) {
            Some(t) => {
                let n = self.heap.llen(t, state)?;
                (0..n).map(|i| state.slot(t, Ns::Pos, key_int(i as i64))).collect()
            }
            None => vec![b.clone()],
        };
        let convs = percent_conversions(text);
        let mut text = text.to_string();
        let instance = |an: &Analyzer, v: &Value| v.pts.iter().any(|o| an.heap.kind(o) == Kind::Instance);
        let mut out = Vec::with_capacity(args.len());
        for (i, v) in args.into_iter().enumerate() {
            let (at, conv) = convs.as_ref().and_then(|c| c.get(i)).map(|(at, c)| (Some(*at), Some(*c))).unwrap_or((None, None));
            if !instance(self, &v) {
                out.push(v);
                continue;
            }
            let names: &[&str] = match conv {
                Some('s') => &["__str__", "__repr__"],
                Some('r' | 'a') => &["__repr__"],
                _ => {
                    self.library_dunders(std::slice::from_ref(&v), state, node);
                    out.push(v);
                    continue;
                }
            };
            let Some((t, rest)) = self.run_dunder(&v, names, vec![], node, state) else {
                out.push(v);
                continue;
            };
            // (the text made, formatted as `%s`; the rest of v as it was: then the text of
            // either is not known; `%a`: the repr with non-ASCII characters escaped)
            if !rest.empty() || conv == Some('a') && !kv_literals(&t.kv).iter().all(|k| intern::key_as_str(k).is_some_and(|x| x.is_ascii())) {
                out.push(Value::data(&typed_label("str", "")));
                continue;
            }
            if let Some(at) = at {
                text.replace_range(at..at + 1, "s");
            }
            out.push(t);
        }
        Some((text, out))
    }

    /// `seq * n` on a list / tuple with known positions and a known count (at most 64 items).
    fn binop_mult_seq(&mut self, a: &Value, b: &Value, node: &Node, state: &mut State) -> Option<Value> {
        // a list with known positions times a known count
        let (seq, cnt) = if !a.pts.is_empty() { (a, b) } else { (b, a) };
        if let (Some(x), Some(n)) = (seq.single(), cnt.kv.as_lit().filter(|_| cnt.pts.is_empty()).and_then(intern::key_as_int)) {
            if matches!(self.heap.kind(x), Kind::List | Kind::Tuple) {
                if let Some(len) = self.heap.llen(x, state) {
                    if len * (n.max(0) as usize) <= 64 {
                        let px: Vec<Value> = (0..len).map(|i| state.slot(x, Ns::Pos, key_int(i as i64))).collect();
                        let mut out: Vec<Value> = Vec::new();
                        for _ in 0..n.max(0) {
                            out.extend(px.iter().cloned());
                        }
                        let kv = state.elems.get(&x).unwrap_or(DATA);
                        return Some(self.alloc_positional(self.heap.kind(x), node, Some(out), Value::new(), kv, state));
                    }
                }
            }
        }
        // lists (tuples) of any length times a count: one of the same kind, of their elements
        let kind = seq.pts.first().map(|o| self.heap.kind(o)).filter(|k| matches!(k, Kind::List | Kind::Tuple))?;
        let count = match &cnt.kv {
            KeyVal::Data(l, _) => label_type(&intern::str_get(*l)).is_none_or(|t| matches!(t, "int" | "bool")),
            kv if kv.is_literal() => kv_literals(kv).iter().all(|k| intern::key_as_int(k).is_some()),
            _ => false,
        };
        if !count || !seq.kv.is_bot() || !cnt.pts.is_empty() || seq.pts.iter().any(|o| self.heap.kind(o) != kind) {
            return None;
        }
        let e = self.elem_of(seq, node, state);
        Some(self.alloc_positional(kind, node, None, Value::objs(e.pts.clone()), e.kv, state))
    }

    /// `r`, what `a op b` gives where either is of `kind` (a list, a dict), and what it gives
    /// where neither is: `"k" + "s"`, an unknown text's (`[1] if c else s`).
    fn with_rest(&mut self, op: ast::Operator, a: &Value, b: &Value, kind: Kind, r: Value, node: &Node, state: &mut State) -> Value {
        let rest = |an: &Analyzer, v: &Value| Value::with_kv(v.pts.filter(|o| an.heap.kind(o) != kind), v.kv.clone());
        let (ra, rb) = (rest(self, a), rest(self, b));
        let empty = |v: &Value| v.pts.is_empty() && v.kv.is_bot();
        if empty(&ra) || empty(&rb) {
            return r;
        }
        let other = self.binop(op, &ra, &rb, node, state);
        r.join(&other)
    }

    /// `a + b` on texts of known affixes, lists with known positions, lists of known elements.
    fn binop_add_seq(&mut self, a: &Value, b: &Value, both: &ObjSet, node: &Node, state: &mut State) -> Option<Value> {
        let la = a.kv.as_lit();
        let lb = b.kv.as_lit();
        // (a known text before or after one of several texts, or keys; beside an unknown
        // object, which may be any text: that object stands for the rest of the result)
        let unknown_only = |v: &Value| v.pts.iter().all(|o| self.heap.kind(o) == Kind::Unknown);
        match (la.and_then(intern::key_as_str), lb.and_then(intern::key_as_str)) {
            (Some(x), _) if a.pts.is_empty() && unknown_only(b) && matches!(b.kv, KeyVal::Lits(_) | KeyVal::KeysOf(_)) => {
                self.objects_op_raises(both, state);
                return Some(Value::with_kv(b.pts.clone(), self.affix(&b.kv, &x, "")));
            }
            (_, Some(y)) if b.pts.is_empty() && unknown_only(a) && matches!(a.kv, KeyVal::Lits(_) | KeyVal::KeysOf(_)) => {
                self.objects_op_raises(both, state);
                return Some(Value::with_kv(a.pts.clone(), self.affix(&a.kv, "", &y)));
            }
            _ => {}
        }
        // two lists with known positions: the concatenation's positions
        if let (Some(x), Some(y)) = (a.single(), b.single()) {
            if matches!((self.heap.kind(x), self.heap.kind(y)), (Kind::List, Kind::List) | (Kind::Tuple, Kind::Tuple)) {
                let (px, py) = (self.heap.llen(x, state).map(|n| (0..n).map(|i| state.slot(x, Ns::Pos, key_int(i as i64))).collect::<Vec<_>>()), self.heap.llen(y, state).map(|n| (0..n).map(|i| state.slot(y, Ns::Pos, key_int(i as i64))).collect::<Vec<_>>()));
                if let (Some(mut px), Some(py)) = (px, py) {
                    px.extend(py);
                    let kv = kv_join(&state.elems.get(&x).unwrap_or(DATA), &state.elems.get(&y).unwrap_or(DATA));
                    return Some(self.alloc_positional(self.heap.kind(x), node, Some(px), Value::new(), kv, state));
                }
            }
        }
        let lists: Vec<u32> = both.iter().filter(|o| self.heap.kind(*o) == Kind::List).collect();
        if !lists.is_empty() {
            // (a list and what may be no list: a TypeError, unknown code's for an unknown)
            if lists.len() < both.len() || !a.kv.is_bot() || !b.kv.is_bot() || a.pts.is_empty() || b.pts.is_empty() {
                self.objects_op_raises(both, state);
            }
            // (the elements of every alternative of either side: a list's, an unknown's)
            let acc = join_values([a, b].map(|v| self.iterate(v, node, state).elem.flat()));
            let oid = self.alloc_at(Kind::List, node, state, &format!("list@L{}", node.line), Payload::None, None, Ns::Item, None, None);
            state.set_slot(oid, Ns::Elem, STAR, Value::with_kv(acc.pts.clone(), crate::models::torch::elem_kv(&acc.kv)), true);
            state.elems.insert(oid, acc.kv);
            return Some(self.with_rest(ast::Operator::Add, a, b, Kind::List, Value::obj(oid), node, state));
        }
        // (two unknowns — sequences, texts, numbers, arrays: as sequences, the elements of
        // either; a slice or concatenation stands for its sequences)
        let unknowns = |v: &Value| !v.pts.is_empty() && matches!(v.kv, KeyVal::Bot | KeyVal::Data(_, _)) && v.pts.iter().all(|o| self.heap.kind(o) == Kind::Unknown && !self.is_any(o));
        if unknowns(a) && unknowns(b) {
            self.objects_op_raises(both, state);
            let bases: Vec<u32> = both.iter().flat_map(|o| self.slice_base(o).map(|s| s.to_vec()).unwrap_or_else(|| vec![o])).collect();
            let sid = self.elems_of(&crate::domain::ObjSet::from_vec(bases), "", state);
            return Some(Value::obj(sid));
        }
        None
    }

    /// `a op b` where every object `a` (or else `b`) may be is an instance of a class defining
    /// `__op__` (`__rop__`): that method's result. None: not decided this way.
    pub(in crate::interp) fn binop_dunder(&mut self, op: ast::Operator, a: &Value, b: &Value, node: &Node, state: &mut State) -> Option<Value> {
        let name = dunder_stem(op);
        let methods = |an: &Analyzer, v: &Value, dunder: &str| -> Option<Vec<(u32, u32)>> {
            if v.pts.is_empty() || !v.kv.is_bot() {
                return None;
            }
            let mut out = Vec::new();
            for o in v.pts.iter() {
                if an.heap.kind(o) != Kind::Instance {
                    return None;
                }
                let cls = an.heap.instance_class(o)?;
                let owner = an.class_lookup(cls, dunder)?;
                let f = an.heap.cls_slots.get(&(owner, Ns::Attr, key_str_lit(dunder)))?.single()?;
                if an.heap.kind(f) != Kind::Function {
                    return None;
                }
                out.push((o, f));
            }
            Some(out)
        };
        let reflected = format!("__r{}__", name);
        let run = |an: &mut Analyzer, calls: Vec<(u32, u32)>, other: &Value, state: &mut State| {
            let outs: Vec<Value> = calls.into_iter().map(|(o, f)| an.call_user(f, Some(Value::obj(o)), Args::pos(&[other.clone()]), state, node, false)).collect();
            join_values(outs)
        };
        // a path where a method returns NotImplemented goes on as Python's does: to the other
        // operand's reflected method, else a TypeError (it is not the result)
        let not_impl = |an: &Analyzer, v: &Value| v.pts.iter().filter(|o| an.heap.kind(*o) == Kind::External && an.heap.external_qual(*o) == "builtins.NotImplemented").collect::<Vec<_>>();
        let (r, other_side) = match methods(self, a, &format!("__{}__", name)) {
            Some(m) => (run(self, m, b, state), Some((b, a))),
            None => {
                let m = methods(self, b, &reflected)?;
                let r = run(self, m, a, state);
                // (an operand that is no instance of the program's classes may have its own
                // method, run first, whose result may be anything; a known builtin's is not one
                // for the program's instance)
                let open = !a.pts.is_empty() || (matches!(a.kv, KeyVal::Data(_, _)) && self.value_type(a).is_none());
                (if open { r.join(&Value::data(&format!("L{}", node.line))) } else { r }, None)
            }
        };
        let ni = not_impl(self, &r);
        if ni.is_empty() {
            return Some(r);
        }
        let mut r = Value::with_kv(r.pts.filter(|o| !ni.contains(&o)), r.kv.clone());
        self.flag_may_raise(state);
        if let Some((b, a)) = other_side {
            match methods(self, b, &reflected) {
                Some(m) => {
                    let back = run(self, m, a, state);
                    let ni = not_impl(self, &back);
                    r = r.join(&Value::with_kv(back.pts.filter(|o| !ni.contains(&o)), back.kv.clone()));
                }
                None if !b.pts.is_empty() || matches!(b.kv, KeyVal::Data(_, _)) => r = r.join(&Value::data(&format!("L{}", node.line))),
                None => {}
            }
        }
        // (only NotImplemented: the operation raises)
        if r.pts.is_empty() && r.kv.is_bot() {
            state.reachable = false;
        }
        Some(r)
    }

    /// The text `pre` + v + `post` for a value v that is one of a few texts or some keys:
    /// literals as their `str`, the rest renamed (a key made from a parameter keeps the text
    /// around it).
    pub(super) fn affix(&mut self, kv: &KeyVal, pre: &str, post: &str) -> KeyVal {
        let to_str = |k: KeyId| if intern::key_is_str(k) { k } else { key_str_lit(&intern::key_py_str(k)) };
        let mut kv = match kv {
            KeyVal::Lit(k) => KeyVal::Lit(to_str(*k)),
            KeyVal::Lits(ks) => KeyVal::Lits(ks.iter().map(to_str).collect()),
            KeyVal::Data(l, _) => KeyVal::Data(*l, kv.hint().iter().map(to_str).collect()),
            kv => kv.clone(),
        };
        for (f, t) in [(FnKind::Prefix, pre), (FnKind::Suffix, post)] {
            if !t.is_empty() {
                kv = self.rename_kv(&kv, &RenameFn::with_arg(f, key_str_lit(t)));
            }
        }
        kv
    }

    /// An f-string: a str, its text known as far as its parts are.
    pub(super) fn expr_fstring(&mut self, f: &'static ast::ExprFString, node: &'static ast::Expr, state: &mut State) -> Value {
        let r = self.fstring_untyped(f, node, state);
        self.as_type(r, Some("str"), false)
    }

    /// An f-string: its text when every part is known (formatted as CPython does), else a str
    /// known to start and end with its literal ends (`Payload::Affix`) or not known.
    fn fstring_untyped(&mut self, f: &'static ast::ExprFString, node: &'static ast::Expr, state: &mut State) -> Value {
        let mut parts: Vec<Part> = Vec::new();
        for p in f.value.iter() {
            match p {
                ast::FStringPart::Literal(sl) => parts.push(Part::Lit(sl.value.to_string())),
                ast::FStringPart::FString(fs) => {
                    for el in fs.elements.iter() {
                        match el {
                            ast::InterpolatedStringElement::Literal(l) => parts.push(Part::Lit(l.value.to_string())),
                            ast::InterpolatedStringElement::Interpolation(i) => {
                                // `{x=}`: the expression's text, then its repr (unless a
                                // conversion or a spec says otherwise)
                                let mut conv = i.conversion;
                                if let Some(dt) = &i.debug_text {
                                    let src = frontend::text_in(node, ruff_text_size::Ranged::range(&*i.expression));
                                    if src.is_empty() {
                                        parts.push(Part::Opaque);
                                        continue;
                                    }
                                    parts.push(Part::Lit(format!("{}{}{}", dt.leading, src, dt.trailing)));
                                    if conv == ast::ConversionFlag::None && i.format_spec.is_none() {
                                        conv = ast::ConversionFlag::Repr;
                                    }
                                }
                                let part = self.fstring_hole(i, conv, node, state);
                                parts.push(part);
                            }
                        }
                    }
                }
            }
        }
        self.fstring_value(parts, node, state)
    }

    /// One `{expr!conv:spec}` of an f-string: its text when known, else what is known of it.
    fn fstring_hole(&mut self, i: &'static ast::InterpolatedElement, conversion: ast::ConversionFlag, node: &'static ast::Expr, state: &mut State) -> Part {
        let v = self.eval(&i.expression, state);
        // the spec's text (`{n:03d}`, `{x:>{w}}`); None: a part of it is not known
        let spec_text: Option<Option<String>> = i.format_spec.as_ref().map(|spec| self.format_spec_text(spec, state));
        // an object of the program's classes: its `__format__` / `__str__` /
        // `__repr__` makes the text
        let nd = frontend::node(node);
        let ran = match conversion {
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
        // (a spec the value's type rejects: `f"{None:d}"` TypeError, `f"{'a':d}"` ValueError)
        let spec_given = matches!(&spec_text, Some(None)) || matches!(&spec_text, Some(Some(t)) if !t.is_empty());
        if let Some((t, rest)) = ran {
            if spec_given {
                self.may_raise_here("TypeError", state);
            }
            // (no text known — a dunder of code rowan cannot see — is any text)
            let whole = rest.pts.is_empty() && rest.kv.is_bot() && (conversion == ast::ConversionFlag::None || spec_text.is_none()) && !t.empty();
            return if whole { Part::Hole(t) } else { Part::Opaque };
        }
        // `{x!r}` / `{x!s}` / `{x!a}`: the converted text
        let v = match conversion {
            ast::ConversionFlag::None => crate::models::enums::enum_text(self, &v, state).unwrap_or(v),
            conv => {
                let name = match conv {
                    ast::ConversionFlag::Repr => "repr",
                    ast::ConversionFlag::Ascii => "ascii",
                    _ => "str",
                };
                let v = if name == "str" { crate::models::enums::enum_text(self, &v, state).unwrap_or(v) } else { v };
                match v.kv.as_lit().filter(|_| v.pts.is_empty()).and_then(|k| crate::models::pyvalues::builtin(name, &[k], &[])) {
                    Some(t) => Value::lit(t),
                    None => {
                        Value::data("conv")
                    }
                }
            }
        };
        // (None takes no spec, a str no number's: TypeError, ValueError)
        if let (Some(Some(t)), Some(k)) = (&spec_text, v.kv.as_lit().filter(|_| v.pts.is_empty() && spec_given)) {
            let number_code = t.ends_with(|c: char| "bcdoxXneEfFgG%".contains(c));
            match intern::key_get(k) {
                intern::Key::None => self.raise_here("TypeError", state),
                intern::Key::Str(_) if number_code => self.raise_here("ValueError", state),
                _ => {}
            }
            if !state.reachable {
                return Part::Opaque;
            }
        }
        match spec_text {
            // a spec applied to a known value
            Some(Some(t)) => match v.kv.as_lit().filter(|_| v.pts.is_empty()).and_then(|k| format_spec_literal(k, &t)) {
                Some(s) => Part::Lit(s),
                None => {
                    if spec_given {
                        self.may_raise_here("TypeError", state);
                        self.may_raise_here("ValueError", state);
                    }
                    Part::Opaque
                }
            },
            Some(None) => {
                self.may_raise_here("TypeError", state);
                self.may_raise_here("ValueError", state);
                Part::Opaque
            }
            None => Part::Hole(v),
        }
    }

    /// The text of a format spec (`{n:03d}`, `{x:>{w}}`); None: a part of it is not known.
    fn format_spec_text(&mut self, spec: &'static ast::InterpolatedStringFormatSpec, state: &mut State) -> Option<String> {
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
    }

    /// An f-string's value from its parts: the text when all are known; one hole between
    /// known texts keeps them as affixes; else unknown.
    fn fstring_value(&mut self, parts: Vec<Part>, node: &'static ast::Expr, state: &mut State) -> Value {
        let holes: Vec<usize> = parts.iter().enumerate().filter(|(_, p)| !matches!(p, Part::Lit(_))).map(|(i, _)| i).collect();
        if holes.is_empty() {
            let s: String = parts.iter().map(|p| if let Part::Lit(l) = p { l.as_str() } else { "" }).collect();
            return Value::lit(key_str_lit(&s));
        }
        if holes.len() == 1 {
            // (an object with no text known, an unknown one's: any text — below)
            if let Part::Hole(v) = parts[holes[0]].clone_if(|v| v.pts.is_empty() || v.kv.is_literal()) {
                let i = holes[0];
                let pre: String = parts[..i].iter().map(|p| if let Part::Lit(l) = p { l.as_str() } else { "" }).collect();
                let post: String = parts[i + 1..].iter().map(|p| if let Part::Lit(l) = p { l.as_str() } else { "" }).collect();
                let kv = self.affix(&v.kv, &pre, &post);
                // (objects in the hole, an unknown str among them: other texts too, one unknown)
                if !v.pts.is_empty() && v.kv.is_literal() {
                    let nd = frontend::node(node);
                    let site = format!("fstring@L{}", nd.line);
                    let o = self.unknown_source(&site, crate::source::SourceKind::Data, state, &site);
                    self.heap.obj_mut(o).payload = Payload::Affix(pre, post);
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
        Value::data(&typed_label("str", ""))
    }
}

/// The conversion characters of a `%` format's specifiers (with their offsets), in order; None when one takes a
/// width or precision from the arguments (`%*d`) or the format is a mapping's (`%(name)s`).
fn percent_conversions(fmt: &str) -> Option<Vec<(usize, char)>> {
    let mut out = Vec::new();
    let mut it = fmt.char_indices();
    while let Some((_, c)) = it.next() {
        if c != '%' {
            continue;
        }
        loop {
            match it.next()? {
                (_, '%') => break,
                (_, '*' | '(') => return None,
                (i, ch) if ch.is_ascii_alphabetic() && !matches!(ch, 'l' | 'h' | 'L') => {
                    out.push((i, ch));
                    break;
                }
                _ => {}
            }
        }
    }
    Some(out)
}

/// `fmt % args` when each argument is one of a few literals: the texts of every combination
/// (at most 64), exactly as Python formats them; None when an argument is not literal or a
/// combination does not format.
fn format_percent_alternatives(fmt: &str, args: &[Value]) -> Option<Vec<String>> {
    let lits = |v: &Value| -> Option<Vec<KeyId>> { if v.pts.is_empty() && v.kv.is_literal() { Some(kv_literals(&v.kv).iter().collect()) } else { None } };
    let alts: Vec<Vec<KeyId>> = args.iter().map(lits).collect::<Option<Vec<_>>>()?;
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

/// `format(value, spec)` for a known literal and a plain spec: `[[fill]align][0][width][.prec][type]`.
pub(crate) fn format_spec_literal(k: KeyId, spec: &str) -> Option<String> {
    crate::models::pyvalues::format_value(k, spec)
}

/// The stem of an operator's dunders: `add` for `__add__`, `__radd__`, `__iadd__`.
pub(in crate::interp) fn dunder_stem(op: ast::Operator) -> &'static str {
    use ast::Operator as O;
    match op {
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
    }
}

/// The builtin type of `a op b` for operands of builtin types `a`, `b` (None: not known, or
/// not one: `**` may give a float or a complex).
fn binop_type(op: ast::Operator, a: Option<&'static str>, b: Option<&'static str>) -> Option<&'static str> {
    use ast::Operator::*;
    let a = a?;
    // (`text % args` formats, whatever the arguments)
    if op == Mod && matches!(a, "str" | "bytes") {
        return Some(a);
    }
    let b = b?;
    let num = |t: &str| matches!(t, "int" | "bool" | "float");
    let int = |t: &str| matches!(t, "int" | "bool");
    match op {
        Add if a == b && matches!(a, "str" | "bytes") => Some(a),
        Mult if matches!(a, "str" | "bytes") && int(b) => Some(a),
        Mult if matches!(b, "str" | "bytes") && int(a) => Some(b),
        Div if num(a) && num(b) => Some("float"),
        Add | Sub | Mult | FloorDiv | Mod if num(a) && num(b) => Some(if a == "float" || b == "float" { "float" } else { "int" }),
        BitAnd | BitOr | BitXor if a == "bool" && b == "bool" => Some("bool"),
        BitAnd | BitOr | BitXor | LShift | RShift if int(a) && int(b) => Some("int"),
        _ => None,
    }
}
