//! Statements (port of the `stmt_*` methods of `rowan/interp.py`).

use ruff_python_ast as ast;

use crate::domain::{KeyVal, Ns, Row, Value};
use crate::frontend::{self, Node};
use crate::intern::{self, key_str_lit, s};
use crate::state::{Kind, State};

use super::{Finally, Handler, contains_jump, is_constant, Analyzer, ClassInfo, Frame, FrameKind, FuncNode, LoopCtx, Payload, LOOP_MAX_ITER, UNROLL_MAX};
use super::funcs::Elem;

impl Analyzer {

    pub fn exec_block(&mut self, stmts: &'static [ast::Stmt], state: &mut State) {
        for st in stmts {
            if !state.reachable {
                return;
            }
            self.exec_stmt(st, state);
            self.after_stmt(st, state);
        }
    }

    pub(in crate::interp) fn exec_stmt(&mut self, node: &'static ast::Stmt, state: &mut State) {
        self.budget.charge(self.cx.frames.last().map(|f| f.code).unwrap_or(0));
        if self.cut_body(state) {
            return;
        }
        let dbg2 = crate::interp::debug_level() == 2;
        let was = state.reachable;
        self.exec_stmt_inner(node, state);
        if dbg2 && was && !state.reachable && !matches!(node, ast::Stmt::Return(_) | ast::Stmt::Raise(_) | ast::Stmt::Break(_) | ast::Stmt::Continue(_)) {
            let n = frontend::node(node);
            eprintln!("  unreachable after M{}:L{} in {}", n.module, n.line, self.cx.frames.last().map(|f| f.label.clone()).unwrap_or_default());
        }
    }

    fn exec_stmt_inner(&mut self, node: &'static ast::Stmt, state: &mut State) {
        match node {
            ast::Stmt::Expr(e) => {
                self.eval(&e.value, state);
            }
            ast::Stmt::Pass(_) | ast::Stmt::Global(_) | ast::Stmt::Nonlocal(_) | ast::Stmt::TypeAlias(_) => {}
            ast::Stmt::Assign(a) => {
                let v = self.eval(&a.value, state);
                for t in &a.targets {
                    self.bind_target(t, &v, state);
                }
            }
            ast::Stmt::AnnAssign(a) => {
                if let Some(value) = &a.value {
                    let v = self.eval(value, state);
                    self.bind_target(&a.target, &v, state);
                }
            }
            ast::Stmt::AugAssign(a) => self.stmt_augassign(a, state),
            ast::Stmt::Return(r) => self.stmt_return(r, state),
            ast::Stmt::Raise(r) => self.stmt_raise(r, state),
            ast::Stmt::Assert(a) => {
                let sp = self.split(&a.test, state.copy());
                state.replace_with(sp.t);
            }
            ast::Stmt::Delete(d) => self.stmt_delete(d, state),
            ast::Stmt::Import(i) => self.stmt_import(i, state),
            ast::Stmt::ImportFrom(i) => self.stmt_import_from(i, state),
            ast::Stmt::If(i) => self.stmt_if(&i.test, &i.body, &i.elif_else_clauses, state),
            ast::Stmt::While(w) => {
                let g = self.loop_gc_begin(state);
                self.stmt_while(w, state);
                self.loop_gc_end(g, state);
            }
            ast::Stmt::For(f) => {
                let g = self.loop_gc_begin(state);
                // (a generator the loop runs: what the body changes, it may read at its yields)
                let depth = self.gens.for_iters.len();
                self.with_gen_restart(state, false, &mut |an, st| an.stmt_for(f, st));
                if let Some(used) = self.gens.for_iters.get(depth).cloned().filter(|_| !contains_jump(&f.body)) {
                    self.gen_used_up(&used, state);
                }
                self.gens.for_iters.truncate(depth);
                self.loop_gc_end(g, state);
            }
            ast::Stmt::Break(_) | ast::Stmt::Continue(_) => self.stmt_jump(node, state),
            ast::Stmt::Try(t) => self.stmt_try(t, state),
            ast::Stmt::With(w) if crate::models::csvio::with_suppress(self, w, state) => {}
            ast::Stmt::With(w) if self.with_contextmanager(w, state) => {}
            ast::Stmt::With(w) => self.stmt_with(w, state),
            ast::Stmt::FunctionDef(f) => {
                let v = self.make_function(FuncNode::Def(f), state, None);
                let v = self.apply_decorators(&f.decorator_list, frontend::node(f), v, state);
                self.store_name(s(f.name.id.as_str()), v, state);
            }
            ast::Stmt::ClassDef(c) => self.stmt_classdef(c, state),
            ast::Stmt::Match(m) => self.stmt_match(m, state),
            ast::Stmt::IpyEscapeCommand(_) => {
                let n = frontend::node(node);
                self.findings.note(format!("unsupported statement IpyEscapeCommand at L{}", n.line));
            }
        }
    }

    /// `break` / `continue`: the finally blocks of the trys it leaves run first; the state joins
    /// the loop's breaks / continues.
    fn stmt_jump(&mut self, node: &'static ast::Stmt, state: &mut State) {
        let mut st = state.copy();
        state.reachable = false;
        // (the finally blocks of the trys this leaves run first)
        let n = self.unwind.loop_stack.len();
        let pending: Vec<std::rc::Rc<Finally>> = self.unwind.finally_stack.iter().rev().take_while(|(l, _, _)| *l == n).filter(|(_, _, a)| *a).map(|(_, b, _)| b.clone()).collect();
        for fin in pending {
            if st.reachable {
                self.run_finally(&fin, &mut st, false);
            }
        }
        if let (true, Some(ctx)) = (st.reachable, self.unwind.loop_stack.last_mut()) {
            let acc = if matches!(node, ast::Stmt::Break(_)) { &mut ctx.breaks } else { &mut ctx.continues };
            *acc = Some(match acc.take() {
                None => st,
                Some(mut b) => b.join(&mut st),
            });
        }
    }

    /// `match`: each case's pattern narrows the subject on its path.
    fn stmt_match(&mut self, m: &'static ast::StmtMatch, state: &mut State) {
        let subj = self.eval(&m.subject, state);
        let mut outs: Vec<State> = Vec::new();
        self.cx.cond_depth += 1;
        self.cx.branch_depth += 1;
        // cases in order: a case that cannot match is skipped, one that surely matches
        // (with no guard, or a guard that is true) ends the match
        let mut matched = false;
        for case in &m.cases {
            let mut cs = state.copy();
            let decided = { self.match_pattern(&case.pattern, &subj, &mut cs) };
            if decided == Some(false) || !cs.reachable {
                continue;
            }
            let mut sure = decided == Some(true);
            if let Some(g) = &case.guard {
                let sp = self.split(g, cs);
                if !sp.t.reachable {
                    continue;
                }
                sure &= !sp.f.reachable;
                cs = sp.t;
            }
            self.exec_block(&case.body, &mut cs);
            outs.push(cs);
            if sure {
                matched = true;
                break;
            }
        }
        self.cx.branch_depth -= 1;
        self.cx.cond_depth -= 1;
        if !matched {
            outs.push(state.copy());
        }
        let mut out = outs.remove(0);
        for mut o in outs {
            out = out.join(&mut o);
        }
        state.replace_with(out);
    }

    /// `with`: a try whose finally is the managers' `__exit__`.
    fn stmt_with(&mut self, w: &'static ast::StmtWith, state: &mut State) {
        let mut managers: Vec<(Value, Node)> = Vec::new();
        for item in &w.items {
            let v = self.eval(&item.context_expr, state);
            let n = frontend::node(&item.context_expr);
            let mut entered = self.call_method_if_defined(&v, "__enter__", &n, state);
            if entered.is_none() {
                entered = crate::models::csvio::enter_value(self, state, &v);
            }
            if entered.is_none() {
                entered = Some(if v.pts.iter().any(|o| self.heap.kind(o).is_record()) { v.clone() } else { Value::data(&format!("with@L{}", frontend::node(w).line)) });
            }
            if let Some(ov) = &item.optional_vars {
                self.bind_target(ov, &entered.unwrap(), state);
            }
            managers.push((v, n));
        }
        // `redirect_stdout(buf)`: prints in the body go to buf
        let redirects = crate::models::csvio::redirect_targets(self, state, &managers.iter().map(|m| m.0.clone()).collect::<Vec<_>>());
        let n_redirects = redirects.len();
        self.m.stdout_redirect.extend(redirects);
        // a try whose finally is the managers' `__exit__`: it runs however the body ends
        // (normally, by return / break / continue, or by an exception); a manager whose
        // `__exit__` returns True swallows the exception: a catch-all handler that does
        // nothing
        let swallows = managers.iter().any(|(v, _)| self.exit_swallows(v, state));
        let handlers = if swallows { vec![Handler { types: None, name: None, body: &[], line: frontend::node(w).line }] } else { vec![] };
        self.stmt_try_parts(&w.body, handlers, &[], std::rc::Rc::new(Finally::Exit(managers)), state);
        for _ in 0..n_redirects {
            self.m.stdout_redirect.pop();
        }
    }

