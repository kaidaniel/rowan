//! Operators and text: `a op b` and `op a` on known values (CPython-exact on literals), the
//! program's operator dunders, f-strings and `%` formatting.

use indexmap::IndexMap;
use ruff_python_ast as ast;

use crate::domain::{kv_join, FnKind, KeyVal, Ns, ObjSet, RenameFn, Value, DATA};
use crate::frontend::{self, Node};
use crate::intern::{self, key_float, key_int, key_str_lit, KeyId, STAR};
use crate::state::{Kind, State};

use super::{Analyzer, Payload};

/// A part of an f-string: literal text, a hole of known value, or a hole whose text is not
/// known.
enum Part {
    Lit(String),
    Hole(Value),
    Opaque,
}

impl Analyzer {
    /// Literals other than None beside objects (`"dev" if c else ["p", "q"]`): a value whose
    /// parts Python treats as different types. (Beside an unknown object, which may be the
    /// literals too, no: what is done with it covers them.)
    pub(in crate::interp) fn mixed(&self, v: &Value) -> bool {
        !v.pts.is_empty() && v.kv.is_literal() && crate::domain::kv_literals(&v.kv).iter().any(|k| k != intern::key_none()) && v.pts.iter().all(|o| self.heap.kind(o) != Kind::Unknown)
    }

