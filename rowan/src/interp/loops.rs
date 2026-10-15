//! Loops: a `for` over at most `UNROLL_MAX` known items is unrolled (each iteration
//! straight-line code); other loops run to a fixpoint (`loop_fixpoint`: `LOOP_MAX_ITER`
//! rounds, then widened), with recency objects for what an iteration allocates.

use ruff_python_ast as ast;

use crate::domain::{ObjId, KeyVal, Ns, Row, Value};
use crate::frontend::{self};
use crate::intern::{self, s};
use crate::state::{Kind, State};

use super::{contains_jump, is_constant, Analyzer, LoopCtx, LOOP_MAX_ITER, UNROLL_MAX};
use super::funcs::Elem;
use super::stmts::{add_facts, collect_assigned, cond_facts_for};

impl Analyzer {
    pub(super) fn stmt_while(&mut self, w: &'static ast::StmtWhile, state: &mut State) {
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
            head.set_lens(o, (None, 0, 0));
        }
    }

    pub(super) fn stmt_while_loop(&mut self, w: &'static ast::StmtWhile, state: &mut State) {
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
    pub(super) fn unrolled_iteration(&mut self, idx: usize, body: &'static [ast::Stmt], state: &mut State, breaks: &mut Option<State>, bind: &mut dyn FnMut(&mut Analyzer, &mut State)) {
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
    pub(super) fn unroll_for(&mut self, f: &'static ast::StmtFor, items: &[(Value, bool)], cursor: Option<(u32, usize)>, state: &mut State) -> Option<State> {
        let mut breaks: Option<State> = None;
        for (idx, (item, sure)) in items.iter().enumerate() {
            if !state.reachable {
                break;
            }
            if let Some((o, c0)) = cursor {
                self.set_cursor(o, Some(c0 + idx + 1), state);
            } else if idx > 0 {
                self.note_resumes(state);
            }
            let skipped = if *sure { None } else { Some(state.copy()) };
            self.unrolled_iteration(idx, &f.body, state, &mut breaks, &mut |an, st| an.bind_iter_target(&f.target, &Elem::One(item.clone()), st));
            join_breaks(state, skipped);
        }
        breaks
    }

    pub(super) fn stmt_for(&mut self, f: &'static ast::StmtFor, state: &mut State) {
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
            if self.for_known_first_items(f, o, c0, state) {
                return;
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
        // unrolled: `continue` rejoins at the next iteration, `break` at the exit; a closed dict
        // with some keys only maybe there: a maybe one runs on one path and is skipped on the other
        let unrolled = match &it.items {
            Some(items) if items.len() <= UNROLL_MAX && !self.identical_summaries(items) => Some((items.iter().map(|i| (i.clone(), true)).collect(), once.filter(|_| has_jump).map(|o| (o, c0)))),
            None => self.maybe_dict_items(&itv, state).map(|items| (items, None)),
            _ => None,
        };
        if let Some((items, cursor)) = unrolled {
            let breaks = self.unroll_for(f, &items, cursor, state);
            if !f.orelse.is_empty() && state.reachable {
                self.exec_block(&f.orelse, state);
            }
            // (a one-shot iterator a `break` may leave: run to the end, used up)
            if cursor.is_some() && state.reachable {
                self.exhaust(&itv, state);
            }
            join_breaks(state, breaks);
            return;
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
        let mut ends: Vec<(ObjId, [Row; 2])> = Vec::new();
        let (head, exit_acc, back) = self.loop_fixpoint(state, &mut |an, body_s| {
            an.bind_iter_target(&f.target, &it.elem, body_s);
            let views = an.bind_views(&f.target, body_s);
            let hits0 = crate::state::view_hits();
            an.exec_block(&f.body, body_s);
            ends = an.view_ends(&views, hits0, body_s);
            // (a generator's next item is asked for at the back edge)
            an.note_resumes(body_s);
        });
        let mut exit_s = head.copy();
        let elem_v = it.elem.value_part();
        self.promote_elem_reqs(&mut exit_s, &back, &elem_v, state);
        if exit_acc.is_none() && back.reachable {
            let own = itv.single().filter(|l| self.heap.obj(*l).single && matches!(self.heap.kind(*l), Kind::List | Kind::Tuple) && back.slots_of(*l) == pre.slots_of(*l) && back.lens.get(l) == pre.lens.get(l));
            self.elements_done(&elem_v, own, &f.target, &back, &ends, &mut exit_s);
        }
        if !f.orelse.is_empty() && exit_s.reachable {
            self.exec_block(&f.orelse, &mut exit_s);
        }
        join_breaks(&mut exit_s, exit_acc);
        if f.orelse.is_empty() && !has_jump && !prefixed && back.reachable && pre.reachable && exit_s.reachable {
            self.loop_ran_facts(f, &back, &pre, &mut exit_s);
            self.counted_rounds(f, &pre, &back, &mut exit_s);
        }
        join_breaks(&mut exit_s, prefix_breaks);
        state.replace_with(exit_s);
    }

    /// The loop ran at least once exactly when its iterable was non-empty: a later test of
    /// that (`if xs:`, `if n > 0:` for `range(n)`) gets the rows the iterations left.
    fn loop_ran_facts(&mut self, f: &'static ast::StmtFor, back: &State, pre: &State, exit_s: &mut State) {
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
            let deps = vec![(name, self.dep_value(exit_s, name))];
            if deps[0].1.pts.iter().any(|o| matches!(self.heap.kind(o), Kind::List | Kind::Set)) {
                continue;
            }
            new.extend(cond_facts_for(s(&format!("{}@{}", text, func)), true, deps, back, pre, exit_s));
        }
        add_facts(exit_s, new);
    }

    /// What a loop of a known number of rounds that ran to its end left: lists appended to
    /// and tensors grown by `cat` once per round.
    fn counted_rounds(&mut self, f: &'static ast::StmtFor, pre: &State, back: &State, exit_s: &mut State) {
        let Some(rounds) = self.loop_rounds(f, pre) else { return };
        self.counted_appends(f, rounds, pre, exit_s);
        if self.m.torch_on {
            for (name, v) in crate::models::torch::counted_cats(self, f, rounds, pre, back) {
                self.store_name(name, v, exit_s);
            }
        }
    }

    /// `for _ in range(T): … xs.append(x) …` with no `break` / `continue`, the append a statement
    /// of the body itself, `xs` named nowhere else in it and held by nothing else: every round
    /// that ends appended once, so after the loop `len(xs)` is its length before plus `T` (a
    /// tensor dim, `sym_lens`).
    fn counted_appends(&mut self, f: &'static ast::StmtFor, rounds: crate::models::shape::DimId, pre: &State, exit_s: &mut State) {
        let appended: Vec<&'static ast::ExprName> = f.body.iter().filter_map(|st| match st {
            ast::Stmt::Expr(e) => match &*e.value {
                ast::Expr::Call(c) if c.arguments.args.len() == 1 && c.arguments.keywords.is_empty() => match &*c.func {
                    ast::Expr::Attribute(a) if a.attr.as_str() == "append" => match &*a.value {
                        ast::Expr::Name(l) => Some(l),
                        _ => None,
                    },
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        }).collect();
        let scope = self.heap.module_info(self.frame().module).scopes.get(self.frame().scope).cells.clone();
        let target_names = super::names_in(&f.target);
        for l in appended {
            let name = l.id.as_str();
            if scope.contains(name) || target_names.iter().any(|t| t == name) || body_mentions(&f.body, name) != 1 {
                continue;
            }
            let Some(o) = self.name_value(pre, s(name)).and_then(|v| v.single().filter(|_| v.kv.is_bot())) else { continue };
            let (Kind::List, true, Some(n0)) = (self.heap.kind(o), self.heap.obj(o).single, self.heap.llen(o, pre)) else { continue };
            if !self.held_only_by_name(o, name, pre) {
                continue;
            }
            let len = crate::models::shape::add(crate::models::shape::lit(n0 as i64), rounds);
            exit_s.set_sym_len(o, s(&crate::models::shape::dim_label(len)));
        }
    }

    /// The number of rounds of `for _ in range(T)` in a function, `T` a name or a literal, as a
    /// dim surely not negative (a literal, tensors' dims: not a named int, which may be).
    fn loop_rounds(&mut self, f: &'static ast::StmtFor, pre: &State) -> Option<crate::models::shape::DimId> {
        let ast::Expr::Call(c) = &*f.iter else { return None };
        let (true, [arg], true) = (matches!(&*c.func, ast::Expr::Name(r) if r.id.as_str() == "range"), &c.arguments.args[..], c.arguments.keywords.is_empty()) else { return None };
        if self.frame().kind != super::FrameKind::Function {
            return None;
        }
        let v = match arg {
            ast::Expr::Name(n) => self.name_value(pre, s(n.id.as_str()))?,
            ast::Expr::NumberLiteral(_) => self.eval(arg, &mut pre.copy()),
            _ => return None,
        };
        crate::models::torch::t_dim(self, &v, pre).filter(|d| crate::models::shape::tensor_count(*d))
    }

    /// Whether nothing but the local `name` holds `o` in `state` (what the code holds, walked).
    fn held_only_by_name(&self, o: ObjId, name: &str, state: &State) -> bool {
        let n = s(name);
        if state.env.iter().any(|(k, v)| *k != n && (v.pts.contains(o) || matches!(&v.kv, crate::domain::KeyVal::KeysOf(ko) if ko.objs.contains(o)))) {
            return false;
        }
        let mut st = state.copy();
        st.env.shift_remove(&n);
        self.held_only_by(o, ObjId::MAX, &ast::Expr::Name(ast::ExprName { range: Default::default(), node_index: Default::default(), id: ast::name::Name::new(""), ctx: ast::ExprContext::Load }), &st)
    }

    /// Every element of the iterable went through the body: its row after the loop is the
    /// body's exit row, not the join with "not processed yet".
    /// (A summary record — `[dict(x) for x in data]` — too, when nothing but the list the loop
    /// went through, which the body did not change, holds it: every one it stands for that is
    /// still in use was an item.)
    /// (`ends`: the rows the item of a summary had at the end of the body, when nothing else
    /// of its family changed it there)
    fn elements_done(&mut self, elem_v: &Value, list: Option<ObjId>, target: &ast::Expr, back: &State, ends: &[(ObjId, [Row; 2])], exit_s: &mut State) {
        for oid in elem_v.pts.iter() {
            let end = ends.iter().find(|(s, _)| *s == oid).map(|(_, r)| r);
            let row_at = |ns: Ns| match (end, ns) {
                (Some(r), Ns::Item) => r[0].clone(),
                (Some(r), _) => r[1].clone(),
                _ => back.row(oid, ns),
            };
            // (rows the body changed; for a summary, only then the walk)
            let rows: Vec<Ns> = [Ns::Item, Ns::Attr].into_iter().filter(|ns| back.has_row(oid, *ns) && row_at(*ns) != exit_s.row(oid, *ns)).collect();
            let held = |an: &Analyzer| list.is_some_and(|l| an.held_only_by(oid, l, target, exit_s));
            if !rows.is_empty() && self.heap.is_record_like(oid) && (self.heap.obj(oid).single || held(self)) {
                for ns in rows {
                    exit_s.set_row_narrowed(oid, ns, row_at(ns));
                }
            }
        }
    }

    /// Each name a loop's target bound to one of the records an object stands for (a summary,
    /// an input's element) is bound to an object of its own for the iteration instead: one
    /// record (`link_view`: what changes it may change the others, and the other way round),
    /// its facts a copy of theirs, folded back into them at the back edge (as a recent object).
    /// (The view an earlier iteration left in use — a `break` out of an inner loop — is not
    /// made again: the name keeps the summary.)
    pub(super) fn bind_views(&mut self, target: &'static ast::Expr, state: &mut State) -> Vec<(ObjId, ObjId)> {
        let mut out = Vec::new();
        if self.cx.recent_stack.is_empty() {
            return out;
        }
        for n in super::names_in(target) {
            let sid = intern::s(&n);
            let Some(v) = self.name_value(state, sid).filter(|v| v.pts.len() == 1) else { continue };
            let o = v.pts.iter().next().unwrap();
            if let Some(i) = self.loop_view(o, target, state) {
                self.store_name(sid, Value::with_kv(Value::obj(i).pts, v.kv.clone()), state);
                out.push((i, o));
            }
        }
        out
    }

    fn loop_view(&mut self, o: ObjId, target: &'static ast::Expr, state: &mut State) -> Option<ObjId> {
        let ob = self.heap.obj(o);
        if ob.single || crate::state::is_any(o) || !(matches!(ob.kind, Kind::Dict | Kind::Instance | Kind::Unknown) || ob.iter_elem) || crate::state::view_of(o).is_some() {
            return None;
        }
        let (kind, label, payload, derived, depth) = (ob.kind, ob.label.to_string(), ob.payload.clone(), ob.derived_from, ob.depth);
        let ctx = self.frame().ctx.clone();
        let site = format!("view:{}:{}", o, target as *const ast::Expr as usize);
        let i = self.heap.alloc_raw(kind, &site, &ctx, true, &label, payload);
        if !self.heap.obj(i).single || self.has_facts(i, state) {
            return None;
        }
        let (ll, kp) = (self.heap.obj(o).literal_len, self.heap.obj(o).known_prefix);
        let io = self.heap.obj_mut(i);
        (io.literal_len, io.known_prefix) = (ll, kp);
        io.derived_from = derived;
        io.depth = depth;
        self.heap.renew(i);
        for ns in [Ns::Item, Ns::Attr] {
            if let Some(r) = state.rows.get(&(o, ns)) {
                state.rows.insert((i, ns), r);
            }
        }
        for (sk, v) in state.slots_of(o) {
            state.slots.set(&i, sk, v);
        }
        if let Some(l) = state.lens.get(&o) {
            state.set_lens(i, l);
        }
        if let Some(e) = state.elems.get(&o) {
            state.elems.insert(i, e);
        }
        crate::state::link_view(i, o);
        if let Some(sym) = self.names.get(o).cloned() {
            let scope = self.names.scope(o);
            self.names.set(i, sym, scope);
        }
        self.cx.recent_stack.last_mut().unwrap().push((i, kind, site, ctx));
        Some(i)
    }

    /// The rows of each view at the end of the body (the continues' joined in), when no other
    /// object of its family changed it since `hits0` (`view_hits`).
    fn view_ends(&self, views: &[(ObjId, ObjId)], hits0: u64, body_s: &State) -> Vec<(ObjId, [Row; 2])> {
        if views.is_empty() || crate::state::view_hits() != hits0 {
            return Vec::new();
        }
        let cont = self.unwind.loop_stack.last().and_then(|c| c.continues.as_ref());
        let ends = [Some(body_s), cont].into_iter().flatten().filter(|s| s.reachable);
        let mut out: Vec<(ObjId, [Row; 2])> = Vec::new();
        for st in ends {
            for (i, s) in views {
                let rows = [st.row(*i, Ns::Item), st.row(*i, Ns::Attr)];
                match out.iter_mut().find(|(o, _)| o == s) {
                    Some((_, r)) => *r = [r[0].join(&rows[0]), r[1].join(&rows[1])],
                    None => out.push((*s, rows)),
                }
            }
        }
        out
    }

    /// An endless generator with known first items (`o`, from its cursor `c0`): those
    /// iterations are unrolled (the cursor follows), the rest of the loop runs on from there.
    /// True: the loop ran.
    fn for_known_first_items(&mut self, f: &'static ast::StmtFor, o: ObjId, c0: usize, state: &mut State) -> bool {
        if let (None, Some(p)) = (self.heap.llen(o, state), self.heap.prefix(o, state)) {
            if p > c0 && p - c0 <= UNROLL_MAX {
                let items: Vec<(Value, bool)> = (c0..p).map(|i| (state.slot(o, Ns::Pos, intern::key_int(i as i64)), true)).collect();
                let breaks = self.unroll_for(f, &items, Some((o, c0)), state);
                if state.reachable {
                    // (the rest: from the end of the known items)
                    self.stmt_for(f, state);
                }
                join_breaks(state, breaks);
                return true;
            }
        }
        false
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

impl Analyzer {
    /// Known items that are all one summary object (`[Block() for _ in range(n)]`): their
    /// iterations cannot be told apart, they run as a loop.
    pub(super) fn identical_summaries(&self, items: &[Value]) -> bool {
        items.len() > 1 && !items[0].pts.is_empty() && items[0].kv.is_bot() && items.iter().all(|v| *v == items[0]) && items[0].pts.iter().all(|o| !self.heap.obj(o).single)
    }

    /// Whether nothing but the list `l` (and the loop `target`'s names) holds `s` in `state`
    /// (a walk from the roots that does not enter `l`; a long one answers no).
    fn held_only_by(&self, s: ObjId, l: ObjId, target: &ast::Expr, state: &State) -> bool {
        let skip: Vec<intern::StrId> = super::names_in(target).iter().map(|n| intern::s(n)).collect();
        let mut work: Vec<ObjId> = Vec::new();
        for fr in &self.cx.frames {
            work.push(fr.module);
            work.extend(fr.cells.values().copied());
        }
        for v in state.ret.iter().chain(state.narrowed.values()) {
            v.push_objs(&mut work);
        }
        for (n, v) in state.env.iter() {
            if !skip.contains(n) {
                v.push_objs(&mut work);
            }
        }
        self.gc_pinned(&mut work, None);
        // (at module level the target is a global: the module's other names)
        let module = matches!(self.frame().kind, super::FrameKind::Module).then(|| self.frame().module);
        let mut seen: rustc_hash::FxHashSet<ObjId> = Default::default();
        while let Some(o) = work.pop() {
            if o == s {
                return false;
            }
            if o == l || !seen.insert(o) {
                continue;
            }
            if Some(o) == module {
                state.slots.each(&o, &mut |(_, k), v| if !skip.iter().any(|n| intern::key_str_lit(&intern::str_get(*n)) == *k) {
                    v.push_objs(&mut work)
                });
                continue;
            }
            self.budget.walk_visit();
            if seen.len() > 200 {
                return false;
            }
            self.obj_edges(o, state, &mut work);
        }
        true
    }
}

/// How many times the plain name `name` occurs in these statements (nested functions too).
pub(crate) fn body_mentions(body: &[ast::Stmt], name: &str) -> usize {
    struct V<'n>(&'n str, usize);
    impl<'a> ast::visitor::source_order::SourceOrderVisitor<'a> for V<'_> {
        fn visit_expr(&mut self, e: &'a ast::Expr) {
            if let ast::Expr::Name(n) = e {
                if n.id.as_str() == self.0 {
                    self.1 += 1;
                }
            }
            ast::visitor::source_order::walk_expr(self, e);
        }
    }
    use ast::visitor::source_order::SourceOrderVisitor;
    let mut v = V(name, 0);
    for st in body {
        v.visit_stmt(st);
    }
    v.1
}