    /// `from m import a [as x]`, `from m import *`.
    fn stmt_import_from(&mut self, i: &'static ast::StmtImportFrom, state: &mut State) {
        let n = frontend::node(i);
        let modname = self.import_from_module(i);
        let mod_v = if modname.is_empty() { Value::new() } else { self.import_module(&modname, &n, state) };
        for a in &i.names {
            if a.name.id.as_str() == "*" {
                self.import_star(&mod_v, &modname, &n, state);
                continue;
            }
            let aname = a.name.id.as_str();
            let mut v = Value::new();
            if let Some(mo) = mod_v.single().filter(|o| self.heap.kind(*o) == Kind::Module) {
                // a name the module does not define may be one of its submodules
                if state.slot_opt(mo, Ns::Attr, key_str_lit(aname)).is_none() {
                    let sub = self.import_module(&format!("{}.{}", modname, aname), &n, state);
                    if sub.single().map(|o| self.heap.kind(o) == Kind::Module).unwrap_or(false) {
                        v = sub;
                    }
                }
            }
            if v.pts.is_empty() && !mod_v.pts.is_empty() {
                v = self.load_attr(&mod_v, aname, &n, state, false);
            }
            let target = a.asname.as_ref().map(|x| x.id.as_str()).unwrap_or(aname);
            self.store_name(s(target), v, state);
        }
    }

    /// `import a.b.c [as x]`.
    fn stmt_import(&mut self, i: &'static ast::StmtImport, state: &mut State) {
        for a in &i.names {
            let name = a.name.id.to_string();
            let top = name.split('.').next().unwrap_or("").to_string();
            let n = frontend::node(i);
            // `import a.b.c` imports the whole chain and binds `a`
            let v = self.import_module(&name, &n, state);
            if let Some(asn) = &a.asname {
                self.store_name(s(asn.id.as_str()), v, state);
            } else {
                let tv = if top == name { v } else { self.import_module(&top, &n, state) };
                self.store_name(s(&top), tv, state);
            }
        }
    }

    /// `raise` (`from`, bare): the exception and its class go to the running try with this state.
    fn stmt_raise(&mut self, r: &'static ast::StmtRaise, state: &mut State) {
        let exc_v = match &r.exc {
            Some(e) => Some(self.eval(e, state)),
            // a bare raise re-raises what the running handler holds: not tracked
            None => None,
        };
        // `raise X from Y`: X.__cause__ is Y (None for `from None`)
        if let (Some(xv), Some(ce)) = (&exc_v, &r.cause) {
            let cv = self.eval(ce, state);
            let ck = key_str_lit("__cause__");
            for o in xv.pts.iter() {
                if matches!(self.heap.kind(o), Kind::Instance | Kind::Namespace) || self.m.exc_objs.contains_key(&o) {
                    let strong = xv.pts.len() == 1 && self.heap.obj(o).single;
                    let row = state.row(o, Ns::Attr).store(ck, strong);
                    state.set_row(o, Ns::Attr, row);
                    state.set_slot(o, Ns::Attr, ck, cv.clone(), strong);
                }
            }
        }
        // a bare `raise` re-raises what the running handler caught
        let names: Vec<String> = match &r.exc {
            Some(_) => vec![],
            None => self.unwind.handling.last().map(|h| h.iter().cloned().collect()).unwrap_or_default(),
        };
        self.unwind.cur_exc = exc_v.clone().filter(|v| v.pts.iter().any(|o| self.heap.kind(o) == Kind::Instance || self.m.exc_objs.contains_key(&o) || matches!(&self.heap.obj(o).payload, Payload::Method(q, _) if q == "fastapi.httpexc")));
        // a bare raise: the exception the handler holds
        if r.exc.is_none() {
            self.unwind.cur_exc = self.unwind.handling_values.last().cloned().filter(|v| !v.pts.is_empty());
        }
        // the class raised: of the exception object, of a raised class, or (a bare
        // raise of something unknown) a subclass of what the handler catches
        self.unwind.cur_exc_cls = match &self.unwind.cur_exc {
            Some(v) => self.exc_value_names(v),
            None => match &exc_v {
                Some(v) if v.kv.is_bot() && v.pts.len() == 1 && self.heap.class_info_opt(v.pts.iter().next().unwrap()).is_some() => self.exc_class_names(v.pts.iter().next().unwrap()),
                Some(v) if v.kv.is_bot() && v.pts.len() == 1 && self.heap.kind(v.pts.iter().next().unwrap()) == Kind::External => {
                    let q = self.heap.external_qual(v.pts.iter().next().unwrap());
                    q.strip_prefix("builtins.").map(crate::interp::builtin_exc_names)
                }
                _ => None,
            },
        };
        if self.unwind.cur_exc_cls.is_none() && r.exc.is_none() && names.len() == 1 && names[0] != "*" {
            self.unwind.cur_exc_cls = Some((crate::interp::builtin_exc_names(&names[0]).0, false));
        }
        // (the running try, if any, takes it with this state; else it leaves the program)
        if state.reachable {
            self.record_raise_state(state);
        }
        self.unwind.cur_exc = None;
        self.unwind.cur_exc_cls = None;
        state.reachable = false;
    }

    /// `return`: the value joins the call's result; what the frame made and nothing reaches is collected.
    fn stmt_return(&mut self, r: &'static ast::StmtReturn, state: &mut State) {
        let v = match &r.value {
            Some(e) => self.eval(e, state),
            None => self.none_v.clone(),
        };
        if !state.reachable {
            // the value's evaluation raised on every path (a guarded access that is
            // definitely missing): no normal exit from here
            return;
        }
        // (names a row check of this line reads after it are still there)
        let mut keep = v.clone();
        if !self.findings.row_check_requests.is_empty() {
            let n = frontend::node(r);
            let file = self.program.rel_file(n.module);
            let names: Vec<String> = self.findings.row_check_requests.iter().filter(|(f, ln, _)| *ln >= n.line && *ln <= n.end_line && *f == file).map(|(_, _, nm)| nm.clone()).collect();
            for nm in names {
                if let Some(x) = state.env.get(&s(&nm)) {
                    keep = keep.join(x);
                }
            }
        }
        self.drop_garbage_at_return(&keep, state);
        state.ret = Some(match &state.ret {
            None => v,
            Some(cur) => cur.join(&v),
        });
        state.record_exit();
        state.reachable = false;
    }

