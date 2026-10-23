//! One closure for followed libraries' code (`LibClosure`): a call that does not run, whose code
//! is a library's and reaches more than `SHARED_PAST` code objects, may change what any such call
//! before it may change. Its closure is walked once and extended by what each call adds (a new
//! class's methods, a decorated function), and applied again only where it may change something
//! again: what it gained, and what the state changed since it was last applied (a trie diff that
//! costs what differs). Calls whose code reaches less keep a closure of their own (`code_closure`).
//!
//! **Why this is sound.** The shared closure holds every closure of those calls, so it may change
//! more than one of them, never less. After an application every object its havoc walks met is as
//! the walk left it (its slots hold anything, its row has a mutator) in the state then
//! (`LibClosure::left`). In a later state, an object no write changed since is still so, and what
//! it holds is either met through it then, unchanged, or changed and walked now (`walk_objects`
//! resumed). A slot whose value changed takes its effects again (`LibClosure::locs`), as does an
//! object changed in place that an effect applies to (`LibClosure::objs`); a name bound since
//! (a module's new global, a function defined later) is in the diff too. Past a diff as large as
//! what it holds, all of it is applied again.

use rustc_hash::{FxHashMap, FxHashSet};

use super::effects::{module_names, Closure, ClosureWalk, Loc, Taken};
use super::flows::Removes;
use super::unknown::{any_object, code_of, drop_facts_on, may_have_removed, Snapshot};
use super::{Analyzer, Payload};
use crate::domain::{Ns, ObjId, Value};
use crate::intern::StrId;
use crate::state::{Kind, State};

/// Code whose closure takes more than this many walk visits (code objects and the slots they
/// name), some of it a followed library's, shares the library's closure.
pub const SHARED_PAST: usize = 128;

// What a slot or object of the closure is to it (bits): its value walked, its parts walked, its
// parts' parts, changed in place, iterated, items removed, rebound; an object between a held2
// slot's value and the parts walked.
const LOC: u8 = 1;
const HELD: u8 = 2;
const HELD2: u8 = 4;
const EDITED: u8 = 8;
const ITER: u8 = 16;
const REMOVED: u8 = 32;
const REBOUND: u8 = 64;
const MID: u8 = 128;

/// The closure the library's calls that do not run share, and what of it was applied: per slot
/// and per object the effects taken, the objects its walks met, the state it left.
#[derive(Default)]
pub struct LibClosure {
    walk: ClosureWalk,
    taken: Taken,
    locs: FxHashMap<Loc, u8>,
    objs: FxHashMap<ObjId, u8>,
    loc_removes: FxHashMap<Loc, Removes>,
    removes: FxHashMap<ObjId, Removes>,
    /// slots its code reads names from that were not bound then; modules walked as values
    unbound: FxHashSet<Loc>,
    modules: FxHashSet<ObjId>,
    /// the objects its havoc walks met
    visited: FxHashSet<ObjId>,
    /// the state the last application left
    left: Option<Snapshot>,
    /// code that reaches more than `SHARED_PAST` code objects alone
    big: FxHashSet<ObjId>,
}

/// What one application does (`follow_shared`): the objects whose values are walked, whose
/// parts are, …, the slots rebound; `taken`, the slots it took (the work measure).
#[derive(Default)]
struct Piece {
    vals: Vec<ObjId>,
    held: Vec<ObjId>,
    held2: Vec<ObjId>,
    mids: Vec<ObjId>,
    edited: Vec<ObjId>,
    iterated: Vec<ObjId>,
    removed: Vec<(ObjId, Removes)>,
    rebound: Vec<Loc>,
    taken: usize,
}

impl LibClosure {
    /// All of it is applied again: what was taken is forgotten (the walk is kept).
    fn restart(&mut self) {
        let walk = std::mem::take(&mut self.walk);
        let big = std::mem::take(&mut self.big);
        *self = LibClosure { walk, big, ..Default::default() };
    }

    /// Slot `l` has effects `bits` (more than before, or its value changed): they apply to what
    /// it holds now.
    fn take_loc(&mut self, l: Loc, bits: u8, state: &State, p: &mut Piece) {
        let new = bits & !self.locs.get(&l).copied().unwrap_or(0);
        *self.locs.entry(l).or_default() |= bits;
        self.take_held(l, new, state, p);
    }

    /// The effects `bits` of slot `l` apply to the value it holds now.
    fn take_held(&mut self, l: Loc, bits: u8, state: &State, p: &mut Piece) {
        if bits == 0 {
            return;
        }
        p.taken += 1;
        let v = state.slot(l.0, l.1, l.2);
        if bits & LOC != 0 {
            p.vals.extend(v.pts.iter());
        }
        if bits & REBOUND != 0 {
            p.rebound.push(l);
        }
        let rm = self.loc_removes.get(&l).copied();
        for o in v.pts.iter() {
            self.take_obj(o, bits & (HELD | HELD2 | EDITED | ITER | REMOVED), rm, false, p);
        }
    }

