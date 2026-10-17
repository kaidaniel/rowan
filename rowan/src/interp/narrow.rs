//! Tests: a test is evaluated once into the states where it is true and where it is false
//! (`split`); narrowing reads the values its parts had then (`traced`), never evaluating
//! them again.

use ruff_python_ast as ast;

use crate::domain::{kv_exclude, kv_literals, kv_restrict, KeyVal, Ns, ObjId, ObjSet, Value};
use crate::frontend::{self, Node};
use crate::intern::{self, key_str_lit, s, KeyId, KeySet, StrId};
use crate::state::{Kind, State};
use crate::syntax::{chain_text, contains_call, names_in};

use super::Analyzer;

enum NarrowTarget {
    Name(String),
    Attr(&'static ast::ExprAttribute, &'static ast::Expr),
}

/// A test evaluated: its value, the state where it is true and the one where it is false (a
/// side it rules out is unreachable), and a comparison's operands.
pub struct Split {
    pub v: Value,
    pub t: State,
    pub f: State,
    pub ops: Option<(Value, Value)>,
}

/// Where a narrowed value is written back (`_narrow_isinstance`'s `store` closure).
#[derive(Clone)]
pub enum StoreTarget {
    Name(StrId),
    Narrowed(StrId),
    Slot(ObjId, Ns, KeyId),
}

impl Analyzer {
    /// Evaluate `test` once from `st`, splitting the state by its truth.
    pub(in crate::interp) fn split(&mut self, test: &'static ast::Expr, mut st: State) -> Split {
        match test {
            ast::Expr::UnaryOp(u) if u.op == ast::UnaryOp::Not => {
                let sp = self.split(&u.operand, st);
                let v = match (sp.t.reachable, sp.f.reachable) {
                    (true, false) => Value::lit(intern::key_bool(false)),
                    (false, true) => Value::lit(intern::key_bool(true)),
                    _ => Value::data("bool"),
                };
                Split { v, t: sp.f, f: sp.t, ops: None }
            }
            ast::Expr::BoolOp(b) => {
                let (v, cont, stops) = self.boolop(b, st, true);
                let stops = stops.unwrap_or_else(State::bottom);
                let (t, f) = if b.op == ast::BoolOp::And { (cont, stops) } else { (stops, cont) };
                Split { v, t, f, ops: None }
            }
            _ => {
                let saved = self.mode.trace.replace((self.cx.frames.len(), Default::default()));
                let v = self.eval(test, &mut st);
                let vals = std::mem::replace(&mut self.mode.trace, saved).map(|t| t.1).unwrap_or_default();
                let truth = if st.reachable { self.truth_of(&v, &mut st, &frontend::node(test)) } else { None };
                let saved = std::mem::replace(&mut self.mode.traced, vals);
                let ops = match test {
                    ast::Expr::Compare(c) if c.ops.len() == 1 => self.traced(&c.left).zip(self.traced(&c.comparators[0])),
                    _ => None,
                };
                let mut t = st.copy();
                let mut f = st;
                t.reachable &= truth != Some(false);
                f.reachable &= truth != Some(true);
                self.narrow(test, true, &mut t);
                self.narrow(test, false, &mut f);
                self.mode.traced = saved;
                Split { v, t, f, ops }
            }
        }
    }

    /// `a and b …` / `a or b …` from `st`: its value, the state where the last operand ran to
    /// its end (narrowed by it too when `test`), and the join of the states where an earlier
    /// operand ended it.
    pub(in crate::interp) fn boolop(&mut self, b: &'static ast::ExprBoolOp, mut st: State, test: bool) -> (Value, State, Option<State>) {
        let is_and = b.op == ast::BoolOp::And;
        let mut v = Value::new();
        let mut stops: Option<State> = None;
        let mut depth_added = false;
        let last = b.values.len() - 1;
        for (i, e) in b.values.iter().enumerate() {
            if i == last && !test {
                let ev = self.eval(e, &mut st);
                if st.reachable {
                    v = v.join(&ev);
                }
                break;
            }
            let sp = self.split(e, st);
            let (mut stop, cont) = if is_and { (sp.f, sp.t) } else { (sp.t, sp.f) };
            if i == last {
                v = v.join(&sp.v);
            } else if stop.reachable {
                // (what ends it here is a value of that truth: `x and y` is x only when x is
                // falsy; a bool is that bool)
                let is_bool = sp.v.pts.is_empty() && matches!(sp.v.kv, KeyVal::Data(d) if &*intern::str_get(d) == "bool" || crate::domain::label_type(&intern::str_get(d)) == Some("bool"));
                v = v.join(&if is_bool { Value::lit(intern::key_bool(!is_and)) } else { truth_narrowed(&sp.v, !is_and).unwrap_or(sp.v) });
            }
            if stop.reachable {
                stops = Some(match stops.take() {
                    Some(mut acc) => acc.join(&mut stop),
                    None => stop,
                });
                // the rest runs on some paths only
                if !depth_added {
                    self.cx.cond_depth += 1;
                    self.cx.branch_depth += 1;
                    depth_added = true;
                }
            }
            st = cont;
            if !st.reachable {
                break;
            }
        }
        if depth_added {
            self.cx.branch_depth -= 1;
            self.cx.cond_depth -= 1;
        }
        (v, st, stops)
    }

    /// The value a part of the test being narrowed had when it was evaluated (None: it was
    /// not, as an operand after a short circuit).
    pub(crate) fn traced(&self, e: &'static ast::Expr) -> Option<Value> {
        self.mode.traced.get(&(e as *const ast::Expr as usize)).cloned()
    }