    /// `t op= v`: the target's object and key evaluated once, read, combined, stored (in place for
    /// lists, dicts and sets).
    fn stmt_augassign(&mut self, a: &'static ast::StmtAugAssign, state: &mut State) {
        let n = frontend::node(a);
        let tn = frontend::node(&*a.target);
        // (the target's object and key are evaluated once: Python reads and stores
        // through them; a tensor's subscript goes through the expression)
        let place: Option<(Value, Result<Value, &str>)> = match &*a.target {
            ast::Expr::Subscript(sub) => Some((self.eval(&sub.value, state), Ok(self.eval_key(&sub.slice, state)))).filter(|(o, _)| !crate::models::torch::is_tensor(o)),
            ast::Expr::Attribute(at) => Some((self.eval(&at.value, state), Err(at.attr.id.as_str()))),
            _ => None,
        };
        let cur = match &place {
            Some((o, Ok(k))) => self.load_item(o, Ns::Item, k, &tn, state, true),
            Some((o, Err(name))) => self.load_attr(o, name, &tn, state, true),
            None => self.eval(&a.target, state),
        };
        let rhs = self.eval(&a.value, state);
        let store = |an: &mut Analyzer, v: &Value, state: &mut State| match &place {
            Some((o, Ok(k))) => an.store_item(o, Ns::Item, k, v, &tn, state),
            Some((o, Err(name))) => an.store_attr(o, name, v, &tn, state),
            None => an.bind_target(&a.target, v, state),
        };
        // `xs += ys` on a list extends it in place (other names see it); `d |= e` on a
        // dict updates it in place; `s |= t` (`&=`, `-=`, `^=`) on a set too
        if !cur.pts.is_empty() {
            let all = |an: &Analyzer, k: Kind| cur.pts.iter().all(|o| an.heap.kind(o) == k);
            let m = match a.op {
                ast::Operator::Add if all(self, Kind::List) => Some("list.extend"),
                ast::Operator::BitOr if all(self, Kind::Dict) => Some("dict.update"),
                ast::Operator::BitOr if all(self, Kind::Set) => Some("set.update"),
                ast::Operator::BitAnd if all(self, Kind::Set) => Some("set.intersection_update"),
                ast::Operator::Sub if all(self, Kind::Set) => Some("set.difference_update"),
                ast::Operator::BitXor if all(self, Kind::Set) => Some("set.symmetric_difference_update"),
                _ => None,
            };
            if let Some(m) = m {
                let mv = self.method_value(m, &cur);
                self.call_value(&mv, &n, None, state, Some(vec![rhs]));
                store(self, &cur, state);
                return;
            }
        }
        let res = self.binop(a.op, &cur, &rhs, &n, state);
        // `t += u` on a tensor is in place: the result must keep t's shape
        if self.m.torch_on && crate::models::torch::is_definite_tensor(&cur) && !crate::models::torch::t_inplace_fits(self, &cur, &res, &n, state) {
            return;
        }
        // `d[k] += v` with an unknown key on a closed dict: the load raised unless k was
        // one of its keys, so the store adds none
        if let Some((obj, Ok(key))) = &place {
            let plain = !obj.pts.is_empty() && obj.kv.is_bot() && obj.pts.iter().all(|o| self.heap.kind(o) == Kind::Dict && !self.m.dd_factories.contains_key(&o) && !self.m.counters.contains(&o) && !crate::models::pandas::is_series_obj(self, o) && { let r = state.row(o, Ns::Item); r.closed() && !r.present.is_empty() });
            if plain && !key.kv.is_literal() {
                for o in obj.pts.iter() {
                    for k in state.row(o, Ns::Item).present.iter() {
                        state.set_slot(o, Ns::Item, k, res.clone(), false);
                    }
                }
                return;
            }
        }
        store(self, &res, state);
    }

    /// `with cm(...) as x: body` where `cm` is a `@contextmanager` generator function: the
    /// generator runs concretely and the body runs at its yield. False: not that case.
    fn with_contextmanager(&mut self, w: &'static ast::StmtWith, state: &mut State) -> bool {
        if w.items.len() != 1 {
            return false;
        }
        let item = &w.items[0];
        let ast::Expr::Call(call) = &item.context_expr else { return false };
        // only a plain name / attribute callee (evaluating it twice has no effect)
        if !matches!(&*call.func, ast::Expr::Name(_) | ast::Expr::Attribute(_)) {
            return false;
        }
        let mut probe = state.copy();
        let fv = self.eval(&call.func, &mut probe);
        let Some(f) = fv.single() else { return false };
        if self.heap.kind(f) != Kind::Function || !self.heap.func_info(f).is_contextmanager.get() {
            return false;
        }
        let prev = self.gens.cm_hook.replace(super::CmHook { body: &w.body, target: item.optional_vars.as_deref(), depth: self.cx.frames.len(), env_after: None, ran: false, then: None, body_died: false, env_before: None, multi: true, body_exits: None, body_ret: None });
        let n = frontend::node(&item.context_expr);
        let fv = self.eval(&call.func, state);
        self.call_value(&fv, &n, Some(call), state, None);
        let h = std::mem::replace(&mut self.gens.cm_hook, prev);
        match h {
            Some(mut h) if h.ran => {
                if let Some(env) = h.env_after.take() {
                    state.env = env;
                }
                // the body's returns: the caller's exits
                if let Some(mut bx) = h.body_exits.take() {
                    bx.reachable = true;
                    let joined = match state.exits.take() {
                        Some(cur) => cur.copy().join(&mut bx),
                        None => bx,
                    };
                    state.exits = Some(std::sync::Arc::new(joined));
                }
                if let Some(r) = h.body_ret.take() {
                    state.ret = Some(match state.ret.take() {
                        Some(cur) => cur.join(&r),
                        None => r,
                    });
                }
                if !state.reachable && !h.body_died {
                    // the generator did not finish normally after a body that did: still go on
                    // after the with (an exception there would be the body's)
                    state.reachable = true;
                }
            }
            _ => {
                // the generator never reached its yield here: the body with an unknown value
                if let Some(t) = item.optional_vars.as_deref() {
                    self.bind_target(t, &Value::data("with"), state);
                }
                self.exec_block(&w.body, state);
            }
        }
        true
    }

    /// Whether `pat` matches `subj` (None: depends), binding its capture names in `state`.
    fn match_pattern(&mut self, pat: &'static ast::Pattern, subj: &Value, state: &mut State) -> Option<bool> {
        match pat {
            ast::Pattern::MatchAs(a) => {
                let r = match &a.pattern {
                    Some(p) => self.match_pattern(p, subj, state),
                    None => Some(true),
                };
                if let Some(n) = &a.name {
                    self.store_name(s(n.id.as_str()), subj.clone(), state);
                }
                r
            }
            ast::Pattern::MatchOr(o) => {
                let mut any_unknown = false;
                for p in &o.patterns {
                    match self.match_pattern(p, subj, state) {
                        Some(true) => return Some(true),
                        Some(false) => {}
                        None => any_unknown = true,
                    }
                }
                if any_unknown { None } else { Some(false) }
            }
            ast::Pattern::MatchValue(v) => {
                let pv = self.eval(&v.value, state);
                let a = crate::domain::kv_literals(&subj.kv);
                let b = crate::domain::kv_literals(&pv.kv);
                if subj.pts.is_empty() && pv.pts.is_empty() && subj.kv.is_literal() && pv.kv.is_literal() && b.len() == 1 && !a.is_empty() {
                    let k = b.iter().next().unwrap();
                    if a.len() == 1 {
                        return Some(a.contains(k));
                    }
                    if !a.contains(k) {
                        return Some(false);
                    }
                    return None;
                }
                if !subj.pts.is_empty() && subj.pts.iter().all(|o| self.heap.kind(o).is_container() || self.heap.kind(o) == Kind::Instance && self.heap.instance_class(o).map(|c| self.class_lookup(c, "__eq__").is_none()).unwrap_or(false)) {
                    // containers / plain instances never equal a literal
                    return Some(false);
                }
                None
            }
            ast::Pattern::MatchSingleton(sg) => {
                let want = match sg.value {
                    ast::Singleton::None => intern::key_none(),
                    ast::Singleton::True => intern::key_bool(true),
                    ast::Singleton::False => intern::key_bool(false),
                };
                let lits = crate::domain::kv_literals(&subj.kv);
                if subj.pts.is_empty() && subj.kv.is_literal() && !lits.is_empty() {
                    if lits.len() == 1 {
                        return Some(lits.contains(want));
                    }
                    return if lits.contains(want) { None } else { Some(false) };
                }
                if !subj.pts.is_empty() && subj.pts.iter().all(|o| self.heap.kind(o).is_container() || self.heap.kind(o) == Kind::Instance) {
                    return Some(false);
                }
                None
            }
            ast::Pattern::MatchMapping(mm) => {
                if subj.pts.is_empty() {
                    if subj.kv.is_literal() {
                        return Some(false);
                    }
                    return None;
                }
                let recs: Vec<u32> = subj.pts.iter().filter(|o| self.heap.kind(*o) == Kind::Dict).collect();
                if recs.len() != subj.pts.len() {
                    if subj.pts.iter().all(|o| matches!(self.heap.kind(o), Kind::List | Kind::Tuple | Kind::Set)) {
                        return Some(false);
                    }
                    for p in &mm.patterns {
                        self.match_pattern(p, &Value::data("match"), state);
                    }
                    return None;
                }
                let mut result = Some(true);
                let node = frontend::node(pat);
                for (kexpr, p) in mm.keys.iter().zip(mm.patterns.iter()) {
                    let kv = self.eval(kexpr, state);
                    let Some(k) = kv.kv.as_lit().filter(|_| kv.pts.is_empty()) else {
                        result = None;
                        continue;
                    };
                    let rows: Vec<Row> = recs.iter().map(|o| state.row(*o, Ns::Item)).collect();
                    let present = if rows.iter().all(|r| r.definite.contains(k)) {
                        Some(true)
                    } else if rows.iter().all(|r| r.closed() && !r.present.contains(k)) {
                        Some(false)
                    } else {
                        None
                    };
                    if present == Some(false) {
                        return Some(false);
                    }
                    let sv = self.load_item(subj, Ns::Item, &kv, &node, state, false);
                    let sub = self.match_pattern(p, &sv, state);
                    result = match (result, present, sub) {
                        (_, _, Some(false)) => return Some(false),
                        (Some(true), Some(true), Some(true)) => Some(true),
                        _ => None,
                    };
                }
                if let Some(rest) = &mm.rest {
                    self.store_name(s(rest.id.as_str()), Value::data("match rest"), state);
                }
                result
            }
            ast::Pattern::MatchClass(c) => {
                for p in c.arguments.patterns.iter() {
                    self.match_pattern(p, &Value::data("match"), state);
                }
                for kw in c.arguments.keywords.iter() {
                    self.match_pattern(&kw.pattern, &Value::data("match"), state);
                }
                None
            }
            ast::Pattern::MatchSequence(sq) => {
                let el = Value::data("match");
                for p in &sq.patterns {
                    self.match_pattern(p, &el, state);
                }
                None
            }
            ast::Pattern::MatchStar(st) => {
                if let Some(n) = &st.name {
                    self.store_name(s(n.id.as_str()), Value::data("match rest"), state);
                }
                Some(true)
            }
        }
    }

