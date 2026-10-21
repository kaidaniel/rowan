//! Comprehensions: each generator level unrolled over known items, else a fixpoint; their
//! variables in their own scope (cells for the functions made inside).

use ruff_python_ast as ast;

use crate::domain::{BOT, DATA, KeyVal, KeysOf, Ns, ObjId, Value};
use crate::frontend::{self, Node};
use crate::intern::{self, key_int, s, KeyId, STAR};
use crate::state::{Kind, State};
use crate::syntax::{is_constant, names_in};

use super::iter::Elem;
use super::{Analyzer, Payload, UNROLL_MAX};

impl Analyzer {
    // comprehensions ---------------------------------------------------------
    /// Run a comprehension's generators (`limit`: the most known items a generator unrolls).
    pub(super) fn comp(&mut self, gens: &'static [ast::Comprehension], limit: usize, state: &mut State, body: &mut dyn FnMut(&mut Analyzer, &mut State), own: ObjId) {
        self.with_gen_restart(state, true, &mut |an, st| an.comp_scoped(gens, limit, st, body, own));
    }

    /// Runs a comprehension's generators (`comp_gen`) with `body` per item, its variables in a
    /// scope of their own (the enclosing names they shadow put back after), and their cells.
    pub(super) fn comp_scoped(&mut self, gens: &'static [ast::Comprehension], limit: usize, state: &mut State, body: &mut dyn FnMut(&mut Analyzer, &mut State), own: ObjId) {
        let mut names: Vec<intern::StrId> = gens.iter().flat_map(|g| names_in(&g.target)).map(|n| s(&n)).collect();
        names.sort_unstable();
        names.dedup();
        let outer: Vec<Option<Value>> = names.iter().map(|n| state.env.get(n).cloned()).collect();
        self.frame_mut().comp_names.push(names.clone());
        self.cx.comp_cells.push(Default::default());
        self.comp_gen(gens, 0, limit, state, body, own);
        self.cx.comp_cells.pop();
        self.frame_mut().comp_names.pop();
        // (the comprehension's variables are its own: the enclosing names are as before)
        for (n, v) in names.into_iter().zip(outer) {
            match v {
                Some(v) => {
                    state.env.insert(n, v);
                }
                None => {
                    state.env.shift_remove(&n);
                }
            }
        }
    }

    /// A generator expression's run (its frame's `.0` the iterator its first iterable gave
    /// where it was made): its generators, a yield per item.
    pub(super) fn genexp_body(&mut self, e: &'static ast::Expr, gid: ObjId, state: &mut State) {
        let ast::Expr::Generator(g) = e else { return };
        let limit = comp_unroll_limit(&g.generators, &[&g.elt], self.is_torch_module(self.frame().module));
        let elt: &'static ast::Expr = &g.elt;
        let site = (elt as *const ast::Expr as usize, 0);
        self.comp(&g.generators, limit, state, &mut |an, st| {
            let v = an.eval(elt, st);
            if an.at_stop_hook() {
                an.stop_at_yield(v, st);
            } else {
                an.yield_value(v, st, site);
            }
        }, gid);
    }

    /// Level 0 of the running generator expression's own generators: the iterator it was given.
    fn genexp_iterator(&self, gens: &'static [ast::Comprehension], i: usize, state: &State) -> Option<Value> {
        let f = self.frame().func.filter(|_| i == 0)?;
        match self.heap.func_info(f).node {
            super::FuncNode::GenExp(ast::Expr::Generator(g)) if std::ptr::eq(g.generators.as_ptr(), gens.as_ptr()) => state.env.get(&s(".0")).cloned(),
            _ => None,
        }
    }

