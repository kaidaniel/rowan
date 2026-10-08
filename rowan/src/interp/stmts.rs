//! Statements (port of the `stmt_*` methods of `rowan/interp.py`).

use ruff_python_ast as ast;
use rustc_hash::FxHashSet;

use crate::domain::{KeyVal, KeysOf, Ns, Row, Value};
use crate::frontend::{self, Node};
use crate::intern::{self, key_str_lit, s, KeySet};
use crate::state::{Kind, State};

use super::{contains_jump, dotted_last, is_main_guard, Analyzer, ClassInfo, Frame, FrameKind, FuncNode, LoopCtx, Payload, LOOP_MAX_ITER, UNROLL_MAX};
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
                let res = self.binop(a.op, &cur, &rhs, &n, state);
                self.bind_target(&a.target, &res, state);
            }
            ast::Stmt::Return(r) => {
                let v = match &r.value {
                    Some(e) => self.eval(e, state),
                    None => self.none_v.clone(),
                };
                state.ret = Some(match &state.ret {
                    None => v,
                    Some(cur) => cur.join(&v),
                });
                state.record_exit();
                state.reachable = false;
            }
            ast::Stmt::Raise(r) => {
                if let Some(e) = &r.exc {
                    self.eval(e, state);
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
                    if let Some(asn) = &a.asname {
                        let v = self.import_module(&name, &n, state);
                        self.store_name(s(asn.id.as_str()), v, state);
                    } else {
                        let v = self.import_module(&top, &n, state);
                        self.store_name(s(&top), v, state);
                    }
                }
            }
            ast::Stmt::ImportFrom(i) => {
                let modname = i.module.as_ref().map(|m| m.id.to_string()).unwrap_or_default();
                let n = frontend::node(i);
                let mod_v = if modname.is_empty() { Value::new() } else { self.import_module(&modname, &n, state) };
                for a in &i.names {
                    if a.name.id.as_str() == "*" {
                        continue;
                    }
                    let v = if !mod_v.pts.is_empty() { self.load_attr(&mod_v, a.name.id.as_str(), &n, state, false) } else { Value::new() };
                    let target = a.asname.as_ref().map(|x| x.id.as_str()).unwrap_or(a.name.id.as_str());
                    self.store_name(s(target), v, state);
                }
            }
            ast::Stmt::If(i) => self.stmt_if(&i.test, &i.body, &i.elif_else_clauses, state),
            ast::Stmt::While(w) => self.stmt_while(w, state),
            ast::Stmt::For(f) => self.stmt_for(f, state),
            ast::Stmt::Break(_) => {
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
            ast::Stmt::With(w) => {
                for item in &w.items {
                    let v = self.eval(&item.context_expr, state);
                    let n = frontend::node(&item.context_expr);
                    let mut entered = self.call_method_if_defined(&v, "__enter__", &n, state);
                    if entered.is_none() {
                        entered = Some(if v.pts.iter().any(|o| self.kind(o).is_record()) { v.clone() } else { Value::data(&format!("with@L{}", frontend::node(w).line)) });
                    }
                    if let Some(ov) = &item.optional_vars {
                        self.bind_target(ov, &entered.unwrap(), state);
                    }
                }
                self.exec_block(&w.body, state);
            }
            ast::Stmt::FunctionDef(f) => {
                let v = self.make_function(FuncNode::Def(f), state, None);
                let v = self.apply_decorators(&f.decorator_list, frontend::node(f), v, state);
                self.store_name(s(f.name.id.as_str()), v, state);
            }
            ast::Stmt::ClassDef(c) => self.stmt_classdef(c, state),
            ast::Stmt::Match(m) => {
                self.eval(&m.subject, state);
                let mut outs: Vec<State> = Vec::new();
                self.cond_depth += 1;
                self.branch_depth += 1;
                for case in &m.cases {
                    let mut cs = state.copy();
                    self.exec_block(&case.body, &mut cs);
                    outs.push(cs);
                }
                self.branch_depth -= 1;
                self.cond_depth -= 1;
                outs.push(state.copy());
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

    fn stmt_delete(&mut self, d: &'static ast::StmtDelete, state: &mut State) {
        for t in &d.targets {
            match t {
                ast::Expr::Subscript(sub) => {
                    let obj = self.eval(&sub.value, state);
                    let key = self.eval_key(&sub.slice, state);
                    let n = frontend::node(t);
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
        match self.truthiness(&tv, state) {
            Some(true) => else_s.reachable = false,
            Some(false) => then_s.reachable = false,
            None => {}
        }
        self.narrow(test, true, &mut then_s);
        self.narrow(test, false, &mut else_s);
        self.cond_depth += 1;
        self.branch_depth += 1;
        if then_s.reachable {
            self.exec_block(body, &mut then_s);
        }
        if else_s.reachable {
            self.exec_orelse(rest, &mut else_s);
        }
        self.branch_depth -= 1;
        self.cond_depth -= 1;
        let j = then_s.join(&mut else_s);
        state.replace_with(j);
    }

    fn exec_orelse(&mut self, rest: &'static [ast::ElifElseClause], state: &mut State) {
        let Some(first) = rest.first() else { return };
        match &first.test {
            None => self.exec_block(&first.body, state),
            Some(t) => self.stmt_if(t, &first.body, &rest[1..], state),
        }
    }

    fn stmt_while(&mut self, w: &'static ast::StmtWhile, state: &mut State) {
        let mut pre = state.copy();
        let mut head = state.copy();
        let mut exit_acc: Option<State> = None;
        self.frame_mut().loop_depth += 1;
        self.cond_depth += 1;
        for _ in 0..LOOP_MAX_ITER {
            let mut body_s = head.copy();
            let mark = self.objects.len() as u32;
            self.recent_stack.push(Vec::new());
            self.eval(&w.test, &mut body_s);
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
            let new_head = pre.join(&mut back);
            if new_head.same(&head) {
                break;
            }
            head = new_head;
        }
        self.cond_depth -= 1;
        self.frame_mut().loop_depth -= 1;
        let mut exit_s = head.copy();
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
        let has_jump = contains_jump(&f.body);
        if let Some(items) = &it.items {
            if items.len() <= UNROLL_MAX {
                // unrolled: `continue` rejoins at the next iteration, `break` at the exit
                let mut breaks: Option<State> = None;
                for item in items {
                    if !state.reachable {
                        break;
                    }
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
            let new_head = pre.join(&mut back);
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
        state.replace_with(exit_s);
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
        let mut pre = state.copy();
        self.guards.push(caught);
        self.may_raise.push(false);
        self.cond_depth += 1;
        self.branch_depth += 1;
        self.exec_block(&t.body, state);
        self.guards.pop();
        let body_may_raise = self.may_raise.pop().unwrap_or(false) || !state.reachable;
        let mut body_end = state.copy();
        let mut handler_entry: Option<State> = if !t.handlers.is_empty() { Some(pre.join(&mut body_end)) } else { None };
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
            if let Some(name) = &h.name {
                let hn = frontend::node(h);
                self.store_name(s(name.id.as_str()), Value::data(&format!("exc@L{}", hn.line)), &mut hs);
            }
            self.exec_block(&h.body, &mut hs);
            results.push(hs);
        }
        self.branch_depth -= 1;
        self.cond_depth -= 1;
        let mut out = results.remove(0);
        for mut r in results {
            out = out.join(&mut r);
        }
        if !t.finalbody.is_empty() {
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
        if let Some(args) = &c.arguments {
            for b in args.args.iter() {
                let bv = self.eval(b, state);
                for oid in bv.pts.iter() {
                    match self.kind(oid) {
                        Kind::Class => bases.push(oid),
                        Kind::External => {
                            let q = self.external_qual(oid);
                            if q.ends_with("TypedDict") {
                                typed_dict = true;
                            } else if q.ends_with("NamedTuple") {
                                named_tuple = true;
                            } else if matches!(q.as_str(), "builtins.object" | "object" | "abc.ABC" | "typing.Protocol" | "typing.Generic") {
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
                }
            }
        }
        if is_dataclass || typed_dict || named_tuple {
            let mut fields: Vec<(String, Option<&'static ast::Expr>)> = Vec::new();
            for st in &c.body {
                if let ast::Stmt::AnnAssign(a) = st {
                    if let ast::Expr::Name(n) = &*a.target {
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
        self.store_name(s(c.name.id.as_str()), Value::obj(coid), state);
    }

    pub fn import_module(&mut self, name: &str, node: &Node, state: &mut State) -> Value {
        if let Some(&m) = self.modules.get(name) {
            return Value::obj(m);
        }
        let _ = node;
        for base in self.search_paths.clone() {
            let mut p = std::path::PathBuf::from(&base);
            for part in name.split('.') {
                p.push(part);
            }
            let p = p.with_extension("py");
            if p.exists() {
                let real = std::fs::canonicalize(&p).map(|x| x.to_string_lossy().to_string()).unwrap_or_else(|_| p.to_string_lossy().to_string());
                for (_, &moid) in self.modules.clone().iter() {
                    let mi = self.module_info(moid);
                    if let Some(mp) = &mi.path {
                        let mreal = std::fs::canonicalize(mp).map(|x| x.to_string_lossy().to_string()).unwrap_or_else(|_| mp.clone());
                        if mreal == real {
                            self.modules.insert(name.to_string(), moid);
                            return Value::obj(moid);
                        }
                    }
                }
                let src = match std::fs::read_to_string(&p) {
                    Ok(s) => s,
                    Err(_) => break,
                };
                let ps = p.to_string_lossy().to_string();
                match frontend::parse(name, Some(&ps), src) {
                    Ok((idx, tree)) => {
                        let moid = self.make_module_pub(name, Some(&ps), idx, tree);
                        self.run_module(moid, state);
                        return Value::obj(moid);
                    }
                    Err(e) => {
                        self.note(format!("could not parse {}: {}", ps, e));
                        break;
                    }
                }
            }
        }
        let oid = self.alloc_raw(Kind::External, &format!("ext:{}", name), "", true, name, Payload::Str(name.to_string()));
        Value::obj(oid)
    }

    pub fn make_module_pub(&mut self, name: &str, path: Option<&str>, idx: u32, tree: &'static ast::ModModule) -> u32 {
        let info = super::ModuleInfo { name: name.to_string(), path: path.map(|p| p.to_string()), idx, tree, scopes: crate::scopes::analyse(tree) };
        let oid = self.alloc_global(Kind::Module, &format!("module:{}", name), &format!("module {}", name), Payload::Module(std::rc::Rc::new(info)));
        self.modules.insert(name.to_string(), oid);
        oid
    }
}