    fn stmt_delete(&mut self, d: &'static ast::StmtDelete, state: &mut State) {
        for t in &d.targets {
            match t {
                ast::Expr::Subscript(sub) => {
                    let obj = self.eval(&sub.value, state);
                    let key = self.eval_key(&sub.slice, state);
                    let n = frontend::node(t);
                    // `del xs[i]` on a list: later positions move down
                    for o in obj.pts.iter() {
                        if self.heap.kind(o) != Kind::List {
                            continue;
                        }
                        let idx = key.kv.as_lit().filter(|_| key.pts.is_empty()).and_then(intern::key_as_int);
                        match (self.known_positions(o, state), idx) {
                            (Some(mut ps), Some(i)) if obj.pts.len() == 1 => {
                                let len = ps.len() as i64;
                                let at = if i < 0 { len + i } else { i };
                                if 0 <= at && at < len {
                                    ps.remove(at as usize);
                                    self.set_positions(o, ps, state);
                                }
                            }
                            _ => {
                                self.heap.set_llen(o, state, None);
                                state.drop_obj_ns(o, Ns::Pos);
                            }
                        }
                    }
                    self.delete_item(&obj, Ns::Item, &key, &n, state, true);
                }
                ast::Expr::Attribute(a) => {
                    let obj = self.eval(&a.value, state);
                    let n = frontend::node(t);
                    self.delete_item(&obj, Ns::Attr, &Value::lit(key_str_lit(a.attr.id.as_str())), &n, state, true);
                }
                ast::Expr::Name(nm) => {
                    state.env.shift_remove(&s(nm.id.as_str()));
                }
                _ => {}
            }
        }
    }

    fn stmt_if(&mut self, test: &'static ast::Expr, body: &'static [ast::Stmt], rest: &'static [ast::ElifElseClause], state: &mut State) {
        if matches!(test, ast::Expr::Name(n) if n.id.as_str() == "TYPE_CHECKING") || matches!(test, ast::Expr::Attribute(a) if a.attr.as_str() == "TYPE_CHECKING") {
            self.note_type_only_imports(body);
        }
        let mut pre = state.copy();
        let sp = self.split(test, state.copy());
        let (mut then_s, mut else_s) = (sp.t, sp.f);
        // a decided test leaves one branch: straight-line code, not a conditional (appends
        // stay positional, requirements stay "always")
        let conditional = then_s.reachable && else_s.reachable;
        let cond = if conditional { self.cond_key(test) } else { None };
        if let Some((key, pos, _)) = &cond {
            // an earlier join on the same test: each branch gets back the rows it had then (the
            // test is pure: the state before it is the one it was evaluated in)
            self.apply_cond_facts(*key, *pos, &mut pre, &mut then_s);
            self.apply_cond_facts(*key, !*pos, &mut pre, &mut else_s);
        }
        if conditional {
            self.cx.cond_depth += 1;
            self.cx.branch_depth += 1;
        }
        if then_s.reachable {
            self.exec_block(body, &mut then_s);
        }
        if else_s.reachable {
            self.exec_orelse(rest, &mut else_s);
        }
        if conditional {
            self.cx.branch_depth -= 1;
            self.cx.cond_depth -= 1;
        }
        // `if k not in d: raise …`: the input must hold k (else the program raises)
        if let (true, Some((kv, target))) = (conditional, &sp.ops) {
            self.require_by_raise(test, body, rest, kv, target, &pre, &mut then_s, &mut else_s);
        }
        let mut j = then_s.join(&mut else_s);
        if then_s.reachable && else_s.reachable && conditional {
            let mut new: Vec<std::sync::Arc<crate::state::CondFact>> = Vec::new();
            let mut assigned: Vec<intern::StrId> = Vec::new();
            for st in body.iter() {
                collect_assigned(st, &mut assigned);
            }
            for c in rest.iter() {
                for st in c.body.iter() {
                    collect_assigned(st, &mut assigned);
                }
            }
            if let Some((key, pos, names)) = cond {
                // the test's names keep their values through the branches (narrowing aside):
                // their joined values stand for them; a branch that assigns one gives no fact
                let deps: Vec<(intern::StrId, Value)> = names.iter().map(|n| (*n, self.dep_value(&mut j, *n))).collect();
                let mutable_seq = deps.iter().any(|(_, v)| v.pts.iter().any(|o| matches!(self.heap.kind(o), Kind::List | Kind::Set)));
                if !names.iter().any(|n| assigned.contains(n)) && !mutable_seq {
                    new.extend(cond_facts_for(key, pos, deps, &then_s, &else_s, &j));
                }
            }
            let vals: Vec<(intern::StrId, Option<Value>, Option<Value>, Option<Value>)> = assigned.iter().map(|n| (*n, self.name_value(&then_s, *n), self.name_value(&else_s, *n), self.name_value(&j, *n))).collect();
            new.extend(name_facts(&vals, &then_s, &else_s, &j, self.frame().func.unwrap_or(0)));
            add_facts(&mut j, new);
        }
        state.replace_with(j);
    }

    /// `if k not in d: raise …` / `if k in d: … else: raise …`: the branch where the key is
    /// missing raises, so the key is required of the inputs that may provide it.
    #[allow(clippy::too_many_arguments)]
    fn require_by_raise(&mut self, test: &'static ast::Expr, body: &'static [ast::Stmt], rest: &'static [ast::ElifElseClause], kv: &Value, target: &Value, pre: &State, then_s: &mut State, else_s: &mut State) {
        let ast::Expr::Compare(c) = test else { return };
        if c.ops.len() != 1 {
            return;
        }
        let missing_branch: &[ast::Stmt] = match c.ops[0] {
            ast::CmpOp::NotIn => body,
            ast::CmpOp::In => match rest.first() {
                Some(cl) if cl.test.is_none() => &cl.body,
                _ => return,
            },
            _ => return,
        };
        if !matches!(missing_branch.last(), Some(ast::Stmt::Raise(_))) {
            return;
        }
        let Some(k) = kv.kv.as_lit().filter(|_| kv.pts.is_empty()) else { return };
        let mut recs: Vec<u32> = Vec::new();
        for o in target.pts.iter() {
            match &self.heap.obj(o).payload {
                Payload::View(super::ViewKind::Keys, inner) => recs.extend(inner.pts.iter().filter(|x| matches!(self.heap.kind(*x), Kind::Dict | Kind::Frame | Kind::Unknown))),
                _ if matches!(self.heap.kind(o), Kind::Dict | Kind::Frame | Kind::Unknown) => recs.push(o),
                _ => {}
            }
        }
        for o in recs {
            let row = pre.row(o, Ns::Item);
            let cls = crate::domain::classify(&row, k);
            if cls.tier == crate::domain::Tier::Input {
                for st in [&mut *then_s, &mut *else_s] {
                    if st.reachable {
                        self.require_providers(&cls, &row, true, st);
                    }
                }
            }
        }
    }

