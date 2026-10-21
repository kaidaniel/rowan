//! A generator raises where its consumer asks for the item it fails to give. Run whole for a
//! loop, a comprehension or `next` inside a try, its raises are kept apart (`TryCtx::gen`), by
//! how many items it had given (`GenRaises`), and taken where the consumer moves its cursor
//! past them (`replay_gen_raises`): from the consumer's state then, with what the generator had
//! changed by the raise joined in. A consumer that uses up one whose body surely raises before
//! its end goes on nowhere after it. A try that ends takes the raises no consumer asked for
//! yet as the run recorded them (code rowan does not follow may ask), and they stay kept for
//! the tries around it.

use rustc_hash::FxHashSet;

use super::heap::Heap;
use super::Analyzer;
use crate::domain::{Ns, ObjId, Value};
use crate::intern::STAR;
use crate::state::{Footprint, State};

/// The raises a generator's run for a consumer recorded (`level`: how many tries ran then, or
/// fewer once those ended), the states before and after the run, its allocation mark.
pub struct GenRaises {
    pub gid: ObjId,
    pub level: usize,
    pre: State,
    after: State,
    mark: u64,
    recs: Vec<Kept>,
    /// the frame of the loop it was run for (None: `next`'s), where it was asked for items, for
    /// its raises whose place is not known, joined while the innermost try (that many tries)
    /// runs: raised from there when that ends or the loop does
    frame: Option<usize>,
    asked: Option<(State, usize)>,
    /// the frame depth it ran at
    depth: usize,
}

/// A raise kept: its record, how many items the generator had given (None: not known), the
/// facts of the objects the run made that differ from those after it, the older objects whose
/// facts it had changed, whether a consumer asked for it.
struct Kept {
    rec: super::RaiseRec,
    pos: Option<usize>,
    made: Footprint,
    touched: FxHashSet<ObjId>,
    taken: bool,
}

/// How many generators' raises are kept at most (`Gens::raises`).
const KEPT: usize = 32;

/// How many items generator `g` had given in `st` (None: not known, or none yet).
pub(super) fn items_given(heap: &Heap, g: ObjId, st: &State) -> Option<usize> {
    st.has_slot(g, Ns::Elem, STAR).then(|| heap.llen(g, st)).flatten()
}

/// The keys of `a`'s maps that differ from `b`'s, of the objects `keep` keeps, added to `fp`.
fn add_diff(fp: &mut Footprint, a: &State, b: &State, keep: &dyn Fn(ObjId) -> bool) {
    fp.rows.extend(a.rows.diff_keys(&b.rows).into_iter().filter(|k| keep(k.0)));
    fp.slots.extend(a.slots.diff_keys(&b.slots).into_iter().filter(|k| keep(k.0)));
    fp.elems.extend(a.elems.diff_keys(&b.elems).into_iter().filter(|o| keep(*o)));
}