    /// Object `o` has effects `bits`: those it had not yet (all of them when it changed since,
    /// `again`) apply to it.
    fn take_obj(&mut self, o: ObjId, bits: u8, rm: Option<Removes>, again: bool, p: &mut Piece) {
        let had = self.objs.get(&o).copied().unwrap_or(0);
        let new = if again { bits } else { bits & !had };
        *self.objs.entry(o).or_default() |= bits;
        if let Some(r) = rm {
            let e = self.removes.entry(o).or_default();
            *e = e.join(r);
        }
        let push = |bit: u8, v: &mut Vec<ObjId>| {
            if new & bit != 0 {
                v.push(o);
            }
        };
        push(HELD, &mut p.held);
        push(HELD2, &mut p.held2);
        push(MID, &mut p.mids);
        push(EDITED, &mut p.edited);
        push(ITER, &mut p.iterated);
        if new & REMOVED != 0 {
            p.removed.push((o, self.removes.get(&o).copied().unwrap_or(Removes::Any)));
        }
    }

    /// What the walk's closure gained: its new slots and values' effects (a name not bound now:
    /// remembered, `unbound`; a module walked as a value: its names now).
    fn take_gained(&mut self, g: Closure, state: &State, p: &mut Piece) {
        for (l, r) in &g.removed_locs {
            let e = self.loc_removes.entry(*l).or_default();
            *e = e.join(*r);
        }
        let lists: [(&[Loc], u8); 6] = [(&g.locs, LOC), (&g.held_locs, HELD), (&g.held2_locs, HELD2), (&g.rebound, REBOUND), (&g.edited_locs, EDITED), (&g.iterated_locs, ITER)];
        for (ls, bit) in lists {
            for &l in ls {
                self.take_loc(l, bit, state, p);
            }
        }
        for &(l, _) in &g.removed_locs {
            self.take_loc(l, REMOVED, state, p);
        }
        p.vals.extend(g.values.iter().flat_map(|v| v.pts.iter()));
        for (vs, bit) in [(&g.held_values, HELD), (&g.edited_values, EDITED), (&g.iterated_values, ITER)] {
            for o in vs.iter().flat_map(|v| v.pts.iter()) {
                self.take_obj(o, bit, None, false, p);
            }
        }
        for (v, r) in &g.removed_values {
            for o in v.pts.iter() {
                self.take_obj(o, REMOVED, Some(*r), false, p);
            }
        }
        for l in g.unbound {
            if state.has_slot(l.0, l.1, l.2) {
                self.take_loc(l, LOC, state, p);
            } else {
                self.unbound.insert(l);
            }
        }
        for (m, h) in g.modules {
            if self.modules.insert(m) && module_names(state, m) != h {
                for (k, _) in state.obj_slots(m, Ns::Attr) {
                    self.take_loc((m, Ns::Attr, k), LOC, state, p);
                }
            }
        }
    }

    /// What changed since the last application that its effects apply to: slots whose values
    /// changed (a name bound since, a module's new global), objects changed in place, objects
    /// its walks met (walked again).
    fn take_changed(&mut self, ch: &FxHashSet<ObjId>, cs: &[(ObjId, crate::state::SlotKey)], state: &State, p: &mut Piece) {
        for &(o, (ns, k)) in cs {
            let l = (o, ns, k);
            let bits = self.locs.get(&l).copied().unwrap_or(0) | if self.unbound.remove(&l) || (ns == Ns::Attr && self.modules.contains(&o)) { LOC } else { 0 };
            if bits != 0 {
                *self.locs.entry(l).or_default() |= bits;
                self.take_held(l, bits, state, p);
            }
        }
        for &o in ch {
            if self.visited.contains(&o) {
                p.vals.push(o);
            }
            if let Some(&b) = self.objs.get(&o) {
                let rm = self.removes.get(&o).copied();
                self.take_obj(o, b, rm, true, p);
            }
        }
    }
}

impl Analyzer {
    /// Is `o` a followed library's code (a function, class, instance of one, bound method,
    /// partial, module)?
    fn library_code(&self, o: ObjId) -> bool {
        let m = match (self.heap.kind(o), &self.heap.obj(o).payload) {
            (Kind::Function, Payload::Func(fi)) => fi.module,
            (Kind::Bound, Payload::Bound(f, _)) => return self.library_code(*f),
            (Kind::Partial, Payload::Partial(f, _, _)) => return f.pts.iter().any(|f| self.library_code(f)),
            (Kind::Class, _) => match self.heap.class_info_opt(o) {
                Some(ci) => ci.module,
                None => return false,
            },
            (Kind::Instance, Payload::Cls(c)) => return self.library_code(*c),
            (Kind::Module, _) => o,
            _ => return false,
        };
        self.heap.module_info(m).library
    }

    /// Is some of `roots` a followed library's code?
    pub(super) fn any_library_code(&self, roots: &[ObjId]) -> bool {
        roots.iter().any(|&r| self.library_code(r))
    }

