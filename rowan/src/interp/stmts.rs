//! Statements: the dispatch by kind, assignments, `if` (and the facts a condition leaves),
//! `del`, `return`, `raise`, `break` / `continue`, imports, `with`, `match`. Loops are in
//! `loops.rs`, `try` in `tries.rs`, class definitions in `classes.rs`.

use ruff_python_ast as ast;

use crate::domain::{classify, kv_literals, Ns, Row, Value};
use crate::frontend::{self, Node};
use crate::intern::{self, key_str_lit, s};
use crate::state::{CondFact, Kind, State};
use crate::syntax::{collect_assigned, contains_jump};

use super::{Finally, Handler, Analyzer, FuncNode, Payload};

impl Analyzer {
    /// Runs statements in order, until the path ends (`after_stmt`: the hover facts and row
    /// checks of each).
    pub fn exec_block(&mut self, stmts: &'static [ast::Stmt], state: &mut State) {
        for st in stmts {
            if !state.reachable {
                return;
            }
            self.exec_stmt(st, state);
            self.after_stmt(st, state);
            // (generators nothing holds any more are closed: `genclose.rs`)
            self.close_unheld(state);
        }
    }

    /// Runs one statement, charged to the running function's budget (past the deadline a
    /// function body stops: `cut_body`).
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

