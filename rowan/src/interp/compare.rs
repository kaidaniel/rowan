//! Comparisons and truth: `==`, `<`, `in`, `is` on known values, the program's rich
//! comparison dunders, and whether a value is true.

use ruff_python_ast as ast;

use crate::domain::{kv_exclude, kv_literals, label_name, DATA, KeyVal, Ns, ObjId, ObjSet, Row, Value};
use crate::frontend::{self, Node};
use crate::intern::{self, key_int, key_str_lit, KeyId, KeySet};
use crate::state::{Kind, State};

use super::{Analyzer, Args, Payload, ViewKind};

impl Analyzer {
    /// `a < b < c`: the chained comparisons (`compare`) joined by `and`.
    pub(super) fn expr_compare(&mut self, c: &'static ast::ExprCompare, node: &'static ast::Expr, state: &mut State) -> Value {
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

    /// `a op b` for one comparison operator (`right`: b as written): a bool when it is `in`,
    /// `is`, or compares values of builtin types.
    pub(super) fn compare(&mut self, op: ast::CmpOp, a: &Value, b: &Value, right: &'static ast::Expr, nd: &Node, state: &mut State) -> Value {
        let r = self.compare_untyped(op, a, b, right, nd, state);
        let always = matches!(op, ast::CmpOp::In | ast::CmpOp::NotIn | ast::CmpOp::Is | ast::CmpOp::IsNot);
        let ty = (always || (self.value_type(a).is_some() && self.value_type(b).is_some())).then_some("bool");
        self.as_type(r, ty, false)
    }

    /// A comparison `a op b` (`right`: b's expression) before the result's type is applied: a
    /// tensor's, a named unknown's against a number, `in` on a container, `is`, CPython's result
    /// on literals, the program's rich comparison dunders.
    fn compare_untyped(&mut self, op: ast::CmpOp, a: &Value, b: &Value, right: &'static ast::Expr, nd: &Node, state: &mut State) -> Value {
        let lit = |t: bool| Value::lit(intern::key_bool(t));
        // a tensor is never None
        if matches!(op, ast::CmpOp::Is | ast::CmpOp::IsNot) && matches!(right, ast::Expr::NoneLiteral(_)) && crate::models::torch::is_tensor(a) && a.pts.is_empty() {
            return lit(op == ast::CmpOp::IsNot);
        }
        let ordering = !matches!(op, ast::CmpOp::In | ast::CmpOp::NotIn | ast::CmpOp::Is | ast::CmpOp::IsNot);
        if ordering && crate::models::torch::is_tensor(a) {
            return crate::models::torch::t_compare(self, a, b, nd, state);
        }
        // a named unknown against a number / tensor
        if ordering && self.m.torch_on && a.kv.is_bot() && a.pts.len() == 1 && a.pts.iter().all(|o| self.heap.kind(o) == Kind::Unknown) {
            if let Some(r) = crate::models::torch::t_compare_named(self, a, b, nd, state) {
                return r;
            }
            return self.compare_const(op, a, b, right, state).map(lit).unwrap_or_else(|| Value::data("bool"));
        }
        // None ordered against a literal or a builtin container: TypeError; the rest of its
        // value goes on
        if matches!(op, ast::CmpOp::Lt | ast::CmpOp::LtE | ast::CmpOp::Gt | ast::CmpOp::GtE) {
            for (x, y, first) in [(a, b, true), (b, a, false)] {
                if x.kv.is_literal() && kv_literals(&x.kv).contains(intern::key_none()) && self.builtin_operand(y) {
                    let rest = Value::with_kv(x.pts.clone(), kv_exclude(&x.kv, &KeySet::one(intern::key_none())).unwrap_or(crate::domain::BOT));
                    if rest.pts.is_empty() && rest.kv.is_bot() {
                        self.raise_here("TypeError", state);
                        return Value::new();
                    }
                    self.may_raise_here("TypeError", state);
                    return if first { self.compare_untyped(op, &rest, y, right, nd, state) } else { self.compare_untyped(op, y, &rest, right, nd, state) };
                }
            }
        }
        if ordering {
            // library values (paths, Decimal, datetime); instances: their comparison methods
            // (`__lt__`, reflected, @total_ordering); tuples item by item
            if let Some(r) = crate::models::builtins::library_compare(self, op, a, b) {
                return r;
            }
            if let Some(r) = self.rich_compare(op, a, b, nd, state) {
                return r;
            }
            if let Some(r) = self.seq_compare(op, a, b, state) {
                return r;
            }
        }
        if let Some(r) = self.compare_const(op, a, b, right, state) {
            return lit(r);
        }
        if crate::models::pandas::is_seriesish(self, a) || crate::models::pandas::is_seriesish(self, b) {
            return Value::with_kv(ObjSet::one(self.m.series), DATA);
        }
        // (tuples holding parameters: decided per call site)
        let both = a.pts.union(&b.pts);
        if both.iter().any(|o| self.heap.kind(o) == Kind::Frame) {
            return Value::with_kv(both.filter(|o| self.heap.kind(o) == Kind::Frame), DATA);
        }
        Value::data("bool")
    }

    /// `a is None`, `a is not None`, `a == lit`, `a != lit` when the outcome is decidable
    /// from the value's literals: a value with only non-placeholder objects is never None.
    pub(super) fn compare_const(&mut self, op: ast::CmpOp, a: &Value, b: &Value, right: &'static ast::Expr, state: &mut State) -> Option<bool> {
        let none = intern::key_none();
        let lits = kv_literals(&a.kv);
        let has_ph = a.pts.iter().any(|o| self.heap.kind(o) == Kind::Unknown);
        let known_lits = a.kv.is_literal() || a.kv.is_bot();
        // an unknown length against 0 (or a negative number): `len(xs) >= 0` holds
        if let (true, KeyVal::Data(d, _), ast::Expr::NumberLiteral(nl)) = (a.pts.is_empty(), &a.kv, right) {
            if label_name(&intern::str_get(*d)) == "len" {
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
            ast::CmpOp::Is | ast::CmpOp::IsNot if !a.pts.is_empty() && a.kv.is_bot() => self.compare_identity(op, a, b),
            ast::CmpOp::In | ast::CmpOp::NotIn if self.mixed(b) => None,
            ast::CmpOp::In | ast::CmpOp::NotIn if b.pts.iter().any(|o| self.heap.obj(o).iterator) => {
                let found = self.in_iterator(a, b, &lits, state);
                found.map(|f| if op == ast::CmpOp::In { f } else { !f })
            }
            ast::CmpOp::In | ast::CmpOp::NotIn => {
                if b.pts.iter().any(|o| self.hashes_keys(o)) {
                    self.check_hashable(a, state);
                }
                self.compare_in(op, a, b, right, &lits, state)
            }
            ast::CmpOp::Eq | ast::CmpOp::NotEq => self.compare_eq(op, a, b, lits),
            ast::CmpOp::Lt | ast::CmpOp::LtE | ast::CmpOp::Gt | ast::CmpOp::GtE => {
                // two single literals of one kind (numbers, or strings) are ordered here;
                // anything else is decided per call site when it is concrete there
                let bl = kv_literals(&b.kv);
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

    /// `a == b` / `a != b` decided on literals, tuples of literals and known members.
    fn compare_eq(&mut self, op: ast::CmpOp, a: &Value, b: &Value, lits: KeySet) -> Option<bool> {
        // Enum members: their keys (a mixed-in one: its value)
        let (a, b, lits) = if self.heap.keys.member_key.is_empty() {
            (a.clone(), b.clone(), lits)
        } else {
            let ka = crate::models::enums::member_base(self, &self.keyify(a));
            let kb = crate::models::enums::member_base(self, &self.keyify(&b));
            let l = kv_literals(&ka.kv);
            (ka, kb, l)
        };
        let bl = kv_literals(&b.kv);
        // a builtin dict / list / tuple / set never equals a str / number / bool / None
        let container = |an: &Analyzer, v: &Value| !v.pts.is_empty() && v.pts.iter().all(|o| matches!(an.heap.kind(o), Kind::Dict | Kind::List | Kind::Tuple | Kind::Set) && !crate::models::pandas::is_series_obj(an, o)) && matches!(v.kv, KeyVal::Bot);
        let scalar = |v: &Value| v.pts.is_empty() && v.kv.is_literal() && !kv_literals(&v.kv).iter().any(|k| intern::with_key(k, |kk| matches!(kk, intern::Key::Tuple(_))));
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

    /// `a is b` on objects: distinct abstract objects are distinct objects; one strong object is
    /// itself.
    fn compare_identity(&mut self, op: ast::CmpOp, a: &Value, b: &Value) -> Option<bool> {
        if b.pts.is_empty() || !b.kv.is_bot() {
            return None;
        }
        // builtin classes (`type(x) is dict`): the same class exactly when the same name
        let builtin_q = |an: &Analyzer, v: &Value| -> Option<Vec<String>> { v.pts.iter().map(|o| if an.heap.kind(o) == Kind::External { Some(an.heap.external_qual(o)).filter(|q| q.starts_with("builtins.")) } else { None }).collect() };
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
        let opaque = |an: &Analyzer, v: &Value| v.pts.iter().any(|o| matches!(an.heap.kind(o), Kind::Unknown | Kind::External | Kind::Class | Kind::Module | Kind::Function));
        if opaque(self, &a) || opaque(self, &b) {
            return None;
        }
        let same = if a.pts.intersection(&b.pts).is_empty() {
            false
        } else if a.pts.len() == 1 && a.pts == b.pts && self.heap.obj(a.pts.first().unwrap()).single {
            true
        } else {
            return None;
        };
        Some(if op == ast::CmpOp::Is { same } else { !same })
    }

    /// `a in it` of a one-shot iterator: it takes items until one equals `a` (or to its end).
    /// Decided for a literal among the known items left, the iterator then just past it (or
    /// used up); else where it stands after is not known.
    fn in_iterator(&mut self, a: &Value, b: &Value, lits: &KeySet, state: &mut State) -> Option<bool> {
        self.force_gen(b, state, false);
        let one = b.single().filter(|_| b.kv.is_bot() && a.pts.is_empty() && lits.len() == 1);
        if let (Some(o), Some((c0, n))) = (one, one.and_then(|o| self.positions_left(o, state))) {
            let k = lits.iter().next().unwrap();
            for i in c0..c0 + n {
                let sv = state.slot(o, Ns::Pos, key_int(i as i64));
                let ks = kv_literals(&sv.kv);
                let eq: Vec<Option<bool>> = ks.iter().map(|x| lit_eq(x, k)).collect();
                if !sv.pts.is_empty() || !sv.kv.is_literal() || eq.iter().any(|e| e.is_none()) || (eq.contains(&Some(true)) && eq.contains(&Some(false))) {
                    break;
                }
                if eq.contains(&Some(true)) {
                    self.set_cursor(o, Some(i + 1), state);
                    return Some(true);
                }
                if i + 1 == c0 + n {
                    self.set_cursor(o, Some(c0 + n), state);
                    return Some(false);
                }
            }
            if n == 0 {
                return Some(false);
            }
        }
        for o in b.pts.iter().filter(|o| self.heap.obj(*o).iterator).collect::<Vec<_>>() {
            self.set_cursor(o, None, state);
        }
        None
    }

    /// `a in b` / `a not in b` decided: a record's keys, a container's known members or elements,
    /// a text's substrings.
    fn compare_in(&mut self, op: ast::CmpOp, a: &Value, b: &Value, right: &'static ast::Expr, lits: &KeySet, state: &mut State) -> Option<bool> {
        let decide = |found: bool| Some(if op == ast::CmpOp::In { found } else { !found });
        // members of a Flag: bits; else a member is its key
        // (and a tuple of known values: its key)
        if !a.pts.is_empty() {
            if !self.heap.keys.member_key.is_empty() {
                if let Some(r) = crate::models::enums::flag_contains(self, a, &b, state) {
                    return decide(r);
                }
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
        if let (Some(Payload::Range(lo, hi, step)), [k]) = (b.single().map(|o| &self.heap.obj(o).payload), &lits.iter().collect::<Vec<_>>()[..]) {
            let x = intern::key_as_int(*k)?;
            let inside = if *step > 0 { x >= *lo && x < *hi } else { x <= *lo && x > *hi };
            return decide(inside && (x - lo) % step == 0);
        }
        // literals in a positional container whose items are known literals (each one of
        // some): decided when every one is surely there, or none may be (by Python's
        // equality: `1.0 in [1, 2]`); of an unknown length, when none may be among its elements
        if let Some(o) = b.single().filter(|o| matches!(self.heap.kind(*o), Kind::List | Kind::Tuple | Kind::Set)) {
            let known = |an: &mut Analyzer, sv: Value, state: &mut State| {
                // (Enum members / identity objects as their keys)
                let sv = if sv.pts.is_empty() { sv } else { an.keyify_in(&sv, state) };
                Some(kv_literals(&sv.kv)).filter(|ks| sv.pts.is_empty() && sv.kv.is_literal() && !ks.is_empty())
            };
            let n = self.heap.llen(o, state);
            let items: Vec<KeySet> = match n {
                Some(n) => (0..n).map(|i| known(self, state.slot(o, Ns::Pos, key_int(i as i64)), state)).collect::<Option<_>>()?,
                None => {
                    let e = Value::with_kv(state.slot(o, Ns::Elem, intern::STAR).pts, state.elems.get(&o).unwrap_or(DATA));
                    vec![known(self, e, state)?]
                }
            };
            let found = |k: KeyId| -> Option<bool> {
                let sure = n.is_some() && items.iter().any(|its| its.len() == 1 && lit_eq(its.iter().next().unwrap(), k) == Some(true));
                // (a NaN is in a container holding that very object: by identity)
                let nan = |x: KeyId| intern::with_key(x, |kk| matches!(kk, intern::Key::Float(b) if f64::from_bits(*b).is_nan()));
                let maybe = items.iter().any(|its| its.iter().any(|x| nan(x) || lit_eq(x, k) != Some(false)));
                if sure { Some(true) } else if !maybe { Some(false) } else { None }
            };
            let eqs: Vec<bool> = lits.iter().map(found).collect::<Option<_>>()?;
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
            match self.heap.obj(o).payload.clone() {
                // a view of a dict: its keys are the dict's (live); its values are decided
                // when they are all known literals
                Payload::View(ViewKind::Keys | ViewKind::Proxy, inner) => {
                    return self.compare_const(op, a, &inner, right, state);
                }
                Payload::View(ViewKind::Values, inner) => {
                    let d = inner.single().filter(|d| self.heap.kind(*d) == Kind::Dict)?;
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
        if recs.iter().all(|o| matches!(self.heap.kind(*o), Kind::Dict | Kind::Frame)) {
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

    /// An instance whose class (or a base) defines `__bool__` or `__len__`.
    pub(in crate::interp) fn has_truth_dunder(&self, oid: u32) -> bool {
        match self.heap.instance_class(oid) {
            Some(c) => self.class_lookup(c, "__bool__").is_some() || self.class_lookup(c, "__len__").is_some(),
            None => false,
        }
    }

    /// `truthiness`, with instances whose class defines `__bool__` / `__len__` decided by
    /// calling it (control-flow tests).
    pub fn truth_of(&mut self, v: &Value, state: &mut State, node: &Node) -> Option<bool> {
        // (a pandas DataFrame or Series the models made: its truth is ambiguous, a ValueError)
        let pandas = |an: &Analyzer, o: ObjId| an.heap.kind(o) == Kind::Frame || crate::models::pandas::is_series_obj(an, o);
        let n_pd = v.pts.iter().filter(|o| pandas(self, *o)).count();
        if n_pd > 0 {
            let rest = Value::with_kv(ObjSet::from_vec(v.pts.iter().filter(|o| !pandas(self, *o)).collect()), v.kv.clone());
            let all = rest.pts.is_empty() && rest.kv.is_bot();
            self.raise_exc("ValueError", None, None, all, state);
            return if all { None } else { self.truth_of(&rest, state, node) };
        }
        if !v.pts.is_empty() && v.pts.iter().any(|o| self.heap.kind(o) == Kind::Instance && self.has_truth_dunder(o)) {
            let mut verdicts: Vec<bool> = Vec::new();
            for o in v.pts.iter() {
                if self.heap.kind(o) != Kind::Instance || !self.has_truth_dunder(o) {
                    verdicts.push(self.truthiness(&Value::obj(o), state)?);
                    continue;
                }
                let cls = self.heap.instance_class(o).unwrap();
                let name = if self.class_lookup(cls, "__bool__").is_some() { "__bool__" } else { "__len__" };
                let r = self.call_method_with(&Value::obj(o), name, Vec::new(), node, state)?;
                verdicts.push(self.truthiness(&r, state)?);
            }
            verdicts.dedup();
            return if verdicts.len() == 1 { Some(verdicts[0]) } else { None };
        }
        self.truthiness(v, state)
    }

    /// Is `v` surely true or surely false (None: either)? Literals by Python's rules; a list,
    /// tuple or set of known length; a dict with a key on every path, or none; instances without
    /// `__bool__` / `__len__`, functions, classes and modules are true.
    pub fn truthiness(&mut self, v: &Value, state: &State) -> Option<bool> {
        if v.pts.is_empty() && v.kv.is_literal() {
            let lits = kv_literals(&v.kv);
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
            match self.heap.kind(oid) {
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
                Kind::List | Kind::Tuple | Kind::Set => match self.heap.llen(oid, state) {
                    None => return None,
                    Some(n) => verdicts.push(n > 0),
                },
                // (a class deriving from a builtin / library class — `class C(list)` — may have
                // its `__len__` / `__bool__`)
                Kind::Instance if self.has_truth_dunder(oid) || self.heap.instance_class(oid).is_some_and(|c| self.heap.mro(c).iter().any(|x| self.heap.class_info_opt(*x).is_some_and(|ci| ci.open_base.get() || ci.lib.dict_base))) => return None,
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

    /// The comparison method `name` an instance's (user) class defines.
    pub(super) fn cmp_dunder(&self, o: ObjId, name: &str) -> Option<ObjId> {
        if self.heap.kind(o) != Kind::Instance {
            return None;
        }
        let cls = self.heap.instance_class(o)?;
        let owner = self.class_lookup(cls, name)?;
        let fv = self.heap.cls_slots.get(&(owner, Ns::Attr, key_str_lit(name)))?;
        fv.pts.iter().find(|f| self.heap.kind(*f) == Kind::Function)
    }

    /// `a OP b` where an operand is an instance with comparison methods: the method, else the
    /// other's reflected one, else what `@functools.total_ordering` derives, else identity for
    /// `==` / `!=` (None: not such a comparison).
    pub(in crate::interp) fn rich_compare(&mut self, op: ast::CmpOp, a: &Value, b: &Value, node: &Node, state: &mut State) -> Option<Value> {
        // (one of several values, some of them instances of the program's classes: each
        // compared on its own path)
        let user = |an: &Analyzer, o: ObjId| an.heap.kind(o) == Kind::Instance && an.heap.instance_class(o).is_some();
        if let Some(v) = self.per_receiver(a, &user, state, &mut |an, p, st| an.rich_compare(op, p, b, node, st).unwrap_or_else(|| Value::data("bool"))) {
            return Some(v);
        }
        if let Some(v) = self.per_receiver(b, &user, state, &mut |an, p, st| an.rich_compare(op, a, p, node, st).unwrap_or_else(|| Value::data("bool"))) {
            return Some(v);
        }
        let inst = |an: &Analyzer, v: &Value| v.single().filter(|o| an.heap.kind(*o) == Kind::Instance && v.kv.is_bot());
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
        let call = |an: &mut Analyzer, f: ObjId, recv: &Value, other: &Value, state: &mut State| an.call_user(f, Some(recv.clone()), Args::pos(&[other.clone()]), state, node, false);
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
        let ordered = |an: &Analyzer, o: Option<ObjId>| o.and_then(|o| an.heap.instance_class(o)).map(|c| an.heap.mro(c).iter().any(|m| an.m.total_ordering.contains(m))).unwrap_or(false);
        if matches!(op, ast::CmpOp::Lt | ast::CmpOp::LtE | ast::CmpOp::Gt | ast::CmpOp::GtE) && ordered(self, ai) {
            if let Some(f) = ai.and_then(|o| self.cmp_dunder(o, "__lt__")) {
                let r = self.total_ordering_compare(op, a, b, f, node, state);
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
            if let Some(o) = self.ordered_instances(a, b, ai, bi, state) {
                return Some(lit(Some(decide(o))));
            }
        }
        // no __eq__: identity
        if matches!(op, ast::CmpOp::Eq | ast::CmpOp::NotEq) {
            let no_eq =|an: &Analyzer, o: Option<ObjId>| o.map(|o| an.cmp_dunder(o, "__eq__").is_none() && an.heap.instance_class(o).map(|c| !an.heap.class_info(c).open_base.get() && an.heap.class_info(c).dataclass_fields.borrow().is_none()).unwrap_or(false)).unwrap_or(true);
            if let (Some(x), Some(y)) = (ai, bi) {
                if no_eq(self, ai) && no_eq(self, bi) {
                    let same = if x == y && self.heap.obj(x).single { Some(true) } else if x != y { Some(false) } else { None };
                    return Some(lit(same.map(|s| if op == ast::CmpOp::Eq { s } else { !s })));
                }
            }
        }
        None
    }

    /// `@total_ordering`: `a op b` from the class's `__lt__` (run on `a`, `f`) and `__eq__`.
    fn total_ordering_compare(&mut self, op: ast::CmpOp, a: &Value, b: &Value, f: ObjId, node: &Node, state: &mut State) -> Option<bool> {
        let lt = self.call_user(f, Some(a.clone()), Args::pos(&[b.clone()]), state, node, false);
        let lt = self.truth_of(&lt, state, node);
        let eqv = self.rich_compare(ast::CmpOp::Eq, a, b, node, state).unwrap_or_else(|| Value::data("bool"));
        let eq = self.truth_of(&eqv, state, node);
        match op {
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
        }
    }

    /// The order of two values of which some are instances: an Enum member mixing in int / str
    /// by its value, two `@dataclass(order=True)` instances by their fields.
    fn ordered_instances(&mut self, a: &Value, b: &Value, ai: Option<ObjId>, bi: Option<ObjId>, state: &mut State) -> Option<std::cmp::Ordering> {
        // a member of an Enum mixing in int / str (IntEnum, StrEnum): its value
        let mixed = |an: &Analyzer, o: Option<ObjId>| -> Option<Value> {
            let o = o?;
            let c = an.heap.instance_class(o)?;
            if !an.m.enum_kind.get(&c).map(|k| k.mixed).unwrap_or(false) {
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
                return Some(o);
            }
        }
        // `@dataclass(order=True)`: two instances of the class compare their fields
        ai.zip(bi).and_then(|(x, y)| crate::models::dataclasses::dataclass_order(self, x, y, state))
    }

    /// Tuples and lists of known items compared item by item, per object of each side (`a`
    /// may be `[1, 2] | [0]`); a list against a tuple is unequal, and ordering them raises
    /// TypeError. None when some pair's order is not known.
    fn seq_compare(&mut self, op: ast::CmpOp, a: &Value, b: &Value, state: &mut State) -> Option<Value> {
        use std::cmp::Ordering::*;
        let parts = |an: &Analyzer, v: &Value| -> Option<Vec<(bool, Value)>> {
            if v.pts.is_empty() {
                let k = v.kv.as_lit()?;
                return intern::with_key(k, |kk| matches!(kk, intern::Key::Tuple(_))).then(|| vec![(false, v.clone())]);
            }
            if !v.kv.is_bot() || v.pts.len() > 4 {
                return None;
            }
            v.pts.iter().map(|o| match an.heap.kind(o) {
                Kind::Tuple | Kind::Pair => Some((false, Value::obj(o))),
                Kind::List if !an.heap.obj(o).iterator => Some((true, Value::obj(o))),
                _ => None,
            }).collect()
        };
        let (pa, pb) = (parts(self, a)?, parts(self, b)?);
        let (mut out, mut raises) = (KeySet::new(), 0);
        for (la, x) in &pa {
            for (lb, y) in &pb {
                let r = if la != lb {
                    match op {
                        ast::CmpOp::Eq => false,
                        ast::CmpOp::NotEq => true,
                        _ => {
                            raises += 1;
                            continue;
                        }
                    }
                } else {
                    let o = crate::models::stdfuncs::value_order(self, state, x, y)?;
                    match op {
                        ast::CmpOp::Lt => o == Less,
                        ast::CmpOp::LtE => o != Greater,
                        ast::CmpOp::Gt => o == Greater,
                        ast::CmpOp::GtE => o != Less,
                        ast::CmpOp::Eq => o == Equal,
                        _ => o != Equal,
                    }
                };
                out = out.insert(intern::key_bool(r));
            }
        }
        if out.is_empty() {
            self.raise_here("TypeError", state);
            return Some(Value::new());
        }
        if raises > 0 {
            self.may_raise_here("TypeError", state);
        }
        Some(Value::kv_only(KeyVal::lits(out)))
    }

    /// `a < b` for sorting / max / min when the items are instances ordering themselves.
    pub fn instance_less(&mut self, a: &Value, b: &Value, node: &Node, state: &mut State) -> Option<bool> {
        let r = self.rich_compare(ast::CmpOp::Lt, a, b, node, state)?;
        self.truth_of(&r, state, node)
    }
}

/// Ordering of two literals of one kind: numbers (ints, floats, bools) or strings.
/// How Python orders two literals (None: they do not compare).
pub fn literal_order(x: KeyId, y: KeyId) -> Option<std::cmp::Ordering> {
    crate::models::pyvalues::py_cmp(x, y)
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
