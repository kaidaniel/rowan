//! Recursion: a call of a running activation (same function and call site; literal arguments
//! that differ from the running ones' make another, up to a depth, keyed by the call's place
//! and receiver rather than its context; past one level, a function's call of itself with
//! nothing known but the innermost activation's arguments is that one's) does not run the body
//! again. It takes the activation's partial result, and widens the activation's entry with
//! its arguments and heap; the activation then runs again until its result and state are
//! stable (`REC_MAX_ITER` rounds), else they are widened.
//!
//! For a call memo: `Recursion` is the dynamic record of the running activations, emptied as
//! each ends; a memo hit inside a recursion would have to replay the entry widening it did.

use indexmap::IndexMap;
use rustc_hash::{FxHashMap, FxHashSet};

use super::{Analyzer, Args, ExcNames, FuncInfo, FuncNode, RaiseRec};
use crate::domain::{join_values, KeyVal, ObjId, Value};
use crate::frontend::Node;
use crate::intern::{self, s, StrId};
use crate::state::{Footprint, State};

/// Running activations of one function past which a call with other literal arguments joins
/// one of them (`activation_key`).
pub const MAX_ACTIVE: usize = 4;
/// Running activations of one function past which a call whose arguments are those of the
/// innermost one or input joins it (`activation_key`).
pub const UNKNOWN_ACTIVE: usize = 2;
/// Rounds an activation runs again until its result and state are stable; then widened.
pub const REC_MAX_ITER: usize = 8;

/// An activation: (function, call site; `entry:` for an entry run, `|…` for literal arguments).
pub type ActKey = (ObjId, String);

/// The running activations and, for the recursive ones, their fixpoint iteration.
#[derive(Default)]
pub struct Recursion {
    active: FxHashSet<ActKey>,
    /// per function, its running activations (innermost last)
    by_func: FxHashMap<ObjId, Vec<ActKey>>,
    /// a recursive activation's last round: its result, end state and what it changed
    table: FxHashMap<ActKey, (Value, State, Footprint)>,
    /// activations a recursive call reached
    hit: FxHashSet<ActKey>,
    /// the arguments an activation runs with (widened by its recursive calls)
    entry: FxHashMap<ActKey, (Option<Value>, Vec<Value>, IndexMap<StrId, Value>)>,
    /// activations whose entry a recursive call widened (they start over)
    widened: FxHashSet<ActKey>,
    /// per running activation: the heap it started from, that heap grown by the heaps its
    /// recursive calls were made in (on older objects they reach), and its birth mark
    heap: FxHashMap<ActKey, (std::rc::Rc<State>, Option<State>, u64)>,
    /// per recursive activation running a round (its frame depth): the raises leaving its
    /// frame, joined per class
    pub(in crate::interp) collecting: Vec<(ActKey, usize, Vec<(Option<ExcNames>, crate::state::Joins, Value)>)>,
    /// a recursive activation's raises of its last round, with what each changed of its entry,
    /// and those its recursive calls raise (more than a few classes: one, of any class, from
    /// their states joined — a replay per class joins every handler's state that often)
    raises: FxHashMap<ActKey, (Vec<RaiseRec>, Vec<RaiseRec>)>,
    /// recursive activations a recursive call of which ran inside a try of their own frames or
    /// deeper (their raises matter there: collected), and each running activation's frame depth
    wants_raises: FxHashMap<ActKey, Vec<(FxHashSet<String>, bool)>>,
    depth_of: FxHashMap<ActKey, usize>,
    /// per running activation, the round it runs
    round_no: FxHashMap<ActKey, u32>,
    /// the places of the calls whose bodies run (`Analyzer::place_key`), outermost first
    places: Vec<String>,
    /// per place activation, its last run (kept while some call's body runs)
    place_runs: FxHashMap<ActKey, PlaceRun>,
    /// place activations whose runs past `REC_MAX_ITER` did not settle: calls there do not run
    place_unsettled: FxHashSet<ActKey>,
    /// the running place activations (innermost last): their frame depth, and the rounds of
    /// the running activations outside them whose results their runs took
    place_deps: Vec<(ActKey, usize, Vec<Dep>)>,
    /// place activations running again, with the runs they took so far
    place_settle: FxHashMap<ActKey, usize>,
    /// running activations a place activation inside whose run did not settle (they run again)
    unstable: FxHashSet<ActKey>,
    /// per place activation, the handlers around its calls whose raises its runs keep
    /// (`wants_raises`), and the running ones a raise left
    place_wants: FxHashMap<ActKey, Vec<(FxHashSet<String>, bool)>>,
    place_raised: FxHashSet<ActKey>,
    /// `ROWAN_DEBUG`: calls answered by a place activation's last run
    pub lookups: usize,
}

/// A running activation a run took results from, in its round (and for a generator's, whether
/// it had yielded: what its reads since then are, `Gens::read`).
pub type Dep = (ActKey, u32, Option<bool>);

/// An activation's entry: the receiver, the positional and the keyword arguments.
pub type Entry = (Option<Value>, Vec<Value>, IndexMap<StrId, Value>);