    /// Does the code `roots` hold, followed where it does not run, share the library's closure
    /// (some of it a library's, its closure more than `SHARED_PAST` walk visits)? Code that
    /// alone reaches that much shares it from then on.
    pub(super) fn shares_lib_closure(&mut self, roots: &[ObjId], state: &State) -> bool {
        if !self.any_library_code(roots) {
            return false;
        }
        if roots.iter().any(|r| self.effects.lib.big.contains(r)) {
            return true;
        }
        if self.closure_within(roots, SHARED_PAST, true, state) {
            return false;
        }
        for &r in roots {
            if roots.len() > 1 && self.library_code(r) && !self.closure_within(&[r], SHARED_PAST, true, state) {
                self.effects.lib.big.insert(r);
            }
        }
        if roots.len() == 1 {
            self.effects.lib.big.insert(roots[0]);
        }
        true
    }

    /// `follow_code` of library code that shares the library's closure: the walk extended by
    /// `roots`, in rounds with the code the data it changes holds; applied where it gained or
    /// the state changed since (module doc). A bound method's receiver and a partial's arguments
    /// are this call's, walked here, not the closure's.
    pub(super) fn follow_shared(&mut self, roots: Vec<ObjId>, rid: ObjId, site: StrId, state: &mut State) {
        let mut lc = std::mem::take(&mut self.effects.lib);
        // (what the walks meet is changed whole: no receiver walked by names alone)
        let guard = std::mem::take(&mut self.havoc.guard);
        let mut p = Piece::default();
        let mut code: Vec<ObjId> = Vec::new();
        for r in roots {
            match (self.heap.kind(r), &self.heap.obj(r).payload) {
                (Kind::Bound, Payload::Bound(f, recv)) => {
                    code.push(*f);
                    p.vals.extend(recv.pts.iter());
                }
                (Kind::Partial, Payload::Partial(f, args, kw)) => {
                    code.extend(f.pts.iter());
                    p.vals.extend(args.iter().chain(kw.values()).flat_map(|v| v.pts.iter()));
                }
                _ => code.push(r),
            }
        }
        // (what changed since it was last applied; past as much as it holds, all of it again)
        let limit = (lc.visited.len() + lc.locs.len()).max(64);
        let mut ch = FxHashSet::default();
        match lc.left.as_ref().map(|l| l.changed_parts(state, limit)) {
            Some(Some((c, cs))) => {
                ch = c;
                // (a list given an item since: its last item, as a walk would)
                ch.extend(state.lasts.iter().map(|(o, _)| *o).filter(|o| lc.visited.contains(o)));
                lc.take_changed(&ch, &cs, state, &mut p);
            }
            Some(None) => lc.restart(),
            None => {}
        }
        let walk = self.havoc.new_walk();
        self.havoc.code.clear();
        loop {
            lc.walk.push(code.drain(..));
            self.walk_closure(&mut lc.walk, state);
            let g = lc.walk.gained(&mut lc.taken);
            lc.take_gained(g, state, &mut p);
            self.apply_piece(&mut lc, std::mem::take(&mut p), &ch, rid, site, state, walk);
            code = std::mem::take(&mut self.havoc.code).into_iter().filter(|c| !lc.walk.walked(*c)).collect();
            if code.is_empty() {
                break;
            }
        }
        // (the tests of what it may change no longer hold, applied again or not: its walks give
        // their rows this call's mutator)
        drop_facts_on(state, |o| lc.visited.contains(&o) || lc.objs.contains_key(&o) || lc.modules.contains(&o));
        lc.left = Some(Snapshot::of(state));
        self.havoc.guard = guard;
        self.effects.lib = lc;
    }

    /// Apply one round's piece: the parts of what it holds walked, the havoc walk resumed past
    /// what is as an earlier walk left it, containers changed in place, items removed, slots
    /// rebound.
    #[allow(clippy::too_many_arguments)]
    fn apply_piece(&mut self, lc: &mut LibClosure, p: Piece, ch: &FxHashSet<ObjId>, rid: ObjId, site: StrId, state: &mut State, walk: u32) {
        (0..p.taken).for_each(|_| self.budget.walk_visit());
        let mut parts: Vec<ObjId> = Vec::new();
        for &o in &p.held {
            self.obj_edges(o, state, &mut parts);
        }
        let mut mids = p.mids;
        for &o in &p.held2 {
            self.obj_edges(o, state, &mut mids);
        }
        for &m in &mids {
            *lc.objs.entry(m).or_default() |= MID;
            self.havoc.code.extend(code_of(&self.heap, m));
            self.obj_edges(m, state, &mut parts);
        }
        let mut vals = p.vals;
        vals.extend(parts);
        vals.sort_unstable();
        vals.dedup();
        let mut code = Vec::new();
        self.havoc.walk_objects(&mut self.heap, vals, &mut lc.visited, Some(ch), &mut code, rid, site, state, walk);
        self.havoc.edit(&mut self.heap, &p.edited, true, site, state);
        self.havoc.edit(&mut self.heap, &p.iterated, false, site, state);
        may_have_removed(&self.heap, &p.removed, state);
        let any = Value::obj(any_object(&mut self.heap));
        for (o, ns, k) in p.rebound {
            let v = state.slot(o, ns, k).join(&any);
            state.set_slot(o, ns, k, v, false);
        }
    }
}