    /// The canonical text of a pure test (names, literals, comparisons, `in`, `is`, `not`,
    /// `and` / `or`), whether it is negated, and the names it reads.
    fn cond_key(&self, test: &'static ast::Expr) -> Option<(intern::StrId, bool, Vec<intern::StrId>)> {
        fn pure(e: &ast::Expr) -> bool {
            match e {
                ast::Expr::Name(_) | ast::Expr::StringLiteral(_) | ast::Expr::NumberLiteral(_) | ast::Expr::BooleanLiteral(_) | ast::Expr::NoneLiteral(_) => true,
                ast::Expr::Compare(c) => pure(&c.left) && c.comparators.iter().all(pure),
                ast::Expr::UnaryOp(u) => u.op == ast::UnaryOp::Not && pure(&u.operand),
                ast::Expr::BoolOp(b) => b.values.iter().all(pure),
                _ => false,
            }
        }
        let (inner, pos) = match test {
            ast::Expr::UnaryOp(u) if u.op == ast::UnaryOp::Not => (&*u.operand, false),
            other => (other, true),
        };
        if !pure(inner) {
            return None;
        }
        let text: String = frontend::text(inner).split_whitespace().collect::<Vec<_>>().join(" ");
        if text.is_empty() {
            return None;
        }
        let names: Vec<intern::StrId> = super::names_in(inner).iter().map(|n| s(n)).collect();
        Some((s(&format!("{}@{}", text, self.frame().func.unwrap_or(0))), pos, names))
    }

    /// The value a plain name has in `st` (a local, else a module-level name), if bound.
    pub(super) fn name_value(&self, st: &State, n: intern::StrId) -> Option<Value> {
        if let Some(v) = st.env.get(&n) {
            return Some(v.clone());
        }
        st.slot_opt(self.frame().module, Ns::Attr, intern::key(intern::Key::Str(intern::str_get(n))))
    }

    /// A name's value for a fact's dependencies: its binding, else the (fixed) unknown object
    /// a free name stands for.
    fn dep_value(&mut self, st: &mut State, n: intern::StrId) -> Value {
        match self.name_value(st, n) {
            Some(v) => v,
            None => self.load_name(n, &Node::none(), st),
        }
    }

    /// Rows a branch of an earlier join on `key` had, put back into `st` where unchanged
    /// since (in `pre`, the state before the test).
    fn apply_cond_facts(&mut self, key: intern::StrId, truth: bool, pre: &mut State, st: &mut State) {
        if !st.reachable {
            return;
        }
        let facts = pre.facts.clone();
        for f in facts.iter().filter(|f| f.cond == key && f.truth == truth) {
            if !f.deps.iter().all(|(n, v)| self.dep_value(pre, *n) == *v) {
                continue;
            }
            if !f.guards.iter().all(|((o, ns), r)| pre.row(*o, *ns) == *r) {
                continue;
            }
            for (k, branch, joined) in &f.rows {
                if pre.row(k.0, k.1) == *joined {
                    st.set_row(k.0, k.1, branch.clone());
                }
            }
            if let Some((branch, joined)) = &f.unbound {
                if pre.unbound == *joined {
                    st.unbound = branch.clone();
                }
            }
        }
    }

    fn exec_orelse(&mut self, rest: &'static [ast::ElifElseClause], state: &mut State) {
        let Some(first) = rest.first() else { return };
        match &first.test {
            None => self.exec_block(&first.body, state),
            Some(t) => self.stmt_if(t, &first.body, &rest[1..], state),
        }
    }

    fn stmt_while(&mut self, w: &'static ast::StmtWhile, state: &mut State) {
        // iterations whose test is decided at the head run as straight-line code (a constant
        // test such as `while True` is decided by the body alone: not unrolled); the rest of
        // the loop, from where the test stops being decided, is analysed as a loop
        let mut breaks: Option<State> = None;
        // (in a generator, `while True:` too: its yields get their positions, and a consumer
        // that takes a few items sees exactly those; only outside other loops and unrolled
        // iterations, as nested unrolling multiplies: email's `_parsegen` nests such loops)
        let outermost = self.frame().loop_depth == 0 && self.frame().unroll.is_empty();
        let gen_body = is_constant(&w.test) && outermost && self.frame().func.map(|f| matches!(self.heap.func_info(f).node, crate::interp::FuncNode::Def(d) if crate::interp::is_generator(&d.body))).unwrap_or(false);
        if !is_constant(&w.test) || gen_body {
            let mut idx = 0usize;
            loop {
                if !state.reachable {
                    break;
                }
                let sp = self.split(&w.test, state.copy());
                match (sp.t.reachable, sp.f.reachable) {
                    (false, _) => {
                        state.replace_with(sp.f);
                        if !w.orelse.is_empty() {
                            self.exec_block(&w.orelse, state);
                        }
                        join_breaks(state, breaks);
                        return;
                    }
                    (true, false) if idx < UNROLL_MAX => {
                        state.replace_with(sp.t);
                        self.unrolled_iteration(1000 + idx, &w.body, state, &mut breaks, &mut |_, _| {});
                        idx += 1;
                    }
                    _ => break,
                }
            }
            if !state.reachable {
                if let Some(b) = breaks {
                    state.replace_with(b);
                }
                return;
            }
        }
        self.stmt_while_loop(w, state);
        join_breaks(state, breaks);
    }

    /// A loop fixpoint cut off at `LOOP_MAX_ITER`: what still changed between the last two
    /// heads is widened to unknown (names and slots get unknown data, rows open, lengths
    /// unknown), so the exit state covers every later iteration.
    pub(crate) fn widen_unconverged(&mut self, head: &mut State, prev: &State) {
        if crate::interp::debug_level() >= 1 {
            let fp0 = head.footprint(prev);
            let nn: Vec<String> = head.env.iter().filter(|(k, v)| prev.env.get(*k) != Some(*v)).map(|(k, _)| intern::str_get(*k).to_string()).collect();
            eprintln!("[rowan] loop fixpoint cut off after {} rounds: widening names {:?}, {} rows, {} slots, {} elems, lens {}", LOOP_MAX_ITER, nn, fp0.rows.len(), fp0.slots.len(), fp0.elems.len(), head.lens.diff_keys(&prev.lens).len());
            if crate::interp::debug_level() >= 2 {
                for (o, ns) in fp0.rows.iter().take(5) {
                    eprintln!("    row {} {:?} {:?} -> {:?}", self.heap.obj(*o).label, ns, prev.row(*o, *ns), head.row(*o, *ns));
                }
                for (o, sk) in fp0.slots.iter().take(5) {
                    eprintln!("    slot {} {:?} {:?} -> {:?}", self.heap.obj(*o).label, sk, prev.slots.get(o, sk), head.slots.get(o, sk));
                }
            }
        }
        // (tensor-model values — shapes, dims — join differing dims into `?` atoms already)
        let shape_like = |v: &Value| matches!(&v.kv, KeyVal::Data(d) if intern::str_get(*d).starts_with('\u{1}'));
        let widen = |v: &Value| if shape_like(v) { v.clone() } else { v.join(&Value::data("loop")) };
        let names: Vec<intern::StrId> = head.env.iter().filter(|(k, v)| prev.env.get(*k) != Some(*v)).map(|(k, _)| *k).collect();
        for n in names {
            let v = widen(&head.env[&n]);
            head.env.insert(n, v);
        }
        let fp = head.footprint(prev);
        for (o, ns) in fp.rows {
            let r = head.row(o, ns).data_store(crate::domain::Source::new("loop widening", crate::domain::SourceKind::Data));
            head.set_row(o, ns, r);
        }
        for (o, sk) in fp.slots {
            if let Some(v) = head.slots.get(&o, &sk) {
                head.slots.set(&o, sk, widen(&v));
            }
        }
        for o in fp.elems {
            let shape_elem = matches!(head.elems.get(&o), Some(KeyVal::Data(d)) if intern::str_get(d).starts_with('\u{1}'));
            if !shape_elem {
                head.elems.insert(o, crate::domain::DATA);
            }
        }
        for o in head.lens.diff_keys(&prev.lens) {
            head.lens.insert(o, (None, 0, 0));
        }
    }

    fn stmt_while_loop(&mut self, w: &'static ast::StmtWhile, state: &mut State) {
        let (head, exit_acc, _) = self.loop_fixpoint(state, &mut |an, body_s| {
            let sp = an.split(&w.test, body_s.copy());
            body_s.replace_with(sp.t);
            if body_s.reachable {
                an.exec_block(&w.body, body_s);
            }
        });
        // (a test that is true at the head never falls through: the loop leaves by break)
        let mut exit_s = self.split(&w.test, head).f;
        if !w.orelse.is_empty() && exit_s.reachable {
            self.exec_block(&w.orelse, &mut exit_s);
        }
        join_breaks(&mut exit_s, exit_acc);
        state.replace_with(exit_s);
    }