    /// A statement, by its kind.
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
                // (a loop whose body yields: the consumer's code runs between the items)
                if let Some(used) = self.gens.for_iters.get(depth).cloned().filter(|_| !contains_jump(&f.body) && !crate::syntax::is_generator(&f.body)) {
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
                let v = match self.decorate_def(f, &v, state) {
                    Some(v) => v,
                    None => self.apply_decorators(&f.decorator_list, frontend::node(f), v, state),
                };
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
        // (a loop over a generator made for it alone drops it: closed)
        if matches!(node, ast::Stmt::Break(_)) {
            self.close_at_break(n, &mut st);
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
        // nothing, for the exceptions it may swallow (the others go on)
        let swallows = managers.iter().any(|(v, _)| self.exit_swallows(v, state));
        let handlers = if swallows { vec![Handler { types: None, name: None, body: &[], line: frontend::node(w).line, maybe: true }] } else { vec![] };
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
                let keep = self.frame().module != self.program.main_module;
                let saved = std::mem::replace(&mut self.mode.keep_deferred, keep);
                v = self.load_attr(&mod_v, aname, &n, state, false);
                self.mode.keep_deferred = saved;
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
                // (and a part that is no object, `9` of `[1] | 9`: its own result, on a path
                // of its own — None's raises there)
                let objs = Value::objs(cur.pts.clone());
                let rest = (!cur.kv.is_bot()).then(|| {
                    let mut alt = state.copy();
                    store(self, &Value::kv_only(cur.kv.clone()), &mut alt);
                    let r = self.binop(a.op, &Value::kv_only(cur.kv.clone()), &rhs, &n, &mut alt);
                    store(self, &r, &mut alt);
                    alt
                });
                let mv = self.method_value(m, &objs);
                self.call_value(&mv, &n, None, state, Some(vec![rhs]));
                store(self, &objs, state);
                if let Some(mut alt) = rest {
                    *state = state.join(&mut alt);
                }
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
            ast::Pattern::MatchValue(v) => self.match_value(v, subj, state),
            ast::Pattern::MatchSingleton(sg) => {
                let want = match sg.value {
                    ast::Singleton::None => intern::key_none(),
                    ast::Singleton::True => intern::key_bool(true),
                    ast::Singleton::False => intern::key_bool(false),
                };
                let lits = kv_literals(&subj.kv);
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
            ast::Pattern::MatchMapping(mm) => self.match_mapping(pat, mm, subj, state),
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

    /// `case 1` / `case Color.RED`: equal to the value.
    fn match_value(&mut self, v: &'static ast::PatternMatchValue, subj: &Value, state: &mut State) -> Option<bool> {
        let pv = self.eval(&v.value, state);
        let a = kv_literals(&subj.kv);
        let b = kv_literals(&pv.kv);
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

    /// `case {"k": p, **rest}`: a mapping with the keys (each value matching its pattern).
    fn match_mapping(&mut self, pat: &'static ast::Pattern, mm: &'static ast::PatternMatchMapping, subj: &Value, state: &mut State) -> Option<bool> {
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

    /// `del` of each target: an item, an attribute or a name.
    fn stmt_delete(&mut self, d: &'static ast::StmtDelete, state: &mut State) {
        for t in &d.targets {
            match t {
                ast::Expr::Subscript(sub) => {
                    let obj = self.eval(&sub.value, state);
                    let key = self.eval_key(&sub.slice, state);
                    let n = frontend::node(t);
                    self.delete_subscript(&obj, &key, &n, state);
                }
                ast::Expr::Attribute(a) => {
                    let obj = self.eval(&a.value, state);
                    let n = frontend::node(t);
                    self.delete_item(&obj, Ns::Attr, &Value::lit(key_str_lit(a.attr.id.as_str())), &n, state, true);
                }
                ast::Expr::Name(nm) => {
                    state.env.shift_remove(&s(nm.id.as_str()));
                    // (a module's global: no longer bound there)
                    let fr = self.frame();
                    let global = fr.kind == super::FrameKind::Module || (fr.kind == super::FrameKind::Function && self.heap.module_info(fr.module).scopes.get(fr.scope).globals_decl.contains(nm.id.as_str()));
                    if global {
                        let (m, k) = (fr.module, intern::key_str_lit(nm.id.as_str()));
                        state.slots.remove(&m, &(Ns::Attr, k));
                        let r = state.row(m, Ns::Attr).delete(k, true);
                        state.set_row(m, Ns::Attr, r);
                    }
                }
                _ => {}
            }
        }
    }

    /// An `if` statement: the test splits the state; both branches run and join (with the facts
    /// a pure test leaves, `CondFact`). `if TYPE_CHECKING:` notes its imports for annotations;
    /// the main module's `__main__` block runs no calls when set up for runs on demand.
    fn stmt_if(&mut self, test: &'static ast::Expr, body: &'static [ast::Stmt], rest: &'static [ast::ElifElseClause], state: &mut State) {
        if matches!(test, ast::Expr::Name(n) if n.id.as_str() == "TYPE_CHECKING") || matches!(test, ast::Expr::Attribute(a) if a.attr.as_str() == "TYPE_CHECKING") {
            self.note_type_only_imports(body);
        }
        // (setting the module code up for runs on demand: the calls of the main module's
        // `if __name__ == "__main__":` block — a script's work, often a demo — run no body)
        if self.entry.main_block_unrun && !self.budget.skip_calls && self.cx.frames.len() == 1 && crate::syntax::is_main_test(test) {
            self.budget.skip_calls = true;
            self.stmt_if(test, body, rest, state);
            self.budget.skip_calls = false;
            return;
        }
        let mut pre = state.copy();
        let sp = self.split(test, state.copy());
        let (mut then_s, mut else_s) = (sp.t, sp.f);
        // a decided test leaves one branch: straight-line code, not a conditional (appends
        // stay positional, requirements stay "always")
        let conditional = then_s.reachable && else_s.reachable;
        let cond = if conditional { self.cond_key(test) } else { None };
        if let Some((owner, pt, nt)) = if conditional { self.branch_tests(test, &mut pre) } else { None } {
            super::guards::add_test(&mut then_s, owner, pt);
            super::guards::add_test(&mut else_s, owner, nt);
        }
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
        match rest.first() {
            Some(first) if else_s.reachable => match &first.test {
                None => self.exec_block(&first.body, &mut else_s),
                Some(t) => self.stmt_if(t, &first.body, &rest[1..], &mut else_s),
            },
            _ => {}
        }
        if conditional {
            self.cx.branch_depth -= 1;
            self.cx.cond_depth -= 1;
        }
        // `if k not in d: raise …`: the input must hold k (else the program raises)
        if let (true, Some((kv, target))) = (conditional, &sp.ops) {
            self.require_by_raise(test, body, rest, kv, target, &pre, &mut then_s, &mut else_s);
        }
        // facts of this same test whose rows the branches left as they put them back: they
        // hold on after the join (which drops facts on rows the two sides differ in)
        let kept = match &cond {
            Some((key, pos, _)) if then_s.reachable && else_s.reachable => same_test_facts(*key, *pos, &then_s, &else_s),
            _ => Vec::new(),
        };
        let mut j = then_s.join(&mut else_s);
        if !kept.is_empty() {
            for f in &kept {
                for (k, _, joined) in &f.rows {
                    j.rows.insert(*k, joined.clone());
                }
            }
            let mut all: Vec<std::sync::Arc<CondFact>> = j.facts.iter().filter(|f| !kept.iter().any(|g| g.cond == f.cond)).cloned().collect();
            all.extend(kept);
            j.facts = std::sync::Arc::new(all);
        }
        if then_s.reachable && else_s.reachable && conditional {
            let mut new: Vec<std::sync::Arc<CondFact>> = Vec::new();
            let mut assigned: Vec<intern::StrId> = Vec::new();
            for st in body.iter() {
                collect_assigned(st, &mut assigned);
            }
            for c in rest.iter() {
                for st in c.body.iter() {
                    collect_assigned(st, &mut assigned);
                }
            }
            if let Some((key, pos, names)) = cond.filter(|(k, _, _)| self.tested(*k) >= 2) {
                // the test's names keep their values through the branches (narrowing aside):
                // their joined values stand for them; a branch that assigns one gives no fact
                let deps: Vec<(intern::StrId, Value)> = names.iter().map(|n| (*n, self.dep_value(&mut j, *n))).collect();
                let mutable_seq = deps.iter().any(|(_, v)| v.pts.iter().any(|o| matches!(self.heap.kind(o), Kind::List | Kind::Set)));
                if !names.iter().any(|n| assigned.contains(n)) && !mutable_seq {
                    new.extend(cond_facts_for(key, pos, deps, &then_s, &else_s, &j));
                }
            }
            let vals: Vec<(intern::StrId, Option<Value>, Option<Value>, Option<Value>)> = assigned.iter().filter(|n| self.tested(**n) >= 1).map(|n| (*n, self.name_value(&then_s, *n), self.name_value(&else_s, *n), self.name_value(&j, *n))).collect();
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
            let cls = classify(&row, k);
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
        let names: Vec<intern::StrId> = crate::syntax::names_in(inner).iter().map(|n| s(n)).collect();
        Some((s(&format!("{}@{}", text, self.frame().func.unwrap_or(0))), pos, names))
    }

    /// How often the running function tests this text (`if` / `elif` / `while`, a `not` taken
    /// off; a test in a loop counts twice: it runs again): a branch fact (`CondFact`) serves
    /// only a later test of its text. Outside functions: always enough.
    pub(super) fn tested(&self, text: intern::StrId) -> u32 {
        let t = intern::str_get(text);
        let t = t.rsplit_once('@').map(|(a, _)| a).unwrap_or(&t);
        let Some(fi) = self.frame().func.and_then(|f| self.heap.func_info_opt(f)) else { return u32::MAX };
        let body = fi.node.body();
        TESTED.with(|m| {
            let mut m = m.borrow_mut();
            let counts = m.entry(body.as_ptr() as usize).or_insert_with(|| {
                let mut c = rustc_hash::FxHashMap::default();
                count_tests(body, false, &mut c);
                c
            });
            counts.get(t).copied().unwrap_or(0)
        })
    }

    /// The value a plain name has in `st` (a local, else a module-level name), if bound.
    pub(super) fn name_value(&self, st: &State, n: intern::StrId) -> Option<Value> {
        if let Some(v) = st.env.get(&n) {
            return Some(v.clone());
        }
        if let Some(&cid) = self.frame().cells.get(&n).filter(|_| self.frame().kind == super::FrameKind::Function) {
            return st.slot_opt(cid, Ns::Cell, super::funcs::cell_key());
        }
        st.slot_opt(self.frame().module, Ns::Attr, intern::key_of_str(n))
    }

    /// A name's value for a fact's dependencies: its binding, else the (fixed) unknown object
    /// a free name stands for.
    pub(super) fn dep_value(&mut self, st: &mut State, n: intern::StrId) -> Value {
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
            if !f.guards.iter().all(|((o, ns), r)| pre.row(*o, *ns).same_ptr(r)) {
                continue;
            }
            for (k, branch, joined) in &f.rows {
                if pre.row(k.0, k.1).same_ptr(joined) {
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

}


/// Facts of a join on test `key` (positive `pos`): the then-branch rows when it holds, the
/// else-branch rows when not, for every row the branches leave different.
pub(super) fn cond_facts_for(key: intern::StrId, pos: bool, deps: Vec<(intern::StrId, Value)>, then_s: &State, else_s: &State, j: &State) -> Vec<std::sync::Arc<CondFact>> {
    let mut guards: Vec<((u32, Ns), Row)> = Vec::new();
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
        std::sync::Arc::new(CondFact { cond: key, truth: pos, deps: deps.clone(), rows: rt, guards: guards.clone(), unbound: ut }),
        std::sync::Arc::new(CondFact { cond: key, truth: !pos, deps, rows: re, guards, unbound: ue }),
    ]
}

/// The facts of test `key` both branches still hold whose every row is the branch's own row
/// on its side (put back by `apply_cond_facts`, unchanged since): the join of the two is the
/// joined row they record, so they hold after the join too.
fn same_test_facts(key: intern::StrId, pos: bool, then_s: &State, else_s: &State) -> Vec<std::sync::Arc<CondFact>> {
    let held = |st: &State, truth: bool| st.facts.iter().find(|f| f.cond == key && f.truth == truth).cloned();
    let (Some(ft), Some(fe)) = (held(then_s, pos), held(else_s, !pos)) else { return Vec::new() };
    if !std::sync::Arc::ptr_eq(&held(else_s, pos).unwrap_or_else(|| ft.clone()), &ft) || !std::sync::Arc::ptr_eq(&held(then_s, !pos).unwrap_or_else(|| fe.clone()), &fe) {
        return Vec::new();
    }
    let put_back = |st: &State, f: &CondFact| f.rows.iter().all(|(k, branch, _)| st.row(k.0, k.1).same_ptr(branch)) && f.guards.iter().all(|(k, r)| st.row(k.0, k.1).same_ptr(r));
    if ft.rows.len() != fe.rows.len() || !put_back(then_s, &ft) || !put_back(else_s, &fe) {
        return Vec::new();
    }
    vec![ft, fe]
}

/// Names bound to literals of different truth in the two branches (`has = True` / `False`):
/// testing the name later tells which branch ran.
pub(super) fn name_facts(vals: &[(intern::StrId, Option<Value>, Option<Value>, Option<Value>)], a: &State, b: &State, j: &State, func: u32) -> Vec<std::sync::Arc<CondFact>> {
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
pub(super) fn add_facts(st: &mut State, new: Vec<std::sync::Arc<CondFact>>) {
    if new.is_empty() {
        return;
    }
    let mut all: Vec<std::sync::Arc<CondFact>> = st.facts.iter().cloned().collect();
    all.extend(new);
    let n = all.len();
    if n > 32 {
        all.drain(0..n - 32);
    }
    st.facts = std::sync::Arc::new(all);
}


thread_local! {
    /// per function body: how often it tests each text (`Analyzer::tested`)
    static TESTED: std::cell::RefCell<rustc_hash::FxHashMap<usize, rustc_hash::FxHashMap<String, u32>>> = Default::default();
}

/// Count the tests of these statements by their text (`cond_key`'s), twice in a loop.
fn count_tests(body: &[ast::Stmt], in_loop: bool, out: &mut rustc_hash::FxHashMap<String, u32>) {
    let add = |e: &ast::Expr, out: &mut rustc_hash::FxHashMap<String, u32>| {
        let inner = match e {
            ast::Expr::UnaryOp(u) if u.op == ast::UnaryOp::Not => &*u.operand,
            other => other,
        };
        let text: String = frontend::text(inner).split_whitespace().collect::<Vec<_>>().join(" ");
        *out.entry(text).or_insert(0) += if in_loop { 2 } else { 1 };
    };
    for st in body {
        match st {
            ast::Stmt::If(i) => {
                add(&i.test, out);
                count_tests(&i.body, in_loop, out);
                for c in &i.elif_else_clauses {
                    if let Some(t) = &c.test {
                        add(t, out);
                    }
                    count_tests(&c.body, in_loop, out);
                }
            }
            ast::Stmt::While(w) => {
                add(&w.test, out);
                count_tests(&w.body, true, out);
                count_tests(&w.orelse, in_loop, out);
            }
            ast::Stmt::For(f) => {
                count_tests(&f.body, true, out);
                count_tests(&f.orelse, in_loop, out);
            }
            ast::Stmt::With(w) => count_tests(&w.body, in_loop, out),
            ast::Stmt::Try(t) => {
                count_tests(&t.body, in_loop, out);
                for h in &t.handlers {
                    let ast::ExceptHandler::ExceptHandler(h) = h;
                    count_tests(&h.body, in_loop, out);
                }
                count_tests(&t.orelse, in_loop, out);
                count_tests(&t.finalbody, in_loop, out);
            }
            ast::Stmt::Match(m) => m.cases.iter().for_each(|c| count_tests(&c.body, in_loop, out)),
            _ => {}
        }
    }
}

