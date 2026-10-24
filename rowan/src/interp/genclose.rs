//! Generators closed where nothing holds them any more. CPython closes a suspended generator
//! when its last reference goes (`for v in gen(): break`, the only name holding it rebound, a
//! function's local at its return, the frame of a generator that held it closed):
//! GeneratorExit is raised at the yield it stands at, so its `finally` blocks and `__exit__`s
//! run there.
//!
//! A generator's whole run (`force_gen`) raises GeneratorExit at each yield inside a try of its
//! own (`close_point`); what comes out of its body (`GenBound`, per yield position, else
//! joined) is kept with its effects (`GenEffects::closed_at` / `closed`). What its suspended
//! frame holds of other such generators is the slot `\u{1}holds` of its object while it is
//! suspended (`Gens::holds`). Where one is dropped those facts are installed (`close_gen`):
//! - a `for`, a comprehension's level or a `yield from` over a generator made for it alone,
//!   at its `break`s, the returns from inside it and the raises leaving it (`closing_loop`);
//! - at a statement's end, a suspended one nothing reaches (`close_unheld`): surely closed; one
//!   some path may still hold, or one that may not have started: closed or not, joined;
//! - one the collector drops (`collect_since`); in a raise state, closed or not (a traceback
//!   keeps the frames it leaves, and their names, until its handler ends).

use rustc_hash::FxHashSet;

use super::Analyzer;
use crate::domain::{Ns, ObjId, Value};
use crate::intern::{self, key_int, key_str_lit};
use crate::state::State;

/// A generator's run whole (`force_gen`) in progress: the states GeneratorExit raised at its
/// yields leaves its body with (the yield's position, when known), and the generators its
/// frame holds at its yields.
pub struct GenBound {
    /// its frame's depth, and how many try contexts ran when it began
    pub depth: usize,
    pub tries: usize,
    pub recs: Vec<(Option<usize>, crate::state::Joins)>,
    pub holds: FxHashSet<ObjId>,
    /// its code may catch GeneratorExit and end normally: its end is a closed state too
    pub catches: bool,
    /// its code may run on without end (`while True:`): an end never reached is no raise
    pub endless: bool,
    /// GeneratorExit raised at a yield ran code on its way out (a finally block, an `__exit__`,
    /// the closing of a generator a loop of its held): else closing it changes nothing
    pub ran: bool,
}

/// How surely a generator is closed where `close_gen` closes it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Close {
    /// surely, and it has started (a loop over it is in its body)
    Started,
    /// surely, if it has started
    Sure,
    /// maybe (something may still hold it)
    Maybe,
}

/// How many objects the walks that look for what still holds a generator visit before they
/// take it as maybe held.
const MUST_WALK: usize = 512;
const MAY_WALK: usize = 4096;

/// The class of GeneratorExit raised at yield position `pos` of a run whole (a name no handler
/// names keeps the positions apart: a finally runs once per position).
pub fn close_class(pos: Option<usize>) -> super::ExcNames {
    let mut names = vec!["GeneratorExit".to_string(), "BaseException".to_string()];
    names.push(match pos {
        Some(p) => format!("\u{1}close{}", p),
        None => "\u{1}close".to_string(),
    });
    (std::rc::Rc::new(names), true)
}

/// The yield position of a class `close_class` made (Some(None): no position); None: not one.
pub fn close_pos(cls: &Option<super::ExcNames>) -> Option<Option<usize>> {
    let n = cls.as_ref()?.0.get(2)?.strip_prefix("\u{1}close")?;
    Some(n.parse::<usize>().ok())
}

fn closing_key() -> intern::KeyId {
    key_str_lit("\u{1}closing")
}

/// The slot `\u{1}closing` of a generator run whole whose code may catch GeneratorExit, on a
/// path not closing it.
const NOT_CLOSING: i64 = -2;