/// A place activation's last run (`Analyzer::place_call`): its result and end state with what it
/// changed, the entry it ran from (its arguments; the heap it started from, grown by its
/// recursive calls', with its birth mark), what it did beyond its state for a later call to do
/// again (the requirements it added, its raises), the output recorded by its end (its verdicts
/// and hover facts stand while that does), the rounds of running activations it took results
/// from, and how many runs it took.
pub(in crate::interp) struct PlaceRun {
    ret: Value,
    exit: State,
    fp: Footprint,
    entry: Entry,
    pre: std::rc::Rc<State>,
    grown: Option<State>,
    mark: u64,
    reqs: Vec<(crate::intern::SourceId, crate::intern::KeyId, crate::domain::ReqInfo)>,
    raises: Vec<RaiseRec>,
    /// whether a raise left it, and the handlers whose raises `raises` are
    raised: bool,
    wants: Vec<(FxHashSet<String>, bool)>,
    out: Vec<super::findings::Recording>,
    deps: Vec<Dep>,
    runs: usize,
}

/// What a call at a place a running call came from does (`Analyzer::place_call`).
pub(in crate::interp) enum PlaceCall {
    /// the place activation's result (its running activation's, or its last run's)
    Answer(Value),
    /// the place activation runs: from this call, or from the entry and heap of its last run
    /// joined with this call's (the heap to join into this call's state)
    Run(Option<(Entry, State, Footprint)>, Option<(Value, State, Footprint)>),
    /// its runs did not settle: the call does not run (`skipped_call`)
    Skip,
}

impl Recursion {
    /// Is this activation running (a call of it is recursive)?
    pub(in crate::interp) fn is_active(&self, key: &ActKey) -> bool {
        self.active.contains(key)
    }

    /// The arguments of the running activations (their callers hold them until they return).
    pub(in crate::interp) fn running_args(&self, work: &mut Vec<ObjId>) {
        for k in &self.active {
            if let Some((r, args, kw)) = self.entry.get(k) {
                r.iter().chain(args.iter()).chain(kw.values()).for_each(|v| v.push_objs(work));
            }
        }
    }

    /// How many activations of `foid` are running.
    pub(in crate::interp) fn depth(&self, foid: ObjId) -> usize {
        self.by_func.get(&foid).map(|v| v.len()).unwrap_or(0)
    }

    /// An activation begins: its arguments, the heap before its parameters are bound, and the
    /// birth mark then.
    pub(in crate::interp) fn begin(&mut self, key: &ActKey, entry: (Option<Value>, Vec<Value>, IndexMap<StrId, Value>), pre: std::rc::Rc<State>, mark: u64) {
        self.active.insert(key.clone());
        self.by_func.entry(key.0).or_default().push(key.clone());
        self.entry.insert(key.clone(), entry);
        self.heap.insert(key.clone(), (pre, None, mark));
    }

    /// The activation's frame depth is `d` (its frame pushed).
    pub(in crate::interp) fn set_depth(&mut self, key: &ActKey, d: usize) {
        self.depth_of.insert(key.clone(), d);
    }

    /// The activation ends.
    pub(in crate::interp) fn end(&mut self, key: &ActKey) {
        self.active.remove(key);
        if let Some(lst) = self.by_func.get_mut(&key.0) {
            lst.pop();
        }
        self.hit.remove(key);
        self.entry.remove(key);
        self.widened.remove(key);
        self.heap.remove(key);
        self.table.remove(key);
        self.raises.remove(key);
        self.wants_raises.remove(key);
        self.depth_of.remove(key);
        self.round_no.remove(key);
        self.unstable.remove(key);
    }

    pub(in crate::interp) fn place_deps_empty(&self) -> bool {
        self.place_deps.is_empty()
    }

    /// The running place activation `key` runs inside (the innermost other).
    pub(in crate::interp) fn place_parent(&self, key: &ActKey) -> Option<ActKey> {
        let n = self.place_deps.len();
        (n >= 2 && self.place_deps[n - 1].0 == *key).then(|| self.place_deps[n - 2].0.clone())
    }

    /// The running activation `key`'s first guess for its recursive calls (a place activation's
    /// last run).
    pub(in crate::interp) fn seed(&mut self, key: &ActKey, guess: (Value, State, Footprint)) {
        self.table.insert(key.clone(), guess);
    }

    /// A round of the running activation `key` begins.
    pub(in crate::interp) fn round_begins(&mut self, key: &ActKey) {
        *self.round_no.entry(key.clone()).or_default() += 1;
    }

    pub(in crate::interp) fn places_dbg(&self) -> Vec<String> {
        self.places.clone()
    }

    /// A call's body at `place` begins to run.
    pub(in crate::interp) fn place_enter(&mut self, place: &str) {
        self.places.push(place.to_string());
    }

    /// It ended: past the outermost, the place activations' runs are forgotten.
    pub(in crate::interp) fn place_leave(&mut self) {
        self.places.pop();
        if self.places.is_empty() {
            self.place_runs.clear();
            self.place_wants.clear();
            self.place_unsettled.clear();
        }
    }

    /// The running place activations outside which the running activations `deps` (each in a
    /// round) are: their runs took those rounds' results.
    fn note_deps(&mut self, deps: &[Dep]) {
        for (d, r, y) in deps {
            let Some(dd) = self.depth_of.get(d).copied() else { continue };
            for (_, depth, ds) in self.place_deps.iter_mut() {
                if dd < *depth && !ds.contains(&(d.clone(), *r, *y)) {
                    ds.push((d.clone(), *r, *y));
                }
            }
        }
    }
}