    /// Generator level `i` of a comprehension: its items unrolled when known (up to `limit`),
    /// else a fixpoint over any item; its filters split the state; the next level, or `body`.
    pub(super) fn comp_gen(&mut self, gens: &'static [ast::Comprehension], i: usize, limit: usize, state: &mut State, body: &mut dyn FnMut(&mut Analyzer, &mut State), own: ObjId) {
        if i == gens.len() {
            body(self, state);
            return;
        }
        let g = &gens[i];
        let nonempty = std::mem::take(&mut self.mode.nonempty_gen);
        let itv = match self.genexp_iterator(gens, i, state) {
            Some(v) => v,
            None => self.eval(&g.iter, state),
        };
        let itv = self.iterable_part(&itv, state);
        // (one made for it alone is closed where a raise leaves it: `genclose.rs`)
        let fresh = self.fresh_gen(&itv, &g.iter, state);
        self.force_gen(&itv, state, true);
        self.closing_loop(fresh, usize::MAX, state, &mut |an, st| an.comp_gen_items(gens, i, limit, &itv, nonempty, st, body, own));
    }

    /// Generator level `i`'s items, of what it iterates (`itv`, run if a generator).
    #[allow(clippy::too_many_arguments)]
    fn comp_gen_items(&mut self, gens: &'static [ast::Comprehension], i: usize, limit: usize, itv: &Value, nonempty: bool, state: &mut State, body: &mut dyn FnMut(&mut Analyzer, &mut State), own: ObjId) {
        let g = &gens[i];
        let itv = itv.clone();
        let exhaust_after = itv.clone();
        let gn = frontend::node(&g.iter);
        let it = self.iterate(&itv, &gn, state);
        let understood = g.ifs.iter().all(|c| cond_is_exclusion(c, &g.target));
        // (a generator with effects or closing facts stands where the items are then)
        let once = self.stops_in(&itv).or_else(|| itv.single().filter(|o| self.heap.obj(*o).iterator && self.heap.obj(*o).single && (self.closes_on_drop(*o) || self.gens.eff.contains_key(o))));
        let cursor = once.map(|o| (o, self.cursor_of(o, state).unwrap_or(0)));
        if self.comp_known_items(gens, i, limit, &itv, &it, state, body, understood, own, cursor) {
            self.exhaust(&exhaust_after, state);
            return;
        }
        self.cx.comp_known.push(false);
        if let Some(o) = once {
            self.set_cursor(o, None, state);
        }
        let depth = i16::try_from(self.cx.cond_depth).unwrap_or(i16::MAX);
        let mut elem = it.elem.clone();
        if it.exhaustive && understood {
            match &elem {
                Elem::One(v) => {
                    if let KeyVal::KeysOf(ko) = &v.kv {
                        elem = Elem::One(Value::with_kv(v.pts.clone(), KeyVal::KeysOf(Box::new(KeysOf { exhaustive_depth: depth, ..(**ko).clone() }))));
                    }
                }
                Elem::Pair(k0, v1) => {
                    if let KeyVal::KeysOf(ko) = &k0.kv {
                        elem = Elem::Pair(Value::with_kv(k0.pts.clone(), KeyVal::KeysOf(Box::new(KeysOf { exhaustive_depth: depth, ..(**ko).clone() }))), v1.clone());
                    }
                }
            }
        }
        let mut s2 = state.copy();
        let mark0 = self.heap.serial();
        let vmark = self.findings.mark();
        // (one pass stands for every item: what it makes is one object per item, a summary)
        let hmark = self.heap.mark();
        self.frame_mut().loop_depth += 1;
        self.cx.recent_stack.push(Vec::new());
        self.comp_run_one(g, gens, i, limit, &elem, &mut s2, body, understood, own);
        let recs = self.cx.recent_stack.pop().unwrap_or_default();
        self.frame_mut().loop_depth -= 1;
        self.rename_recent(&mut s2, state, recs, hmark);
        if it.exhaustive && understood && s2.reachable {
            self.cx.comp_known.pop();
            // (one pass stands for every key; for none, too, only where the keys may be none:
            // what it stores under keys of its own may not be there)
            let out = if nonempty || surely_some_keys(&elem, state) { s2 } else { state.join(&mut s2) };
            state.replace_with(out);
            self.exhaust(&exhaust_after, state);
            return;
        }
        // (an iteration that changed nothing an older object or a name holds, but the result
        // it builds: the next one starts from the same state, one pass is the fixpoint)
        // (the element itself narrowed by a read is not a change the next element sees)
        let elem_objs = elem.flat().pts;
        let older = |o: ObjId| self.heap.born_before(o, mark0) && o != own && !elem_objs.contains(o) && !self.heap.obj(o).iter_elem;
        let fp = s2.footprint(state);
        // (the comprehension's own variables aside)
        let own_var = |n: &intern::StrId| self.frame().comp_names.iter().any(|ns| ns.contains(n));
        let same_env = s2.env.iter().all(|(n, v)| own_var(n) || state.env.get(n) == Some(v)) && state.env.keys().all(|n| own_var(n) || s2.env.contains_key(n));
        let quiet = same_env && fp.rows.iter().all(|(o, _)| !older(*o)) && fp.slots.iter().all(|(o, _)| !older(*o)) && fp.elems.iter().all(|o| !older(*o)) && s2.lens.diff_keys(&state.lens).iter().all(|o| !older(*o));
        if quiet {
            self.cx.comp_known.pop();
            let mut joined = if nonempty { s2.copy() } else { state.join(&mut s2) };
            let ev = elem.value_part();
            self.promote_elem_reqs(&mut joined, &s2, &ev, state);
            state.replace_with(joined);
            self.exhaust(&exhaust_after, state);
            return;
        }
        // items of unknown number: what one iteration did, the next one sees too (a walrus
        // accumulator, a `pop` from a dict): rounds to a fixpoint, as a `for` loop has (whose
        // verdicts supersede the first pass's)
        self.findings.drop_since(vmark);
        let (mut head, _, s2) = self.loop_fixpoint(state, &mut |an, body_s| {
            an.comp_run_one(g, gens, i, limit, &elem, body_s, body, understood, own);
        });
        self.cx.comp_known.pop();
        // (at least one iteration: the states after one, from the fixpoint)
        if nonempty && self.mode.loop_converged && s2.reachable {
            head = s2.copy();
        }
        let ev = elem.value_part();
        self.promote_elem_reqs(&mut head, &s2, &ev, state);
        state.replace_with(head);
        // (a generator iterated: used up, its effects at its end)
        self.exhaust(&exhaust_after, state);
    }