    /// Narrow `state` to the paths where the evaluated `test` is `truth`.
    fn narrow(&mut self, test: &'static ast::Expr, truth: bool, state: &mut State) {
        if !state.reachable {
            return;
        }
        if self.m.torch_on {
            crate::models::torch::t_narrow(self, test, truth, state);
            if !state.reachable {
                return;
            }
        }
        match test {
            ast::Expr::Named(n) => self.narrow(&n.target, truth, state),
            ast::Expr::Compare(c) if c.ops.len() == 1 => self.narrow_compare(c, truth, state),
            ast::Expr::Call(call) => self.narrow_call(test, call, truth, state),
            // `if items:` / `if not items:` on a list of unknown length: on this path it has a
            // first element / is empty (this path's length only, not the object's)
            ast::Expr::Name(n) => self.narrow_name(n, truth, state),
            // `if obj.attr:` — the attribute chain's literals of the other truth are not it here
            ast::Expr::Attribute(_) => {
                if let (Some(key), Some(v)) = (chain_text(test), self.traced(test)) {
                    if let Some(nv) = truth_narrowed(&v, truth) {
                        if nv.empty() {
                            state.reachable = false;
                        } else {
                            state.narrowed.insert(s(&key), nv);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// `if x:` / `if not x:` on the path where it is `truth`: the literals of the other truth are
    /// not `x`; a list of unknown length has a first element / is empty (on this path only).
    fn narrow_name(&mut self, n: &'static ast::ExprName, truth: bool, state: &mut State) {
        let mut v = self.load_name(s(n.id.as_str()), &Node::none(), state);
        if let Some(nv) = truth_narrowed(&v, truth) {
            if nv.empty() {
                state.reachable = false;
                return;
            }
            self.rebind_name(s(n.id.as_str()), nv.clone(), state);
            v = nv;
        }
        // several containers: on the truthy path the ones known to be empty are not it
        if truth && v.pts.len() > 1 && v.kv.is_bot() {
            let keep: Vec<u32> = v.pts.iter().filter(|o| !(matches!(self.heap.kind(*o), Kind::List | Kind::Tuple | Kind::Set) && self.heap.llen(*o, state) == Some(0))).collect();
            if !keep.is_empty() && keep.len() < v.pts.len() {
                v = Value::objs(crate::domain::ObjSet::from_vec(keep));
                self.rebind_name(s(n.id.as_str()), v.clone(), state);
            }
        }
        if v.pts.len() == 1 && v.kv.is_bot() {
            let o = v.pts.first().unwrap();
            if matches!(self.heap.kind(o), Kind::List | Kind::Tuple) && self.heap.obj(o).single && self.heap.llen(o, state).is_none() {
                if truth {
                    if self.heap.prefix(o, state).is_none() {
                        let e = state.slot(o, Ns::Elem, intern::STAR);
                        let ekv = state.elems.get(&o).unwrap_or(crate::domain::DATA);
                        state.set_slot(o, Ns::Pos, intern::key_int(0), Value::with_kv(e.pts, ekv), true);
                        state.lens.insert(o, (None, 1, 0));
                    }
                } else if self.heap.prefix(o, state).is_none() {
                    state.lens.insert(o, (Some(0), 0, 0));
                }
            }
            // a falsy dict is empty on this path (`while queue:` ends with no keys)
            if !truth && self.heap.kind(o) == Kind::Dict && self.heap.obj(o).single {
                state.set_row(o, Ns::Item, crate::domain::Row::new());
                state.drop_obj_ns(o, Ns::Item);
            }
            // a falsy unknown value (a JSON document, an input): an empty container if
            // len() or a key read accepts it at all (a falsy number makes them raise)
            if !truth && self.heap.kind(o) == Kind::Unknown && self.heap.obj(o).single && v.kv.is_bot() {
                state.set_row(o, Ns::Item, crate::domain::Row::new());
                state.drop_obj_ns(o, Ns::Item);
                state.drop_obj_ns(o, Ns::Pos);
                state.drop_obj_ns(o, Ns::Elem);
                state.lens.insert(o, (Some(0), 0, 0));
            }
        }
    }

    /// A test that is a call on the path where it is `truth`: `isinstance`, `hasattr`, `callable`,
    /// `d.get(k)`, a user predicate, …
    fn narrow_call(&mut self, test: &'static ast::Expr, call: &'static ast::ExprCall, truth: bool, state: &mut State) {
        let args = &call.arguments.args;
        // `isclass(x)` / `inspect.isclass(x)`
        let fname = match &*call.func {
            ast::Expr::Name(n) => Some(n.id.as_str()),
            ast::Expr::Attribute(a) => Some(a.attr.id.as_str()),
            _ => None,
        };
        if let (Some("isclass"), [ast::Expr::Name(n)]) = (fname, &args[..]) {
            self.narrow_isclass(s(n.id.as_str()), truth, state);
            return;
        }
        if let ast::Expr::Name(fnm) = &*call.func {
            let fname = fnm.id.as_str();
            if fname == "isinstance" && args.len() == 2 {
                match &args[0] {
                    ast::Expr::Name(n) => self.narrow_isinstance(NarrowTarget::Name(n.id.to_string()), &args[1], truth, state),
                    ast::Expr::Attribute(a) => self.narrow_isinstance(NarrowTarget::Attr(a, &args[0]), &args[1], truth, state),
                    _ => {}
                }
                return;
            }
            if fname == "hasattr" && args.len() == 2 {
                let (Some(kv), Some(mut rec)) = (self.traced(&args[1]), self.traced(&args[0])) else { return };
                if let Some(k) = kv.kv.as_lit() {
                    // (an attribute row describes instances and namespaces; a builtin
                    // container's attributes are its type's: decided by the type, or not)
                    let builtin = |an: &Analyzer, o: u32| matches!(an.heap.kind(o), Kind::Dict | Kind::List | Kind::Tuple | Kind::Set);
                    if !rec.pts.is_empty() && rec.pts.iter().all(|o| builtin(self, o)) {
                        if let Some(name) = intern::key_as_str(k) {
                            if rec.pts.iter().all(|o| crate::models::builtin_has_attr(self, o, &name) == Some(!truth)) {
                                state.reachable = false;
                            }
                        }
                        return;
                    }
                    // (an instance whose class defines the name has it whatever its own
                    // attributes are: its row says nothing about it)
                    let by_class = |an: &Analyzer, o: u32| intern::key_as_str(k).is_some_and(|n| an.class_resolves(o, &n));
                    let kept = rec.pts.filter(|o| !builtin(self, o) && !by_class(self, o));
                    // (where it holds, it may hold for those others: the rest say nothing)
                    if truth && kept.len() < rec.pts.len() {
                        return;
                    }
                    rec = Value::with_kv(kept, KeyVal::Bot);
                    self.narrow_record(&rec, Ns::Attr, k, truth, state);
                }
                return;
            }
            if fname == "all" && args.len() == 1 {
                if let ast::Expr::Generator(g) = &args[0] {
                    if truth && g.generators.len() == 1 {
                        if let ast::Expr::Compare(cmp) = &*g.elt {
                            if cmp.ops.first() == Some(&ast::CmpOp::In) {
                                let (Some(itv), Some(rec)) = (self.traced(&g.generators[0].iter), self.traced(&cmp.comparators[0])) else { return };
                                let ks = self.sure_lit_keys(&itv, state);
                                if let Some(ks) = ks {
                                    if matches!(&*cmp.left, ast::Expr::Name(_)) {
                                        for k in ks.iter() {
                                            self.narrow_record(&rec, Ns::Item, k, true, state);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                return;
            }
        }
        if let ast::Expr::Attribute(fa) = &*call.func {
            if fa.attr.id.as_str() == "issubset" && args.len() == 1 && truth {
                let (Some(v), Some(rec)) = (self.traced(&fa.value), self.traced(&args[0])) else { return };
                let ks = self.sure_lit_keys(&v, state);
                if let Some(ks) = ks {
                    for k in ks.iter() {
                        self.narrow_record(&rec, Ns::Item, k, true, state);
                    }
                }
                return;
            }
            if fa.attr.id.as_str() == "get" && truth {
                self.narrow_get_truthy(test, state);
            }
        }
    }

    /// A test `a op b` on the path where it is `truth`: `is None`, `==` literals, `in` a
    /// record or a container, `len(x) == n`, …
    fn narrow_compare(&mut self, c: &'static ast::ExprCompare, truth: bool, state: &mut State) {
        let op = c.ops[0];
        let left: &'static ast::Expr = &c.left;
        let right: &'static ast::Expr = &c.comparators[0];
        match op {
            ast::CmpOp::In | ast::CmpOp::NotIn => {
                let member = if op == ast::CmpOp::In { truth } else { !truth };
                self.narrow_membership(left, right, member, state);
            }
            ast::CmpOp::Eq | ast::CmpOp::NotEq => self.narrow_eq(op, left, right, truth, state),
            ast::CmpOp::Is | ast::CmpOp::IsNot if !matches!(right, ast::Expr::NoneLiteral(_)) => {
                // `t is a` with `a` one object: on the true side t is a, on the false side
                // a is not among t's objects
                let same = if op == ast::CmpOp::Is { truth } else { !truth };
                for (name_e, other_e) in [(left, right), (right, left)] {
                    let ast::Expr::Name(nm) = name_e else { continue };
                    let name = s(nm.id.as_str());
                    let cur = self.load_name(name, &Node::none(), state);
                    let Some(o) = self.traced(other_e) else { continue };
                    let Some(target) = o.single().filter(|x| o.kv.is_bot() && self.heap.obj(*x).single && !crate::state::is_merged(*x)) else { continue };
                    if !cur.pts.contains(target) || cur.pts.len() < 2 || !cur.kv.is_bot() {
                        continue;
                    }
                    if same {
                        self.rebind_name(name, Value::obj(target), state);
                    } else {
                        let rest = cur.pts.filter(|x| x != target);
                        self.rebind_name(name, Value::objs(rest), state);
                    }
                    break;
                }
            }
            ast::CmpOp::Is | ast::CmpOp::IsNot => {
                if matches!(right, ast::Expr::NoneLiteral(_)) {
                    let is_none = if op == ast::CmpOp::Is { truth } else { !truth };
                    if !is_none {
                        self.narrow_get_truthy(left, state);
                    }
                    // the name is None / is not None
                    if let ast::Expr::Name(l) = left {
                        let cur = self.load_name(s(l.id.as_str()), &Node::none(), state);
                        let has_none = kv_literals(&cur.kv).contains(intern::key_none()) || matches!(cur.kv, KeyVal::Data(l) if intern::str_get(l).starts_with(crate::models::shape::OPT_PREFIX));
                        if has_none || is_none {
                            self.narrow_kv(s(l.id.as_str()), &KeySet::one(intern::key_none()), is_none, state);
                        }
                    }
                }
            }
            ast::CmpOp::LtE | ast::CmpOp::Lt => {
                if let ast::Expr::Call(rc) = right {
                    if let ast::Expr::Attribute(ra) = &*rc.func {
                        if ra.attr.id.as_str() == "keys" {
                            let (Some(lv), Some(rec)) = (self.traced(left), self.traced(&ra.value)) else { return };
                            let ks = self.sure_lit_keys(&lv, state);
                            if let Some(ks) = ks {
                                if truth {
                                    for k in ks.iter() {
                                        self.narrow_record(&rec, Ns::Item, k, true, state);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// `a == b` / `a != b` on the path where the test is `truth`.
    fn narrow_eq(&mut self, op: ast::CmpOp, left: &'static ast::Expr, right: &'static ast::Expr, truth: bool, state: &mut State) {
        if let ast::Expr::Name(l) = left {
            let eq = if op == ast::CmpOp::Eq { truth } else { !truth };
            let Some(rv) = self.traced(right) else { return };
            let ks = kv_literals(&rv.kv);
            // (Decimal / Fraction / datetime literals equal others by value, not
            // as keys: no narrowing by key identity)
            let lv0 = self.load_name(s(l.id.as_str()), &Node::none(), state);
            // (numbers too: `1 == 1.0 == True` though their keys differ)
            let valued = |k: KeyId| crate::models::pynum::is_num_key(k) || crate::models::chrono::is_chrono_key(k) || intern::key_is_number(k) || intern::with_key(k, |kk| matches!(kk, intern::Key::Bool(_)));
            let by_value = ks.iter().any(valued) || kv_literals(&lv0.kv).iter().any(valued);
            if !ks.is_empty() && !by_value {
                self.narrow_kv(s(l.id.as_str()), &ks, eq, state);
            }
        } else if let Some(la) = match left { ast::Expr::Attribute(la) if matches!(&*la.value, ast::Expr::Name(_)) => Some(la), _ => None } {
            // `obj.attr == lit` on one object whose attribute holds known
            // literals: the attribute narrows (`self.unit == "F"`)
            let eq = if op == ast::CmpOp::Eq { truth } else { !truth };
            let (Some(rv), Some(ov)) = (self.traced(right), self.traced(&la.value)) else { return };
            let ks = kv_literals(&rv.kv);
            let ak = key_str_lit(la.attr.id.as_str());
            if let Some(o) = ov.single().filter(|o| self.heap.obj(*o).single && matches!(self.heap.kind(*o), Kind::Instance | Kind::Namespace) && rv.pts.is_empty() && ks.len() == 1) {
                let cur = state.slot_opt(o, Ns::Attr, ak);
                if let Some(cur) = cur.filter(|c| c.pts.is_empty() && c.kv.is_literal()) {
                    let lits = kv_literals(&cur.kv);
                    let valued = |k: KeyId| crate::models::pynum::is_num_key(k) || crate::models::chrono::is_chrono_key(k) || intern::key_is_number(k);
                    if !lits.iter().any(valued) && !ks.iter().any(valued) {
                        let keep: KeySet = if eq { lits.intersection(&ks) } else { lits.difference(&ks) };
                        if keep.is_empty() {
                            state.reachable = false;
                        } else if keep.len() < lits.len() {
                            state.set_slot(o, Ns::Attr, ak, Value::kv_only(KeyVal::lits(keep)), true);
                        }
                    }
                }
            }
        }
    }

    fn narrow_isinstance(&mut self, target: NarrowTarget, cls_expr: &'static ast::Expr, truth: bool, state: &mut State) {
        let (cur, store): (Value, StoreTarget) = match target {
            NarrowTarget::Name(name) => {
                let sid = s(&name);
                let cur = self.load_name(sid, &Node::none(), state);
                (cur, StoreTarget::Name(sid))
            }
            NarrowTarget::Attr(a, whole) => {
                let Some(base) = self.traced(&a.value) else { return };
                let recs: Vec<u32> = base.pts.iter().filter(|o| matches!(self.heap.kind(*o), Kind::Instance | Kind::Namespace)).collect();
                if recs.len() != 1 || base.pts.len() != 1 || !self.heap.obj(recs[0]).single {
                    let Some(key) = chain_text(whole) else { return };
                    if !truth {
                        return;
                    }
                    let Some(cur) = self.traced(whole).filter(|c| !c.pts.is_empty()) else { return };
                    if cur.pts.is_empty() {
                        return;
                    }
                    self.narrow_isinstance_value(&cur, cls_expr, truth, state, StoreTarget::Narrowed(s(&key)));
                    return;
                }
                let bo = recs[0];
                let attr = key_str_lit(a.attr.id.as_str());
                // (one the instance has on some paths only reads its class's on the others: its
                // slot is not all it may be)
                if !state.row(bo, Ns::Attr).definite.contains(attr) {
                    return;
                }
                let cur = state.slot(bo, Ns::Attr, attr);
                (cur, StoreTarget::Slot(bo, Ns::Attr, attr))
            }
        };
        if cur.pts.is_empty() {
            return;
        }
        self.narrow_isinstance_value(&cur, cls_expr, truth, state, store);
    }

    /// After `isclass(x)`: x is one of its values that may be a class (or not one that surely
    /// is). Literals are not classes; a library object may be one.
    fn narrow_isclass(&mut self, name: StrId, truth: bool, state: &mut State) {
        let cur = self.load_name(name, &Node::none(), state);
        let keep: Vec<u32> = cur.pts.iter().filter(|o| match self.heap.kind(*o) {
            Kind::Class => truth,
            Kind::External | Kind::Unknown => true,
            _ => !truth,
        }).collect();
        let kv = if truth && cur.kv.is_literal() { KeyVal::Bot } else { cur.kv.clone() };
        if keep.len() == cur.pts.len() && kv == cur.kv {
            return;
        }
        if keep.is_empty() && kv.is_bot() {
            state.reachable = false;
            return;
        }
        self.store_narrowed(&StoreTarget::Name(name), Value::with_kv(ObjSet::from_vec(keep), kv), state);
    }

    fn store_narrowed(&mut self, store: &StoreTarget, v: Value, state: &mut State) {
        match store {
            StoreTarget::Name(n) => self.rebind_name(*n, v, state),
            StoreTarget::Narrowed(k) => {
                state.narrowed.insert(*k, v);
            }
            StoreTarget::Slot(o, ns, k) => state.set_slot(*o, *ns, *k, v, true),
        }
    }

    pub(in crate::interp) fn narrow_isinstance_value(&mut self, cur: &Value, cls_expr: &'static ast::Expr, truth: bool, state: &mut State, store: StoreTarget) {
        let Some(cv) = self.traced(cls_expr) else { return };
        let Some(classes) = self.isinstance_classes(&cv, state) else { return };
        let keep: Vec<u32> = cur.pts.iter().filter(|o| self.instance_of(*o, &classes, state) != Some(!truth)).collect();
        let new_pts = ObjSet::from_vec(keep);
        if new_pts == cur.pts {
            return;
        }
        if new_pts.is_empty() && cur.kv.is_bot() {
            state.reachable = false;
            return;
        }
        self.store_narrowed(&store, Value::with_kv(new_pts, cur.kv.clone()), state);
    }

    /// The classes an `isinstance` test names (a class, or a tuple of them); None when it is
    /// something else.
    pub fn isinstance_classes(&self, cv: &Value, state: &State) -> Option<Vec<u32>> {
        let mut out: Vec<u32> = Vec::new();
        for o in cv.pts.iter() {
            match self.heap.kind(o) {
                Kind::Class | Kind::External => out.push(o),
                Kind::Tuple | Kind::List => {
                    for (_, cs) in state.slots_of(o) {
                        out.extend(cs.pts.iter().filter(|x| matches!(self.heap.kind(*x), Kind::Class | Kind::External)));
                    }
                }
                _ => return None,
            }
        }
        (!out.is_empty()).then_some(out)
    }

    /// Is object `o` an instance of one of `classes` (None: not known)? For both the value of
    /// an `isinstance` test and what it narrows.
    pub fn instance_of(&self, o: u32, classes: &[u32], st: &State) -> Option<bool> {
        let k = if crate::models::pandas::is_series_obj(self, o) { Kind::Series } else { self.heap.kind(o) };
        if k == Kind::Unknown {
            return None;
        }
        let per: Vec<Option<bool>> = classes.iter().map(|c| self.instance_of_one(o, k, *c, st)).collect();
        if per.contains(&Some(true)) {
            Some(true)
        } else if per.iter().all(|x| *x == Some(false)) {
            Some(false)
        } else {
            None
        }
    }

    fn instance_of_one(&self, o: u32, k: Kind, cls: u32, st: &State) -> Option<bool> {
        if self.heap.kind(cls) == Kind::Class {
            // (a Protocol: an instance whose class defines all its members is one; one of a
            // closed hierarchy without `__getattr__` that surely lacks a member is not; anything
            // else may have them as attributes)
            if let Some(ms) = crate::models::protocol_members(self, cls) {
                let ic = self.heap.instance_class(o).filter(|_| k == Kind::Instance)?;
                if ms.iter().all(|m| self.class_lookup(ic, m).is_some()) {
                    return Some(true);
                }
                let row = st.row(o, Ns::Attr);
                let lacks = |m: &String| self.class_lookup(ic, m).is_none() && !row.present.contains(intern::key_str_lit(m));
                let closed = self.hierarchy_closed(ic) && row.closed() && ["__getattr__", "__getattribute__"].iter().all(|h| self.class_lookup(ic, h).is_none());
                return (closed && ms.iter().any(lacks)).then_some(false);
            }
            return if k == Kind::Instance { Some(self.heap.instance_class(o).is_some_and(|ic| ic == cls || self.is_subclass(ic, cls))) } else { Some(false) };
        }
        let full = self.heap.external_qual(cls);
        let b = full.rsplit('.').next().unwrap_or("");
        let structural = is_structural_qual(&full);
        if b == "object" {
            return Some(true);
        }
        if k == Kind::Instance {
            // (a class deriving from a builtin / library class, or one an ABC may recognise by
            // its methods: not decided)
            let ic = self.heap.instance_class(o)?;
            let open = self.heap.mro(ic).iter().any(|c| self.heap.class_info_opt(*c).is_some_and(|ci| ci.open_base || ci.lib.dict_base));
            return (!open && !structural).then_some(false);
        }
        let container = matches!(k, Kind::Dict | Kind::List | Kind::Tuple | Kind::Pair | Kind::Set | Kind::Frame | Kind::Series);
        let code = matches!(k, Kind::Class | Kind::Function | Kind::Bound | Kind::Module);
        match b {
            // (classes are `type`'s instances; functions, classes and modules are no plain value's)
            "type" if code => Some(k == Kind::Class),
            // (a library class other than an ABC or a `types` one: no class, function or module
            // is its instance, short of a metaclass)
            _ if code && !structural && !full.starts_with("types.") && !b.ends_with("Meta") => Some(false),
            "dict" => container.then_some(k == Kind::Dict),
            // (a dict object may be one of these)
            "defaultdict" | "OrderedDict" | "Counter" | "deque" | "UserDict" | "ChainMap" if matches!(k, Kind::Dict | Kind::List) => None,
            "list" => container.then_some(k == Kind::List),
            "tuple" => container.then_some(matches!(k, Kind::Tuple | Kind::Pair)),
            "set" | "frozenset" => container.then_some(k == Kind::Set),
            "DataFrame" => container.then_some(k == Kind::Frame),
            "Series" => container.then_some(k == Kind::Series),
            "Iterable" | "Collection" | "Container" | "Sized" if structural => container.then_some(true),
            "Mapping" | "MutableMapping" if structural => container.then_some(k == Kind::Dict),
            "Sequence" if structural => container.then_some(matches!(k, Kind::List | Kind::Tuple | Kind::Pair)),
            "MutableSequence" if structural => container.then_some(k == Kind::List),
            "Set" | "AbstractSet" | "MutableSet" if structural => container.then_some(k == Kind::Set),
            // (no builtin container is a library class's instance; an ABC's or a protocol's
            // it may be; functions and modules are no plain value's)
            _ if container && !structural => Some(false),
            _ => None,
        }
    }

    /// A user class whose bases are all user classes (or `object`), transitively.
    pub fn hierarchy_closed(&self, cls: u32) -> bool {
        let mut work = vec![cls];
        let mut seen: Vec<u32> = Vec::new();
        while let Some(c) = work.pop() {
            if seen.contains(&c) {
                continue;
            }
            seen.push(c);
            let Some(ci) = self.heap.class_info_opt(c) else { return false };
            if ci.open_base {
                return false;
            }
            work.extend(ci.bases.iter().copied());
        }
        true
    }

    pub(in crate::interp) fn is_subclass(&self, cls: u32, base: u32) -> bool {
        let mut seen: Vec<u32> = Vec::new();
        let mut order = vec![cls];
        while !order.is_empty() {
            let c = order.remove(0);
            if c == base {
                return true;
            }
            if seen.contains(&c) {
                continue;
            }
            seen.push(c);
            if let Some(ci) = self.heap.class_info_opt(c) {
                order.extend(ci.bases.iter().copied());
            }
        }
        false
    }

    fn narrow_get_truthy(&mut self, expr: &'static ast::Expr, state: &mut State) {
        if let ast::Expr::Call(c) = expr {
            if let ast::Expr::Attribute(a) = &*c.func {
                if a.attr.id.as_str() == "get" && !c.arguments.args.is_empty() {
                    let (Some(kv), Some(rec)) = (self.traced(&c.arguments.args[0]), self.traced(&a.value)) else { return };
                    if let Some(k) = kv.kv.as_lit() {
                        // only a dict-like receiver: a user instance's `get` is its own method
                        if rec.pts.iter().any(|o| matches!(self.heap.kind(o), Kind::Instance | Kind::Namespace)) {
                            return;
                        }
                        // a name holding several dicts: those that surely lack the key are not it
                        if let ast::Expr::Name(n) = &*a.value {
                            if rec.pts.len() > 1 {
                                let keep: Vec<u32> = rec.pts.iter().filter(|o| !(self.heap.kind(*o) == Kind::Dict && { let r = state.row(*o, Ns::Item); !r.open() && !r.present.contains(k) })).collect();
                                if !keep.is_empty() && keep.len() < rec.pts.len() {
                                    let nv = Value::with_kv(ObjSet::from_vec(keep), rec.kv.clone());
                                    self.store_narrowed(&StoreTarget::Name(s(n.id.as_str())), nv.clone(), state);
                                    self.narrow_record(&nv, Ns::Item, k, true, state);
                                    return;
                                }
                            }
                        }
                        self.narrow_record(&rec, Ns::Item, k, true, state);
                    }
                }
            }
        }
    }

    /// `left in right` held (`member`) or not: `right`'s record (of `d.keys()`, `df.columns`)
    /// and `left`'s value as the test evaluated them.
    fn narrow_membership(&mut self, left: &'static ast::Expr, right: &'static ast::Expr, member: bool, state: &mut State) {
        let rec_expr = membership_record(right);
        let (Some(lv), Some(rv)) = (self.traced(left), self.traced(rec_expr)) else { return };
        self.narrow_member_values(left, &lv, rec_expr, &rv, member, state);
    }

    fn narrow_member_values(&mut self, left: &'static ast::Expr, lv: &Value, rec_expr: &'static ast::Expr, rv: &Value, member: bool, state: &mut State) {
        let (lv, rv) = (lv.clone(), rv.clone());
        // user classes defining `__contains__` as `return <x> in <y>`: narrow <y> instead
        let insts: Vec<u32> = rv.pts.iter().filter(|o| self.heap.kind(*o) == Kind::Instance).collect();
        if !insts.is_empty() && rv.pts.len() == 1 {
            if let Some(cls) = self.heap.instance_class(insts[0]) {
                if let Some(m) = self.class_lookup(cls, "__contains__") {
                    let fv = self.heap.cls_slots.get(&(m, Ns::Attr, key_str_lit("__contains__"))).cloned().unwrap_or_default();
                    for f in fv.pts.iter() {
                        if self.heap.kind(f) != Kind::Function {
                            continue;
                        }
                        let fi = self.heap.func_info(f);
                        let body = fi.node.body();
                        if body.len() == 1 {
                            if let ast::Stmt::Return(r) = &body[0] {
                                if let Some(ast::Expr::Compare(cmp)) = r.value.as_deref() {
                                    if cmp.ops.len() == 1 && cmp.ops[0] == ast::CmpOp::In {
                                        let cl: &'static ast::Expr = &cmp.left;
                                        let cr: &'static ast::Expr = &cmp.comparators[0];
                                        let lv2 = lv.clone();
                                        self.in_method_frame(f, Value::obj(insts[0]), vec![lv2], state, &mut |an: &mut Analyzer, st: &mut State| {
                                            let cr = membership_record(cr);
                                            let (l, r) = (an.eval(cl, st), an.eval(cr, st));
                                            an.narrow_member_values(cl, &l, cr, &r, member, st);
                                        });
                                    }
                                }
                            }
                        }
                    }
                    return;
                }
            }
        }
        let recs: Vec<u32> = rv.pts.iter().filter(|o| self.heap.is_record(*o)).collect();
        // (a value that is a literal or an unknown object may be any key: no narrowing by its
        // literal part)
        let lit_only = lv.pts.iter().all(|o| !matches!(self.heap.kind(o), Kind::Unknown | Kind::Instance | Kind::External));
        if let (ast::Expr::Tuple(t), true) = (left, member) {
            self.narrow_tuple_parts(t, &recs, state);
        }
        if !recs.is_empty() {
            match &lv.kv {
                // (the key is one of the literals or one of the objects: where it is in closed
                // records, a literal they cannot have is not it; the objects stay)
                KeyVal::Lit(_) | KeyVal::Lits(_) if !lit_only => {
                    if let (true, Some(may)) = (member, closed_keys(&recs, state)) {
                        let ks = match &lv.kv {
                            KeyVal::Lit(k) => KeySet::from_vec(vec![*k]),
                            KeyVal::Lits(ks) => ks.clone(),
                            _ => unreachable!(),
                        };
                        self.narrow_key_expr(left, &lv, &ks, &may, true, state);
                    }
                }
                KeyVal::Lit(k) => {
                    // whichever object the name holds has the key (several objects: none narrows)
                    if member && recs.len() > 1 {
                        if let ast::Expr::Name(nm) = rec_expr {
                            state.add_name_key(s(nm.id.as_str()), *k);
                        }
                    }
                    self.narrow_record(&rv, Ns::Item, *k, member, state)
                }
                // an unknown key (data, or an unknown value's object) in closed dicts of string
                // keys: one of those keys
                KeyVal::KeysOf(_) | KeyVal::Data(_) | KeyVal::Bot if member && (lv.pts.is_empty() || lv.pts.iter().all(|o| self.heap.kind(o) == Kind::Unknown)) && !(lv.pts.is_empty() && lv.kv.is_bot()) => {
                    if let Some(may) = closed_keys(&recs, state) {
                        if !may.is_empty() && may.iter().all(intern::key_is_str) {
                            self.narrow_key_expr(left, &lv, &may, &may, false, state);
                        } else if may.is_empty() {
                            state.reachable = false;
                        }
                    }
                }
                KeyVal::Lits(ks) if member => {
                    if let Some(may) = closed_keys(&recs, state) {
                        let ks = ks.clone();
                        self.narrow_key_expr(left, &lv, &ks, &may, false, state);
                    }
                }
                _ => {}
            }
            return;
        }
        // (a literal right side is a string: `in` tests substrings, not keys)
        if let (ast::Expr::Name(l), false) = (left, !rv.kv.is_bot() || rv.pts.is_empty()) {
            // (in: one of the literals it may hold; not in: none of those it surely holds)
            let ks = if member { self.lit_keys(&rv, state) } else { self.sure_lit_keys(&rv, state) };
            if let Some(ks) = ks {
                self.narrow_kv(s(l.id.as_str()), &ks, member, state);
            }
        }
    }

    /// `(a, b) in d` with `d` among closed records `recs`: each name of the tuple only the
    /// literals some key has at its position (the objects stay).
    fn narrow_tuple_parts(&mut self, t: &ast::ExprTuple, recs: &[u32], state: &mut State) {
        let Some(may) = closed_keys(recs, state).filter(|_| !recs.is_empty()) else { return };
        let keys: Vec<Vec<KeyId>> = may.iter().filter_map(|k| match intern::key_get(k) {
            intern::Key::Tuple(parts) if parts.len() == t.elts.len() => Some(parts.clone()),
            _ => None,
        }).collect();
        for (i, e) in t.elts.iter().enumerate() {
            let ast::Expr::Name(n) = e else { continue };
            let v = self.load_name(s(n.id.as_str()), &Node::none(), state);
            let lits = match &v.kv {
                KeyVal::Lit(k) => KeySet::from_vec(vec![*k]),
                KeyVal::Lits(ks) => ks.clone(),
                _ => continue,
            };
            let keep = lits.filter(|c| keys.iter().any(|k| k[i] == c));
            self.keep_lits(s(n.id.as_str()), &keep, state);
        }
    }

    /// Narrow the variable a key expression is built from to the alternatives whose key the
    /// record may have (`objects_stay`: only its literals that cannot be the key are dropped).
    fn narrow_key_expr(&mut self, left: &'static ast::Expr, lv: &Value, lits: &KeySet, may: &KeySet, objects_stay: bool, state: &mut State) {
        if let ast::Expr::Name(l) = left {
            let keep = lits.filter(|k| may.contains(k));
            if objects_stay {
                self.keep_lits(s(l.id.as_str()), &keep, state);
            } else {
                self.narrow_kv(s(l.id.as_str()), &keep, true, state);
            }
            return;
        }
        let _ = lv;
        if contains_call(left) {
            return;
        }
        let mut names = names_in(left);
        names.sort();
        names.dedup();
        let mut cands: Vec<String> = Vec::new();
        for n in &names {
            let v = self.load_name(s(n), &Node::none(), state);
            if matches!(v.kv, KeyVal::Lits(_)) || (objects_stay && matches!(v.kv, KeyVal::Lit(_))) {
                cands.push(n.clone());
            }
        }
        if cands.len() != 1 {
            return;
        }
        let name = s(&cands[0]);
        let cur = self.load_name(name, &Node::none(), state);
        let cur_keys = match &cur.kv {
            KeyVal::Lits(ks) => ks.clone(),
            KeyVal::Lit(k) => KeySet::from_vec(vec![*k]),
            _ => return,
        };
        if cur_keys.len() > 64 {
            return;
        }
        let mut keep: Vec<KeyId> = Vec::new();
        for c in intern::sorted_by_str(cur_keys.iter()) {
            let mut st = state.copy();
            // (the literal alone: what the key is when the name is it)
            let alone = if objects_stay { crate::domain::ObjSet::default() } else { cur.pts.clone() };
            self.rebind_name(name, Value::with_kv(alone, KeyVal::Lit(c)), &mut st);
            let v = self.eval(left, &mut st);
            match &v.kv {
                KeyVal::Lit(k) => {
                    if may.contains(*k) {
                        keep.push(c);
                    }
                }
                KeyVal::Lits(ks) => {
                    if ks.intersects(may) {
                        keep.push(c);
                    }
                }
                _ => return,
            }
        }
        if objects_stay {
            self.keep_lits(name, &KeySet::from_vec(keep), state);
        } else {
            self.narrow_kv(name, &KeySet::from_vec(keep), true, state);
        }
    }

    /// `name`'s literals only those of `keep`; its objects stay (none, and no literal left: the
    /// path cannot be).
    fn keep_lits(&mut self, name: StrId, keep: &KeySet, state: &mut State) {
        let cur = self.load_name(name, &Node::none(), state);
        let kv = if keep.is_empty() { None } else { kv_restrict(&cur.kv, keep) };
        match kv {
            Some(kv) if kv != cur.kv => self.rebind_name(name, Value::with_kv(cur.pts.clone(), kv), state),
            Some(_) => {}
            None if cur.pts.is_empty() => state.reachable = false,
            None => self.rebind_name(name, Value::objs(cur.pts.clone()), state),
        }
    }

    pub(in crate::interp) fn narrow_kv(&mut self, name: StrId, keys: &KeySet, restrict: bool, state: &mut State) {
        if keys.is_empty() {
            return;
        }
        let cur = self.load_name(name, &Node::none(), state);
        let new = if restrict { kv_restrict(&cur.kv, keys) } else { kv_exclude(&cur.kv, keys) };
        // objects that may equal a literal (an unknown value, a placeholder, an instance with
        // its own __eq__)
        let maybe_eq = cur.pts.filter(|o| matches!(self.heap.kind(o), Kind::Unknown | Kind::Instance | Kind::External));
        match new {
            None if !maybe_eq.is_empty() => {
                // none of the literals is left: the objects are what remains (one of the keys)
                let v = if restrict {
                    Value::with_kv(maybe_eq.filter(|o| self.heap.kind(o) == Kind::Instance), KeyVal::lits(keys.clone()))
                } else {
                    Value::objs(cur.pts.clone())
                };
                self.rebind_name(name, v, state);
            }
            None => {
                if restrict || cur.pts.is_empty() {
                    // (a dict / list never equals a literal)
                    if cur.kv.is_literal() {
                        state.reachable = false;
                    }
                } else if cur.kv.is_literal() {
                    // every literal excluded: the objects remain
                    self.rebind_name(name, Value::objs(cur.pts.clone()), state);
                }
            }
            Some(nk) => {
                // equal to one of the literals: an unknown value is that literal now
                let pts = if restrict { cur.pts.filter(|o| self.heap.kind(o) != Kind::Unknown) } else { cur.pts.clone() };
                if nk != cur.kv || pts != cur.pts {
                    self.rebind_name(name, Value::with_kv(pts, nk), state);
                }
            }
        }
    }

    pub(in crate::interp) fn narrow_record(&mut self, rec: &Value, ns: Ns, k: KeyId, member: bool, state: &mut State) {
        let targets: Vec<u32> = rec.pts.iter().filter(|o| self.heap.is_record(*o)).collect();
        let single = targets.len() == 1;
        for oid in targets {
            let row = state.row(oid, ns);
            if member {
                if single && row.closed() && !row.present.contains(k) {
                    // `k in d` on a closed record without k: this path is infeasible
                    state.reachable = false;
                    continue;
                }
                let strong = single && self.heap.obj(oid).single;
                // (an open record's key: its value is what the input or unknown code put there,
                // kept as a slot so a store on another path joins with it)
                if strong && row.open() && !state.has_slot(oid, ns, k) && ns == Ns::Item {
                    let v = self.path_placeholder_value(oid, ns, k, &row, state);
                    state.set_slot_found(oid, ns, k, v, false);
                }
                state.set_row_narrowed(oid, ns, row.narrow_in(k, strong));
                if strong {
                    self.propagate_narrowing(oid, ns, k, state);
                }
            } else if single {
                if row.definite.contains(k) {
                    state.reachable = false;
                }
                state.set_row_narrowed(oid, ns, row.delete(k, true));
            }
        }
    }

    /// A key proven on a view of a record is proven on the record it was derived from.
    pub(in crate::interp) fn propagate_narrowing(&mut self, oid: u32, ns: Ns, k: KeyId, state: &mut State) {
        let mut src = self.heap.obj(oid).derived_from;
        let mut seen: Vec<u32> = Vec::new();
        while let Some(sid) = src {
            if seen.contains(&sid) {
                break;
            }
            seen.push(sid);
            if self.heap.obj(sid).single && state.has_row(sid, ns) {
                let r = state.row(sid, ns).narrow_in(k, true);
                state.set_row_narrowed(sid, ns, r);
            }
            src = self.heap.obj(sid).derived_from;
        }
    }
}

/// Classes whose isinstance checks go by structure (ABCs with subclass hooks, protocols):
/// an unrelated user class may pass them.
/// A value on the path where a truth test of it was `truth`: without its literals of the
/// other truth (None, 0, "" on the true path). None: nothing to remove.
pub fn truth_narrowed(v: &Value, truth: bool) -> Option<Value> {
    let lits = kv_literals(&v.kv);
    if !v.kv.is_literal() || lits.is_empty() {
        return None;
    }
    let keep: KeySet = lits.iter().filter(|k| crate::models::pyvalues::truthy(*k) != Some(!truth)).collect();
    (keep.len() < lits.len()).then(|| Value::with_kv(v.pts.clone(), if keep.is_empty() { KeyVal::Bot } else { KeyVal::lits(keep) }))
}

/// The record a membership test's right side stands for: `d` of `d.keys()`, `df` of
/// `df.columns` / `df.index`.
fn membership_record(right: &'static ast::Expr) -> &'static ast::Expr {
    match right {
        ast::Expr::Call(c) => match &*c.func {
            ast::Expr::Attribute(a) if a.attr.id.as_str() == "keys" => &a.value,
            _ => right,
        },
        ast::Expr::Attribute(a) if matches!(a.attr.id.as_str(), "columns" | "index") => &a.value,
        _ => right,
    }
}

pub fn is_structural_qual(q: &str) -> bool {
    q.starts_with("collections.abc") || q.starts_with("typing") || q.starts_with("abc.") || q.starts_with("numbers") || q.starts_with("os.PathLike")
}

/// The keys closed records may have, all of them (None: one is open).
fn closed_keys(recs: &[u32], state: &State) -> Option<KeySet> {
    let rows: Vec<crate::domain::Row> = recs.iter().map(|o| state.row(*o, Ns::Item)).collect();
    rows.iter().all(|r| r.closed()).then(|| rows.iter().fold(KeySet::new(), |m, r| m.union(&r.present)))
}