impl Analyzer {
    /// The activation a call of `foid` at `site` runs as. Past `MAX_ACTIVE` running activations
    /// of the function, or a recursion that makes no progress through known data
    /// (`recursion_without_progress`), the innermost one (the call is then recursive).
    pub(super) fn activation_key(&self, foid: ObjId, site: &str, entry: bool, self_v: Option<&Value>, a: Args, state: &State) -> ActKey {
        let Args { pos: args, kw: kwargs, star: star_args, star_kw: star_kwargs } = a;
        let mut key = (foid, if !entry { site.to_string() } else { format!("entry:{}", site) });
        // literal arguments that differ from the running activations': not the same activation
        // (fib(9) inside fib(10)), up to a depth
        let mut lit_key = false;
        if !entry && !args.is_empty() && kwargs.is_empty() && star_args.is_empty() && star_kwargs.is_empty() {
            // (a tensor of a fully known shape counts like a literal: halving until length 1)
            let lits: Option<Vec<String>> = args
                .iter()
                .map(|a| {
                    if let Some(k) = a.kv.as_lit().filter(|_| a.pts.is_empty()) {
                        return Some(intern::key_repr(k));
                    }
                    if self.m.torch_on {
                        if let Some(sh) = crate::models::torch::tensor_shape(a) {
                            if sh.rest.is_none() && sh.suffix.is_empty() && sh.assume.is_empty() {
                                let dims: Option<Vec<i64>> = sh.prefix.iter().map(|d| crate::models::shape::as_lit(*d)).collect();
                                return dims.map(|ds| format!("T{:?}", ds));
                            }
                        }
                    }
                    None
                })
                .collect();
            let depth = self.recursion.depth(foid);
            if let Some(ls) = lits.filter(|_| depth < 40) {
                // (by the call's place in the code, not its context: a call from where a running
                // activation was called, with its literals, is that activation's recursive call
                // however the chain of contexts grew, `__getattr__("x")` reading `self.x` again)
                let place = site.rsplit('/').next().unwrap_or(site);
                // (and its receiver: a wrapper's call of the same method of the object it wraps is not
                // recursive)
                let recv = self_v.map(|v| v.pts.iter().map(|o| o.to_string()).collect::<Vec<_>>().join(".")).unwrap_or_default();
                key.1 = format!("{}|{}|{}", place.split('#').next().unwrap_or(place), recv, ls.join(","));
                lit_key = true;
            }
        }
        if !self.recursion.active.contains(&key) && !lit_key && (self.recursion.depth(foid) >= MAX_ACTIVE || self.recursion_without_progress(foid, self_v, args, kwargs, state)) {
            key = self.recursion.by_func[&foid].last().unwrap().clone();
        }
        key
    }

    /// A function's call of itself (not a generator's) inside `UNKNOWN_ACTIVE` running
    /// activations of it, each argument the innermost one's or input (`input_like`): no step
    /// through known data — a shorter list, a child node — that could end the recursion where it
    /// is known. Another activation would only nest its fixpoint (and its loops' rounds) inside
    /// the one above, at every call site of the recursion: it is the innermost one's recursive
    /// call.
    fn recursion_without_progress(&self, foid: ObjId, self_v: Option<&Value>, args: &[Value], kwargs: &IndexMap<StrId, Value>, state: &State) -> bool {
        if self.recursion.depth(foid) < UNKNOWN_ACTIVE || self.frame().func != Some(foid) || self.heap.func_info(foid).node.is_generator() {
            return false;
        }
        let Some((e_self, e_args, e_kw)) = self.recursion.by_func.get(&foid).and_then(|v| v.last()).and_then(|k| self.recursion.entry.get(k)) else { return false };
        let same_or_input = |v: &Value, e: Option<&Value>| e == Some(v) || self.input_like(v, state);
        self_v.is_none_or(|v| same_or_input(v, e_self.as_ref())) && args.iter().enumerate().all(|(i, v)| same_or_input(v, e_args.get(i))) && kwargs.iter().all(|(k, v)| same_or_input(v, e_kw.get(k)))
    }

    /// Is the value only input or unknown: no literal, its objects (if any) unknown or open
    /// through data from outside the program?
    fn input_like(&self, v: &Value, state: &State) -> bool {
        if v.kv.as_lit().is_some() || matches!(v.kv, KeyVal::Lits(_)) {
            return false;
        }
        v.pts.iter().all(|o| {
            self.heap.kind(o) == crate::state::Kind::Unknown
                || [crate::domain::Ns::Item, crate::domain::Ns::Attr].iter().any(|ns| state.has_row(o, *ns) && {
                    let r = state.row(o, *ns);
                    r.top || (!r.sources.is_empty() && r.sources.iter().all(|(s, _)| crate::source::source_is_outside(s)))
                })
        })
    }