    /// The one-shot iterator a generator expression run for a consumer that stops early takes
    /// its items from: it may stop at any of them, the iterator then just after it (as a `for`
    /// that may break, `unroll_for`).
    fn stops_in(&self, itv: &Value) -> Option<ObjId> {
        itv.single().filter(|o| self.at_stop_hook() && self.heap.obj(*o).iterator && self.heap.obj(*o).single)
    }

    /// Generator `i`'s items when they are known (at most `limit`), each its own iteration; a
    /// closed dict with some keys only maybe there: one iteration per key, a maybe one joined
    /// with skipping it. False: not known.
    #[allow(clippy::too_many_arguments)]
    fn comp_known_items(&mut self, gens: &'static [ast::Comprehension], i: usize, limit: usize, itv: &Value, it: &super::iter::Iteration, state: &mut State, body: &mut dyn FnMut(&mut Analyzer, &mut State), understood: bool, own: ObjId, cursor: Option<(ObjId, usize)>) -> bool {
        if let Some(items) = it.any_order_items() {
            let same = self.identical_summaries(items);
            if i == 0 && gens.len() == 1 && gens[i].ifs.is_empty() && (limit == SUMMARY || same || items.len() > limit) {
                self.cx.summary_items.insert(own, items.len());
            }
            if (items.len() <= limit || (limit != SUMMARY && self.distinct_objects(items))) && !same {
                let unordered = it.unordered;
                self.cx.comp_unordered += usize::from(unordered);
                let items: Vec<(Value, bool)> = items.iter().map(|v| (v.clone(), true)).collect();
                self.comp_unrolled(gens, i, limit, &items, state, body, understood, own, cursor);
                self.cx.comp_unordered -= usize::from(unordered);
                return true;
            }
            return false;
        }
        match self.maybe_dict_items(itv, state).filter(|its| its.len() <= limit) {
            Some(items) => {
                self.comp_unrolled(gens, i, limit, &items, state, body, understood, own, cursor);
                true
            }
            None => false,
        }
    }