impl Analyzer {
    /// `force_gen` for a consumer that asks for items one at a time (a loop, a comprehension,
    /// `next`): inside a try, the raises of the generator's run are kept for the points where
    /// it asks for them (`Gens::raises`). The generator, when they were.
    pub(super) fn force_gen_deferring(&mut self, itv: &Value, state: &mut State, looped: bool) -> Option<ObjId> {
        let o = itv.single().filter(|o| itv.kv.is_bot() && !self.unwind.tries.is_empty() && self.gens.lazy.contains_key(o) && super::gens::lazy_state(state, *o) == (true, false));
        let Some(o) = o else {
            self.force_gen(itv, state, true);
            return None;
        };
        let (level, mark) = (self.unwind.tries.len(), self.heap.serial());
        let mut pre = state.copy();
        self.enter_try(FxHashSet::default(), &mut pre);
        if let Some(t) = self.unwind.tries.last_mut() {
            t.gen = Some(o);
        }
        self.force_gen(itv, state, true);
        let exit = self.leave_try();
        // (no item on any path: it raised where it was asked for its first one, here)
        if !state.reachable {
            exit.recs.into_iter().for_each(|r| self.reraise_rec(r));
            return None;
        }
        let born = |x: ObjId| !self.heap.born_before(x, mark);
        let recs: Vec<Kept> = exit.recs.into_iter().zip(exit.pos).map(|(rec, pos)| {
            let (mut made, mut old) = (Footprint::default(), Footprint::default());
            add_diff(&mut made, &rec.1, state, &born);
            add_diff(&mut old, &rec.1, &pre, &|x| !born(x));
            let touched = old.rows.iter().map(|k| k.0).chain(old.slots.iter().map(|k| k.0)).chain(old.elems.iter().copied()).collect();
            Kept { rec, pos, made, touched, taken: false }
        }).collect();
        // (one run before at this site, or the oldest past `KEPT`: what was not asked for, where
        // it is now)
        if let Some(i) = self.gens.raises.iter().position(|r| r.gid == o).or_else(|| (self.gens.raises.len() >= KEPT).then_some(0)) {
            self.flush_asked(i);
            let old = self.gens.raises.remove(i);
            old.recs.into_iter().filter(|k| !k.taken).for_each(|k| self.reraise_recorded(k.rec, old.depth));
        }
        let frame = looped.then_some(self.cx.frames.len());
        let depth = self.cx.frames.len();
        self.gens.raises.push(GenRaises { gid: o, level, pre, after: state.copy(), mark, recs, frame, asked: None, depth });
        Some(o)
    }

    /// Are raises of generator `o`'s run kept for where it is asked for items?
    pub(super) fn raises_kept(&self, o: ObjId) -> bool {
        self.gens.raises.iter().any(|r| r.gid == o && !r.recs.is_empty())
    }

    /// The loop over generator `o` ended: its raises from where the loop asked for items; one
    /// made for it alone (`fresh`): nothing asks it for more.
    pub(super) fn loop_gen_raises_end(&mut self, o: ObjId, fresh: bool) {
        if let Some(i) = self.gens.raises.iter().rposition(|r| r.gid == o) {
            self.flush_asked(i);
            if fresh {
                self.gens.raises.remove(i);
            }
        }
    }

    /// The try at `level` ends (the `level`-th running): the raises kept since it began that no
    /// consumer asked for yet are taken as the generator's run recorded them, as code it does
    /// not see may ask for them; they are kept for the try around it.
    pub(super) fn flush_gen_raises(&mut self, level: usize) {
        for i in 0..self.gens.raises.len() {
            if self.gens.raises[i].asked.as_ref().is_some_and(|a| a.1 >= level) {
                self.flush_asked(i);
            }
        }
        let mut flushed: Vec<(super::RaiseRec, usize)> = Vec::new();
        for g in self.gens.raises.iter_mut().filter(|r| r.level >= level) {
            g.level = level - 1;
            flushed.extend(g.recs.iter().filter(|k| !k.taken).map(|k| ((k.rec.0.clone(), k.rec.1.copy(), k.rec.2.clone()), g.depth)));
        }
        flushed.into_iter().for_each(|(r, d)| self.reraise_recorded(r, d));
    }

    /// A raise a run at frame depth `depth` recorded, raised as recorded into the running try:
    /// from a shallower frame now, with the names the try's frame had where it began (the
    /// recorded ones are the deeper frame's).
    fn reraise_recorded(&mut self, mut rec: super::RaiseRec, depth: usize) {
        if let Some(t) = self.unwind.tries.last().filter(|_| depth != self.cx.frames.len()) {
            rec.1.env = t.pre.env.clone();
            rec.1.unbound = t.pre.unbound.clone();
            rec.1.narrowed = t.pre.narrowed.clone();
            rec.1.ret = t.pre.ret.clone();
            rec.1.exits = t.pre.exits.clone();
        }
        self.reraise_rec(rec);
    }

    /// A loop's rounds over `itv` ended at `head`, which each round's start (where it asks for an
    /// item) joins: the raises of a generator it iterates whose cursor no round knows.
    pub(super) fn gen_raises_at_rounds(&mut self, itv: &Value, head: &State) {
        let Some(o) = itv.single().filter(|o| self.gens.raises.iter().any(|r| r.gid == *o)) else { return };
        let mut h = head.copy();
        self.replay_gen_raises(o, None, None, false, &mut h);
    }