/// On this path (or on some of the paths joined here), generator `gid`'s code caught
/// GeneratorExit raised at a yield to close it: at that yield's position (None: not known),
/// and whether on every path joined here.
fn closing_at(gid: ObjId, state: &State) -> Option<(Option<usize>, bool)> {
    let v = state.slot_opt(gid, Ns::Attr, closing_key())?;
    let ks: Vec<Option<i64>> = crate::domain::kv_literals(&v.kv).iter().map(intern::key_as_int).collect();
    let sure = v.kv.is_literal() && !ks.contains(&Some(NOT_CLOSING));
    let at: Vec<Option<i64>> = ks.into_iter().filter(|k| *k != Some(NOT_CLOSING)).collect();
    if at.is_empty() && v.kv.is_literal() {
        return None;
    }
    let pos = match at[..] {
        [Some(p)] if p >= 0 => Some(p as usize),
        _ => None,
    };
    Some((pos, sure))
}

fn holds_key() -> intern::KeyId {
    key_str_lit("\u{1}holds")
}

fn ended_key() -> intern::KeyId {
    key_str_lit("\u{1}ended")
}

/// Whether generator `o` with closing facts is used up on this path: surely, surely not, or
/// on some of the paths joined here only (None).
fn ended(o: ObjId, state: &State) -> Option<bool> {
    let Some(v) = state.slot_opt(o, Ns::Attr, ended_key()) else { return Some(false) };
    let ks = crate::domain::kv_literals(&v.kv);
    match (ks.contains(intern::key_bool(true)), ks.contains(intern::key_bool(false))) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        _ => None,
    }
}

impl Analyzer {
    /// A yield of a generator run whole (one with a `GenBound` at this frame): GeneratorExit
    /// may be raised here, when a try of its own is running (its handlers, finally blocks and
    /// with statements, a loop over a generator made for it, run on it); the generators its
    /// frame holds are what its object holds while suspended.
    pub(super) fn close_point(&mut self, gid: ObjId, pos: Option<usize>, state: &mut State) {
        let depth = self.cx.frames.len();
        let Some(b) = self.gens.bounds.last().filter(|b| b.depth == depth) else { return };
        let tries = b.tries;
        // (reached on a path where its code caught GeneratorExit raised at an earlier yield:
        // closing it ends here — `close()` raises RuntimeError, nothing of it runs on — and
        // no consumer resumes it from here)
        if let Some((p, sure)) = closing_at(gid, state) {
            let b = self.gens.bounds.last_mut().unwrap();
            b.ran = true;
            match b.recs.iter_mut().find(|r| r.0 == p) {
                Some(r) => r.1.add(state.copy()),
                None => {
                    let mut j = crate::state::Joins::default();
                    j.add(state.copy());
                    b.recs.push((p, j));
                }
            }
            if sure {
                state.reachable = false;
                return;
            }
            state.set_slot(gid, Ns::Attr, closing_key(), Value::lit(key_int(NOT_CLOSING)), true);
        }
        if !self.gens.closable.is_empty() {
            let mut held: Vec<ObjId> = self.gens.fresh_loops.iter().filter(|l| l.1 == depth).map(|l| l.2).collect();
            for v in state.env.values() {
                held.extend(v.pts.iter().filter(|o| self.gens.closable.contains_key(o)));
            }
            if let Some(b) = self.gens.bounds.last_mut() {
                b.holds.extend(held);
            }
        }
        if self.unwind.tries.len() > tries {
            let mut here = state.copy();
            self.unwind.cur_exc_cls = Some(close_class(pos));
            self.may_raise_here("GeneratorExit", &mut here);
        }
    }

    /// A generator's run whole begins (its frame pushed): GeneratorExit raised at its yields is
    /// recorded where it leaves the body (`GenBound`). True.
    pub(super) fn bound_begin(&mut self, fnode: super::FuncNode) -> bool {
        let body = match fnode {
            super::FuncNode::Def(d) => Some(&d.body),
            _ => None,
        };
        let catches = body.is_some_and(|b| crate::syntax::catches_generator_exit(b));
        let endless = body.is_some_and(|b| crate::syntax::has_endless_loop(b));
        self.gens.bounds.push(GenBound { depth: self.cx.frames.len(), tries: self.unwind.tries.len(), recs: Vec::new(), holds: Default::default(), catches, endless, ran: false });
        true
    }

    /// A raise of class `cls` runs code on its way out: when it is GeneratorExit raised at a
    /// yield, closing that generator does something.
    pub(in crate::interp) fn note_close_ran(&mut self, cls: &Option<super::ExcNames>) {
        if close_pos(cls).is_some() {
            if let Some(b) = self.gens.bounds.last_mut() {
                b.ran = true;
            }
        }
    }

