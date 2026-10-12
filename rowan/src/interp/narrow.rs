//! Narrowing (guards): port of `narrow` and its helpers.

use indexmap::IndexMap;
use ruff_python_ast as ast;

use crate::domain::{kv_exclude, kv_literals, kv_restrict, KeyVal, Ns, ObjSet, Value};
use crate::frontend::Node;
use crate::intern::{self, key_str_lit, s, KeyId, KeySet, StrId};
use crate::state::{Kind, State};

use super::{chain_text, contains_call, names_in, Analyzer, StoreTarget};

enum NarrowTarget {
    Name(String),
    Attr(&'static ast::ExprAttribute, &'static ast::Expr),
}

impl Analyzer {
    pub fn narrow(&mut self, test: &'static ast::Expr, truth: bool, state: &mut State) {
        self.narrow_depth += 1;
        self.narrow_impl(test, truth, state);
        self.narrow_depth -= 1;
    }

    fn narrow_impl(&mut self, test: &'static ast::Expr, truth: bool, state: &mut State) {
        if !state.reachable {
            return;
        }
        if self.torch_on {
            self.t_narrow(test, truth, state);
            if !state.reachable {
                return;
            }
        }
        match test {
            ast::Expr::UnaryOp(u) if u.op == ast::UnaryOp::Not => {
                self.narrow(&u.operand, !truth, state);
            }
            ast::Expr::BoolOp(b) => {
                if (b.op == ast::BoolOp::And && truth) || (b.op == ast::BoolOp::Or && !truth) {
                    for v in &b.values {
                        self.narrow(v, truth, state);
                    }
                }
            }
            ast::Expr::Named(n) => self.narrow(&n.target, truth, state),
            ast::Expr::Compare(c) if c.ops.len() == 1 => {
                let op = c.ops[0];
                let left: &'static ast::Expr = &c.left;
                let right: &'static ast::Expr = &c.comparators[0];
                match op {
                    ast::CmpOp::In | ast::CmpOp::NotIn => {
                        let member = if op == ast::CmpOp::In { truth } else { !truth };
                        self.narrow_membership(left, right, member, state);
                    }
                    ast::CmpOp::Eq | ast::CmpOp::NotEq => {
                        if let ast::Expr::Name(l) = left {
                            let eq = if op == ast::CmpOp::Eq { truth } else { !truth };
                            let rv = self.eval(right, state);
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
                            let rv = self.eval(right, state);
                            let ks = kv_literals(&rv.kv);
                            let ov = self.eval(&la.value, state);
                            let ak = key_str_lit(la.attr.id.as_str());
                            if let Some(o) = ov.single().filter(|o| self.obj(*o).single && matches!(self.kind(*o), Kind::Instance | Kind::Namespace) && rv.pts.is_empty() && ks.len() == 1) {
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
                    ast::CmpOp::Is | ast::CmpOp::IsNot if !matches!(right, ast::Expr::NoneLiteral(_)) => {
                        // `t is a` with `a` one object: on the true side t is a, on the false side
                        // a is not among t's objects
                        let same = if op == ast::CmpOp::Is { truth } else { !truth };
                        for (name_e, other_e) in [(left, right), (right, left)] {
                            let ast::Expr::Name(nm) = name_e else { continue };
                            let name = s(nm.id.as_str());
                            let cur = self.load_name(name, &Node::none(), state);
                            let o = self.eval(other_e, &mut state.copy());
                            let Some(target) = o.single().filter(|x| o.kv.is_bot() && self.obj(*x).single && !crate::state::is_merged(*x)) else { continue };
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
                                let has_none = kv_literals(&cur.kv).contains(intern::key_none());
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
                                    let lv = self.eval(left, state);
                                    let ks = self.lit_keys(&lv, state);
                                    if let Some(ks) = ks {
                                        if truth {
                                            let rec = self.eval(&ra.value, state);
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
            ast::Expr::Call(call) => {
                let args = &call.arguments.args;
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
                        let kv = self.eval(&args[1], state);
                        if let Some(k) = kv.kv.as_lit() {
                            let mut rec = self.eval(&args[0], state);
                            // (an attribute row describes instances and namespaces; a builtin
                            // container's attributes are its type's: decided by the type, or not)
                            let builtin = |an: &Analyzer, o: u32| matches!(an.kind(o), Kind::Dict | Kind::List | Kind::Tuple | Kind::Set);
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
                            rec = Value::with_kv(rec.pts.filter(|o| !builtin(self, o) && !by_class(self, o)), KeyVal::Bot);
                            self.narrow_record(&rec, Ns::Attr, k, truth, state);
                        }
                        return;
                    }
                    if fname == "all" && args.len() == 1 {
                        if let ast::Expr::Generator(g) = &args[0] {
                            if truth && g.generators.len() == 1 {
                                if let ast::Expr::Compare(cmp) = &*g.elt {
                                    if cmp.ops.first() == Some(&ast::CmpOp::In) {
                                        let itv = self.eval(&g.generators[0].iter, state);
                                        let ks = self.lit_keys(&itv, state);
                                        if let Some(ks) = ks {
                                            if matches!(&*cmp.left, ast::Expr::Name(_)) {
                                                let rec = self.eval(&cmp.comparators[0], state);
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
                        let v = self.eval(&fa.value, state);
                        let ks = self.lit_keys(&v, state);
                        if let Some(ks) = ks {
                            let rec = self.eval(&args[0], state);
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
            // `if items:` / `if not items:` on a list of unknown length: on this path it has a
            // first element / is empty (this path's length only, not the object's)
            ast::Expr::Name(n) => {
                let mut v = self.load_name(s(n.id.as_str()), &Node::none(), state);
                // several containers: on the truthy path the ones known to be empty are not it
                if truth && v.pts.len() > 1 && v.kv.is_bot() {
                    let keep: Vec<u32> = v.pts.iter().filter(|o| !(matches!(self.kind(*o), Kind::List | Kind::Tuple | Kind::Set) && self.llen(*o, state) == Some(0))).collect();
                    if !keep.is_empty() && keep.len() < v.pts.len() {
                        v = Value::objs(crate::domain::ObjSet::from_vec(keep));
                        self.rebind_name(s(n.id.as_str()), v.clone(), state);
                    }
                }
                if v.pts.len() == 1 && v.kv.is_bot() {
                    let o = v.pts.first().unwrap();
                    if matches!(self.kind(o), Kind::List | Kind::Tuple) && self.obj(o).single && self.llen(o, state).is_none() {
                        if truth {
                            if self.prefix(o, state).is_none() {
                                let e = state.slot(o, Ns::Elem, intern::STAR);
                                let ekv = state.elems.get(&o).unwrap_or(crate::domain::DATA);
                                state.set_slot(o, Ns::Pos, intern::key_int(0), Value::with_kv(e.pts, ekv), true);
                                state.lens.insert(o, (None, 1, 0));
                            }
                        } else if self.prefix(o, state).is_none() {
                            state.lens.insert(o, (Some(0), 0, 0));
                        }
                    }
                    // a falsy dict is empty on this path (`while queue:` ends with no keys)
                    if !truth && self.kind(o) == Kind::Dict && self.obj(o).single {
                        state.set_row(o, Ns::Item, crate::domain::Row::new());
                        state.drop_obj_ns(o, Ns::Item);
                    }
                    // a falsy unknown value (a JSON document, an input): an empty container if
                    // len() or a key read accepts it at all (a falsy number makes them raise)
                    if !truth && self.kind(o) == Kind::Unknown && self.obj(o).single && v.kv.is_bot() {
                        state.set_row(o, Ns::Item, crate::domain::Row::new());
                        state.drop_obj_ns(o, Ns::Item);
                        state.drop_obj_ns(o, Ns::Pos);
                        state.drop_obj_ns(o, Ns::Elem);
                        state.lens.insert(o, (Some(0), 0, 0));
                    }
                }
            }
            _ => {}
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
                let base = self.eval(&a.value, state);
                let recs: Vec<u32> = base.pts.iter().filter(|o| matches!(self.kind(*o), Kind::Instance | Kind::Namespace)).collect();
                if recs.len() != 1 || base.pts.len() != 1 || !self.strongable(recs[0]) {
                    let Some(key) = chain_text(whole) else { return };
                    if !truth {
                        return;
                    }
                    let cur = self.eval(whole, state);
                    if cur.pts.is_empty() {
                        return;
                    }
                    self.narrow_isinstance_value(&cur, cls_expr, truth, state, StoreTarget::Narrowed(s(&key)));
                    return;
                }
                let bo = recs[0];
                let attr = key_str_lit(a.attr.id.as_str());
                let cur = state.slot(bo, Ns::Attr, attr);
                (cur, StoreTarget::Slot(bo, Ns::Attr, attr))
            }
        };
        if cur.pts.is_empty() {
            return;
        }
        self.narrow_isinstance_value(&cur, cls_expr, truth, state, store);
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

    fn kinds_for_external(q: &str, strict: bool) -> Option<Vec<Kind>> {
        Some(match q {
            "dict" | "Mapping" | "MutableMapping" => vec![Kind::Dict, Kind::Unknown],
            "list" => vec![Kind::List],
            "tuple" => vec![Kind::Tuple, Kind::Pair],
            "set" | "frozenset" => vec![Kind::Set],
            "str" | "int" | "float" | "bool" | "bytes" => vec![],
            "DataFrame" => vec![Kind::Frame],
            "Series" if !strict => vec![Kind::Series],
            "Sequence" if !strict => vec![Kind::List, Kind::Tuple],
            "Iterable" if !strict => vec![Kind::List, Kind::Tuple, Kind::Set, Kind::Dict, Kind::Unknown, Kind::View],
            _ => {
                if strict {
                    if q.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                        return None;
                    }
                    vec![]
                } else {
                    return None;
                }
            }
        })
    }

    pub fn narrow_isinstance_value(&mut self, cur: &Value, cls_expr: &'static ast::Expr, truth: bool, state: &mut State, store: StoreTarget) {
        let cv = self.eval(cls_expr, state);
        let mut classes: Vec<u32> = Vec::new();
        let mut kinds: Vec<Kind> = Vec::new();
        let mut star = false;
        // an external class we know nothing about: still never a base of a user class whose
        // whole hierarchy is known (unless it is an ABC / protocol checked by structure)
        let mut star_structural = false;
        // (builtin classes tested: a placeholder narrowed by one of them alone is a placeholder
        // of it, so what the branch reads replays only on its instances)
        let mut ext_classes: Vec<u32> = Vec::new();
        // (some builtin / library class is tested, alone or in a tuple)
        let mut ext_tested = false;
        for c in cv.pts.iter() {
            match self.kind(c) {
                Kind::Class => classes.push(c),
                Kind::External => {
                    ext_classes.push(c);
                    ext_tested = true;
                    let q = self.external_qual(c);
                    let q = q.rsplit('.').next().unwrap_or("").to_string();
                    let full = self.external_qual(c);
                    match Self::kinds_for_external(&q, false) {
                        Some(ks) => kinds.extend(ks),
                        None => {
                            star = true;
                            star_structural |= is_structural_qual(&full);
                        }
                    }
                }
                Kind::Tuple | Kind::List => {
                    let n = self.llen(c, state).unwrap_or(0);
                    for i in 0..n {
                        let sub = state.slot(c, Ns::Pos, intern::key_int(i as i64));
                        for c2 in sub.pts.iter() {
                            match self.kind(c2) {
                                Kind::Class => classes.push(c2),
                                Kind::External => {
                                    ext_tested = true;
                                    let q = self.external_qual(c2);
                                    let q = q.rsplit('.').next().unwrap_or("").to_string();
                                    let full = self.external_qual(c2);
                                    match Self::kinds_for_external(&q, true) {
                                        Some(ks) => kinds.extend(ks),
                                        None => {
                                            star = true;
                                            star_structural |= is_structural_qual(&full);
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                _ => return,
            }
        }
        if star && (star_structural) {
            return;
        }
        let mut keep: Vec<u32> = Vec::new();
        for oid in cur.pts.iter() {
            let m: Option<bool> = match self.kind(oid) {
                Kind::Instance => match self.instance_class(oid) {
                    None => None,
                    Some(pc) if classes.iter().any(|c| self.is_subclass(pc, *c)) || classes.contains(&pc) => Some(true),
                    Some(pc) if star && !self.hierarchy_closed(pc) => None,
                    // (a class deriving from a builtin / library class — `class C(dict)` — is an
                    // instance of it: undecided against such classes)
                    Some(pc) if ext_tested && self.mro(pc).iter().any(|c| self.class_info_opt(*c).is_some_and(|ci| ci.open_base || ci.dict_base)) => None,
                    Some(_) => Some(false),
                },
                _ if star => None,
                Kind::Unknown => None,
                k => Some(kinds.contains(&k)),
            };
            if m.is_none() || m == Some(truth) {
                keep.push(oid);
            }
        }
        let new_pts = ObjSet::from_vec(keep);
        if new_pts == cur.pts {
            return;
        }
        if new_pts.is_empty() && (truth || cur.kv.is_bot()) {
            if cur.kv.is_bot() {
                state.reachable = false;
                return;
            }
        }
        self.store_narrowed(&store, Value::with_kv(new_pts, cur.kv.clone()), state);
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
            let Some(ci) = self.class_info_opt(c) else { return false };
            if ci.open_base {
                return false;
            }
            work.extend(ci.bases.iter().copied());
        }
        true
    }

    pub fn is_subclass(&self, cls: u32, base: u32) -> bool {
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
            if let Some(ci) = self.class_info_opt(c) {
                order.extend(ci.bases.iter().copied());
            }
        }
        false
    }

    fn narrow_get_truthy(&mut self, expr: &'static ast::Expr, state: &mut State) {
        if let ast::Expr::Call(c) = expr {
            if let ast::Expr::Attribute(a) = &*c.func {
                if a.attr.id.as_str() == "get" && !c.arguments.args.is_empty() {
                    let kv = self.eval(&c.arguments.args[0], state);
                    if let Some(k) = kv.kv.as_lit() {
                        let rec = self.eval(&a.value, state);
                        // only a dict-like receiver: a user instance's `get` is its own method
                        if rec.pts.iter().any(|o| matches!(self.kind(o), Kind::Instance | Kind::Namespace)) {
                            return;
                        }
                        // a name holding several dicts: those that surely lack the key are not it
                        if let ast::Expr::Name(n) = &*a.value {
                            if rec.pts.len() > 1 {
                                let keep: Vec<u32> = rec.pts.iter().filter(|o| !(self.kind(*o) == Kind::Dict && { let r = state.row(*o, Ns::Item); !r.open() && !r.present.contains(k) })).collect();
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

    fn narrow_membership(&mut self, left: &'static ast::Expr, right: &'static ast::Expr, member: bool, state: &mut State) {
        let mut rec_expr: &'static ast::Expr = right;
        if let ast::Expr::Call(c) = right {
            if let ast::Expr::Attribute(a) = &*c.func {
                if a.attr.id.as_str() == "keys" {
                    rec_expr = &a.value;
                }
            }
        } else if let ast::Expr::Attribute(a) = right {
            if matches!(a.attr.id.as_str(), "columns" | "index") {
                rec_expr = &a.value;
            }
        }
        let rv = self.eval(rec_expr, state);
        let lv = self.eval(left, state);
        // user classes defining `__contains__` as `return <x> in <y>`: narrow <y> instead
        let insts: Vec<u32> = rv.pts.iter().filter(|o| self.kind(*o) == Kind::Instance).collect();
        if !insts.is_empty() && rv.pts.len() == 1 {
            if let Some(cls) = self.instance_class(insts[0]) {
                if let Some(m) = self.class_lookup(cls, "__contains__") {
                    let fv = self.cls_slots.get(&(m, Ns::Attr, key_str_lit("__contains__"))).cloned().unwrap_or_default();
                    for f in fv.pts.iter() {
                        if self.kind(f) != Kind::Function {
                            continue;
                        }
                        let fi = self.func_info(f);
                        let body = fi.node.body();
                        if body.len() == 1 {
                            if let ast::Stmt::Return(r) = &body[0] {
                                if let Some(ast::Expr::Compare(cmp)) = r.value.as_deref() {
                                    if cmp.ops.len() == 1 && cmp.ops[0] == ast::CmpOp::In {
                                        let cl: &'static ast::Expr = &cmp.left;
                                        let cr: &'static ast::Expr = &cmp.comparators[0];
                                        let lv2 = lv.clone();
                                        self.in_method_frame(f, Value::obj(insts[0]), vec![lv2], state, &mut |an: &mut Analyzer, st: &mut State| {
                                            an.narrow_membership(cl, cr, member, st);
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
        let recs: Vec<u32> = rv.pts.iter().filter(|o| self.is_record_like(*o)).collect();
        // (a value that is a literal or an unknown object may be any key: no narrowing by its
        // literal part)
        let lit_only = lv.pts.iter().all(|o| !matches!(self.kind(o), Kind::Unknown | Kind::Instance | Kind::External));
        if !recs.is_empty() {
            match &lv.kv {
                KeyVal::Lit(_) | KeyVal::Lits(_) if !lit_only => {}
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
                KeyVal::KeysOf(_) | KeyVal::Data(_) | KeyVal::Bot if member && (lv.pts.is_empty() || lv.pts.iter().all(|o| self.kind(o) == Kind::Unknown)) && !(lv.pts.is_empty() && lv.kv.is_bot()) => {
                    let rows: Vec<crate::domain::Row> = recs.iter().map(|o| state.row(*o, Ns::Item)).collect();
                    if rows.iter().all(|r| r.closed()) {
                        let mut may = KeySet::new();
                        for r in &rows {
                            may = may.union(&r.present);
                        }
                        if !may.is_empty() && may.iter().all(intern::key_is_str) {
                            self.narrow_key_expr(left, &lv, &may, &may, state);
                        } else if may.is_empty() {
                            state.reachable = false;
                        }
                    }
                }
                KeyVal::Lits(ks) if member => {
                    let rows: Vec<crate::domain::Row> = recs.iter().map(|o| state.row(*o, Ns::Item)).collect();
                    if rows.iter().all(|r| r.closed()) {
                        let mut may = KeySet::new();
                        for r in &rows {
                            may = may.union(&r.present);
                        }
                        let ks = ks.clone();
                        self.narrow_key_expr(left, &lv, &ks, &may, state);
                    } else {
                    }
                }
                _ => {}
            }
            return;
        }
        // (a literal right side is a string: `in` tests substrings, not keys)
        if let (ast::Expr::Name(l), false) = (left, !rv.kv.is_bot() || rv.pts.is_empty()) {
            let ks = self.lit_keys(&rv, state);
            if let Some(ks) = ks {
                self.narrow_kv(s(l.id.as_str()), &ks, member, state);
            }
        }
    }

    /// Narrow the variable a key expression is built from to the alternatives whose key the
    /// record may have.
    fn narrow_key_expr(&mut self, left: &'static ast::Expr, lv: &Value, lits: &KeySet, may: &KeySet, state: &mut State) {
        if let ast::Expr::Name(l) = left {
            let keep = lits.filter(|k| may.contains(k));
            self.narrow_kv(s(l.id.as_str()), &keep, true, state);
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
            if matches!(v.kv, KeyVal::Lits(_)) {
                cands.push(n.clone());
            }
        }
        if cands.len() != 1 {
            return;
        }
        let name = s(&cands[0]);
        let cur = self.load_name(name, &Node::none(), state);
        let KeyVal::Lits(cur_keys) = &cur.kv else { return };
        if cur_keys.len() > 64 {
            return;
        }
        let mut keep: Vec<KeyId> = Vec::new();
        for c in intern::sorted_by_str(cur_keys.iter()) {
            let mut ov = IndexMap::new();
            ov.insert(name, Value::with_kv(cur.pts.clone(), KeyVal::Lit(c)));
            self.frame_mut().overlays.push(ov);
            let v = self.eval(left, state);
            self.frame_mut().overlays.pop();
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
        self.narrow_kv(name, &KeySet::from_vec(keep), true, state);
    }

    pub fn narrow_kv(&mut self, name: StrId, keys: &KeySet, restrict: bool, state: &mut State) {
        if keys.is_empty() {
            return;
        }
        let cur = self.load_name(name, &Node::none(), state);
        let new = if restrict { kv_restrict(&cur.kv, keys) } else { kv_exclude(&cur.kv, keys) };
        // objects that may equal a literal (an unknown value, a placeholder, an instance with
        // its own __eq__)
        let maybe_eq = cur.pts.filter(|o| matches!(self.kind(o), Kind::Unknown | Kind::Instance | Kind::External));
        match new {
            None if !maybe_eq.is_empty() => {
                // none of the literals is left: the objects are what remains (one of the keys)
                let v = if restrict {
                    Value::with_kv(maybe_eq.filter(|o| self.kind(o) == Kind::Instance), KeyVal::lits(keys.clone()))
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
                let pts = if restrict { cur.pts.filter(|o| self.kind(o) != Kind::Unknown) } else { cur.pts.clone() };
                if nk != cur.kv || pts != cur.pts {
                    self.rebind_name(name, Value::with_kv(pts, nk), state);
                }
            }
        }
    }

    pub fn narrow_record(&mut self, rec: &Value, ns: Ns, k: KeyId, member: bool, state: &mut State) {
        let targets: Vec<u32> = rec.pts.iter().filter(|o| self.is_record_like(*o)).collect();
        let single = targets.len() == 1;
        for oid in targets {
            let row = state.row(oid, ns);
            if member {
                if single && row.closed() && !row.present.contains(k) {
                    // `k in d` on a closed record without k: this path is infeasible
                    state.reachable = false;
                    continue;
                }
                let strong = single && self.strongable(oid);
                state.set_row(oid, ns, row.narrow_in(k, strong));
                if strong {
                    self.propagate_narrowing(oid, ns, k, state);
                }
            } else if single {
                if row.definite.contains(k) {
                    state.reachable = false;
                }
                state.set_row(oid, ns, row.narrow_out(k));
            }
        }
    }

    pub fn strongable(&self, oid: u32) -> bool {
        let o = self.obj(oid);
        o.single || o.iter_elem
    }

    /// A key proven on a view of a record is proven on the record it was derived from.
    pub fn propagate_narrowing(&mut self, oid: u32, ns: Ns, k: KeyId, state: &mut State) {
        let mut src = self.obj(oid).derived_from;
        let mut seen: Vec<u32> = Vec::new();
        while let Some(sid) = src {
            if seen.contains(&sid) {
                break;
            }
            seen.push(sid);
            if self.obj(sid).single && state.has_row(sid, ns) {
                let r = state.row(sid, ns).narrow_in(k, true);
                state.set_row(sid, ns, r);
            }
            src = self.obj(sid).derived_from;
        }
    }
}

/// Classes whose isinstance checks go by structure (ABCs with subclass hooks, protocols):
/// an unrelated user class may pass them.
fn is_structural_qual(q: &str) -> bool {
    q.starts_with("collections.abc") || q.starts_with("typing") || q.starts_with("abc.") || q.starts_with("numbers") || q.starts_with("os.PathLike")
}