    /// A recursive call of the running activation `key`: its entry widens with this call's
    /// arguments and heap, and the call gives the activation's last result (none in its first
    /// round).
    pub(super) fn recursive_call(&mut self, key: &ActKey, self_v: &Option<Value>, args: &[Value], kwargs: &IndexMap<StrId, Value>, state: &mut State) -> Value {
        self.recursion.hit.insert(key.clone());
        if !self.recursion.place_deps.is_empty() {
            let r = self.recursion.round_no.get(key).copied().unwrap_or(0);
            self.recursion.note_deps(&[(key.clone(), r, None)]);
        }
        // (inside a try of the activation's frames: what the activation raises matters here)
        if let Some(d) = self.recursion.depth_of.get(key).copied() {
            // (what those tries' handlers catch, and whether a finally runs: the raises they see)
            let seen: Vec<(FxHashSet<String>, bool)> = self.unwind.tries.iter().filter(|t| t.frames >= d).flat_map(|t| t.hclasses.iter().cloned().chain(t.runs.then(|| (std::iter::once("*".to_string()).collect(), false)))).collect();
            if !seen.is_empty() {
                let w = self.recursion.wants_raises.entry(key.clone()).or_default();
                for h in seen {
                    if !w.contains(&h) {
                        w.push(h);
                    }
                }
            }
        }
        // widen the entry arguments with the ones of this recursive call
        if let Some((e_self, e_args, e_kwargs)) = self.recursion.entry.get(key).cloned() {
            let n_self = match (&e_self, &self_v) {
                (Some(a), Some(b)) => Some(a.join(b)),
                _ => e_self.clone(),
            };
            let mut n_args: Vec<Value> = Vec::new();
            for (i, a) in e_args.iter().enumerate() {
                n_args.push(if i < args.len() { a.join(&args[i]) } else { a.clone() });
            }
            if args.len() > e_args.len() {
                n_args.extend(args[e_args.len()..].iter().cloned());
            }
            let mut n_kwargs = e_kwargs.clone();
            for (k, v) in kwargs.iter() {
                let nv = match n_kwargs.get(k) {
                    Some(cur) => cur.join(v),
                    None => v.clone(),
                };
                n_kwargs.insert(*k, nv);
            }
            let n_args: Vec<Value> = n_args.iter().map(widen_value).collect();
            let n_kwargs: IndexMap<StrId, Value> = n_kwargs.iter().map(|(k, v)| (*k, widen_value(v))).collect();
            if n_self != e_self || n_args != e_args || n_kwargs != e_kwargs {
                self.recursion.entry.insert(key.clone(), (n_self, n_args, n_kwargs));
                self.recursion.widened.insert(key.clone());
            }
        }
        // the heap of this call, where it differs from the activation's start on objects
        // older than it that the call reaches: the activation starts from it too (else a
        // deeper call's paths that the first heap rules out would never be analysed; the
        // objects made since have no facts there, which reads as unknown)
        if let Some((pre, grown, mark)) = self.recursion.heap.get(key) {
            let (pre, mark) = (pre.clone(), *mark);
            let had = grown.is_some();
            // (every object older than the activation it changed: what the arguments reach, a
            // global, a cell — what the callee may read)
            let fp = state.footprint(&pre);
            let old = |o: ObjId| self.heap.born_before(o, mark);
            let fp = Footprint { rows: fp.rows.into_iter().filter(|k| old(k.0)).collect(), slots: fp.slots.into_iter().filter(|k| old(k.0)).collect(), elems: fp.elems.into_iter().filter(|o| old(*o)).collect() };
            if !fp.rows.is_empty() || !fp.slots.is_empty() || !fp.elems.is_empty() {
                let mut g = self.recursion.heap.get_mut(key).and_then(|e| e.1.take()).unwrap_or_else(|| pre.copy());
                let before = g.copy();
                g.join_heap_fp(state, &fp);
                let d = g.footprint(&before);
                if !had || !d.rows.is_empty() || !d.slots.is_empty() || !d.elems.is_empty() {
                    self.recursion.widened.insert(key.clone());
                }
                if let Some(e) = self.recursion.heap.get_mut(key) {
                    e.1 = Some(g);
                }
            }
        }
        // (the activation's raises of its last round, from here with what they changed)
        if let Some((_, rs)) = self.recursion.raises.get(key).filter(|_| state.reachable) {
            let rs = rs.clone();
            self.replay_raises(rs, state);
        }
        if let Some((ret, exit_state, fp)) = self.recursion.table.get(key) {
            let ret = ret.clone();
            state.join_heap_fp(exit_state, fp);
            return ret;
        }
        Value::new()
    }

    /// Raises of a run (`collect_rec_raise`) raised again by a later call it stands for: from
    /// this call's state, joined with what each raise state says of the objects this state has
    /// facts of (the objects only that run made, which nothing here reaches, are left out: they
    /// only grew the handlers' states, which then differed at every call).
    fn replay_raises(&mut self, rs: Vec<RaiseRec>, state: &State) {
        for (cls, rst, exc) in rs {
            let mut here = state.copy();
            let has = |o: ObjId| state.has_row(o, crate::domain::Ns::Attr) || state.has_row(o, crate::domain::Ns::Item) || state.lens.get(&o).is_some() || state.elems.get(&o).is_some();
            here.join_heap_where(&rst, has);
            here.env = state.env.clone();
            here.unbound = state.unbound.clone();
            here.path = state.path.clone();
            self.unwind.cur_exc = (!exc.pts.is_empty()).then_some(exc);
            self.unwind.cur_exc_cls = cls;
            self.record_raise_state(&mut here);
            self.unwind.cur_exc = None;
            self.unwind.cur_exc_cls = None;
        }
    }

