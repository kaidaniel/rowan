//! Narrowing (guards, §8.4): port of `narrow` and its helpers.

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
                            if !ks.is_empty() {
                                self.narrow_kv(s(l.id.as_str()), &ks, eq, state);
                            }
                        }
                    }
                    ast::CmpOp::Is | ast::CmpOp::IsNot => {
                        if matches!(right, ast::Expr::NoneLiteral(_)) {
                            let is_none = if op == ast::CmpOp::Is { truth } else { !truth };
                            if !is_none {
                                self.narrow_get_truthy(left, state);
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
                            let rec = self.eval(&args[0], state);
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
                let recs: Vec<u32> = base.pts.iter().filter(|o| matches!(self.kind(*o), Kind::Instance | Kind::Ph | Kind::Namespace)).collect();
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
                if self.kind(bo) == Kind::Ph && truth && !self.sum_stack.is_empty() {
                    let cv0 = self.eval(cls_expr, state);
                    let cl: Vec<u32> = cv0.pts.iter().filter(|c| self.kind(*c) == Kind::Class).collect();
                    if cl.len() == 1 && cv0.pts.len() == 1 {
                        let pid = self.path_placeholder(bo, Ns::Attr, attr, state, Some(cl[0]), &format!(":is{}", cl[0]));
                        state.set_slot(bo, Ns::Attr, attr, Value::with_kv(ObjSet::one(pid), KeyVal::Param(pid)), true);
                    }
                    return;
                }
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
            StoreTarget::Name(n) => self.store_name(*n, v, state),
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
        let in_summary = !self.sum_stack.is_empty();
        let mut narrowed: Vec<(u32, u32)> = Vec::new();
        for c in cv.pts.iter() {
            match self.kind(c) {
                Kind::Class => classes.push(c),
                Kind::External => {
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
        if star && (star_structural || crate::interp::disabled("extinst")) {
            return;
        }
        let mut keep: Vec<u32> = Vec::new();
        for oid in cur.pts.iter() {
            let m: Option<bool> = match self.kind(oid) {
                Kind::Instance => match self.instance_class(oid) {
                    None => None,
                    Some(pc) if classes.iter().any(|c| self.is_subclass(pc, *c)) || classes.contains(&pc) => Some(true),
                    Some(pc) if star && !self.hierarchy_closed(pc) => None,
                    Some(_) => Some(false),
                },
                _ if star => None,
                Kind::Unknown => None,
                Kind::Ph => {
                    self.record_event("isinstance", oid);
                    let ph = self.ph_info(oid);
                    if let (Some(pc), false) = (ph.cls.get(), classes.is_empty()) {
                        Some(classes.iter().any(|c| self.is_subclass(pc, *c)))
                    } else {
                        if truth && classes.len() == 1 && kinds.is_empty() && in_summary {
                            narrowed.push((oid, classes[0]));
                        }
                        None
                    }
                }
                k => Some(kinds.contains(&k)),
            };
            if m.is_none() || m == Some(truth) {
                keep.push(oid);
            }
        }
        if !narrowed.is_empty() {
            let mut repl: IndexMap<u32, u32> = IndexMap::new();
            for (ph, c) in &narrowed {
                let np = self.narrowed_placeholder(*ph, *c, state);
                repl.insert(*ph, np);
            }
            let keep: Vec<u32> = keep.into_iter().map(|o| *repl.get(&o).unwrap_or(&o)).collect();
            let mut kv = cur.kv.clone();
            if let KeyVal::Param(p) = &kv {
                if let Some(r) = repl.get(p) {
                    kv = KeyVal::Param(*r);
                }
            }
            self.store_narrowed(&store, Value::with_kv(ObjSet::from_vec(keep), kv), state);
            return;
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
                            if rec.pts.len() > 1 && !crate::interp::disabled("getfilter") {
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
        if !recs.is_empty() {
            match &lv.kv {
                KeyVal::Lit(k) => {
                    // whichever object the name holds has the key (several objects: none narrows)
                    if member && recs.len() > 1 && !crate::interp::disabled("innamekey") {
                        if let ast::Expr::Name(nm) = rec_expr {
                            state.add_name_key(s(nm.id.as_str()), *k);
                        }
                    }
                    self.narrow_record(&rv, Ns::Item, *k, member, state)
                }
                KeyVal::Param(ph) => {
                    // `k in d` with k a parameter: decided per call site when k is a literal there
                    self.record_event("narrow", *ph);
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
                        for (o, r) in recs.iter().zip(rows.iter()) {
                            if r.symbolic() {
                                self.record_event("narrow", *o);
                            }
                        }
                    }
                }
                _ => {}
            }
            return;
        }
        if let ast::Expr::Name(l) = left {
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
        if let Some(ph) = cur.kv.as_param() {
            self.record_event("narrow", ph);
            return;
        }
        let new = if restrict { kv_restrict(&cur.kv, keys) } else { kv_exclude(&cur.kv, keys) };
        match new {
            None => {
                if cur.kv.is_literal() {
                    state.reachable = false;
                }
            }
            Some(nk) => {
                // equal to one of the literals: an unknown value is that literal now
                let pts = if restrict && !crate::interp::disabled("restrictpts") { cur.pts.filter(|o| self.kind(o) != Kind::Unknown) } else { cur.pts.clone() };
                if nk != cur.kv || pts != cur.pts {
                    self.store_name(name, Value::with_kv(pts, nk), state);
                }
            }
        }
    }

    pub fn narrow_record(&mut self, rec: &Value, ns: Ns, k: KeyId, member: bool, state: &mut State) {
        let targets: Vec<u32> = rec.pts.iter().filter(|o| self.is_record_like(*o)).collect();
        let single = targets.len() == 1;
        for oid in targets {
            if self.kind(oid) == Kind::Ph {
                self.record_event("narrow", oid);
            }
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