    /// GeneratorExit raised at a yield leaves the body of the generator run whole there:
    /// recorded for its closed state (true), or not such a raise (false).
    pub(super) fn record_close(&mut self, cls: &Option<super::ExcNames>, state: &State) -> bool {
        let Some(pos) = close_pos(cls) else { return false };
        let n = self.unwind.tries.len();
        match self.gens.bounds.last_mut().filter(|b| b.tries == n) {
            Some(b) => {
                match b.recs.iter_mut().find(|r| r.0 == pos) {
                    Some(r) => r.1.add(state.copy()),
                    None => {
                        let mut j = crate::state::Joins::default();
                        j.add(state.copy());
                        b.recs.push((pos, j));
                    }
                }
                true
            }
            None => false,
        }
    }

    /// What a run whole recorded of its closing (`GenBound`): the states closed at each yield
    /// position, joined over all (its end too where its code may catch GeneratorExit), and
    /// whether every one had its position. What its frame held becomes `Gens::holds`.
    pub(super) fn closed_states(&mut self, gid: ObjId, b: Option<GenBound>, end: Option<&State>) -> (Vec<Option<State>>, Option<State>, bool) {
        let Some(b) = b else {
            self.gens.holds.remove(&gid);
            return (Vec::new(), None, true);
        };
        if b.holds.is_empty() {
            self.gens.holds.remove(&gid);
        } else {
            let mut h: Vec<ObjId> = b.holds.into_iter().collect();
            h.sort_unstable();
            self.gens.holds.insert(gid, Value::objs(crate::domain::ObjSet::from_vec(h)));
        }
        // (no code ran on GeneratorExit's way out: closing it changes nothing)
        if !b.ran && !b.catches {
            return (Vec::new(), None, true);
        }
        let join = |acc: &mut Option<State>, s: &State| {
            *acc = Some(match acc.take() {
                Some(mut a) => a.join(&mut s.copy()),
                None => s.copy(),
            });
        };
        let mut at: Vec<Option<State>> = Vec::new();
        let mut all: Option<State> = None;
        let mut marked_only = true;
        for (pos, s) in b.recs.into_iter().filter_map(|(p, j)| Some((p, j.finish()?))) {
            match pos {
                Some(p) => {
                    if at.len() <= p {
                        at.resize_with(p + 1, || None);
                    }
                    join(&mut at[p], &s);
                }
                None => marked_only = false,
            }
            join(&mut all, &s);
        }
        // (its code caught GeneratorExit raised at a yield and went on to its end)
        if let Some((p, e)) = end.and_then(|e| Some((closing_at(gid, e)?.0, e))) {
            match p {
                Some(p) => {
                    if at.len() <= p {
                        at.resize_with(p + 1, || None);
                    }
                    join(&mut at[p], e);
                }
                None => marked_only = false,
            }
            join(&mut all, e);
        }
        (at, all, marked_only)
    }

    /// A generator's run whole begins: where its code may catch GeneratorExit, its paths are
    /// not closing it yet (`closing_caught`).
    pub(super) fn closing_begin(&self, gid: ObjId, state: &mut State) {
        if self.gens.bounds.last().is_some_and(|b| b.catches) {
            state.set_slot(gid, Ns::Attr, closing_key(), Value::lit(key_int(NOT_CLOSING)), true);
        }
    }

    /// A handler of the running generator's own catches GeneratorExit raised at its yield
    /// `pos` to close it (`state`: the handler's): its path is one of closing it.
    pub(in crate::interp) fn closing_caught(&self, pos: Option<usize>, only: bool, state: &mut State) {
        let Some(foid) = self.frame().func else { return };
        let Some(gid) = self.heap.find(crate::state::Kind::List, &format!("gen:{}", foid), &self.frame().ctx) else { return };
        // (beside other raises it catches: on some of its paths only)
        let mut v = Value::lit(key_int(pos.map_or(-1, |p| p as i64)));
        if !only {
            v = v.join(&Value::lit(key_int(NOT_CLOSING)));
        }
        state.set_slot(gid, Ns::Attr, closing_key(), v, true);
    }