    /// A raise of class `cls` (exception `exc`) leaving the frames of recursive activations
    /// running a round (no try of theirs or deeper sees it): kept for their recursive calls.
    pub(in crate::interp) fn collect_rec_raise(&mut self, cls: &Option<ExcNames>, exc: &Value, state: &State) {
        let depth = self.cx.frames.len();
        for i in 0..self.recursion.collecting.len() {
            let d = self.recursion.collecting[i].1;
            if depth < d || self.unwind.tries.iter().any(|t| t.frames >= d && t.sees(cls)) {
                continue;
            }
            // (one no try around the activation's recursive calls may see: nothing to replay)
            let key = &self.recursion.collecting[i].0;
            if !self.recursion.wants_raises.get(key).is_some_and(|hs| hs.iter().any(|hc| super::tries::handler_matches(hc, cls) != Some(false))) {
                continue;
            }
            let recs = &mut self.recursion.collecting[i].2;
            match recs.iter_mut().find(|r| r.0 == *cls) {
                Some(r) => {
                    r.1.add(state.copy());
                    r.2 = r.2.join(exc);
                }
                None => {
                    let mut j = crate::state::Joins::default();
                    j.add(state.copy());
                    recs.push((cls.clone(), j, exc.clone()));
                }
            }
        }
    }

    /// A call's body run once from `st` (its parameters bound): a lambda's expression, a
    /// `def`'s statements, a generator expression's generators.
    fn run_body(&mut self, fnode: FuncNode, gen_id: Option<ObjId>, st: &mut State) {
        match fnode {
            FuncNode::Lambda(l) => {
                let v = self.eval(&l.body, st);
                // (a body that raised on every path returns nothing)
                if st.reachable {
                    st.ret = Some(v);
                    st.record_exit();
                }
                st.reachable = false;
            }
            FuncNode::Def(f) => self.with_gen_restart(st, true, &mut |an, s| an.exec_block(&f.body, s)),
            FuncNode::GenExp(e) => self.genexp_body(e, gen_id.unwrap_or_default(), st),
        }
    }