    /// A loop run to a fixpoint (at most `LOOP_MAX_ITER` rounds, then widened): `round` runs
    /// one iteration from the head; the head grows by each round's back edge (joined into it: the
    /// rounds cannot oscillate). Returns the head at the fixpoint, the states at the `break`s,
    /// and the last round's state at the back edge.
    pub(crate) fn loop_fixpoint(&mut self, state: &mut State, round: &mut dyn FnMut(&mut Analyzer, &mut State)) -> (State, Option<State>, State) {
        let mut head = state.copy();
        let mut breaks: Option<State> = None;
        let mut back = State::bottom();
        self.frame_mut().loop_depth += 1;
        self.cx.cond_depth += 1;
        let mut converged = false;
        let mut prev_head = head.copy();
        let mut vmark = self.findings.mark();
        for _ in 0..LOOP_MAX_ITER {
            // (a round before the fixpoint ran on less than the loop's states: what its accesses
            // said is superseded by the next round's)
            self.findings.drop_since(vmark);
            vmark = self.findings.mark();
            let mut body_s = head.copy();
            let mark = self.heap.len() as u32;
            self.cx.recent_stack.push(Vec::new());
            self.unwind.loop_stack.push(LoopCtx::default());
            round(self, &mut body_s);
            let ctx = self.unwind.loop_stack.pop().unwrap();
            if let Some(b) = ctx.breaks {
                join_into(&mut breaks, b);
            }
            back = match ctx.continues {
                None => body_s,
                Some(mut c) => body_s.join(&mut c),
            };
            let recs = self.cx.recent_stack.pop().unwrap_or_default();
            self.rename_recent(&mut back, &head, recs, mark);
            let new_head = head.join_loop(&mut back);
            if new_head.same(&head) {
                converged = true;
                break;
            }
            prev_head = std::mem::replace(&mut head, new_head);
        }
        self.mode.loop_converged = converged;
        if !converged {
            self.widen_unconverged(&mut head, &prev_head);
            // (the accesses as the widened states make them)
            self.findings.drop_since(vmark);
            self.unwind.loop_stack.push(LoopCtx::default());
            self.cx.recent_stack.push(Vec::new());
            round(self, &mut head.copy());
            self.cx.recent_stack.pop();
            self.unwind.loop_stack.pop();
        }
        self.cx.cond_depth -= 1;
        self.frame_mut().loop_depth -= 1;
        (head, breaks, back)
    }

    /// One unrolled iteration (`idx`: its place in the call context): `bind` the item, run the
    /// body; a `continue` rejoins here, a `break` joins `breaks`.
    fn unrolled_iteration(&mut self, idx: usize, body: &'static [ast::Stmt], state: &mut State, breaks: &mut Option<State>, bind: &mut dyn FnMut(&mut Analyzer, &mut State)) {
        self.frame_mut().unroll.push(idx);
        bind(self, state);
        self.unwind.loop_stack.push(LoopCtx::default());
        self.exec_block(body, state);
        let ctx = self.unwind.loop_stack.pop().unwrap();
        self.frame_mut().unroll.pop();
        if let Some(c) = ctx.continues {
            join_breaks(state, Some(c));
        }
        if let Some(b) = ctx.breaks {
            join_into(breaks, b);
        }
    }

    /// The iterations of a `for` over known items, unrolled: an item there on some paths only
    /// (`false`) runs on one path and is skipped on the other; a one-shot iterator's cursor
    /// (`cursor`: the object and where the items start) follows. Returns the `break` states.
    fn unroll_for(&mut self, f: &'static ast::StmtFor, items: &[(Value, bool)], cursor: Option<(u32, usize)>, state: &mut State) -> Option<State> {
        let mut breaks: Option<State> = None;
        for (idx, (item, sure)) in items.iter().enumerate() {
            if !state.reachable {
                break;
            }
            if let Some((o, c0)) = cursor {
                self.set_cursor(o, Some(c0 + idx + 1), state);
            }
            let skipped = if *sure { None } else { Some(state.copy()) };
            self.unrolled_iteration(idx, &f.body, state, &mut breaks, &mut |an, st| an.bind_iter_target(&f.target, &Elem::One(item.clone()), st));
            join_breaks(state, skipped);
        }
        breaks
    }

    fn stmt_for(&mut self, f: &'static ast::StmtFor, state: &mut State) {
        let itv = self.eval(&f.iter, state);
        let iter_node = frontend::node(&*f.iter);
        self.force_gen(&itv, state, true);
        self.gens.for_iters.push(itv.pts.clone());
        let it = self.iterate(&itv, &iter_node, state);
        let has_jump = contains_jump(&f.body);
        // a one-shot iterator is used up by the loop; a `break` leaves the rest: the cursor
        // follows the unrolled iterations, else it is not known after a loop that may break
        let once = itv.single().filter(|o| self.heap.obj(*o).iterator && self.heap.obj(*o).single);
        let c0 = once.and_then(|o| self.cursor_of(o, state)).unwrap_or(0);
        // an endless generator with known first items: those iterations are unrolled (the
        // cursor follows), the rest of the loop runs on from there
        if let (Some(o), None, true) = (once, &it.items, matches!(&*f.iter, ast::Expr::Name(_))) {
            if let (None, Some(p)) = (self.heap.llen(o, state), self.heap.prefix(o, state)) {
                if p > c0 && p - c0 <= UNROLL_MAX {
                    let items: Vec<(Value, bool)> = (c0..p).map(|i| (state.slot(o, Ns::Pos, intern::key_int(i as i64)), true)).collect();
                    let breaks = self.unroll_for(f, &items, Some((o, c0)), state);
                    if state.reachable {
                        // (the rest: from the end of the known items)
                        self.stmt_for(f, state);
                    }
                    join_breaks(state, breaks);
                    return;
                }
            }
        }
        if once.is_none() || !has_jump {
            if has_jump {
                // (a lazy iterator a loop may break off: its sources' positions are not known)
                self.consume_lazy(&itv, false, state);
            } else {
                self.exhaust(&itv, state);
            }
        }
        if let Some(items) = &it.items {
            if items.len() <= UNROLL_MAX {
                // unrolled: `continue` rejoins at the next iteration, `break` at the exit
                let items: Vec<(Value, bool)> = items.iter().map(|i| (i.clone(), true)).collect();
                let breaks = self.unroll_for(f, &items, once.filter(|_| has_jump).map(|o| (o, c0)), state);
                if !f.orelse.is_empty() && state.reachable {
                    self.exec_block(&f.orelse, state);
                }
                if let (Some(_), true) = (once, has_jump) {
                    // (run to the end: used up)
                    if state.reachable {
                        self.exhaust(&itv, state);
                    }
                }
                join_breaks(state, breaks);
                return;
            }
        }
        // a closed dict with some keys only maybe there: each item unrolled, a maybe one run
        // on one path and skipped on the other
        if it.items.is_none() {
            if let Some(mitems) = self.maybe_dict_items(&itv, state) {
                let breaks = self.unroll_for(f, &mitems, None, state);
                if !f.orelse.is_empty() && state.reachable {
                    self.exec_block(&f.orelse, state);
                }
                join_breaks(state, breaks);
                return;
            }
        }
        // a list of unknown length whose first items are known on every path: those
        // iterations are unrolled, the loop runs on from there (any later item)
        let mut prefix_breaks: Option<State> = None;
        let mut prefixed = false;
        if let (None, None, Some(o)) = (&it.items, once, itv.single().filter(|_| itv.kv.is_bot())) {
            if matches!(self.heap.kind(o), Kind::List | Kind::Tuple) && !self.heap.obj(o).iterator && self.heap.obj(o).single && self.heap.llen(o, state).is_none() {
                if let Some(p) = self.heap.prefix(o, state).filter(|p| *p <= UNROLL_MAX) {
                    let items: Vec<(Value, bool)> = (0..p).map(|i| (state.slot(o, Ns::Pos, intern::key_int(i as i64)), true)).collect();
                    prefixed = true;
                    prefix_breaks = self.unroll_for(f, &items, None, state);
                    if !state.reachable {
                        if let Some(b) = prefix_breaks.take() {
                            state.replace_with(b);
                        }
                        return;
                    }
                }
            }
        }
        // (a loop that may break off: where the iterator stands after it is not known)
        if let (Some(o), true) = (once, has_jump) {
            self.set_cursor(o, None, state);
        }
        let pre = state.copy();
        let (head, exit_acc, back) = self.loop_fixpoint(state, &mut |an, body_s| {
            an.bind_iter_target(&f.target, &it.elem, body_s);
            an.exec_block(&f.body, body_s);
        });
        let mut exit_s = head.copy();
        let elem_v = it.elem.value_part();
        self.promote_elem_reqs(&mut exit_s, &back, &elem_v, state);
        if exit_acc.is_none() && back.reachable {
            // every element of the iterable went through the body: its row after the loop is
            // the body's exit row, not the join with "not processed yet"
            for oid in elem_v.pts.iter() {
                if self.heap.is_record_like(oid) && self.heap.obj(oid).single {
                    for ns in [Ns::Item, Ns::Attr] {
                        if back.has_row(oid, ns) {
                            exit_s.set_row(oid, ns, back.row(oid, ns));
                        }
                    }
                }
            }
        }
        if !f.orelse.is_empty() && exit_s.reachable {
            self.exec_block(&f.orelse, &mut exit_s);
        }
        join_breaks(&mut exit_s, exit_acc);
        // the loop ran at least once exactly when its iterable was non-empty: a later test of
        // that (`if xs:`, `if n > 0:` for `range(n)`) gets the rows the iterations left
        if f.orelse.is_empty() && !has_jump && !prefixed && back.reachable && pre.reachable && exit_s.reachable {
            let mut assigned: Vec<intern::StrId> = Vec::new();
            f.body.iter().for_each(|x| collect_assigned(x, &mut assigned));
            let func = self.frame().func.unwrap_or(0);
            let tests: Vec<(String, intern::StrId)> = match &*f.iter {
                ast::Expr::Name(n) => vec![(n.id.to_string(), s(n.id.as_str()))],
                ast::Expr::Call(c) if c.arguments.args.len() == 1 && matches!(&*c.func, ast::Expr::Name(r) if r.id.as_str() == "range") => match &c.arguments.args[0] {
                    ast::Expr::Name(n) => vec![(format!("{} > 0", n.id), s(n.id.as_str())), (format!("{} >= 1", n.id), s(n.id.as_str()))],
                    _ => vec![],
                },
                _ => vec![],
            };
            let mut new = Vec::new();
            for (text, name) in tests {
                if assigned.contains(&name) {
                    continue;
                }
                let deps = vec![(name, self.dep_value(&mut exit_s, name))];
                if deps[0].1.pts.iter().any(|o| matches!(self.heap.kind(o), Kind::List | Kind::Set)) {
                    continue;
                }
                new.extend(cond_facts_for(s(&format!("{}@{}", text, func)), true, deps, &back, &pre, &exit_s));
            }
            add_facts(&mut exit_s, new);
        }
        join_breaks(&mut exit_s, prefix_breaks);
        state.replace_with(exit_s);
    }