    /// Is `o` a generator that may be suspended on this path, with closing facts of its own,
    /// not closed yet (maybe closed: left alone)?
    pub(in crate::interp) fn may_close(&self, o: ObjId, state: &State) -> bool {
        self.has_facts(o, state) && !self.at_end(o, state) && self.cursor_of(o, state) != Some(0) && state.slot_opt(o, Ns::Attr, key_str_lit("\u{1}mayclose")).is_none() && ended(o, state) != Some(true)
    }

    /// May generator `o` not have given an item yet on this path (its cursor 0 on some path:
    /// a literal 0, or one kept beside a position not known)?
    pub(in crate::interp) fn may_be_unstarted(&self, o: ObjId, state: &State) -> bool {
        let zero = key_int(0);
        match state.slot_opt(o, Ns::Attr, key_str_lit("__cursor")).map(|v| v.kv) {
            None => true,
            Some(crate::domain::KeyVal::Data(_, h)) => h.contains(zero) || h.contains(intern::STAR),
            Some(kv) => crate::domain::kv_literals(&kv).contains(zero) || !kv.is_literal(),
        }
    }

    /// Generator `o` made anew on this path: not closed, not used up, holding nothing yet.
    pub(super) fn reset_closing(&self, o: ObjId, state: &mut State) {
        if self.gens.closable.contains_key(&o) {
            for k in [key_str_lit("\u{1}mayclose"), ended_key(), holds_key()] {
                state.slots.remove(&o, &(Ns::Attr, k));
            }
        }
    }

    /// Generator `o` with closing facts was used up on this path (`end`), or asked for an item.
    pub(super) fn note_ended(&self, o: ObjId, end: bool, state: &mut State) {
        if self.gens.closable.contains_key(&o) {
            state.set_slot(o, Ns::Attr, ended_key(), Value::lit(intern::key_bool(end)), true);
        }
    }

    /// Generator `o` is closed here (`how`): its facts as GeneratorExit at the yield it stands
    /// at leaves them, its cursor at its end; maybe: those facts joined, where it stands not
    /// known. One not started yet runs nothing.
    pub(in crate::interp) fn close_gen(&mut self, o: ObjId, state: &mut State, how: Close) {
        if !state.reachable || !self.has_facts(o, state) || self.at_end(o, state) {
            return;
        }
        let k = self.cursor_of(o, state);
        let unstarted = self.may_be_unstarted(o, state);
        let how = match how {
            Close::Started => Close::Sure,
            _ if k == Some(0) => return,
            Close::Sure if unstarted || ended(o, state).is_none() => Close::Maybe,
            h => h,
        };
        // (closing resumes it: what the program changed since, it sees — `gen_interference`)
        self.note_resume(o, state);
        let ge = self.gens.eff.get(&o).cloned();
        if super::debug_enabled() {
            eprintln!("close {} at {:?} ({:?})", self.heap.obj(o).label, k, how);
        }
        let target = ge.as_ref().and_then(|ge| {
            let at = k.filter(|c| *c > 0).and_then(|c| ge.closed_at.get(c - 1).and_then(|s| s.as_ref()).map(|_| c - 1));
            match at {
                Some(i) => Some((format!("closed{}", i), ge.closed_at[i].as_ref()?.copy())),
                None => Some(("closed".to_string(), ge.closed.as_ref()?.copy())),
            }
        });
        let tk = key_str_lit("\u{1}effpos");
        let ck = key_str_lit("__cursor");
        // (maybe: where it stands, its facts as given joined with the closed ones — a later item
        // installs its own facts joined with these, as they then differ from those given)
        if how == Close::Maybe {
            if let (Some(ge), Some((_, t))) = (&ge, &target) {
                let given = super::gens::effects_given(state, o, ge).unwrap_or(&ge.pre).copy();
                super::gens::gen_join_facts(state, t, ge, &given);
            }
            let c = state.slot(o, Ns::Attr, ck).join(&Value::data("cursor-end"));
            state.set_slot(o, Ns::Attr, ck, c, true);
            state.set_slot(o, Ns::Attr, key_str_lit("\u{1}mayclose"), Value::lit(key_int(1)), true);
            return;
        }
        if let (Some(ge), Some((tag, t))) = (&ge, &target) {
            let given = super::gens::effects_given(state, o, ge).is_some_and(|e| super::gens::gen_facts_equal(state, e, ge));
            if given {
                super::gens::gen_copy_facts(state, t, ge);
                state.set_slot(o, Ns::Attr, tk, Value::lit(key_str_lit(tag)), true);
            } else {
                let base = super::gens::effects_given(state, o, ge).unwrap_or(&ge.pre).copy();
                super::gens::gen_join_facts(state, t, ge, &base);
                state.set_slot(o, Ns::Attr, tk, Value::data("effpos"), true);
            }
        }
        state.slots.remove(&o, &(Ns::Attr, holds_key()));
        state.set_slot(o, Ns::Attr, ck, Value::data("cursor-end"), true);
        self.note_ended(o, true, state);
    }

