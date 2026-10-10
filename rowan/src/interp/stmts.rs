//! Statements (port of the `stmt_*` methods of `rowan/interp.py`).

use ruff_python_ast as ast;
use rustc_hash::FxHashSet;

use crate::domain::{KeyVal, KeysOf, Ns, Row, Value};
use crate::frontend::{self, Node};
use crate::intern::{self, key_str_lit, s, KeySet};
use crate::state::{Kind, State};

use super::{contains_jump, dotted_last, is_constant, is_main_guard, Analyzer, ClassInfo, Frame, FrameKind, FuncNode, LoopCtx, Payload, LOOP_MAX_ITER, UNROLL_MAX};
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

    pub fn exec_stmt(&mut self, node: &'static ast::Stmt, state: &mut State) {
        self.work += 1;
        let dbg2 = crate::interp::debug_level() == 2;
        let was = state.reachable;
        self.exec_stmt_inner(node, state);
        if dbg2 && was && !state.reachable && !matches!(node, ast::Stmt::Return(_) | ast::Stmt::Raise(_) | ast::Stmt::Break(_) | ast::Stmt::Continue(_)) {
            let n = frontend::node(node);
            eprintln!("  unreachable after M{}:L{} in {}", n.module, n.line, self.frames.last().map(|f| f.label.clone()).unwrap_or_default());
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
            ast::Stmt::AugAssign(a) => {
                let cur = self.eval(&a.target, state);
                let rhs = self.eval(&a.value, state);
                let n = frontend::node(a);
                // `xs += ys` on a list extends it in place (other names see it)
                if a.op == ast::Operator::Add && !cur.pts.is_empty() && cur.pts.iter().all(|o| self.kind(o) == Kind::List) && !crate::interp::disabled("iadd") {
                    let m = self.method_value("list.extend", &cur);
                    self.call_value(&m, &n, None, state, Some(vec![rhs]));
                    self.bind_target(&a.target, &cur, state);
                    return;
                }
                // `d |= e` on a dict updates it in place; `s |= t` (`&=`, `-=`, `^=`) on a set too
                if !cur.pts.is_empty() && !crate::interp::disabled("iupdate") {
                    let all_dict = cur.pts.iter().all(|o| self.kind(o) == Kind::Dict);
                    let all_set = cur.pts.iter().all(|o| self.kind(o) == Kind::Set);
                    let m = match (a.op, all_dict, all_set) {
                        (ast::Operator::BitOr, true, _) => Some("dict.update"),
                        (ast::Operator::BitOr, _, true) => Some("set.update"),
                        (ast::Operator::BitAnd, _, true) => Some("set.intersection_update"),
                        (ast::Operator::Sub, _, true) => Some("set.difference_update"),
                        (ast::Operator::BitXor, _, true) => Some("set.symmetric_difference_update"),
                        _ => None,
                    };
                    if let Some(m) = m {
                        let mv = self.method_value(m, &cur);
                        self.call_value(&mv, &n, None, state, Some(vec![rhs]));
                        self.bind_target(&a.target, &cur, state);
                        return;
                    }
                }
                let res = self.binop(a.op, &cur, &rhs, &n, state);
                // `t += u` on a tensor is in place: the result must keep t's shape
                if self.torch_on && crate::models::torch::is_definite_tensor(&cur) && !self.t_inplace_fits(&cur, &res, &n, state) {
                    return;
                }
                // `d[k] += v` with an unknown key on a closed dict: the load raised unless k was
                // one of its keys, so the store adds none
                if let ast::Expr::Subscript(sub) = &*a.target {
                    if matches!(&*sub.value, ast::Expr::Name(_) | ast::Expr::Attribute(_)) && matches!(&*sub.slice, ast::Expr::Name(_) | ast::Expr::Attribute(_) | ast::Expr::Subscript(_)) && !crate::interp::disabled("augkey") {
                        let mut probe = state.copy();
                        let obj = self.eval(&sub.value, &mut probe);
                        let key = self.eval_key(&sub.slice, &mut probe);
                        let plain = !obj.pts.is_empty() && obj.kv.is_bot() && obj.pts.iter().all(|o| self.kind(o) == Kind::Dict && !self.dd_factories.contains_key(&o) && !self.counters.contains(&o) && !crate::models::pandas::is_series_obj(self, o) && { let r = state.row(o, Ns::Item); r.closed() && !r.present.is_empty() });
                        if plain && !key.kv.is_literal() {
                            for o in obj.pts.iter() {
                                for k in state.row(o, Ns::Item).present.iter() {
                                    state.set_slot(o, Ns::Item, k, res.clone(), false);
                                }
                            }
                            return;
                        }
                    }
                }
                self.bind_target(&a.target, &res, state);
            }
            ast::Stmt::Return(r) => {
                let v = match &r.value {
                    Some(e) => self.eval(e, state),
                    None => self.none_v.clone(),
                };
                if !state.reachable {
                    // the value's evaluation raised on every path (a guarded access that is
                    // definitely missing): no normal exit from here
                    return;
                }
                state.ret = Some(match &state.ret {
                    None => v,
                    Some(cur) => cur.join(&v),
                });
                state.record_exit();
                state.reachable = false;
                self.exit_count += 1;
            }
            ast::Stmt::Raise(r) => {
                let exc_v = match &r.exc {
                    Some(e) => Some(self.eval(e, state)),
                    // a bare raise re-raises what the running handler holds: not tracked
                    None => None,
                };
                // uncaught here (no enclosing handler for it): the running call raises
                let name = r.exc.as_ref().map(|e| match &**e {
                    ast::Expr::Call(c) => dotted_last(&c.func),
                    other => dotted_last(other),
                });
                // a bare `raise` re-raises what the running handler caught
                let names: Vec<String> = match &name {
                    Some(n) => vec![n.clone()],
                    None => self.handling.last().map(|h| h.iter().cloned().collect()).unwrap_or_default(),
                };
                let caught = self.guards.iter().any(|g| {
                    g.contains("*") || g.contains("Exception") || g.contains("BaseException") || names.iter().any(|n| g.contains(n) || n == "*" && !g.is_empty())
                });
                self.cur_exc = exc_v.filter(|v| v.pts.iter().any(|o| self.kind(o) == Kind::Instance || self.exc_objs.contains_key(&o) || matches!(&self.obj(o).payload, Payload::Method(q, _) if q == "fastapi.httpexc")));
                // a bare raise: the exception the handler holds
                if r.exc.is_none() && !crate::interp::disabled("reraisevalue") {
                    self.cur_exc = self.handling_values.last().cloned().filter(|v| !v.pts.is_empty());
                }
                let recorded = state.reachable && !crate::interp::disabled("raisestate") && self.record_raise_state(state);
                self.cur_exc = None;
                if !caught {
                    if crate::interp::debug_level() == 2 {
                        eprintln!("  raise statement at L{} in {}", frontend::node(r).line, self.frames.last().map(|f| f.label.clone()).unwrap_or_default());
                    }
                    if let Some(t) = self.raise_stack.last_mut() {
                        *t = true;
                    }
                    // the enclosing try already has this state for its handlers / finally
                    if !recorded {
                        if let Some(m) = self.may_raise.last_mut() {
                            *m = true;
                        }
                    }
                }
                state.reachable = false;
            }
            ast::Stmt::Assert(a) => {
                self.eval(&a.test, state);
                self.narrow(&a.test, true, state);
            }
            ast::Stmt::Delete(d) => self.stmt_delete(d, state),
            ast::Stmt::Import(i) => {
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
            ast::Stmt::ImportFrom(i) => {
                let n = frontend::node(i);
                // relative imports resolve against the current module's package
                let modname = if i.level > 0 {
                    let pkg = self.module_info(self.frame().module).pkg.clone();
                    let mut base: Vec<&str> = pkg.split('.').filter(|x| !x.is_empty()).collect();
                    for _ in 1..i.level {
                        base.pop();
                    }
                    let mut full = base.join(".");
                    if let Some(m) = &i.module {
                        if !full.is_empty() {
                            full.push('.');
                        }
                        full.push_str(m.id.as_str());
                    }
                    full
                } else {
                    i.module.as_ref().map(|m| m.id.to_string()).unwrap_or_default()
                };
                let mod_v = if modname.is_empty() { Value::new() } else { self.import_module(&modname, &n, state) };
                for a in &i.names {
                    if a.name.id.as_str() == "*" {
                        continue;
                    }
                    let aname = a.name.id.as_str();
                    let mut v = Value::new();
                    if let Some(mo) = mod_v.single().filter(|o| self.kind(*o) == Kind::Module) {
                        // a name the module does not define may be one of its submodules
                        if state.slot_opt(mo, Ns::Attr, key_str_lit(aname)).is_none() {
                            let sub = self.import_module(&format!("{}.{}", modname, aname), &n, state);
                            if sub.single().map(|o| self.kind(o) == Kind::Module).unwrap_or(false) {
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
            ast::Stmt::If(i) => self.stmt_if(&i.test, &i.body, &i.elif_else_clauses, state),
            ast::Stmt::While(w) => self.stmt_while(w, state),
            ast::Stmt::For(f) => self.stmt_for(f, state),
            ast::Stmt::Break(_) => {
                self.exit_count += 1;
                if let Some(ctx) = self.loop_stack.last_mut() {
                    let mut st = state.copy();
                    ctx.breaks = Some(match ctx.breaks.take() {
                        None => st,
                        Some(mut b) => b.join(&mut st),
                    });
                }
                state.reachable = false;
            }
            ast::Stmt::Continue(_) => {
                self.exit_count += 1;
                if let Some(ctx) = self.loop_stack.last_mut() {
                    let mut st = state.copy();
                    ctx.continues = Some(match ctx.continues.take() {
                        None => st,
                        Some(mut b) => b.join(&mut st),
                    });
                }
                state.reachable = false;
            }
            ast::Stmt::Try(t) => self.stmt_try(t, state),
            ast::Stmt::With(w) if self.with_suppress(w, state) => {}
            ast::Stmt::With(w) if self.with_contextmanager(w, state) => {}
            ast::Stmt::With(w) => {
                let mut managers: Vec<(Value, Node)> = Vec::new();
                for item in &w.items {
                    let v = self.eval(&item.context_expr, state);
                    let n = frontend::node(&item.context_expr);
                    let mut entered = self.call_method_if_defined(&v, "__enter__", &n, state);
                    if entered.is_none() {
                        entered = crate::models::csvio::enter_value(self, state, &v);
                    }
                    if entered.is_none() {
                        entered = Some(if v.pts.iter().any(|o| self.kind(o).is_record()) { v.clone() } else { Value::data(&format!("with@L{}", frontend::node(w).line)) });
                    }
                    if let Some(ov) = &item.optional_vars {
                        self.bind_target(ov, &entered.unwrap(), state);
                    }
                    managers.push((v, n));
                }
                // `redirect_stdout(buf)`: prints in the body go to buf
                let redirects = crate::models::csvio::redirect_targets(self, state, &managers.iter().map(|m| m.0.clone()).collect::<Vec<_>>());
                let n_redirects = redirects.len();
                self.stdout_redirect.extend(redirects);
                // a manager whose `__exit__` returns True swallows the body's exception: the
                // body is a try whose catch-all handler continues after the with
                let swallows = managers.iter().any(|(v, _)| self.exit_swallows(v, state));
                if swallows {
                    let mut pre = state.copy();
                    self.guards.push(FxHashSet::from_iter(["*".to_string()]));
                    self.may_raise.push(false);
                    self.raise_states.push(None);
                    self.raise_values.push(None);
                    self.try_marks.push((self.frames.len(), self.env_stack.len(), pre.copy()));
                    self.try_sum_depth.push(self.sum_stack.len());
                    let exits_before = self.exit_count;
                    self.exec_block(&w.body, state);
                    self.guards.pop();
                    self.try_marks.pop();
                    self.try_sum_depth.pop();
                    let flagged = self.may_raise.pop().unwrap_or(false);
                    let raised_at = self.raise_states.pop().flatten();
                    self.raise_values.pop();
                    let died = !state.reachable && self.exit_count == exits_before;
                    let generic = flagged || (died && raised_at.is_none());
                    let mut after = state.copy();
                    if generic {
                        after = pre.join(&mut after);
                    }
                    if let Some(mut r) = raised_at {
                        after = if after.reachable { after.join(&mut r) } else { r };
                    }
                    state.replace_with(after);
                } else {
                    self.exec_block(&w.body, state);
                }
                for _ in 0..n_redirects {
                    self.stdout_redirect.pop();
                }
                // the managers' __exit__ runs on the way out (innermost first)
                if state.reachable {
                    for (v, n) in managers.iter().rev() {
                        // the body is analysed as not raising: __exit__(None, None, None)
                        let none = self.none_v.clone();
                        self.call_method_with(v, "__exit__", vec![none.clone(), none.clone(), none], n, state);
                        // an ExitStack: its callbacks, last registered first; closing(x): x.close()
                        crate::models::csvio::exitstack_close(self, state, v, n);
                        crate::models::csvio::closing_exit(self, state, v, n);
                    }
                }
            }
            ast::Stmt::FunctionDef(f) => {
                let v = self.make_function(FuncNode::Def(f), state, None);
                let v = self.apply_decorators(&f.decorator_list, frontend::node(f), v, state);
                self.store_name(s(f.name.id.as_str()), v, state);
            }
            ast::Stmt::ClassDef(c) => self.stmt_classdef(c, state),
            ast::Stmt::Match(m) => {
                let subj = self.eval(&m.subject, state);
                let mut outs: Vec<State> = Vec::new();
                self.cond_depth += 1;
                self.branch_depth += 1;
                // cases in order: a case that cannot match is skipped, one that surely matches
                // (with no guard, or a guard that is true) ends the match
                let mut matched = false;
                for case in &m.cases {
                    let mut cs = state.copy();
                    let decided = if crate::interp::disabled("matchpat") { None } else { self.match_pattern(&case.pattern, &subj, &mut cs) };
                    if decided == Some(false) || !cs.reachable {
                        continue;
                    }
                    let mut sure = decided == Some(true);
                    if let Some(g) = &case.guard {
                        let gv = self.eval(g, &mut cs);
                        match self.truthiness(&gv, &cs) {
                            Some(false) => continue,
                            Some(true) => {}
                            None => sure = false,
                        }
                        self.narrow(g, true, &mut cs);
                    }
                    self.exec_block(&case.body, &mut cs);
                    outs.push(cs);
                    if sure {
                        matched = true;
                        break;
                    }
                }
                self.branch_depth -= 1;
                self.cond_depth -= 1;
                if !matched {
                    outs.push(state.copy());
                }
                let mut out = outs.remove(0);
                for mut o in outs {
                    out = out.join(&mut o);
                }
                state.replace_with(out);
            }
            ast::Stmt::IpyEscapeCommand(_) => {
                let n = frontend::node(node);
                self.note(format!("unsupported statement IpyEscapeCommand at L{}", n.line));
            }
        }
    }

    /// `with contextlib.suppress(E, …): body`: `try: body / except (E, …): pass`. False: not
    /// that case.
    fn with_suppress(&mut self, w: &'static ast::StmtWith, state: &mut State) -> bool {
        if w.items.len() != 1 || crate::interp::disabled("suppress") {
            return false;
        }
        let ast::Expr::Call(call) = &w.items[0].context_expr else { return false };
        if !matches!(&*call.func, ast::Expr::Name(_) | ast::Expr::Attribute(_)) || !call.arguments.keywords.is_empty() {
            return false;
        }
        let mut probe = state.copy();
        let fv = self.eval(&call.func, &mut probe);
        let is_suppress = !fv.pts.is_empty() && fv.pts.iter().all(|o| self.kind(o) == Kind::External && self.external_qual(o) == "contextlib.suppress");
        if !is_suppress {
            return false;
        }
        let names: FxHashSet<String> = call.arguments.args.iter().map(dotted_last).collect();
        self.exec_handled(&w.body, names, state);
        true
    }

    /// `body` under a handler that catches `names` and does nothing (`suppress`).
    pub fn exec_handled(&mut self, body: &'static [ast::Stmt], names: FxHashSet<String>, state: &mut State) {
        let caught = expand_caught(names);
        let mut pre = state.copy();
        self.guards.push(caught);
        self.may_raise.push(false);
        self.raise_states.push(None);
        self.raise_values.push(None);
        self.try_marks.push((self.frames.len(), self.env_stack.len(), pre.copy()));
        self.try_sum_depth.push(self.sum_stack.len());
        self.cond_depth += 1;
        self.branch_depth += 1;
        let exits_before = self.exit_count;
        self.exec_block(body, state);
        self.guards.pop();
        self.try_marks.pop();
        self.try_sum_depth.pop();
        let flagged = self.may_raise.pop().unwrap_or(false);
        let raised_at = self.raise_states.pop().flatten();
        self.raise_values.pop();
        self.branch_depth -= 1;
        self.cond_depth -= 1;
        let died = !state.reachable && self.exit_count == exits_before;
        let generic = flagged || (died && raised_at.is_none());
        let mut body_end = state.copy();
        let mut handled = if generic { pre.join(&mut body_end) } else { State::bottom() };
        if let Some(mut r) = raised_at {
            handled = if handled.reachable { handled.join(&mut r) } else { r };
        }
        let out = match (body_end.reachable, handled.reachable) {
            (true, true) => body_end.join(&mut handled),
            (true, false) => body_end,
            (false, _) => {
                // (the body's returns stay recorded in `exits`)
                handled.exits = body_end.exits.clone().or(handled.exits.clone());
                handled.ret = body_end.ret.clone().or(handled.ret.clone());
                handled
            }
        };
        state.replace_with(out);
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
        if self.kind(f) != Kind::Function || !self.func_info(f).is_contextmanager.get() {
            return false;
        }
        let prev = self.cm_hook.replace(super::CmHook { body: &w.body, target: item.optional_vars.as_deref(), depth: self.frames.len(), env_after: None, ran: false, then: None, body_died: false });
        self.force_tab_once = true;
        let n = frontend::node(&item.context_expr);
        let fv = self.eval(&call.func, state);
        self.call_value(&fv, &n, Some(call), state, None);
        self.force_tab_once = false;
        let h = std::mem::replace(&mut self.cm_hook, prev);
        match h {
            Some(h) if h.ran => {
                if let Some(env) = h.env_after {
                    state.env = env;
                }
                if !state.reachable && !h.body_died && !crate::interp::disabled("cmdied") {
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
        let record_events = |an: &mut Analyzer, v: &Value, kind: &'static str| {
            for o in v.pts.iter() {
                if an.kind(o) == Kind::Ph {
                    an.record_event(kind, o);
                }
            }
            if let Some(ph) = v.kv.as_param() {
                an.record_event(kind, ph);
            }
        };
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
                if !subj.pts.is_empty() && subj.pts.iter().all(|o| self.kind(o).is_container() || self.kind(o) == Kind::Instance && self.instance_class(o).map(|c| self.class_lookup(c, "__eq__").is_none()).unwrap_or(false)) {
                    // containers / plain instances never equal a literal
                    return Some(false);
                }
                record_events(self, subj, "truth");
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
                if !subj.pts.is_empty() && subj.pts.iter().all(|o| self.kind(o).is_container() || self.kind(o) == Kind::Instance) {
                    return Some(false);
                }
                record_events(self, subj, "truth");
                None
            }
            ast::Pattern::MatchMapping(mm) => {
                if subj.pts.is_empty() {
                    if subj.kv.is_literal() {
                        return Some(false);
                    }
                    return None;
                }
                let recs: Vec<u32> = subj.pts.iter().filter(|o| self.kind(*o) == Kind::Dict).collect();
                if recs.len() != subj.pts.len() {
                    if subj.pts.iter().all(|o| matches!(self.kind(o), Kind::List | Kind::Tuple | Kind::Set)) {
                        return Some(false);
                    }
                    record_events(self, subj, "narrow");
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
                    } else if rows.iter().all(|r| r.closed() && !r.present.contains(k) && !r.symbolic()) {
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
                record_events(self, subj, "isinstance");
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
                        if self.kind(o) != Kind::List {
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
                                self.set_llen(o, state, None);
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
        if is_main_guard(test) {
            self.exec_block(body, state);
            return;
        }
        let tv = self.eval(test, state);
        let mut then_s = state.copy();
        let mut else_s = state.copy();
        let decided = self.truth_of(&tv, state, &frontend::node(test));
        match decided {
            Some(true) => else_s.reachable = false,
            Some(false) => then_s.reachable = false,
            None => {}
        }
        let cond = if decided.is_none() && !crate::interp::disabled("condfacts") { self.cond_key(test) } else { None };
        if let Some((key, pos, _)) = &cond {
            // an earlier join on the same test: each branch gets back the rows it had then
            self.apply_cond_facts(*key, *pos, &mut then_s);
            self.apply_cond_facts(*key, !*pos, &mut else_s);
        }
        self.narrow(test, true, &mut then_s);
        self.narrow(test, false, &mut else_s);
        // a decided test leaves one branch: straight-line code, not a conditional (appends
        // stay positional, requirements stay "always")
        let conditional = decided.is_none();
        if conditional {
            self.cond_depth += 1;
            self.branch_depth += 1;
        }
        if then_s.reachable {
            self.exec_block(body, &mut then_s);
        }
        if else_s.reachable {
            self.exec_orelse(rest, &mut else_s);
        }
        if conditional {
            self.branch_depth -= 1;
            self.cond_depth -= 1;
        }
        // `if k not in d: raise …`: the input must hold k (else the program raises)
        if conditional && !crate::interp::disabled("raiserequire") {
            self.require_by_raise(test, body, rest, state, &mut then_s, &mut else_s);
        }
        let mut j = then_s.join(&mut else_s);
        if then_s.reachable && else_s.reachable && conditional && !crate::interp::disabled("condfacts") {
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
                let mutable_seq = deps.iter().any(|(_, v)| v.pts.iter().any(|o| matches!(self.kind(o), Kind::List | Kind::Set)));
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
    fn require_by_raise(&mut self, test: &'static ast::Expr, body: &'static [ast::Stmt], rest: &'static [ast::ElifElseClause], pre: &mut State, then_s: &mut State, else_s: &mut State) {
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
        let kv = self.eval(&c.left, pre);
        let Some(k) = kv.kv.as_lit().filter(|_| kv.pts.is_empty()) else { return };
        let target = self.eval(&c.comparators[0], pre);
        let mut recs: Vec<u32> = Vec::new();
        for o in target.pts.iter() {
            match &self.obj(o).payload {
                Payload::View(super::ViewKind::Keys, inner) => recs.extend(inner.pts.iter().filter(|x| matches!(self.kind(*x), Kind::Dict | Kind::Frame | Kind::Unknown))),
                _ if matches!(self.kind(o), Kind::Dict | Kind::Frame | Kind::Unknown) => recs.push(o),
                _ => {}
            }
        }
        let site = format!("L{}", frontend::node(test).line);
        for o in recs {
            let row = pre.row(o, Ns::Item);
            let cls = crate::domain::classify(&row, k);
            if cls.tier == crate::domain::Tier::Input {
                for st in [&mut *then_s, &mut *else_s] {
                    if st.reachable {
                        self.require_providers(&cls, &row, &site, true, st);
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
    fn name_value(&self, st: &State, n: intern::StrId) -> Option<Value> {
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

    /// Rows a branch of an earlier join on `key` had, put back where unchanged since.
    fn apply_cond_facts(&mut self, key: intern::StrId, truth: bool, st: &mut State) {
        if !st.reachable {
            return;
        }
        let facts = st.facts.clone();
        for f in facts.iter().filter(|f| f.cond == key && f.truth == truth) {
            if !f.deps.iter().all(|(n, v)| self.dep_value(st, *n) == *v) {
                continue;
            }
            if !f.guards.iter().all(|((o, ns), r)| st.row(*o, *ns) == *r) {
                continue;
            }
            for (k, branch, joined) in &f.rows {
                if st.row(k.0, k.1) == *joined {
                    st.set_row(k.0, k.1, branch.clone());
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
        if !is_constant(&w.test) && !crate::interp::disabled("whileunroll") {
            let mut idx = 0usize;
            loop {
                if !state.reachable {
                    break;
                }
                let mut probe = state.copy();
                let tv = self.eval(&w.test, &mut probe);
                let decided = self.truth_of(&tv, &mut probe, &frontend::node(&*w.test));
                match decided {
                    Some(false) => {
                        state.replace_with(probe);
                        self.narrow(&w.test, false, state);
                        if !w.orelse.is_empty() {
                            self.exec_block(&w.orelse, state);
                        }
                        if let Some(mut b) = breaks {
                            let j = if state.reachable { state.join(&mut b) } else { b };
                            state.replace_with(j);
                        }
                        return;
                    }
                    Some(true) if idx < UNROLL_MAX => {
                        state.replace_with(probe);
                        self.narrow(&w.test, true, state);
                        self.frame_mut().unroll.push(1000 + idx);
                        self.loop_stack.push(LoopCtx::default());
                        self.exec_block(&w.body, state);
                        let ctx = self.loop_stack.pop().unwrap();
                        self.frame_mut().unroll.pop();
                        if let Some(mut c) = ctx.continues {
                            let j = if state.reachable { state.join(&mut c) } else { c };
                            state.replace_with(j);
                        }
                        if let Some(mut b) = ctx.breaks {
                            breaks = Some(match breaks.take() {
                                None => b,
                                Some(mut e) => e.join(&mut b),
                            });
                        }
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
        if let Some(mut b) = breaks {
            let j = if state.reachable { state.join(&mut b) } else { b };
            state.replace_with(j);
        }
    }

    fn stmt_while_loop(&mut self, w: &'static ast::StmtWhile, state: &mut State) {
        let mut pre = state.copy();
        let mut head = state.copy();
        let mut exit_acc: Option<State> = None;
        self.frame_mut().loop_depth += 1;
        self.cond_depth += 1;
        for _ in 0..LOOP_MAX_ITER {
            let mut body_s = head.copy();
            let mark = self.objects.len() as u32;
            self.recent_stack.push(Vec::new());
            let tv = self.eval(&w.test, &mut body_s);
            // a test decided at the loop head prunes: false means no iteration from here
            if self.truthiness(&tv, &body_s) == Some(false) {
                body_s.reachable = false;
            }
            self.narrow(&w.test, true, &mut body_s);
            self.loop_stack.push(LoopCtx::default());
            if body_s.reachable {
                self.exec_block(&w.body, &mut body_s);
            }
            let ctx = self.loop_stack.pop().unwrap();
            if let Some(mut b) = ctx.breaks {
                exit_acc = Some(match exit_acc.take() {
                    None => b,
                    Some(mut e) => e.join(&mut b),
                });
            }
            let mut back = match ctx.continues {
                None => body_s,
                Some(mut c) => body_s.join(&mut c),
            };
            let recs = self.recent_stack.pop().unwrap_or_default();
            self.rename_recent(&mut back, &head, recs, mark);
            let new_head = pre.join_loop(&mut back);
            if new_head.same(&head) {
                break;
            }
            head = new_head;
        }
        self.cond_depth -= 1;
        self.frame_mut().loop_depth -= 1;
        let mut exit_s = head.copy();
        // a test that is true at the head never falls through (the loop leaves by break)
        let tv = self.eval(&w.test, &mut exit_s);
        if self.truthiness(&tv, &exit_s) == Some(true) {
            exit_s.reachable = false;
        }
        self.narrow(&w.test, false, &mut exit_s);
        if !w.orelse.is_empty() && exit_s.reachable {
            self.exec_block(&w.orelse, &mut exit_s);
        }
        if let Some(mut e) = exit_acc {
            exit_s = exit_s.join(&mut e);
        }
        state.replace_with(exit_s);
    }

    fn stmt_for(&mut self, f: &'static ast::StmtFor, state: &mut State) {
        let itv = self.eval(&f.iter, state);
        let iter_node = frontend::node(&*f.iter);
        let it = self.iterate(&itv, &iter_node, state);
        // a one-shot iterator is used up by the loop (a break leaves it partly consumed: the
        // later iterations see the rest, which is not tracked)
        self.exhaust(&itv, state);
        let has_jump = contains_jump(&f.body);
        if let Some(items) = &it.items {
            if items.len() <= UNROLL_MAX {
                // unrolled: `continue` rejoins at the next iteration, `break` at the exit
                let mut breaks: Option<State> = None;
                for (idx, item) in items.iter().enumerate() {
                    if !state.reachable {
                        break;
                    }
                    self.frame_mut().unroll.push(idx);
                    self.bind_iter_target(&f.target, &Elem::One(item.clone()), state);
                    if has_jump {
                        self.loop_stack.push(LoopCtx::default());
                        self.exec_block(&f.body, state);
                        let ctx = self.loop_stack.pop().unwrap();
                        if let Some(mut c) = ctx.continues {
                            let j = state.join(&mut c);
                            state.replace_with(j);
                        }
                        if let Some(mut b) = ctx.breaks {
                            breaks = Some(match breaks.take() {
                                None => b,
                                Some(mut e) => e.join(&mut b),
                            });
                        }
                    } else {
                        self.exec_block(&f.body, state);
                    }
                    self.frame_mut().unroll.pop();
                }
                if !f.orelse.is_empty() && state.reachable {
                    self.exec_block(&f.orelse, state);
                }
                if let Some(mut b) = breaks {
                    let j = state.join(&mut b);
                    state.replace_with(j);
                }
                return;
            }
        }
        let mut pre = state.copy();
        // dry run for exhaustive key iteration (§8.3)
        let mut exh_ops: Vec<(u32, Ns, Row, KeySet, intern::LensId)> = Vec::new();
        let key_part = it.elem.key_part();
        if it.exhaustive && !has_jump {
            if let KeyVal::KeysOf(kk) = &key_part.kv {
                let saved_log = self.exh_log.take();
                self.exh_log = Some(Vec::new());
                let saved_verdicts = self.verdicts.len();
                let mut dry = state.copy();
                self.recent_stack.push(Vec::new());
                self.frame_mut().loop_depth += 1;
                self.cond_depth += 1;
                let ekv = KeyVal::KeysOf(KeysOf { objs: kk.objs.clone(), ns: kk.ns, exclude: kk.exclude.clone(), chain: kk.chain, exhaustive_depth: self.cond_depth });
                let dry_elem = match &it.elem {
                    Elem::Pair(_, v) => Elem::Pair(Value::with_kv(key_part.pts.clone(), ekv), v.clone()),
                    Elem::One(_) => Elem::One(Value::with_kv(key_part.pts.clone(), ekv)),
                };
                self.bind_iter_target(&f.target, &dry_elem, &mut dry);
                self.exec_block(&f.body, &mut dry);
                self.cond_depth -= 1;
                self.frame_mut().loop_depth -= 1;
                self.recent_stack.pop();
                for v in self.verdicts[saved_verdicts..].iter() {
                    self.verdict_keys.remove(&v.dedupe_key());
                }
                self.verdicts.truncate(saved_verdicts);
                exh_ops = self.exh_log.take().unwrap_or_default();
                self.exh_log = saved_log;
            }
        }
        let mut head = state.copy();
        let mut exit_acc: Option<State> = None;
        self.frame_mut().loop_depth += 1;
        self.cond_depth += 1;
        let mut back = State::bottom();
        for _ in 0..LOOP_MAX_ITER {
            let mut body_s = head.copy();
            let mark = self.objects.len() as u32;
            self.recent_stack.push(Vec::new());
            self.loop_stack.push(LoopCtx::default());
            self.bind_iter_target(&f.target, &it.elem, &mut body_s);
            self.exec_block(&f.body, &mut body_s);
            let ctx = self.loop_stack.pop().unwrap();
            if let Some(mut b) = ctx.breaks {
                exit_acc = Some(match exit_acc.take() {
                    None => b,
                    Some(mut e) => e.join(&mut b),
                });
            }
            back = match ctx.continues {
                None => body_s,
                Some(mut c) => body_s.join(&mut c),
            };
            let recs = self.recent_stack.pop().unwrap_or_default();
            self.rename_recent(&mut back, &head, recs, mark);
            let new_head = pre.join_loop(&mut back);
            if new_head.same(&head) {
                break;
            }
            head = new_head;
        }
        self.cond_depth -= 1;
        self.frame_mut().loop_depth -= 1;
        let mut exit_s = head.copy();
        let elem_v = it.elem.value_part();
        self.promote_elem_reqs(&mut exit_s, &back, &elem_v, state);
        self.promote_elem_narrowing(&mut exit_s, &back, &elem_v);
        if exit_acc.is_none() && back.reachable {
            // every element of the iterable went through the body: its row after the loop is
            // the body's exit row, not the join with "not processed yet"
            for oid in elem_v.pts.iter() {
                if self.is_record_like(oid) && self.obj(oid).single {
                    for ns in [Ns::Item, Ns::Attr] {
                        if back.has_row(oid, ns) {
                            exit_s.set_row(oid, ns, back.row(oid, ns));
                        }
                    }
                }
            }
        }
        for (oid, ns, srcrow, kill, chain) in exh_ops {
            let r = exit_s.row(oid, ns).copy_from(&srcrow, true, &kill, chain);
            exit_s.set_row(oid, ns, r);
        }
        if !f.orelse.is_empty() && exit_s.reachable {
            self.exec_block(&f.orelse, &mut exit_s);
        }
        if let Some(mut e) = exit_acc {
            exit_s = exit_s.join(&mut e);
        }
        // the loop ran at least once exactly when its iterable was non-empty: a later test of
        // that (`if xs:`, `if n > 0:` for `range(n)`) gets the rows the iterations left
        if f.orelse.is_empty() && !has_jump && back.reachable && pre.reachable && exit_s.reachable && !crate::interp::disabled("loopfacts") {
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
                if deps[0].1.pts.iter().any(|o| matches!(self.kind(o), Kind::List | Kind::Set)) {
                    continue;
                }
                new.extend(cond_facts_for(s(&format!("{}@{}", text, func)), true, deps, &back, &pre, &exit_s));
            }
            add_facts(&mut exit_s, new);
        }
        state.replace_with(exit_s);
    }

    /// Does the manager's class define `__exit__` as returning True (a known literal)?
    fn exit_swallows(&mut self, v: &Value, state: &mut State) -> bool {
        for oid in v.pts.iter() {
            if self.kind(oid) != Kind::Instance {
                continue;
            }
            let Some(cls) = self.instance_class(oid) else { continue };
            let Some(m) = self.class_lookup(cls, "__exit__") else { continue };
            let fv = self.cls_slots.get(&(m, Ns::Attr, key_str_lit("__exit__"))).cloned().unwrap_or_default();
            for f in fv.pts.iter() {
                if self.kind(f) != Kind::Function {
                    continue;
                }
                if let Some(summ) = self.summarise(f, state) {
                    if summ.ret.pts.is_empty() {
                        if let Some(k) = summ.ret.kv.as_lit() {
                            if intern::with_key(k, |kk| matches!(kk, intern::Key::Bool(true))) {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }

    fn stmt_try(&mut self, t: &'static ast::StmtTry, state: &mut State) {
        let mut caught: FxHashSet<String> = FxHashSet::default();
        for h in &t.handlers {
            let ast::ExceptHandler::ExceptHandler(h) = h;
            match &h.type_ {
                None => {
                    caught.insert("*".into());
                }
                Some(ty) => {
                    if let ast::Expr::Tuple(tup) = &**ty {
                        for e in &tup.elts {
                            caught.insert(dotted_last(e));
                        }
                    } else {
                        caught.insert(dotted_last(ty));
                    }
                }
            }
        }
        // a handler for a class catches its subclasses too
        let mut caught = expand_caught(caught);
        // handlers that all end by re-raising: the body's reads are still required
        let reraise = !t.handlers.is_empty() && t.handlers.iter().all(|h| {
            let ast::ExceptHandler::ExceptHandler(h) = h;
            matches!(h.body.last(), Some(ast::Stmt::Raise(r)) if r.exc.is_none())
        });
        if reraise && !crate::interp::disabled("reraisereq") {
            caught.insert("!reraise".into());
        }
        // returns inside the try are collected apart from the earlier ones: the finally runs
        // on their way out
        let has_finally = !t.finalbody.is_empty() && !crate::interp::disabled("finally");
        let outer_exits = if has_finally { state.exits.take() } else { None };
        let raise_before = self.raise_stack.last().copied().unwrap_or(false);
        let mut pre = state.copy();
        self.guards.push(caught);
        self.may_raise.push(false);
        self.raise_states.push(None);
        self.raise_values.push(None);
        self.try_marks.push((self.frames.len(), self.env_stack.len(), pre.copy()));
        self.try_sum_depth.push(self.sum_stack.len());
        self.cond_depth += 1;
        self.branch_depth += 1;
        let exits_before = self.exit_count;
        let fl_mark = self.flag_reqs.len();
        let fl_depth = self.may_raise.len();
        self.exec_block(&t.body, state);
        self.guards.pop();
        self.try_marks.pop();
        self.try_sum_depth.pop();
        // the requirements known where the body first may have raised anywhere
        let mut flag_reqs = self.flag_reqs.drain(fl_mark..).find(|(d, _)| *d == fl_depth).map(|(_, r)| r);
        // a body that ended because it returned (or broke out) did not raise
        let flagged = self.may_raise.pop().unwrap_or(false);
        let mut raised_at = self.raise_states.pop().flatten();
        let raised_exc = self.raise_values.pop().flatten();
        let died = !state.reachable && self.exit_count == exits_before;
        let generic = flagged || (died && raised_at.is_none());
        let body_may_raise = generic || raised_at.is_some();
        // where an exception leaves the body (for a finally without handlers)
        let mut finally_escape: Option<State> = None;
        if has_finally && t.handlers.is_empty() && body_may_raise && (contains_return(&t.finalbody) || contains_jump(&t.finalbody)) {
            let mut e = if generic { pre.copy() } else { State::bottom() };
            if let Some(r) = raised_at.as_mut() {
                let mut r2 = r.copy();
                e = if e.reachable { e.join(&mut r2) } else { r2 };
            }
            finally_escape = Some(e);
        }
        if crate::interp::debug_level() == 2 {
            eprintln!("  try at L{}: may_raise={} at_access={} reachable={} exited={}", frontend::node(t).line, flagged, raised_at.is_some(), state.reachable, self.exit_count != exits_before);
        }
        let mut body_end = state.copy();
        // the handler starts from the states at the accesses that may raise; anything else
        // that may raise (unknown calls, raise statements, callees) can happen anywhere
        let mut handler_entry: Option<State> = if !t.handlers.is_empty() {
            let mut e = if generic {
                let mut g = pre.copy();
                if let Some(r) = flag_reqs.as_mut() {
                    g.reqs = r.copy();
                }
                g.join(&mut body_end)
            } else {
                State::bottom()
            };
            if let Some(mut r) = raised_at {
                e = if e.reachable { e.join(&mut r) } else { r };
            }
            Some(e)
        } else {
            None
        };
        if !body_may_raise && !t.handlers.is_empty() {
            handler_entry = Some(State::bottom());
        }
        let mut results: Vec<State> = Vec::new();
        if !t.orelse.is_empty() && body_end.reachable {
            self.exec_block(&t.orelse, &mut body_end);
        }
        results.push(body_end);
        for h in &t.handlers {
            let ast::ExceptHandler::ExceptHandler(h) = h;
            let mut hs = handler_entry.as_mut().unwrap().copy();
            if !hs.reachable {
                continue;
            }
            // the exceptions raised at the recorded points (anything else: unknown)
            let hn = frontend::node(h);
            let unknown = Value::data(&format!("exc@L{}", hn.line));
            let ev = match &raised_exc {
                Some(v) if !generic => v.clone(),
                Some(v) => v.join(&unknown),
                None => unknown,
            };
            if let Some(name) = &h.name {
                self.store_name(s(name.id.as_str()), ev.clone(), &mut hs);
            }
            let mut names: FxHashSet<String> = FxHashSet::default();
            match &h.type_ {
                None => {
                    names.insert("*".into());
                }
                Some(ty) => match &**ty {
                    ast::Expr::Tuple(tup) => names.extend(tup.elts.iter().map(dotted_last)),
                    other => {
                        names.insert(dotted_last(other));
                    }
                },
            }
            self.handling.push(names);
            self.handling_values.push(ev);
            self.exec_block(&h.body, &mut hs);
            self.handling_values.pop();
            self.handling.pop();
            results.push(hs);
        }
        self.branch_depth -= 1;
        self.cond_depth -= 1;
        // two ways out of the try (the body and one handler) that set a flag differently: a
        // later test of the flag tells which ran
        let two: Vec<usize> = (0..results.len()).filter(|i| results[*i].reachable).collect();
        let pair = if two.len() == 2 && !crate::interp::disabled("condfacts") { Some((results[two[0]].copy(), results[two[1]].copy())) } else { None };
        let mut out = results.remove(0);
        for mut r in results {
            out = out.join(&mut r);
        }
        if let Some((a, b)) = pair {
            let mut assigned: Vec<intern::StrId> = Vec::new();
            for st in t.body.iter().chain(t.orelse.iter()) {
                collect_assigned(st, &mut assigned);
            }
            for h in &t.handlers {
                let ast::ExceptHandler::ExceptHandler(h) = h;
                h.body.iter().for_each(|x| collect_assigned(x, &mut assigned));
            }
            let vals: Vec<(intern::StrId, Option<Value>, Option<Value>, Option<Value>)> = assigned.iter().map(|n| (*n, self.name_value(&a, *n), self.name_value(&b, *n), self.name_value(&out, *n))).collect();
            let new = name_facts(&vals, &a, &b, &out, self.frame().func.unwrap_or(0));
            add_facts(&mut out, new);
        }
        if has_finally {
            let inner_exits = out.exits.take();
            let mut ret_acc = out.ret.take();
            let mut exits_acc: Option<State> = None;
            let add_exits = |e: Option<std::sync::Arc<State>>, acc: &mut Option<State>| {
                if let Some(e) = e {
                    let mut s2 = e.copy_ref();
                    *acc = Some(match acc.take() {
                        None => s2,
                        Some(mut a) => a.join(&mut s2),
                    });
                }
            };
            // the normal way out
            if out.reachable {
                self.exec_block(&t.finalbody, &mut out);
            }
            add_exits(out.exits.take(), &mut exits_acc);
            if let Some(r) = out.ret.take() {
                ret_acc = Some(match ret_acc { None => r, Some(a) => a.join(&r) });
            }
            // returns made inside the try: the finally runs before the function returns
            let mut ways: Vec<State> = Vec::new();
            if let Some(ie) = inner_exits {
                ways.push(ie.copy_ref());
            }
            // an exception leaving the try (no handler here) that a `return` in the finally
            // swallows
            let n_exit_ways = ways.len();
            // (a `break` / `continue` in the finally swallows it as well)
            if t.handlers.is_empty() && (contains_return(&t.finalbody) || contains_jump(&t.finalbody)) && body_may_raise {
                if let Some(esc) = finally_escape.take() {
                    ways.push(esc);
                }
            }
            for (wi, mut w) in ways.into_iter().enumerate() {
                let is_escape = wi >= n_exit_ways;
                w.reachable = true;
                w.ret = None;
                w.exits = None;
                self.exec_block(&t.finalbody, &mut w);
                if is_escape {
                    if !w.reachable && !flagged {
                        // the finally returned on every path: the exception does not leave
                        if let Some(r) = self.raise_stack.last_mut() {
                            *r = raise_before;
                        }
                    }
                    // a finally that completes re-raises: nothing continues from here
                    w.reachable = false;
                } else if w.reachable {
                    w.record_exit();
                }
                add_exits(w.exits.take(), &mut exits_acc);
                if let Some(r) = w.ret.take() {
                    ret_acc = Some(match ret_acc { None => r, Some(a) => a.join(&r) });
                }
            }
            add_exits(outer_exits, &mut exits_acc);
            out.exits = exits_acc.map(std::sync::Arc::new);
            out.ret = ret_acc;
        } else if !t.finalbody.is_empty() {
            if !out.reachable {
                out = pre.copy();
            }
            self.exec_block(&t.finalbody, &mut out);
        }
        state.replace_with(out);
    }

    fn stmt_classdef(&mut self, c: &'static ast::StmtClassDef, state: &mut State) {
        let fr_module = self.frame().module;
        let fr_ctx = self.frame().ctx.clone();
        let fr_single = self.frame().single;
        let fr_cells = self.frame().cells.clone();
        let mut bases: Vec<u32> = Vec::new();
        let mut open_base = false;
        let mut typed_dict = false;
        let mut named_tuple = false;
        let mut is_enum = false;
        let mut ek = crate::interp::EnumKind::default();
        let mut str_mixin = false;
        let mut dict_base = false;
        let mut pydantic = false;
        let mut nn_module = false;
        if let Some(args) = &c.arguments {
            for b in args.args.iter() {
                // `RootModel[list[int]]`, `Generic[T]`: the subscripted class decides
                let b: &ast::Expr = match b {
                    ast::Expr::Subscript(sb) => &sb.value,
                    other => other,
                };
                let bv = self.eval(b, state);
                for oid in bv.pts.iter() {
                    match self.kind(oid) {
                        Kind::Class => {
                            if let Some(k) = self.enum_kind.get(&oid) {
                                ek.mixed |= k.mixed;
                                ek.is_str |= k.is_str;
                                ek.repr_value |= k.repr_value;
                                ek.flag |= k.flag;
                            }
                            bases.push(oid)
                        }
                        Kind::External => {
                            let q = self.external_qual(oid);
                            if (q.ends_with("Enum") || q.ends_with("Flag")) && q.starts_with("enum.") {
                                is_enum = true;
                            }
                            match q.as_str() {
                                "enum.StrEnum" | "enum.IntEnum" | "enum.IntFlag" | "enum.ReprEnum" => {
                                    ek.mixed = true;
                                    ek.repr_value = true;
                                }
                                "builtins.str" | "builtins.int" => ek.mixed = true,
                                _ => {}
                            }
                            if q == "enum.StrEnum" {
                                ek.is_str = true;
                            }
                            if q == "builtins.str" {
                                str_mixin = true;
                            }
                            if matches!(q.as_str(), "enum.Flag" | "enum.IntFlag") {
                                ek.flag = true;
                            }
                            if q == "builtins.dict" {
                                dict_base = true;
                            }
                            if q.starts_with("torch.nn.") {
                                nn_module = true;
                            }
                            if crate::models::pydantic::is_pydantic_base(&q) {
                                pydantic = true;
                            } else if q.ends_with("TypedDict") {
                                typed_dict = true;
                            } else if q.ends_with("NamedTuple") {
                                named_tuple = true;
                            } else if matches!(q.as_str(), "builtins.object" | "object" | "abc.ABC" | "typing.Protocol" | "typing.Generic") || q.ends_with("Generic") || (q.strip_prefix("builtins.").map(crate::interp::exprs::is_exception_name).unwrap_or(false) && !crate::interp::disabled("excbase"))
                                || (q.starts_with("enum.") && !crate::interp::disabled("enumbase")) {
                                // (a builtin exception base adds nothing to the instance's `vars()`)
                            } else {
                                open_base = true;
                            }
                        }
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
                    let abc = mv.pts.iter().all(|o| self.kind(o) == Kind::External && self.external_qual(o).ends_with("ABCMeta")) && !mv.pts.is_empty();
                    if !abc {
                        open_base = true;
                    }
                }
            }
        }
        let info = ClassInfo {
            name: c.name.id.to_string(),
            node: c,
            bases,
            module: fr_module,
            open_base,
            dataclass_fields: std::cell::RefCell::new(None),
            typed_dict,
            named_tuple,
            dict_base,
            pydantic,
            pyd_dataclass: std::cell::Cell::new(false),
            dc_frozen: std::cell::Cell::new(false),
            dc_kw_only: std::cell::Cell::new(false),
            nn_module,
            field_classes: std::cell::RefCell::new(None),
            any_fields: std::cell::RefCell::new(None),
        };
        let info = std::rc::Rc::new(info);
        let cnode = frontend::node(c);
        let coid = self.alloc_at(Kind::Class, &cnode, state, &format!("class {}", c.name.id), Payload::Class(info.clone()), None, Ns::Attr, None, None);
        self.user_classes.push(coid);
        let mut is_dataclass = false;
        for d in &c.decorator_list {
            if let Some(q) = self.decorator_qualname(&d.expression, state) {
                if q.ends_with("dataclass") || q.ends_with("attrs.define") || q.ends_with("attr.s") {
                    is_dataclass = true;
                    if q.starts_with("pydantic") {
                        info.pyd_dataclass.set(true);
                    }
                    if let ast::Expr::Call(dc) = &d.expression {
                        for k in dc.arguments.keywords.iter() {
                            match (k.arg.as_ref().map(|x| x.as_str()), &k.value) {
                                (Some("frozen"), ast::Expr::BooleanLiteral(b)) => info.dc_frozen.set(b.value),
                                (Some("kw_only"), ast::Expr::BooleanLiteral(b)) => info.dc_kw_only.set(b.value),
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
        if is_dataclass || typed_dict || named_tuple || pydantic {
            let mut fields: Vec<(String, Option<&'static ast::Expr>)> = Vec::new();
            for st in &c.body {
                if let ast::Stmt::AnnAssign(a) = st {
                    if let ast::Expr::Name(n) = &*a.target {
                        // a `ClassVar` is a class attribute; pydantic's `_x` are private attributes
                        let classvar = match &*a.annotation {
                            ast::Expr::Subscript(sb) => dotted_last(&sb.value) == "ClassVar",
                            other => dotted_last(other) == "ClassVar",
                        };
                        let pseudo = match &*a.annotation {
                            ast::Expr::Subscript(sb) => dotted_last(&sb.value) == "InitVar",
                            other => dotted_last(other) == "KW_ONLY",
                        };
                        if classvar || pseudo || (pydantic && (n.id.starts_with('_') || n.id.as_str() == "model_config")) {
                            continue;
                        }
                        fields.push((n.id.to_string(), a.value.as_deref()));
                    }
                }
            }
            *info.dataclass_fields.borrow_mut() = Some(fields);
        }
        // class body
        let mi = self.module_info(fr_module);
        let cscope = mi.scopes.of_node(crate::scopes::node_addr(c)).unwrap_or_else(|| mi.scopes.module());
        let fr_single_nl = self.frame().single_nl;
        let mut cf = Frame::new(FrameKind::Class, cscope, fr_module, &fr_ctx, fr_single, &format!("class {}", c.name.id));
        cf.single_nl = fr_single_nl;
        cf.class_oid = Some(coid);
        cf.cells = fr_cells;
        self.frames.push(cf);
        for st in &c.body {
            if let ast::Stmt::AnnAssign(a) = st {
                if a.value.is_none() {
                    continue;
                }
            }
            self.exec_stmt(st, state);
        }
        self.frames.pop();
        // an Enum: each plain class-level `NAME = value` is a member, an instance of the class
        // with known `name` and `value`, in definition order
        let enum_base = info.bases.iter().any(|b| self.enum_members.contains_key(b));
        if (is_enum || enum_base) && !crate::interp::disabled("enummembers") {
            let mut members: Vec<(String, u32)> = Vec::new();
            self.enum_kind.insert(coid, ek);
            // `auto()`: the lower-cased name (StrEnum), the next power of two (Flag), else the
            // last int value + 1
            let mut last_int: i64 = 0;
            for st in &c.body {
                let ast::Stmt::Assign(a) = st else { continue };
                let [ast::Expr::Name(n)] = &a.targets[..] else { continue };
                let name = n.id.to_string();
                if name.starts_with('_') {
                    continue;
                }
                let nk = key_str_lit(&name);
                let is_auto = match &*a.value {
                    ast::Expr::Call(cl) => match &*cl.func {
                        ast::Expr::Name(f) => f.id.as_str() == "auto",
                        ast::Expr::Attribute(at) => at.attr.as_str() == "auto",
                        _ => false,
                    },
                    _ => false,
                };
                // (`class M(str, Enum)` with `auto()`: str of an int, deprecated: left unknown)
                if is_auto && !(str_mixin && !ek.is_str) && !crate::interp::disabled("enumauto") {
                    let av = if ek.is_str && !ek.flag {
                        Value::lit(key_str_lit(&name.to_lowercase()))
                    } else if ek.flag {
                        let nx = if last_int <= 0 { 1 } else { 1i64 << (64 - (last_int as u64).leading_zeros()) };
                        Value::lit(crate::intern::key_int(nx))
                    } else {
                        Value::lit(crate::intern::key_int(last_int + 1))
                    };
                    self.cls_slots.insert((coid, Ns::Attr, nk), av);
                }
                let Some(v) = self.cls_slots.get(&(coid, Ns::Attr, nk)).cloned() else { continue };
                if v.pts.iter().any(|o| matches!(self.kind(o), Kind::Function | Kind::Class)) {
                    continue;
                }
                if let Some(i) = v.kv.as_lit().filter(|_| v.pts.is_empty()).and_then(crate::intern::key_as_int) {
                    last_int = last_int.max(i);
                }
                let mid = self.alloc_raw(Kind::Instance, &format!("enum:{}:{}", coid, name), "", true, &format!("{}.{}", c.name.id, name), Payload::Cls(coid));
                let mut row = Row::new();
                for (k, val) in [("name", Value::lit(nk)), ("value", v.clone())] {
                    let kk = key_str_lit(k);
                    row = row.store(kk, true);
                    state.set_slot(mid, Ns::Attr, kk, val, true);
                }
                state.set_row(mid, Ns::Attr, row);
                self.cls_slots.insert((coid, Ns::Attr, nk), Value::obj(mid));
                let mval = if ek.mixed { v.kv.as_lit().filter(|_| v.pts.is_empty()) } else { None };
                let mk = crate::intern::key_member(&format!("{}.{}", c.name.id, name), mval);
                self.member_key.insert(mid, mk);
                self.key_member.insert(mk, mid);
                members.push((name, mid));
            }
            self.enum_members.insert(coid, members);
        }
        // `__set_name__(owner, name)` on descriptor objects assigned in the class body
        if !crate::interp::disabled("descriptors") {
            for st in &c.body {
                let ast::Stmt::Assign(a) = st else { continue };
                let [ast::Expr::Name(n)] = &a.targets[..] else { continue };
                let v = self.cls_slots.get(&(coid, Ns::Attr, key_str_lit(n.id.as_str()))).cloned().unwrap_or_default();
                for d in v.pts.iter() {
                    if self.kind(d) == Kind::Instance && self.instance_class(d).and_then(|dc| self.class_lookup(dc, "__set_name__")).is_some() {
                        self.call_method_with(&Value::obj(d), "__set_name__", vec![Value::obj(coid), Value::lit(key_str_lit(n.id.as_str()))], &cnode, state);
                    }
                }
            }
        }
        // a base's `__init_subclass__` runs for the new class (an implicit classmethod)
        if !crate::interp::disabled("initsubclass") {
            let found = info.bases.iter().find_map(|b| self.class_lookup(*b, "__init_subclass__"));
            if let Some(defc) = found {
                let fv = self.cls_slots.get(&(defc, Ns::Attr, key_str_lit("__init_subclass__"))).cloned().unwrap_or_default();
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
                    if self.kind(f) == Kind::Function {
                        // concretely: the class's own attributes (`cls.__name__`) are known here
                        self.force_tab_once = true;
                        self.call_user(f, Some(Value::obj(coid)), vec![], kwargs.clone(), vec![], vec![], state, &cnode, None, false);
                        self.force_tab_once = false;
                    }
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

    /// `import a.b.c`: the parents are imported first and each submodule becomes an attribute
    /// of its package; a module that is not found is an external object.
    pub fn import_module(&mut self, name: &str, node: &Node, state: &mut State) -> Value {
        if let Some(&m) = self.modules.get(name) {
            return Value::obj(m);
        }
        if let Some((parent, leaf)) = name.rsplit_once('.') {
            let pv = self.import_module(parent, node, state);
            let mut found: Option<Value> = None;
            if let Some(po) = pv.single() {
                if self.kind(po) == Kind::Module {
                    let ppath = self.module_info(po).path.clone();
                    if let Some(pp) = ppath.filter(|p| p.ends_with("__init__.py")) {
                        if let Some(dir) = std::path::Path::new(&pp).parent() {
                            let dir = dir.to_path_buf();
                            found = self.load_module_file(name, &dir, leaf, state);
                        }
                    }
                    if let Some(v) = &found {
                        state.set_slot(po, Ns::Attr, key_str_lit(leaf), v.clone(), true);
                    }
                }
            }
            if let Some(v) = found {
                return v;
            }
        } else {
            for base in self.search_paths.clone() {
                let dir = std::path::PathBuf::from(&base);
                if let Some(v) = self.load_module_file(name, &dir, name, state) {
                    return v;
                }
            }
            // an installed package on the allowlist is analysed from site-packages, unless a
            // model covers it (the model knows more than its source would yield)
            let followed = self.follow.iter().any(|f| f == name || f == "*");
            if followed && !crate::models::has_module_models(name) {
                for base in self.site_paths.clone() {
                    let dir = std::path::PathBuf::from(&base);
                    if let Some(v) = self.load_module_file(name, &dir, name, state) {
                        return v;
                    }
                }
            }
        }
        let _ = node;
        if name == "pandas" || name.starts_with("pandas.") {
            self.pandas_on = true;
        }
        if name == "torch" || name.starts_with("torch.") {
            self.torch_on = true;
            let m = self.frame().module;
            self.torch_modules.insert(m);
        }
        let oid = self.alloc_raw(Kind::External, &format!("ext:{}", name), "", true, name, Payload::Str(name.to_string()));
        Value::obj(oid)
    }

    /// Load `dir/leaf/__init__.py` or `dir/leaf.py` as module `name` (a file already loaded
    /// under another name is shared).
    fn load_module_file(&mut self, name: &str, dir: &std::path::Path, leaf: &str, state: &mut State) -> Option<Value> {
        let cands = [dir.join(leaf).join("__init__.py"), dir.join(format!("{}.py", leaf))];
        for p in cands.iter() {
            if !p.exists() {
                continue;
            }
            let real = std::fs::canonicalize(p).map(|x| x.to_string_lossy().to_string()).unwrap_or_else(|_| p.to_string_lossy().to_string());
            for (_, &moid) in self.modules.clone().iter() {
                let mi = self.module_info(moid);
                if let Some(mp) = &mi.path {
                    let mreal = std::fs::canonicalize(mp).map(|x| x.to_string_lossy().to_string()).unwrap_or_else(|_| mp.clone());
                    if mreal == real {
                        self.modules.insert(name.to_string(), moid);
                        return Some(Value::obj(moid));
                    }
                }
            }
            let src = match std::fs::read_to_string(p) {
                Ok(s) => s,
                Err(_) => return None,
            };
            let ps = p.to_string_lossy().to_string();
            return match frontend::parse(name, Some(&ps), src) {
                Ok((idx, tree)) => {
                    let moid = self.make_module_pub(name, Some(&ps), idx, tree);
                    self.run_module(moid, state);
                    Some(Value::obj(moid))
                }
                Err(e) => {
                    self.note(format!("could not parse {}: {}", ps, e));
                    None
                }
            };
        }
        None
    }

    pub fn make_module_pub(&mut self, name: &str, path: Option<&str>, idx: u32, tree: &'static ast::ModModule) -> u32 {
        let pkg = match path {
            Some(p) if p.ends_with("__init__.py") => name.to_string(),
            _ => name.rsplit_once('.').map(|(a, _)| a.to_string()).unwrap_or_default(),
        };
        let info = super::ModuleInfo { name: name.to_string(), path: path.map(|p| p.to_string()), pkg, idx, tree, scopes: crate::scopes::analyse(tree) };
        let oid = self.alloc_global(Kind::Module, &format!("module:{}", name), &format!("module {}", name), Payload::Module(std::rc::Rc::new(info)));
        self.modules.insert(name.to_string(), oid);
        oid
    }
}

/// A `return` somewhere in these statements (not inside nested functions or classes).
fn contains_return(body: &[ast::Stmt]) -> bool {
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
    if rt.is_empty() {
        return Vec::new();
    }
    vec![
        std::sync::Arc::new(crate::state::CondFact { cond: key, truth: pos, deps: deps.clone(), rows: rt, guards: guards.clone() }),
        std::sync::Arc::new(crate::state::CondFact { cond: key, truth: !pos, deps, rows: re, guards }),
    ]
}

/// Names bound to literals of different truth in the two branches (`has = True` / `False`):
/// testing the name later tells which branch ran.
fn name_facts(vals: &[(intern::StrId, Option<Value>, Option<Value>, Option<Value>)], a: &State, b: &State, j: &State, func: u32) -> Vec<std::sync::Arc<crate::state::CondFact>> {
    let truth_of = |v: &Value| -> Option<bool> {
        if !v.pts.is_empty() {
            return None;
        }
        let k = v.kv.as_lit()?;
        intern::with_key(k, |kk| match kk {
            intern::Key::Bool(x) => Some(*x),
            intern::Key::None => Some(false),
            intern::Key::Int(i) => Some(*i != 0),
            intern::Key::Str(s) => Some(!s.is_empty()),
            _ => None,
        })
    };
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
fn add_facts(st: &mut State, new: Vec<std::sync::Arc<crate::state::CondFact>>) {
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
fn collect_assigned(st: &ast::Stmt, out: &mut Vec<intern::StrId>) {
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

/// The exception names a handler set catches, with the known subclasses of each
/// (`except ValueError` catches pydantic's `ValidationError`, `except LookupError` a
/// `KeyError`).
pub fn expand_caught(mut caught: FxHashSet<String>) -> FxHashSet<String> {
    const SUBS: &[(&str, &[&str])] = &[
        ("ValueError", &["ValidationError", "JSONDecodeError", "UnicodeDecodeError", "UnicodeEncodeError", "UnicodeError"]),
        ("LookupError", &["KeyError", "IndexError"]),
        ("ArithmeticError", &["ZeroDivisionError", "OverflowError", "FloatingPointError"]),
        ("OSError", &["FileNotFoundError", "PermissionError", "IsADirectoryError", "NotADirectoryError", "FileExistsError", "TimeoutError", "ConnectionError"]),
        ("IOError", &["FileNotFoundError", "PermissionError"]),
        ("EnvironmentError", &["FileNotFoundError", "PermissionError"]),
        ("RuntimeError", &["NotImplementedError", "RecursionError"]),
        ("ImportError", &["ModuleNotFoundError"]),
        ("AttributeError", &["FrozenInstanceError"]),
    ];
    let mut more: Vec<String> = Vec::new();
    for (p, subs) in SUBS {
        if caught.contains(*p) {
            more.extend(subs.iter().map(|x| x.to_string()));
        }
    }
    caught.extend(more);
    caught
}