    /// Known items: each is its own straight-line iteration; a filter decided for the item
    /// keeps it straight-line (positions survive), an undecided one makes the iteration
    /// conditional (joined with skipping the item).
    #[allow(clippy::too_many_arguments)]
    fn comp_unrolled(&mut self, gens: &'static [ast::Comprehension], i: usize, limit: usize, items: &[(Value, bool)], state: &mut State, body: &mut dyn FnMut(&mut Analyzer, &mut State), understood: bool, own: ObjId, cursor: Option<(ObjId, usize)>) {
        self.cx.comp_known.push(true);
        for (idx, (item, sure)) in items.iter().enumerate() {
            if !state.reachable {
                break;
            }
            let mut s2 = state.copy();
            if let Some((o, c0)) = cursor {
                self.set_cursor(o, Some(c0 + idx + 1), &mut s2);
            }
            self.frame_mut().unroll.push(idx);
            // (an item there on some paths only: its iteration is conditional)
            let maybe = usize::from(!*sure);
            self.cx.comp_maybe += maybe;
            self.cx.cond_depth += maybe as i32;
            self.comp_run_one(&gens[i], gens, i, limit, &Elem::One(item.clone()), &mut s2, body, understood, own);
            self.cx.comp_maybe -= maybe;
            self.cx.cond_depth -= maybe as i32;
            self.frame_mut().unroll.pop();
            // (the item's iteration, a filter's skip included; one that may not be there: or
            // not; one that raised or stopped its consumer everywhere: the items after it are
            // not taken)
            let j = if *sure { s2 } else { state.join(&mut s2) };
            state.replace_with(j);
        }
        self.cx.comp_known.pop();
    }

    /// Runs one iteration of generator `i`: `st` after it, joined with where a filter dropped
    /// the item (the state its test left).
    pub(super) fn comp_run_one(&mut self, g: &'static ast::Comprehension, gens: &'static [ast::Comprehension], i: usize, limit: usize, it: &Elem, st: &mut State, body: &mut dyn FnMut(&mut Analyzer, &mut State), understood: bool, own: ObjId) {
        self.comp_bind(&g.target, it, st);
        let mut decided = true;
        let mut dropped: Option<State> = None;
        for cond in &g.ifs {
            let mut sp = self.split(cond, st.copy());
            decided &= !sp.f.reachable;
            if sp.f.reachable {
                dropped = Some(match dropped {
                    Some(mut d) => d.join(&mut sp.f),
                    None => sp.f,
                });
            }
            st.replace_with(sp.t);
            if !st.reachable {
                break;
            }
        }
        if st.reachable {
            // (an item a filter may drop has no position of its own, understood filter or not)
            let conditional = !decided && !understood;
            self.cx.cond_depth += i32::from(conditional);
            self.cx.comp_maybe += usize::from(!decided);
            self.comp_gen(gens, i + 1, limit, st, body, own);
            self.cx.comp_maybe -= usize::from(!decided);
            self.cx.cond_depth -= i32::from(conditional);
        }
        if let Some(mut d) = dropped {
            let j = if st.reachable { st.join(&mut d) } else { d };
            st.replace_with(j);
        }
    }