    /// What generator `o`'s suspended frame holds (`Gens::holds`), as its slot: there while it
    /// is suspended, gone at its end.
    pub(super) fn set_holds(&self, o: ObjId, ended: bool, state: &mut State) {
        let Some(h) = self.gens.holds.get(&o) else { return };
        if ended {
            state.slots.remove(&o, &(Ns::Attr, holds_key()));
        } else {
            state.set_slot(o, Ns::Attr, holds_key(), h.clone(), true);
        }
    }

    /// Does closing generator `o` (its last run) do something: closing facts of its own, or
    /// generators with them its frame holds?
    pub(in crate::interp) fn closes_on_drop(&self, o: ObjId) -> bool {
        self.gens.eff.get(&o).is_some_and(|ge| ge.closed.is_some()) || self.gens.holds.contains_key(&o)
    }

    /// A generator with closing facts was asked for items here: what code at this depth or
    /// above drops may close it (code a model calls back runs deeper: what the model holds is
    /// not dropped there).
    pub(super) fn note_closable(&mut self, o: ObjId) {
        if self.closes_on_drop(o) {
            self.gens.closable.insert(o, self.cx.frames.len());
        }
    }

    /// At a statement's end: the suspended generators with closing facts nothing holds any
    /// more are closed (some path may hold one, or the walk was cut: maybe closed).
    pub(in crate::interp) fn close_unheld(&mut self, state: &mut State) {
        if self.gens.closable.is_empty() || !state.reachable {
            return;
        }
        let depth = self.cx.frames.len();
        loop {
            let mut cands: Vec<ObjId> = self.gens.closable.iter().filter(|(_, d)| **d >= depth).map(|(o, _)| *o).filter(|o| self.may_close(*o, state)).collect();
            if cands.is_empty() {
                return;
            }
            cands.sort_unstable();
            let must = self.must_held(&cands, state);
            cands.retain(|o| !must.contains(o));
            if cands.is_empty() {
                return;
            }
            let (may, cut) = self.may_held(&cands, state);
            let mut surely = false;
            for o in cands {
                let how = if cut || may.contains(&o) { Close::Maybe } else { Close::Sure };
                surely |= how == Close::Sure;
                self.close_gen(o, state, how);
            }
            // (one surely closed drops what its frame held: those may be unheld now)
            if !surely {
                return;
            }
        }
    }

    /// Which of `targets` (sorted) are surely held: reached from the names bound on every path,
    /// the modules' globals, the loops running, through values that are one object each, under
    /// keys surely there.
    fn must_held(&self, targets: &[ObjId], state: &State) -> FxHashSet<ObjId> {
        let mut work: Vec<ObjId> = Vec::new();
        let one = |v: &Value, work: &mut Vec<ObjId>| work.extend(v.single());
        for (n, v) in state.env.iter() {
            if !state.unbound.contains(n) {
                one(v, &mut work);
            }
        }
        for (env, ub) in self.cx.env_stack.iter().zip(self.cx.unbound_stack.iter()) {
            for (n, v) in env.iter() {
                if !ub.contains(n) {
                    one(v, &mut work);
                }
            }
        }
        for fr in &self.cx.frames {
            work.push(fr.module);
            work.extend(fr.cells.values().copied());
        }
        for it in &self.gens.for_iters {
            if it.len() == 1 {
                work.extend(it.first());
            }
        }
        work.extend(self.gens.fresh_loops.iter().map(|l| l.2));
        let mut sure = |o: ObjId, work: &mut Vec<ObjId>| {
            let len = self.heap.llen(o, state);
            for ((ns, k), v) in state.slots_of(o) {
                let sure = match ns {
                    Ns::Attr | Ns::Item => k == holds_key() || state.rows.get(&(o, ns)).is_some_and(|r| r.definite.contains(k)),
                    Ns::Pos => intern::key_as_int(k).is_some_and(|i| len.is_some_and(|n| i >= 0 && (i as usize) < n)),
                    Ns::Cell => true,
                    _ => false,
                };
                if sure {
                    one(&v, work);
                }
            }
        };
        self.find_among(work, targets, MUST_WALK + 1, false, &mut sure).unwrap_or_else(|found| found)
    }