    fn stmt_classdef(&mut self, c: &'static ast::StmtClassDef, state: &mut State) {
        let fr_module = self.frame().module;
        let fr_ctx = self.frame().ctx.clone();
        let fr_single = self.frame().single;
        let fr_cells = self.frame().cells.clone();
        let mut bases: Vec<u32> = Vec::new();
        let mut open_base = false;
        let mut lib = crate::models::ClassBase::default();
        let mut ek = crate::interp::EnumKind::default();
        if let Some(args) = &c.arguments {
            for b in args.args.iter() {
                // `RootModel[list[int]]`, `Generic[T]`: the subscripted class decides
                let b: &ast::Expr = match b {
                    ast::Expr::Subscript(sb) => &sb.value,
                    other => other,
                };
                let bv = self.eval(b, state);
                for oid in bv.pts.iter() {
                    match self.heap.kind(oid) {
                        Kind::Class => {
                            if let Some(k) = self.m.enum_kind.get(&oid) {
                                ek.mixed |= k.mixed;
                                ek.is_str |= k.is_str;
                                ek.repr_value |= k.repr_value;
                                ek.flag |= k.flag;
                            }
                            // (a base that derives from an external class — `list`, an unseen
                            // library class — makes this one's instances open too)
                            if self.heap.class_info_opt(oid).is_some_and(|ci| ci.open_base) {
                                open_base = true;
                            }
                            bases.push(oid)
                        }
                        Kind::External => lib.join(crate::models::class_base(&self.heap.external_qual(oid))),
                        _ => open_base = true,
                    }
                }
                if bv.pts.is_empty() {
                    open_base = true;
                }
            }
            // a metaclass other than ABCMeta may add anything to the class and its instances
            for kw in args.keywords.iter() {
                if kw.arg.as_ref().map(|a| a.as_str() == "metaclass").unwrap_or(false) {
                    let mv = self.eval(&kw.value, state);
                    let abc = mv.pts.iter().all(|o| self.heap.kind(o) == Kind::External && self.heap.external_qual(o).ends_with("ABCMeta")) && !mv.pts.is_empty();
                    if !abc {
                        open_base = true;
                    }
                }
            }
        }
        open_base |= lib.open;
        let mut lek = lib.enum_kind;
        lek.join(ek);
        let (is_enum, str_mixin) = (lib.is_enum, lib.str_mixin);
        let (typed_dict, named_tuple, pydantic) = (lib.typed_dict, lib.named_tuple, lib.pydantic);
        let info = ClassInfo {
            name: c.name.id.to_string(),
            node: c,
            bases,
            module: fr_module,
            open_base,
            dataclass_fields: std::cell::RefCell::new(None),
            lib,
            pyd_dataclass: std::cell::Cell::new(false),
            dc_frozen: std::cell::Cell::new(false),
            dc_kw_only: std::cell::Cell::new(false),
            dc_order: std::cell::Cell::new(false),
            any_fields: std::cell::RefCell::new(None),
        };
        let info = std::rc::Rc::new(info);
        let cnode = frontend::node(c);
        let coid = self.alloc_at(Kind::Class, &cnode, state, &format!("class {}", c.name.id), Payload::Class(info.clone()), None, Ns::Attr, None, None);
        self.program.user_classes.push(coid);
        let is_dataclass = crate::models::dataclasses::class_decorators(self, c, coid, &info, state);
        if is_dataclass || typed_dict || named_tuple || pydantic {
            *info.dataclass_fields.borrow_mut() = Some(crate::models::dataclasses::declared_fields(c, pydantic));
        }
        // class body
        let mi = self.heap.module_info(fr_module);
        let cscope = mi.scopes.of_node(crate::scopes::node_addr(c)).unwrap_or_else(|| mi.scopes.module());
        let fr_single_nl = self.frame().single_nl;
        let mut cf = Frame::new(FrameKind::Class, cscope, fr_module, &fr_ctx, fr_single, &format!("class {}", c.name.id));
        cf.single_nl = fr_single_nl;
        cf.class_oid = Some(coid);
        cf.cells = fr_cells;
        self.cx.frames.push(cf);
        for st in &c.body {
            if let ast::Stmt::AnnAssign(a) = st {
                if a.value.is_none() {
                    continue;
                }
            }
            self.exec_stmt(st, state);
        }
        self.cx.frames.pop();
        let enum_base = info.bases.iter().any(|b| self.m.enum_members.contains_key(b));
        if is_enum || enum_base {
            crate::models::enums::make_enum_members(self, c, coid, lek, str_mixin, state);
        }
        // `__set_name__(owner, name)` on descriptor objects assigned in the class body
        for st in &c.body {
            let ast::Stmt::Assign(a) = st else { continue };
            let [ast::Expr::Name(n)] = &a.targets[..] else { continue };
            let v = self.heap.cls_slots.get(&(coid, Ns::Attr, key_str_lit(n.id.as_str()))).cloned().unwrap_or_default();
            for d in v.pts.iter() {
                if self.heap.kind(d) == Kind::Instance && self.heap.instance_class(d).and_then(|dc| self.class_lookup(dc, "__set_name__")).is_some() {
                    self.call_method_with(&Value::obj(d), "__set_name__", vec![Value::obj(coid), Value::lit(key_str_lit(n.id.as_str()))], &cnode, state);
                }
            }
        }
        // a base's `__init_subclass__` runs for the new class (an implicit classmethod)
        let found = info.bases.iter().find_map(|b| self.class_lookup(*b, "__init_subclass__"));
        if let Some(defc) = found {
            let fv = self.heap.cls_slots.get(&(defc, Ns::Attr, key_str_lit("__init_subclass__"))).cloned().unwrap_or_default();
            let mut kwargs: indexmap::IndexMap<intern::StrId, Value> = indexmap::IndexMap::new();
            if let Some(args) = &c.arguments {
                for kw in args.keywords.iter() {
                    if let Some(a) = &kw.arg {
                        if a.as_str() != "metaclass" {
                            let v = self.eval(&kw.value, state);
                            kwargs.insert(s(a.as_str()), v);
                        }
                    }
                }
            }
            for f in fv.pts.iter() {
                if self.heap.kind(f) == Kind::Function {
                    // concretely: the class's own attributes (`cls.__name__`) are known here
                    self.call_user(f, Some(Value::obj(coid)), vec![], kwargs.clone(), vec![], vec![], state, &cnode, None, false);
                }
            }
        }
        let mut cv = Value::obj(coid);
        if !is_dataclass && !c.decorator_list.is_empty() {
            cv = self.apply_decorators(&c.decorator_list, cnode, cv, state);
            if cv.pts.is_empty() {
                cv = Value::obj(coid);
            }
        }
        self.store_name(s(c.name.id.as_str()), cv, state);
    }

}

