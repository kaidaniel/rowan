//! Loops: a `for` over at most `UNROLL_MAX` known items is unrolled (each iteration
//! straight-line code); other loops run to a fixpoint (`loop_fixpoint`: `LOOP_MAX_ITER`
//! rounds, then widened), with recency objects for what an iteration allocates.

use ruff_python_ast as ast;

use crate::domain::{ObjId, KeyVal, Ns, Value};
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
            head.lens.insert(o, (None, 0, 0));
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
        let focus0 = self.cx.focus.len();
        if let Elem::One(v) = &it.elem {
            if let Some(o) = v.single().filter(|o| !self.heap.obj(*o).single && self.heap.is_record_like(*o)) {
                self.cx.focus.push(o);
            }
        }
        let (head, exit_acc, back) = self.loop_fixpoint(state, &mut |an, body_s| {
            an.bind_iter_target(&f.target, &it.elem, body_s);
            an.exec_block(&f.body, body_s);
        });
        self.cx.focus.truncate(focus0);
        let mut exit_s = head.copy();
        let elem_v = it.elem.value_part();
        self.promote_elem_reqs(&mut exit_s, &back, &elem_v, state);
        if exit_acc.is_none() && back.reachable {
            let own = itv.single().filter(|l| self.heap.obj(*l).single && matches!(self.heap.kind(*l), Kind::List | Kind::Tuple) && back.slots_of(*l) == pre.slots_of(*l) && back.lens.get(l) == pre.lens.get(l));
            self.elements_done(&elem_v, own, &f.target, &back, &mut exit_s);
        }
        if !f.orelse.is_empty() && exit_s.reachable {
            self.exec_block(&f.orelse, &mut exit_s);
        }
        join_breaks(&mut exit_s, exit_acc);
        if f.orelse.is_empty() && !has_jump && !prefixed && back.reachable && pre.reachable && exit_s.reachable {
            self.loop_ran_facts(f, &back, &pre, &mut exit_s);
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

    /// Every element of the iterable went through the body: its row after the loop is the
    /// body's exit row, not the join with "not processed yet".
    /// (A summary record — `[dict(x) for x in data]` — too, when nothing but the list the loop
    /// went through, which the body did not change, holds it: every one it stands for that is
    /// still in use was an item.)
    fn elements_done(&mut self, elem_v: &Value, list: Option<ObjId>, target: &ast::Expr, back: &State, exit_s: &mut State) {
        for oid in elem_v.pts.iter() {
            // (rows the body changed; for a summary, only then the walk)
            let rows: Vec<Ns> = [Ns::Item, Ns::Attr].into_iter().filter(|ns| back.has_row(oid, *ns) && back.row(oid, *ns) != exit_s.row(oid, *ns)).collect();
            let held = |an: &Analyzer| list.is_some_and(|l| an.held_only_by(oid, l, target, exit_s));
            if !rows.is_empty() && self.heap.is_record_like(oid) && (self.heap.obj(oid).single || held(self)) {
                for ns in rows {
                    exit_s.set_row(oid, ns, back.row(oid, ns));
                }
            }
        }
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