    /// Which of `targets` anything may still hold (a walk from every root the collector has,
    /// the loops running), and whether the walk was cut before it looked everywhere.
    fn may_held(&self, targets: &[ObjId], state: &State) -> (FxHashSet<ObjId>, bool) {
        let mut work: Vec<ObjId> = Vec::new();
        self.state_roots(state, usize::MAX, usize::MAX, &mut work);
        // (not the side tables' entries: what their keys hold is followed from them; the
        // arguments of running calls, which their callers hold until they return)
        self.held_by_analyzer(&mut work, None);
        self.recursion.running_args(&mut work);
        for (o, v) in state.lasts.iter() {
            work.push(*o);
            v.push_objs(&mut work);
        }
        for it in &self.gens.for_iters {
            work.extend(it.iter());
        }
        work.extend(self.gens.fresh_loops.iter().map(|l| l.2));
        match self.find_among(work, targets, MAY_WALK, true, &mut |o, work| self.obj_edges(o, state, work)) {
            Ok(found) => (found, false),
            Err(found) => (found, true),
        }
    }

    /// `body` runs a loop over generator `o` made for it alone (a `for`, a comprehension's
    /// level, a `yield from`; `level`: the loop contexts running inside it, for its `break`s):
    /// where it leaves before `o` is used up — a break, a return from inside it, a raise — `o`
    /// is closed. Other generators: `body` alone.
    pub(in crate::interp) fn closing_loop(&mut self, o: Option<ObjId>, level: usize, state: &mut State, body: &mut dyn FnMut(&mut Analyzer, &mut State)) {
        let Some(o) = o.filter(|o| self.closes_on_drop(*o)) else {
            body(self, state);
            return;
        };
        let outer_exits = state.exits.take();
        let mut pre = state.copy();
        self.enter_try(FxHashSet::default(), &mut pre);
        // (a raise leaving the loop closes `o`: its code runs)
        if let Some(t) = self.unwind.tries.last_mut() {
            t.runs = true;
        }
        self.gens.fresh_loops.push((level, self.cx.frames.len(), o));
        body(self, state);
        self.gens.fresh_loops.pop();
        let exit = self.leave_try();
        for mut rec in exit.recs {
            self.note_close_ran(&rec.0);
            self.close_gen(o, &mut rec.1, Close::Started);
            self.reraise_rec(rec);
        }
        let inner = state.exits.take().map(|ex| {
            let mut e = ex.copy();
            e.reachable = true;
            self.close_gen(o, &mut e, Close::Started);
            e
        });
        state.exits = match (outer_exits, inner) {
            (Some(a), Some(mut b)) => Some(std::sync::Arc::new(a.copy().join(&mut b))),
            (a, b) => a.or(b.map(std::sync::Arc::new)),
        };
    }

    /// `break` in loop context `level` closes the generators made for that loop alone.
    pub(super) fn close_at_break(&mut self, level: usize, state: &mut State) {
        let depth = self.cx.frames.len();
        let gens: Vec<ObjId> = self.gens.fresh_loops.iter().filter(|l| l.0 == level && l.1 == depth).map(|l| l.2).collect();
        for o in gens {
            self.close_gen(o, state, Close::Started);
        }
    }

    /// The generator `v` is, when it is one made by `expr` and by nothing else, not run yet
    /// (a loop over it is all that holds it).
    pub(in crate::interp) fn fresh_gen(&self, v: &Value, expr: &ruff_python_ast::Expr, state: &State) -> Option<ObjId> {
        let o = v.single()?;
        let lg = self.gens.lazy.get(&o)?;
        (lg.node.addr == crate::frontend::node(expr).addr && super::gens::lazy_state(state, o) == (true, false)).then_some(o)
    }
}