    /// Generator `o` is asked for the items after the first `from` (None: not known) up to
    /// `to` (None: all) in `state`: its raises there (`raise_kept`); those whose place is not
    /// known, asked for in its loop's frame, once that loop or the innermost try ends, from where
    /// it was asked joined (`GenRaises::asked`: the try joins its raises all the same). Used up
    /// (`end`) where its body surely raises before its end: nothing goes on.
    pub(in crate::interp) fn replay_gen_raises(&mut self, o: ObjId, from: Option<usize>, to: Option<usize>, end: bool, state: &mut State) {
        let Some(g) = self.gens.raises.iter().rposition(|r| r.gid == o).filter(|_| state.reachable) else { return };
        let within = |p: &Option<usize>| p.is_some_and(|p| from.is_none_or(|f| p >= f) && to.is_none_or(|t| p < t));
        self.raise_kept(g, &within, state);
        let tries = self.unwind.tries.len();
        let own = self.gens.raises[g].frame == Some(self.cx.frames.len());
        if own && self.gens.raises[g].recs.iter().any(|k| k.pos.is_none()) {
            match self.gens.raises[g].asked.take() {
                Some((mut a, n)) if n == tries => self.gens.raises[g].asked = Some((a.join(&mut state.copy()), n)),
                other => {
                    self.gens.raises[g].asked = other;
                    self.flush_asked(g);
                    self.gens.raises[g].asked = Some((state.copy(), tries));
                }
            }
        } else {
            self.raise_kept(g, &|p: &Option<usize>| p.is_none(), state);
        }
        if end && to.is_none() && self.gens.raising.contains_key(&o) {
            state.reachable = false;
        }
    }

    /// The raises of generator `g` (in `Gens::raises`) whose place is not known, from where it
    /// was asked for items (`GenRaises::asked`).
    fn flush_asked(&mut self, g: usize) {
        if let Some((a, _)) = self.gens.raises[g].asked.take() {
            self.raise_kept(g, &|p: &Option<usize>| p.is_none(), &a);
        }
    }

    /// The raises of generator `g` whose place `which` picks, raised from `state`, with what it
    /// changed since the facts the consumer was given of it (`effects_given`; else since before
    /// it ran), and where the consumer changed what it made or changed too: both.
    fn raise_kept(&mut self, g: usize, which: &dyn Fn(&Option<usize>) -> bool, state: &State) {
        if !state.reachable || !self.gens.raises[g].recs.iter().any(|k| which(&k.pos)) {
            return;
        }
        let gr = &self.gens.raises[g];
        let (o, mark) = (gr.gid, gr.mark);
        let born = |x: ObjId| !self.heap.born_before(x, mark);
        let ge = self.gens.eff.get(&o).cloned();
        let base = ge.as_ref().and_then(|ge| super::gens::effects_given(state, o, ge)).unwrap_or(&gr.pre);
        let mut changed = Footprint::default();
        add_diff(&mut changed, state, &gr.after, &|_| true);
        let mut picked: Vec<(super::RaiseRec, Footprint)> = Vec::new();
        for k in gr.recs.iter().filter(|k| which(&k.pos)) {
            let mut fp = k.made.clone();
            add_diff(&mut fp, &k.rec.1, base, &|x| !born(x));
            let mine = |x: ObjId| born(x) || k.touched.contains(&x);
            fp.rows.extend(changed.rows.iter().copied().filter(|x| mine(x.0)));
            fp.slots.extend(changed.slots.iter().copied().filter(|x| mine(x.0)));
            fp.elems.extend(changed.elems.iter().copied().filter(|x| mine(*x)));
            picked.push(((k.rec.0.clone(), k.rec.1.copy(), k.rec.2.clone()), fp));
        }
        self.gens.raises[g].recs.iter_mut().filter(|k| which(&k.pos)).for_each(|k| k.taken = true);
        for ((cls, raised, exc), fp) in picked {
            let mut here = state.copy();
            here.join_heap_fp(&raised, &fp);
            self.reraise_rec((cls, here, exc));
        }
    }
}