    /// The rounds of a call's body from `state` (its parameters bound): one, or for a recursive
    /// activation until its result and state are stable, then widened. Its result and end state.
    pub(super) fn run_rounds(&mut self, key: &ActKey, fi: &FuncInfo, gen_id: Option<ObjId>, pre_bind: &State, star_args: &[Value], star_kwargs: &[Value], state: &mut State, node: &Node) -> (Value, State) {
        let fnode = fi.node;
        let mut entry_state = state.copy();
        let mut ret = Value::new();
        let mut st_final: Option<State> = None;
        let mut vmark = self.findings.mark();
        // (a place activation inside another's run: one round per run of the other, which iterates
        // until neither changes — a cycle's fixpoint at its head, not one nested per level)
        let parent = key.1.starts_with('@').then(|| self.recursion.place_parent(key)).flatten();
        for _round in 0..REC_MAX_ITER {
            self.recursion.round_begins(key);
            // (a round of a recursion before its fixpoint: its accesses' verdicts are superseded)
            self.findings.drop_since(vmark);
            vmark = self.findings.mark();
            self.budget.runs += 1;
            let mut st = entry_state.copy();
            if let Some(g) = gen_id {
                self.gens.reads.push((g, false, Default::default(), self.cx.frames.len()));
            }
            let collect = gen_id.is_none() && self.recursion.wants_raises.contains_key(key);
            if collect {
                self.recursion.collecting.push((key.clone(), self.cx.frames.len(), Vec::new()));
            }
            self.run_body(fnode, gen_id, &mut st);
            // (what this round raised; its recursive calls raised the last round's)
            let raised: Option<Vec<RaiseRec>> = collect.then(|| {
                let (_, _, recs) = self.recursion.collecting.pop().unwrap();
                recs.into_iter().filter_map(|(c, j, e)| Some((c, j.finish()?, e))).collect()
            });
            // (joined into the earlier rounds' per class: they only grow, so they settle; a round
            // that began collecting only now is not the last)
            let mut raises_stable = collect || !self.recursion.wants_raises.contains_key(key);
            // (a raise state settles where it changes no object older than the activation: what
            // a round made anew, made again the next, is no change of the program's state)
            let mark = self.recursion.heap.get(key).map(|h| h.2).unwrap_or(0);
            let changes_old = |an: &Analyzer, a: &State, b: &State| {
                let fp = a.footprint(b);
                let old = |o: ObjId| an.heap.born_before(o, mark);
                fp.rows.iter().any(|k| old(k.0)) || fp.slots.iter().any(|k| old(k.0)) || fp.elems.iter().any(|o| old(*o))
            };
            if let Some(now) = raised {
                let mut acc: Vec<RaiseRec> = self.recursion.raises.get(key).map(|r| r.0.clone()).unwrap_or_default();
                for (c, mut st, e) in now {
                    match acc.iter_mut().find(|r| r.0 == c) {
                        Some(r) => {
                            let j = r.1.join(&mut st);
                            let x = r.2.join(&e);
                            if changes_old(self, &j, &r.1) || x.kv != r.2.kv {
                                raises_stable = false;
                            }
                            r.1 = j;
                            r.2 = x;
                        }
                        None => {
                            raises_stable = false;
                            acc.push((c, st, e));
                        }
                    }
                }
                if !acc.is_empty() {
                    let replayed = if acc.len() > 3 {
                        let st = State::join_all(acc.iter().map(|r| r.1.copy()).collect());
                        let exc = join_values(acc.iter().map(|r| r.2.clone()).collect::<Vec<_>>());
                        st.map(|st| vec![(Some(super::any_exc()), st, exc)]).unwrap_or_default()
                    } else {
                        acc.clone()
                    };
                    self.recursion.raises.insert(key.clone(), (acc, replayed));
                }
            }
            if gen_id.is_some() {
                self.gens.last_reads = self.gens.reads.pop().map(|r| r.2);
            }
            ret = st.ret.clone().unwrap_or_default();
            if st.reachable {
                ret = ret.join(&Value::none());
            }
            // no normal completion (every path raises): the caller is unreachable after the call
            let completed = st.reachable || st.exits.is_some();
            if !completed && crate::interp::debug_level() == 2 {
                eprintln!("  callee {} did not complete (called at L{})", fi.qualname, node.line);
            }
            if let Some(ex) = st.exits.take() {
                let mut exs = ex.copy();
                st = if st.reachable { st.join(&mut exs) } else { exs };
                st.exits = None;
            }
            self.fold_call_recent(&mut st, &entry_state, &mut ret);
            st.reachable = completed;
            if let Some(par) = &parent {
                // (settled: what its recursive calls took, from its last run or this one's rounds, is
                // what it gives now)
                let fp = st.footprint(&entry_state);
                let widened = self.recursion.widened.remove(key);
                let inner = self.recursion.unstable.remove(key);
                let same = match self.recursion.table.get(key) {
                    Some((pr, pst, pfp)) => *pr == ret && pst.same_effect(pfp, &st, &fp),
                    None => !self.recursion.hit.contains(key),
                };
                if widened || inner || !raises_stable || !same {
                    self.recursion.unstable.insert(par.clone());
                }
                st_final = Some(st);
                break;
            }
            if self.recursion.widened.contains(key) {
                self.recursion.widened.remove(key);
                let (w_self, w_args, w_kwargs) = self.recursion.entry.get(key).cloned().unwrap();
                let fp = st.footprint(&entry_state);
                self.recursion.table.insert(key.clone(), (ret.clone(), st.copy(), fp));
                state.replace_with(pre_bind.copy());
                if let Some((p, Some(g), _)) = self.recursion.heap.get(key) {
                    let fp = g.footprint(p);
                    state.join_heap_fp(g, &fp);
                }
                self.bind_params(&fi, &w_self, Args::new(&w_args, &w_kwargs, &star_args, &star_kwargs), state, node);
                entry_state = state.copy();
                continue;
            }
            // (a place activation inside its run did not settle: it runs again)
            if self.recursion.unstable.remove(key) {
                let fp = st.footprint(&entry_state);
                self.recursion.table.insert(key.clone(), (ret.clone(), st.copy(), fp));
                continue;
            }
            if self.recursion.hit.contains(key) {
                if let Some((pr, pst, pfp)) = self.recursion.table.get(key) {
                    // (the frame's own names are not what its recursive calls take from it: a
                    // round that changed only them is the fixpoint; a generator's are resumed; a
                    // place activation's first guess, its last run, came from another heap: what it
                    // changed)
                    let same = if gen_id.is_some() {
                        pst.same(&st)
                    } else if key.1.starts_with('@') {
                        pst.same_effect(pfp, &st, &st.footprint(&entry_state))
                    } else {
                        pst.same_but_names(&st)
                    };
                    if *pr == ret && same && raises_stable {
                        st_final = Some(st);
                        break;
                    }
                }
                let fp = st.footprint(&entry_state);
                self.recursion.table.insert(key.clone(), (ret.clone(), st.copy(), fp));
                continue;
            }
            st_final = Some(st);
            break;
        }
        let converged = st_final.is_some();
        // (the result stabilised even if the state did not: it stands)
        let ret_stable = self.recursion.table.get(key).map(|(pr, _, _)| *pr == ret).unwrap_or(false);
        let st = match st_final {
            Some(s) => s,
            None => {
                // did not converge: the last state, widened (its result may be any
                // value, the records it touched may hold other keys)
                let mut s0 = match self.recursion.table.get(key) {
                    Some((_, pst, _)) => pst.copy(),
                    None => entry_state.copy(),
                };
                let fp = s0.footprint(&entry_state);
                let site = s(&format!("recursion@{}", fi.qualname));
                for (o, ns) in fp.rows.iter().copied().collect::<Vec<_>>() {
                    let r = s0.row(o, ns).mutate(site);
                    s0.set_row(o, ns, r);
                }
                s0
            }
        };
        if !converged && !ret_stable {
            ret = ret.join(&Value::data(&format!("recursion@{}", fi.qualname)));
        }
        (ret, st)
    }
}

/// A call site's place in the code (its last part: no context).
pub fn place_of(site: &str) -> &str {
    let p = site.rsplit('/').next().unwrap_or(site);
    p.split('#').next().unwrap_or(p)
}