    /// An operation on values of which one is `mixed`: on its objects and on its literals, each
    /// on a copy of the state; the results and the states joined. None: no such value.
    pub(in crate::interp) fn split_mixed(&mut self, vs: &[&Value], state: &mut State, f: &mut dyn FnMut(&mut Analyzer, &[Value], &mut State) -> Value) -> Option<Value> {
        let i = vs.iter().position(|v| self.mixed(v))?;
        let mut out = Value::new();
        let mut acc: Option<State> = None;
        for part in [Value::objs(vs[i].pts.clone()), Value::kv_only(vs[i].kv.clone())] {
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
        // an unknown result is named by the expression (`1 + b`), of its type when known
        if let (true, KeyVal::Data(l)) = (r.pts.is_empty(), &r.kv) {
            let l = intern::str_get(*l);
            let text = frontend::text_at(&nd).split_whitespace().collect::<Vec<_>>().join(" ");
            if !l.starts_with('\u{1}') && !text.is_empty() && text.len() <= 60 {
                // the expression names the value only inside this function (its names)
                let scope = self.frame().func.unwrap_or(0);
                let name = format!("expr:{}:{}", scope, text);
                return Value::data(&match crate::domain::label_type(&l) {
                    Some(t) => crate::domain::typed_label(t, &name),
                    None => name,
                });
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
            KeyVal::Data(l) => add(crate::domain::label_type(&intern::str_get(*l)))?,
            KeyVal::Lit(_) | KeyVal::Lits(_) => {
                for k in crate::domain::kv_literals(&v.kv).iter() {
                    add(crate::domain::TYPE_NAMES.iter().copied().find(|t| crate::domain::key_type(k, t)))?;
                }
            }
            KeyVal::KeysOf(_) => return None,
        }
        ty
    }

    /// `v`, an unknown of type `ty` when it is an unknown scalar (named as it is, unless `named`
    /// is false: its name is what it was made from, not what it is).
    pub fn as_type(&self, v: Value, ty: Option<&'static str>, named: bool) -> Value {
        let (Some(t), true, KeyVal::Data(l)) = (ty, v.pts.is_empty() && !self.m.torch_on, &v.kv) else { return v };
        let l = intern::str_get(*l);
        let name = crate::domain::label_name(&l);
        // (labels that carry more than a name — a JSON text, a shape, a pattern — are the
        // models': left as they are; `len` is kept, comparisons read it)
        let plain = name.is_empty() || name.starts_with("expr:") || matches!(name, "str" | "int" | "bool" | "len") || (name.starts_with('L') && name[1..].chars().all(|c| c.is_ascii_digit()));
        if !plain || crate::domain::label_type(&l) == Some(t) {
            return v;
        }
        let keep = (named && !matches!(name, "str" | "int" | "bool")) || name == "len";
        Value::data(&crate::domain::typed_label(t, if keep { name } else { "" }))
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
        let r = self.unary_untyped(u, v, node, state);
        self.as_type(r, ty, false)
    }

    fn unary_untyped(&mut self, u: &'static ast::ExprUnaryOp, v: Value, node: &'static ast::Expr, state: &mut State) -> Value {
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

    pub fn binop(&mut self, op: ast::Operator, a: &Value, b: &Value, node: &Node, state: &mut State) -> Value {
        if let Some(r) = self.split_mixed(&[a, b], state, &mut |an, vs, st| an.binop(op, &vs[0], &vs[1], node, st)) {
            return r;
        }
        let r = self.binop_untyped(op, a, b, node, state);
        let ty = binop_type(op, self.value_type(a), self.value_type(b));
        self.as_type(r, ty, true)
    }

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
                return Value::obj(oid);
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
            if let Some(v) = self.binop_percent(a, b, node, state) {
                return v;
            }
        }
        if crate::models::pandas::is_seriesish(self, &a) || crate::models::pandas::is_seriesish(self, &b) {
            return Value::with_kv(ObjSet::one(self.m.series), DATA);
        }
        let frames = both.filter(|o| self.heap.kind(o) == Kind::Frame);
        if !frames.is_empty() {
            return Value::with_kv(frames, DATA);
        }
        Value::data(&format!("L{}", node.line))
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
        if let Some(x) = a.kv.as_lit() {
            if let Some(sx) = intern::key_as_str(x) {
                // arguments that are each one of a few literals: every combination, as
                // Python formats it
                if let Some(rs) = format_percent_alternatives(&sx, b, state, self) {
                    return Some(Value::kv_only(KeyVal::lits(crate::intern::KeySet::from_iter(rs.iter().map(|r| key_str_lit(r))))));
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
                            Some(t) if matches!(self.heap.kind(t), Kind::Tuple) && self.heap.llen(t, state) == Some(n) => (0..n).map(|i| state.slot(t, Ns::Pos, key_int(i as i64)).kv).collect(),
                            _ => Vec::new(),
                        }
                    };
                    let lit = |i: usize| args[i].as_lit().map(intern::key_py_str);
                    if let [h] = (0..args.len()).filter(|i| lit(*i).is_none()).collect::<Vec<_>>()[..] {
                        if args.len() == n {
                            let pre: String = (0..h).map(|i| format!("{}{}", segs[i], lit(i).unwrap())).collect::<String>() + &segs[h];
                            let post: String = segs[h + 1].clone() + &(h + 1..n).map(|i| format!("{}{}", lit(i).unwrap(), segs[i + 1])).collect::<String>();
                            return Some(Value::kv_only(self.affix(&args[h], &pre, &post)));
                        }
                    }
                }
            }
        }
        None
    }

    /// `seq * n` on a list / tuple with known positions and a known count (at most 64 items).
    fn binop_mult_seq(&mut self, a: &Value, b: &Value, node: &Node, state: &mut State) -> Option<Value> {
        // a list with known positions times a known count
        let (seq, cnt) = if a.pts.len() == 1 { (a, b) } else { (b, a) };
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
        None
    }

    /// `a + b` on texts of known affixes, lists with known positions, lists of known elements.
    fn binop_add_seq(&mut self, a: &Value, b: &Value, both: &crate::domain::ObjSet, node: &Node, state: &mut State) -> Option<Value> {
        let la = a.kv.as_lit();
        let lb = b.kv.as_lit();
        // (a known text before or after one of several texts, or keys)
        match (la.and_then(intern::key_as_str), lb.and_then(intern::key_as_str)) {
            (Some(x), _) if matches!(b.kv, KeyVal::Lits(_) | KeyVal::KeysOf(_)) => return Some(Value::kv_only(self.affix(&b.kv, &x, ""))),
            (_, Some(y)) if matches!(a.kv, KeyVal::Lits(_) | KeyVal::KeysOf(_)) => return Some(Value::kv_only(self.affix(&a.kv, "", &y))),
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
            // (the elements of every alternative of either side: a list's, an unknown's)
            let acc = crate::domain::join_values([a, b].map(|v| self.iterate(v, node, state).elem.flat()));
            let oid = self.alloc_at(Kind::List, node, state, &format!("list@L{}", node.line), Payload::None, None, Ns::Item, None, None);
            state.set_slot(oid, Ns::Elem, STAR, Value::with_kv(acc.pts.clone(), crate::models::torch::elem_kv(&acc.kv)), true);
            state.elems.insert(oid, acc.kv);
            return Some(Value::obj(oid));
        }
        None
    }

    /// `a op b` where every object `a` (or else `b`) may be is an instance of a class defining
    /// `__op__` (`__rop__`): that method's result. None: not decided this way.
    pub(in crate::interp) fn binop_dunder(&mut self, op: ast::Operator, a: &Value, b: &Value, node: &Node, state: &mut State) -> Option<Value> {
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
        let (calls, other) = match methods(self, a, &format!("__{}__", name)) {
            Some(m) => (m, b.clone()),
            None => (methods(self, b, &format!("__r{}__", name))?, a.clone()),
        };
        let mut outs = Vec::new();
        for (o, f) in calls {
            outs.push(self.call_user(f, Some(Value::obj(o)), vec![other.clone()], IndexMap::new(), vec![], vec![], state, node, false));
        }
        Some(crate::domain::join_values(outs))
    }

    /// The text `pre` + v + `post` for a value v that is one of a few texts or some keys:
    /// literals as their `str`, the rest renamed (a key made from a parameter keeps the text
    /// around it).
    pub(super) fn affix(&mut self, kv: &KeyVal, pre: &str, post: &str) -> KeyVal {
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

    /// An f-string: a str, its text known as far as its parts are.
    pub(super) fn expr_fstring(&mut self, f: &'static ast::ExprFString, node: &'static ast::Expr, state: &mut State) -> Value {
        let r = self.fstring_untyped(f, node, state);
        self.as_type(r, Some("str"), false)
    }

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
                                let part = self.fstring_hole(i, node, state);
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
    fn fstring_hole(&mut self, i: &'static ast::InterpolatedElement, node: &'static ast::Expr, state: &mut State) -> Part {
        let v = self.eval(&i.expression, state);
        // the spec's text (`{n:03d}`, `{x:>{w}}`); None: a part of it is not known
        let spec_text: Option<Option<String>> = i.format_spec.as_ref().map(|spec| self.format_spec_text(spec, state));
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
            return if whole { Part::Hole(t) } else { Part::Opaque };
        }
        // `{x!r}` / `{x!s}` / `{x!a}`: the converted text
        let v = match i.conversion {
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
        match spec_text {
            // a spec applied to a known value
            Some(Some(t)) => match v.kv.as_lit().filter(|_| v.pts.is_empty()).and_then(|k| format_spec_literal(k, &t)) {
                Some(s) => Part::Lit(s),
                None => Part::Opaque,
            },
            Some(None) => Part::Opaque,
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
        Value::data(&crate::domain::typed_label("str", ""))
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
        let o = b.single().filter(|o| an.heap.kind(*o) == Kind::Tuple)?;
        let n = an.heap.llen(o, state)?;
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

/// `format(value, spec)` for a known literal and a plain spec: `[[fill]align][0][width][.prec][type]`.
pub(crate) fn format_spec_literal(k: KeyId, spec: &str) -> Option<String> {
    crate::models::pyvalues::format_value(k, spec)
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