    /// Binds a comprehension's target to an item (each pass over any item a view of its own),
    /// and the cells of its variables.
    pub(super) fn comp_bind(&mut self, target: &'static ast::Expr, it: &Elem, state: &mut State) {
        self.bind_iter_target(target, it, state);
        // (a pass standing for every item: each one an object of its own)
        if self.cx.comp_known.last() == Some(&false) {
            self.bind_views(target, state);
        }
        if !self.cx.comp_cells.is_empty() {
            // the variable's cell holds its latest value
            let ctx = self.frame().ctx.clone();
            let addr = target as *const ast::Expr as usize;
            for n in names_in(target) {
                let sid = s(&n);
                let cid = self.heap.alloc_raw(Kind::Cell, &format!("compcell:{}:{}", addr, n), &ctx, true, &format!("cell {}", n), Payload::None);
                let v = state.env.get(&sid).cloned().unwrap_or_default();
                state.set_slot(cid, Ns::Cell, super::funcs::cell_key(), v, true);
                self.cx.comp_cells.last_mut().unwrap().insert(sid, cid);
            }
        }
    }

    /// A dict comprehension: a dict whose keys and values the items give (strong per item when
    /// unrolled, else open through the keys).
    pub(super) fn expr_dictcomp(&mut self, dc: &'static ast::ExprDictComp, node: &'static ast::Expr, state: &mut State) -> Value {
        let nd = frontend::node(node);
        let oid = self.alloc_at(Kind::Dict, &nd, state, &format!("dictcomp@L{}", nd.line), Payload::None, None, Ns::Item, None, None);
        let base_depth = self.cx.cond_depth;
        let key_e: &'static ast::Expr = &dc.key;
        let val_e: &'static ast::Expr = &dc.value;
        // (items in an order Python does not promise: the keys' order is not known either)
        let mut unordered = false;
        let mut body = |an: &mut Analyzer, st: &mut State| {
            let k = an.eval_key(key_e, st);
            an.check_hashable(&k, st);
            let v = an.eval(val_e, st);
            let strong = an.heap.obj(oid).single && an.cx.cond_depth == base_depth && an.cx.comp_maybe == 0;
            an.store_key(oid, Ns::Item, &k, &v, strong, &nd, st);
            unordered |= an.cx.comp_unordered > 0;
        };
        let limit = comp_unroll_limit(&dc.generators, &[key_e, val_e], self.is_torch_module(self.frame().module));
        self.comp(&dc.generators, limit, state, &mut body, oid);
        if unordered {
            let row = state.row(oid, Ns::Item).with_order(Vec::new());
            state.set_row(oid, Ns::Item, row);
        }
        self.cx.summary_items.remove(&oid);
        Value::obj(oid)
    }