impl Analyzer {
    /// The place activation a call of `foid` at `site` runs as, when the call's place is one a
    /// running call came from (the program came back to it through a cycle of calls): one per
    /// function and place, instead of an activation per call site and context, nested once per
    /// cycle — and with a dispatch of many callees there (a visitor's handlers, a parser's
    /// phases), once per callee at every level. Not for literal arguments (a step through known
    /// data: `activation_key`), generators, a model's callbacks, a run whose reads a generator
    /// watches, or a place whose runs did not settle.
    pub(super) fn place_key(&self, foid: ObjId, site: &str, key: &ActKey, a: &Args) -> Option<ActKey> {
        if super::debug_enabled() && self.recursion.places.iter().any(|p| p == place_of(site)) {
            eprintln!("place first lit={} active={} star={} key={}", key.1.contains('|'), self.recursion.is_active(key), !a.star.is_empty() || !a.star_kw.is_empty(), key.1);
        }
        if key.1.contains('|') || key.1.starts_with("entry:") || self.recursion.is_active(key) || !a.star.is_empty() || !a.star_kw.is_empty() {
            return None;
        }
        if self.cx.called_by_model || self.gens.cm_hook.as_ref().is_some_and(|h| !h.ran) || self.heap.func_info(foid).node.is_generator() {
            return None;
        }
        if super::debug_enabled() && self.recursion.places.iter().any(|p| p == place_of(site)) {
            eprintln!("place reject? model={} reads={} cm={} gen={}", self.cx.called_by_model, self.gens.reads.len(), self.gens.cm_hook.is_some(), self.heap.func_info(foid).node.is_generator());
        }
        let place = place_of(site);
        let k = (foid, format!("@{}", place));
        self.recursion.places.iter().any(|p| p == place).then_some(k)
    }

    /// A call of the place activation `k`: while it runs, its recursive call; else, if its last
    /// run's entry covers this call's (arguments and heap, as a recursive call widens them) and
    /// the rounds it took results from still run, that run's result and changes, with what it
    /// did beyond its state replayed (requirements, raises, verdicts, hover facts). Otherwise
    /// it runs again, from its entry joined with this call's.
    pub(super) fn place_call(&mut self, k: &ActKey, self_v: &Option<Value>, args: &[Value], kwargs: &IndexMap<StrId, Value>, state: &mut State) -> PlaceCall {
        if self.recursion.is_active(k) {
            return PlaceCall::Answer(self.recursive_call(k, self_v, args, kwargs, state));
        }
        if self.recursion.place_unsettled.contains(k) {
            return PlaceCall::Skip;
        }
        let Some(run) = self.recursion.place_runs.get(k) else { return PlaceCall::Run(None, None) };
        let (e_self, e_args, e_kw) = &run.entry;
        if e_args.len() != args.len() || e_self.is_some() != self_v.is_some() || kwargs.keys().any(|n| !e_kw.contains_key(n)) || e_kw.len() != kwargs.len() {
            self.recursion.place_runs.remove(k);
            return PlaceCall::Run(None, None);
        }
        let fresh = run.deps.iter().all(|(d, r, y)| self.recursion.round_no.get(d) == Some(r) && y.is_none_or(|y| self.gen_yielded(d) == Some(y)));
        let n_self = e_self.as_ref().zip(self_v.as_ref()).map(|(a, b)| a.join(b));
        let n_args: Vec<Value> = e_args.iter().zip(args).map(|(a, b)| widen_value(&a.join(b))).collect();
        let n_kw: IndexMap<StrId, Value> = e_kw.iter().map(|(n, a)| (*n, widen_value(&a.join(&kwargs[n])))).collect();
        let args_covered = n_self == *e_self && n_args == *e_args && n_kw == *e_kw;
        // (the heap: as a recursive call's, on the objects older than the run)
        let old = |o: ObjId| self.heap.born_before(o, run.mark);
        let fp = state.footprint(&run.pre);
        let fp = Footprint { rows: fp.rows.into_iter().filter(|k| old(k.0)).collect(), slots: fp.slots.into_iter().filter(|k| old(k.0)).collect(), elems: fp.elems.into_iter().filter(|o| old(*o)).collect() };
        let mut g = run.grown.as_ref().map(|g| g.copy()).unwrap_or_else(|| run.pre.copy());
        let before = g.copy();
        g.join_heap_fp(state, &fp);
        let d = g.footprint(&before);
        let heap_covered = d.rows.is_empty() && d.slots.is_empty() && d.elems.is_empty();
        // (a raise of it would reach handlers whose raises its run did not keep: it runs again,
        // keeping them)
        let seen: Vec<(FxHashSet<String>, bool)> = if run.raised { self.unwind.tries.iter().flat_map(|t| t.hclasses.iter().cloned().chain(t.runs.then(|| (std::iter::once("*".to_string()).collect(), false)))).collect() } else { Vec::new() };
        let raises_kept = seen.iter().all(|h| run.wants.contains(h));
        if !raises_kept {
            let w = self.recursion.place_wants.entry(k.clone()).or_default();
            for h in seen {
                if !w.contains(&h) {
                    w.push(h);
                }
            }
        }
        if !(fresh && args_covered && heap_covered && raises_kept) {
            if super::debug_enabled() {
                eprintln!("place rerun {} stale={} args={} heap={}", self.heap.func_info(k.0).qualname, !fresh, !args_covered, !heap_covered);
                for (o, sk) in d.slots.iter().take(2) { eprintln!("   slot {} {:?}", self.heap.obj(*o).label, sk); }
                for (o, ns) in d.rows.iter().take(2) { eprintln!("   row {} {:?}", self.heap.obj(*o).label, ns); }
            }
            // (it runs again: from its entry and heap joined with this call's — what it ran from only
            // grows — and its last result and changes the first guess of its recursive calls; past
            // `REC_MAX_ITER` runs within the same rounds, it did not settle)
            let run = self.recursion.place_runs.remove(k).unwrap();
            let fresh = fresh && raises_kept;
            if fresh && run.runs >= REC_MAX_ITER {
                self.recursion.place_unsettled.insert(k.clone());
                return PlaceCall::Skip;
            }
            self.recursion.place_settle.insert(k.clone(), if fresh { run.runs } else { 0 });
            let gfp = g.footprint(&run.pre);
            return PlaceCall::Run(Some(((n_self, n_args, n_kw), g, gfp)), Some((run.ret, run.exit, run.fp)));
        }
        let PlaceRun { ret, exit, fp, reqs, raises, out, deps, .. } = run;
        let (ret, exit, fp, reqs, raises, out, deps) = (ret.clone(), exit.copy(), fp.clone(), reqs.clone(), raises.clone(), out.clone(), deps.clone());
        self.recursion.note_deps(&deps);
        if state.reachable {
            self.replay_raises(raises, state);
        }
        state.join_heap_fp(&exit, &fp);
        for (src, term, info) in &reqs {
            state.replay_req(*src, *term, info);
        }
        self.replay_output(&out);
        if super::debug_enabled() {
            self.recursion.lookups += 1;
        }
        PlaceCall::Answer(ret)
    }

