//! A generator raises where its consumer asks for the item it fails to give. Run whole for a
//! `for` loop inside a try, its raises are kept apart (`TryCtx::gen`), by how many items it had
//! given (`GenRaises`), and taken where the loop moves its cursor past them
//! (`replay_gen_raises`): from the loop's state then, with what the generator had changed by
//! the raise joined in. A loop that uses up one whose body surely raises before its end goes on
//! nowhere after it.

use rustc_hash::FxHashSet;

use super::heap::Heap;
use super::Analyzer;
use crate::domain::{Ns, ObjId, Value};
use crate::intern::STAR;
use crate::state::{Footprint, State};

/// The raises a generator's run for a loop recorded, not taken yet: per raise, its record, how
/// many items the generator had given (None: not known), what the run had changed by then.
pub struct GenRaises {
    pub gid: ObjId,
    pub recs: Vec<(super::RaiseRec, Option<usize>, Footprint)>,
}

/// How many items generator `g` had given in `st` (None: not known, or none yet).
pub(super) fn items_given(heap: &Heap, g: ObjId, st: &State) -> Option<usize> {
    st.has_slot(g, Ns::Elem, STAR).then(|| heap.llen(g, st)).flatten()
}

impl Analyzer {
    /// `force_gen` for a `for` loop: inside a try, the raises of the generator's run are kept
    /// for the points where the loop asks for its items (`Gens::raises`).
    pub(super) fn force_gen_for_loop(&mut self, itv: &Value, state: &mut State) {
        let o = itv.single().filter(|o| itv.kv.is_bot() && !self.unwind.tries.is_empty() && self.gens.lazy.contains_key(o) && super::gens::lazy_state(state, *o) == (true, false));
        let Some(o) = o else {
            self.force_gen(itv, state, true);
            return;
        };
        let mut pre = state.copy();
        self.enter_try(FxHashSet::default(), &mut pre);
        if let Some(t) = self.unwind.tries.last_mut() {
            t.gen = Some(o);
        }
        self.force_gen(itv, state, true);
        let exit = self.leave_try();
        // (no item on any path: it raised where the loop asked for its first one, here)
        if !state.reachable {
            exit.recs.into_iter().for_each(|r| self.reraise_rec(r));
            return;
        }
        let recs = exit.recs.into_iter().zip(exit.pos).map(|(r, p)| {
            let fp = r.1.footprint(&pre);
            (r, p, fp)
        });
        self.gens.raises.push(GenRaises { gid: o, recs: recs.collect() });
    }

    /// The loop asks generator `o` for the items after the first `from` (None: not known) up to
    /// `to` (None: all): the raises its run recorded there are raised from `state`, with what it
    /// changed by then. Used up (`end`) where its body surely raises before its end: nothing
    /// goes on.
    pub(in crate::interp) fn replay_gen_raises(&mut self, o: ObjId, from: Option<usize>, to: Option<usize>, end: bool, state: &mut State) {
        let Some(g) = self.gens.raises.iter().rposition(|r| r.gid == o).filter(|_| state.reachable) else { return };
        let within = |p: &Option<usize>| p.is_none_or(|p| from.is_none_or(|f| p >= f) && to.is_none_or(|t| p < t));
        let picked: Vec<(super::RaiseRec, Footprint)> = self.gens.raises[g].recs.iter().filter(|(_, p, _)| within(p)).map(|((c, s, e), _, fp)| ((c.clone(), s.copy(), e.clone()), fp.clone())).collect();
        for ((cls, raised, exc), fp) in picked {
            let mut here = state.copy();
            here.join_heap_fp(&raised, &fp);
            self.reraise_rec((cls, here, exc));
        }
        if end && to.is_none() && self.gens.raising.contains_key(&o) {
            state.reachable = false;
        }
    }
}