    /// A list, set or generator comprehension: a container of the items' values (positions per
    /// unrolled item when the filters are decided, else elements of no known length).
    pub(super) fn seq_comp(&mut self, node: &'static ast::Expr, kind: Kind, elt: &'static ast::Expr, gens: &'static [ast::Comprehension], state: &mut State) -> Value {
        let nd = frontend::node(node);
        let oid = self.alloc_at(kind, &nd, state, &format!("{}comp@L{}", kind.name(), nd.line), Payload::None, None, Ns::Item, None, None);
        self.heap.set_llen(oid, state, None);
        state.drop_obj_ns(oid, Ns::Pos);
        let base_depth = self.cx.cond_depth;
        let strong = self.heap.obj(oid).single && matches!(kind, Kind::List | Kind::Tuple | Kind::Set);
        // positions: every item known and every filter decided (straight-line iterations)
        let mut n: usize = 0;
        let mut positional = strong;
        let limit = comp_unroll_limit(gens, &[elt], self.is_torch_module(self.frame().module));
        // (a set's positions are its distinct members: a literal seen before adds none)
        let mut seen: Vec<KeyId> = Vec::new();
        let mut counted: Option<(usize, usize)> = Some((0, 0));
        let mut body = |an: &mut Analyzer, st: &mut State| {
            let v = an.eval(elt, st);
            if kind == Kind::Set {
                an.check_hashable(&v, st);
            }
            let dup = kind == Kind::Set && v.pts.is_empty() && v.kv.as_lit().map(|k| seen.contains(&k) || {
                seen.push(k);
                false
            }) == Some(true);
            let known = an.cx.comp_known.iter().all(|k| *k) && an.cx.cond_depth == base_depth;
            let straight = positional && known && an.cx.comp_maybe == 0 && an.cx.comp_unordered == 0;
            // (items known one by one: how many surely, how many a filter may drop)
            match (known, an.cx.comp_maybe) {
                (false, _) => counted = None,
                (true, 0) => counted = counted.map(|(s, m)| (s + usize::from(!dup), m)),
                (true, _) => counted = counted.map(|(s, m)| (s, m + 1)),
            }
            if straight && dup {
                // (already a member)
            } else if straight && n < limit {
                st.set_slot(oid, Ns::Pos, key_int(n as i64), v.clone(), true);
                n += 1;
            } else {
                positional = false;
            }
            an.add_elem(oid, &v, st);
        };
        if state.elems.get(&oid).is_none() {
            state.elems.insert(oid, BOT);
        }
        // (a source list with a first element, read before iterating it)
        let src_nonempty = strong && kind != Kind::Set && gens.len() == 1 && gens[0].ifs.is_empty() && !gens[0].is_async && match &gens[0].iter {
            ast::Expr::Name(src) => {
                let sv = self.load_name(s(src.id.as_str()), &Node::none(), state);
                sv.single().filter(|o| matches!(self.heap.kind(*o), Kind::List | Kind::Tuple) && sv.kv.is_bot()).map(|so| self.heap.prefix(so, state).is_some()).unwrap_or(false)
            }
            _ => false,
        };
        let mark = self.cx.comp_known.len();
        // (the tensors `torch.cat` joins: at least one iteration ran, or it raised)
        if std::mem::take(&mut self.mode.nonempty_comp) == node as *const ast::Expr as usize && gens.len() == 1 && gens[0].ifs.is_empty() {
            self.mode.nonempty_gen = true;
        }
        self.comp(gens, limit, state, &mut body, oid);
        self.cx.comp_known.truncate(mark);
        let summary_n = self.cx.summary_items.remove(&oid).filter(|_| strong && kind != Kind::Set);
        if positional {
            self.heap.set_llen(oid, state, Some(n));
        } else if let Some((sure, maybe)) = counted.filter(|(s, m)| strong && kind != Kind::Set && s + m < 16) {
            // (known items some filter may drop: the leading positions, and one of a few lengths)
            let mask = (sure..=sure + maybe).fold(0u16, |m, l| m | (1 << l));
            state.set_lens(oid, (None, n, mask));
        } else if let Some(n) = summary_n.filter(|n| *n <= 64) {
            // (known items in one pass: as many as there were, each the pass's element)
            let e = state.slot(oid, Ns::Elem, STAR);
            let ev = Value::with_kv(e.pts, state.elems.get(&oid).unwrap_or(DATA));
            state.drop_obj_ns(oid, Ns::Pos);
            for i in 0..n {
                state.set_slot(oid, Ns::Pos, key_int(i as i64), ev.clone(), true);
            }
            self.heap.set_llen(oid, state, Some(n));
        } else {
            self.heap.set_llen(oid, state, None);
            state.drop_obj_ns(oid, Ns::Pos);
            // `[f(x) for x in items]` over a list with a first element (`if items:`): the
            // result has one too (one of the body's values)
            if src_nonempty {
                let e = state.slot(oid, Ns::Elem, STAR);
                let ekv = state.elems.get(&oid).unwrap_or(DATA);
                if !(e.pts.is_empty() && ekv.is_bot()) {
                    state.set_slot(oid, Ns::Pos, key_int(0), Value::with_kv(e.pts, ekv), true);
                    state.set_lens(oid, (None, 1, 0));
                }
            }
        }
        if kind == Kind::Set {
            self.normalize_set(oid, state);
        }
        Value::obj(oid)
    }
}