    /// A place activation's run begins (its frame pushed): what it raises is kept, the running
    /// activations whose results it takes noted (and the generators running around it).
    pub(super) fn place_run_begin(&mut self, k: &ActKey) -> super::findings::Mark {
        // (its raises are kept for the handlers around its calls that a raise of it reached)
        if let Some(w) = self.recursion.place_wants.get(k).filter(|w| !w.is_empty()) {
            self.recursion.wants_raises.insert(k.clone(), w.clone());
        }
        self.recursion.place_raised.remove(k);
        // (the generators running around it: what they read after a yield includes its reads)
        let gens: Vec<Dep> = self.gens.reads.iter().filter_map(|(_, y, _, d)| self.recursion.depth_of.iter().find(|(_, dd)| *dd == d).map(|(g, _)| (g.clone(), self.recursion.round_no.get(g).copied().unwrap_or(0), Some(*y)))).collect();
        let mut deps = gens;
        // (inside another place activation's run: one of its rounds — it iterates for both)
        if let Some((p, _, _)) = self.recursion.place_deps.last() {
            deps.push((p.clone(), self.recursion.round_no.get(p).copied().unwrap_or(0), None));
        }
        self.recursion.place_deps.push((k.clone(), self.cx.frames.len(), deps));
        self.findings.capture_begin()
    }

    /// The place activation's run ended (before `Recursion::end`) with result `ret` and end
    /// state `st`, from `pre` (before its parameters were bound): kept for later calls there.
    /// A raise of class `cls` leaves the frames of the running place activations no try of
    /// theirs sees it in.
    pub(in crate::interp) fn note_place_raise(&mut self, cls: &Option<ExcNames>) {
        for (k, d, _) in &self.recursion.place_deps {
            if !self.unwind.tries.iter().any(|t| t.frames >= *d && t.sees(cls)) {
                self.recursion.place_raised.insert(k.clone());
            }
        }
    }

    /// Whether the running generator activation `g` has yielded.
    fn gen_yielded(&self, g: &ActKey) -> Option<bool> {
        let d = self.recursion.depth_of.get(g)?;
        self.gens.reads.iter().find(|r| r.3 == *d).map(|r| r.1)
    }

    pub(super) fn place_run_end(&mut self, k: &ActKey, cap: super::findings::Mark, ret: &Value, st: &State, pre: &State) {
        let out = self.findings.capture_end(cap);
        let (_, _, deps) = self.recursion.place_deps.pop().unwrap_or_default();
        let raises = self.recursion.raises.get(k).map(|r| r.1.clone()).unwrap_or_default();
        let raised = self.recursion.place_raised.remove(k);
        let wants = self.recursion.wants_raises.get(k).cloned().unwrap_or_default();
        let (Some(entry), Some((pre_rc, grown, mark))) = (self.recursion.entry.get(k).cloned(), self.recursion.heap.get(k)) else { return };
        let reqs = st.reqs.diff_keys(&pre.reqs).into_iter().filter_map(|(src, term)| st.reqs.get(&src, &term).map(|i| (src, term, i))).collect();
        let runs = self.recursion.place_settle.remove(k).unwrap_or(0) + 1;
        let run = PlaceRun { ret: ret.clone(), exit: st.copy(), fp: st.footprint(pre), entry, pre: pre_rc.clone(), grown: grown.as_ref().map(|g| g.copy()), mark: *mark, reqs, raises, raised, wants, out, deps, runs };
        self.recursion.place_runs.insert(k.clone(), run);
    }
}

/// A recursive activation's argument, widened: more than two literals become an unknown.
fn widen_value(v: &Value) -> Value {
    if let KeyVal::Lits(ks) = &v.kv {
        if ks.len() > 2 {
            return Value::with_kv(v.pts.clone(), KeyVal::data("recursion"));
        }
    }
    v.clone()
}