/// A `return` somewhere in these statements (not inside nested functions or classes).
/// May this code leave its function early (a `return`, or a `yield`: a generator)?
pub fn may_return(body: &[ast::Stmt]) -> bool {
    contains_return(body) || crate::interp::is_generator(body)
}

pub(super) fn contains_return(body: &[ast::Stmt]) -> bool {
    body.iter().any(|s| match s {
        ast::Stmt::Return(_) => true,
        ast::Stmt::If(i) => contains_return(&i.body) || i.elif_else_clauses.iter().any(|c| contains_return(&c.body)),
        ast::Stmt::For(f) => contains_return(&f.body) || contains_return(&f.orelse),
        ast::Stmt::While(w) => contains_return(&w.body) || contains_return(&w.orelse),
        ast::Stmt::With(w) => contains_return(&w.body),
        ast::Stmt::Try(t) => {
            contains_return(&t.body)
                || contains_return(&t.orelse)
                || contains_return(&t.finalbody)
                || t.handlers.iter().any(|h| {
                    let ast::ExceptHandler::ExceptHandler(h) = h;
                    contains_return(&h.body)
                })
        }
        ast::Stmt::Match(m) => m.cases.iter().any(|c| contains_return(&c.body)),
        _ => false,
    })
}

/// Facts of a join on test `key` (positive `pos`): the then-branch rows when it holds, the
/// else-branch rows when not, for every row the branches leave different.
fn cond_facts_for(key: intern::StrId, pos: bool, deps: Vec<(intern::StrId, Value)>, then_s: &State, else_s: &State, j: &State) -> Vec<std::sync::Arc<crate::state::CondFact>> {
    let mut guards: Vec<((u32, Ns), crate::domain::Row)> = Vec::new();
    for (_, v) in &deps {
        for o in v.pts.iter() {
            for ns in [Ns::Item, Ns::Attr] {
                guards.push(((o, ns), j.row(o, ns)));
            }
        }
    }
    let mut rt = Vec::new();
    let mut re = Vec::new();
    let mut keys = then_s.rows.diff_keys(&else_s.rows);
    keys.extend(else_s.rows.diff_keys(&then_s.rows));
    keys.sort_by_key(|(o, n)| (*o, *n));
    keys.dedup();
    for k in keys.into_iter().take(64) {
        let (tr, er, jr) = (then_s.row(k.0, k.1), else_s.row(k.0, k.1), j.row(k.0, k.1));
        if tr == er {
            continue;
        }
        rt.push((k, tr, jr.clone()));
        re.push((k, er, jr));
    }
    // (a branch's unbound names: its own, and the joined names it did not bind)
    let branch_unbound = |b: &State| -> std::sync::Arc<Vec<intern::StrId>> {
        let mut u: Vec<intern::StrId> = b.unbound.iter().copied().chain(j.env.keys().filter(|k| !b.env.contains_key(*k)).copied()).collect();
        u.sort();
        u.dedup();
        std::sync::Arc::new(u)
    };
    let (ut, ue) = (branch_unbound(then_s), branch_unbound(else_s));
    let unb = |b: std::sync::Arc<Vec<intern::StrId>>| if b != j.unbound { Some((b, j.unbound.clone())) } else { None };
    let (ut, ue) = (unb(ut), unb(ue));
    if rt.is_empty() && ut.is_none() && ue.is_none() {
        return Vec::new();
    }
    vec![
        std::sync::Arc::new(crate::state::CondFact { cond: key, truth: pos, deps: deps.clone(), rows: rt, guards: guards.clone(), unbound: ut }),
        std::sync::Arc::new(crate::state::CondFact { cond: key, truth: !pos, deps, rows: re, guards, unbound: ue }),
    ]
}

/// Names bound to literals of different truth in the two branches (`has = True` / `False`):
/// testing the name later tells which branch ran.
pub(super) fn name_facts(vals: &[(intern::StrId, Option<Value>, Option<Value>, Option<Value>)], a: &State, b: &State, j: &State, func: u32) -> Vec<std::sync::Arc<crate::state::CondFact>> {
    let truth_of = |v: &Value| v.kv.as_lit().filter(|_| v.pts.is_empty()).and_then(crate::models::pyvalues::truthy);
    let mut out = Vec::new();
    for (name, va, vb, jv) in vals.iter() {
        let (Some(va), Some(vb), Some(jv)) = (va, vb, jv) else { continue };
        let (Some(ta), Some(tb)) = (truth_of(va), truth_of(vb)) else { continue };
        if ta == tb {
            continue;
        }
        // the key a later bare `if name:` test computes (`cond_key`)
        let key = s(&format!("{}@{}", intern::str_get(*name), func));
        let deps = vec![(*name, jv.clone())];
        let mut facts = cond_facts_for(key, ta, deps, a, b, j);
        out.append(&mut facts);
    }
    out
}

/// Add facts to a state (keeping the most recent few).
pub(super) fn add_facts(st: &mut State, new: Vec<std::sync::Arc<crate::state::CondFact>>) {
    if new.is_empty() {
        return;
    }
    let mut all: Vec<std::sync::Arc<crate::state::CondFact>> = st.facts.iter().cloned().collect();
    all.extend(new);
    let n = all.len();
    if n > 32 {
        all.drain(0..n - 32);
    }
    st.facts = std::sync::Arc::new(all);
}

/// Plain names a statement (and the blocks inside it) assigns.
pub(super) fn collect_assigned(st: &ast::Stmt, out: &mut Vec<intern::StrId>) {
    let add = |t: &ast::Expr, out: &mut Vec<intern::StrId>| {
        for n in super::names_in(t) {
            let id = s(&n);
            if !out.contains(&id) {
                out.push(id);
            }
        }
    };
    match st {
        ast::Stmt::Assign(a) => a.targets.iter().for_each(|t| add(t, out)),
        ast::Stmt::AnnAssign(a) => add(&a.target, out),
        ast::Stmt::AugAssign(a) => add(&a.target, out),
        ast::Stmt::If(i) => {
            i.body.iter().for_each(|x| collect_assigned(x, out));
            i.elif_else_clauses.iter().for_each(|c| c.body.iter().for_each(|x| collect_assigned(x, out)));
        }
        ast::Stmt::Try(t) => {
            t.body.iter().chain(t.orelse.iter()).chain(t.finalbody.iter()).for_each(|x| collect_assigned(x, out));
            for h in &t.handlers {
                let ast::ExceptHandler::ExceptHandler(h) = h;
                h.body.iter().for_each(|x| collect_assigned(x, out));
            }
        }
        ast::Stmt::With(w) => w.body.iter().for_each(|x| collect_assigned(x, out)),
        _ => {}
    }
}

/// `state` joined with `other` (its replacement when `state` is unreachable).
pub(super) fn join_breaks(state: &mut State, other: Option<State>) {
    if let Some(mut o) = other {
        let j = if state.reachable { state.join(&mut o) } else { o };
        state.replace_with(j);
    }
}

/// `acc` joined with `s`.
pub(super) fn join_into(acc: &mut Option<State>, mut s: State) {
    *acc = Some(match acc.take() {
        None => s,
        Some(mut a) => a.join(&mut s),
    });
}