/// How many known items a comprehension's generators unroll: up to 64 (as many literals as a
/// value keeps) when what it makes of each item calls nothing (`{w: i for i, w in
/// enumerate(words)}`: each word a sure key), else UNROLL_MAX. In a torch module, the
/// iterations of a comprehension that cannot be told apart (one generator, no filter, a target
/// its element does not use: `[Block(d) for _ in range(n)]`, a model's repeated layers) and
/// call something run as one summary iteration standing for all of them (its objects
/// summaries, the count kept): unrolled, every forward would run each layer's methods again
/// (a GPT of 12 blocks of 8 heads: 96 head runs per forward).
pub(super) fn comp_unroll_limit(gens: &[ast::Comprehension], parts: &[&ast::Expr], torch: bool) -> usize {
    let costly = |e: &ast::Expr| matches!(e, ast::Expr::Call(_) | ast::Expr::Lambda(_) | ast::Expr::ListComp(_) | ast::Expr::SetComp(_) | ast::Expr::DictComp(_) | ast::Expr::Generator(_) | ast::Expr::Await(_) | ast::Expr::Yield(_) | ast::Expr::YieldFrom(_) | ast::Expr::Named(_));
    if !parts.iter().any(|p| ruff_python_ast::helpers::any_over_expr(p, &costly)) {
        return 64;
    }
    let indistinct = match gens {
        [g] if g.ifs.is_empty() && !g.is_async => {
            let t = names_in(&g.target);
            !parts.iter().any(|p| names_in(p).iter().any(|n| t.contains(n)))
        }
        _ => false,
    };
    if indistinct && torch {
        SUMMARY
    } else {
        UNROLL_MAX
    }
}

/// `comp_unroll_limit` of a summary comprehension: no unrolling.
const SUMMARY: usize = 0;

/// `k != "x"`, `k not in (...)`, `k in (...)`, and conjunctions of those, on the key target.
pub fn cond_is_exclusion(cond: &ast::Expr, target: &ast::Expr) -> bool {
    let tnames = names_in(target);
    match cond {
        ast::Expr::BoolOp(b) if b.op == ast::BoolOp::And => b.values.iter().all(|c| cond_is_exclusion(c, target)),
        ast::Expr::UnaryOp(u) if u.op == ast::UnaryOp::Not => cond_is_exclusion(&u.operand, target),
        ast::Expr::Compare(c) if c.ops.len() == 1 => {
            let ast::Expr::Name(l) = &*c.left else { return false };
            if !tnames.iter().any(|n| n == l.id.as_str()) {
                return false;
            }
            let op = c.ops[0];
            let rhs = &c.comparators[0];
            let lit = is_constant(rhs)
                || match rhs {
                    ast::Expr::Tuple(t) => t.elts.iter().all(is_constant),
                    ast::Expr::List(l) => l.elts.iter().all(is_constant),
                    ast::Expr::Set(s) => s.elts.iter().all(is_constant),
                    _ => false,
                };
            if lit && matches!(op, ast::CmpOp::NotEq | ast::CmpOp::Eq | ast::CmpOp::In | ast::CmpOp::NotIn) {
                return true;
            }
            if matches!(op, ast::CmpOp::NotIn | ast::CmpOp::In) && matches!(rhs, ast::Expr::Name(_)) {
                return true;
            }
            false
        }
        _ => false,
    }
}

/// The keys an iteration's element ranges over are surely some (every object they are the
/// keys of surely has one).
fn surely_some_keys(elem: &Elem, state: &State) -> bool {
    let kv = match elem {
        Elem::One(v) => &v.kv,
        Elem::Pair(k, _) => &k.kv,
    };
    match kv {
        KeyVal::KeysOf(ko) => ko.objs.iter().all(|o| !state.row(o, ko.ns).definite.is_empty()),
        _ => false,
    }
}
