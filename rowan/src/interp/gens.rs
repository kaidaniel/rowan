//! Generators: a generator's body runs whole at its first consumption (`force_gen`), in the
//! creating call's context; a consumer of item *k* sees the effects up to yield *k*
//! (`GenEffects`); code a consumer runs between items is watched (`with_gen_restart`). A
//! `@contextmanager`'s `with` body runs at each yield (`CmHook`).
//!
//! For a call memo: `Gens` is the analyzer's flow-insensitive record of generator runs in
//! progress and done (lazy generators not run yet, the per-yield states and effects of the
//! generators run, the watched runs); a run that makes, runs or consumes a generator changes it.

use std::rc::Rc;

use indexmap::IndexMap;
use ruff_python_ast as ast;
use rustc_hash::{FxHashMap, FxHashSet};

use super::{Analyzer, Args, Payload};
use crate::domain::{join_values, kv_join, kv_literals, DATA, KeyVal, Ns, ObjId, ObjSet, Row, Value};
use crate::frontend::Node;
use crate::intern::{self, key_int, key_str_lit, KeySet, StrId, STAR};
use crate::state::{join_env, join_len, Footprint, Kind, SlotKey, State};

/// A generator's suspension point: the yield node, the `yield from` item, the unrolled loop
/// positions
pub type YieldKey = (usize, usize, Vec<usize>);

/// The states of one generator activation at its suspension points, in order, keyed by the
/// yield site and its unroll path (a key may repeat), with the first index of each key.
#[derive(Default)]
pub struct YieldStates {
    pub list: Vec<(YieldKey, State)>,
    first: FxHashMap<YieldKey, usize>,
}

impl YieldStates {
    pub(in crate::interp) fn len(&self) -> usize {
        self.list.len()
    }

    pub(in crate::interp) fn push(&mut self, key: YieldKey, st: State) {
        self.first.entry(key.clone()).or_insert(self.list.len());
        self.list.push((key, st));
    }

    /// The first state recorded under `key`.
    pub(in crate::interp) fn get_mut(&mut self, key: &YieldKey) -> Option<&mut State> {
        let i = *self.first.get(key)?;
        Some(&mut self.list[i].1)
    }

    pub(in crate::interp) fn states(&self) -> impl Iterator<Item = &State> {
        self.list.iter().map(|(_, s)| s)
    }
}

/// The body of a `with` whose manager is a `@contextmanager` generator: run where the
/// generator (one frame above `depth`) yields.
pub struct CmHook {
    pub body: &'static [ast::Stmt],
    pub target: Option<&'static ast::Expr>,
    pub depth: usize,
    pub env_after: Option<IndexMap<StrId, Value>>,
    pub ran: bool,
    /// instead of a body: code run with the yielded value (a FastAPI yield dependency: the rest
    /// of the request)
    pub then: Option<Box<dyn FnOnce(&mut Analyzer, Value, &mut State)>>,
    /// the body did not finish normally (it raised or returned) — on every run
    pub body_died: bool,
    /// the caller's names before the body (each run at another yield starts from them)
    pub env_before: Option<IndexMap<StrId, Value>>,
    /// the body may run again at another reachable yield (not a `then` callback: it runs once)
    pub multi: bool,
    /// the caller's exits from the body (its `return`s), joined over the runs: the caller's
    /// once the with statement ends (the generator's own are restored after each run)
    pub body_exits: Option<State>,
    /// the values the body returned (with `body_exits`)
    pub body_ret: Option<Value>,
}

/// What a generator's body does to objects older than its call, by how far it has run: the
/// facts of those objects before it ran (`pre`), at its k-th yield (`at[k]`), at its end,
/// and joined over all of them.
pub struct GenEffects {
    pub rows: Vec<(ObjId, Ns)>,
    pub slots: Vec<(ObjId, SlotKey)>,
    pub elems: Vec<ObjId>,
    pub lens: Vec<ObjId>,
    pub pre: State,
    pub at: Vec<Option<State>>,
    pub end: Option<State>,
    pub join: State,
    /// joined over the yields and the end, when each yield has its state in `at`: after an
    /// item was taken, at a position not known
    pub taken: std::cell::OnceCell<Option<State>>,
}

impl GenEffects {
    /// The effects given once every yield's are (all items taken), joined with the end's;
    /// computed once.
    fn taken(&self) -> Option<&State> {
        self.taken
            .get_or_init(|| {
                if self.at.is_empty() || self.at.iter().any(|s| s.is_none()) {
                    return None;
                }
                let mut it = self.at.iter().flatten().chain(self.end.as_ref());
                let mut acc = it.next()?.copy();
                for s in it {
                    acc = acc.join(&mut s.copy());
                }
                Some(acc)
            })
            .as_ref()
    }
}

/// What code changed of an object: one stored key, or anything (None).
pub type Change = (ObjId, Option<SlotKey>);

/// A change code between a generator's items made, as the generator sees it at its yields: a
/// `Change`, and whether the key may have been deleted.
pub type Hit = (ObjId, Option<SlotKey>, bool);

/// A generator run whole for a consumer that runs code between its items: the state after
/// the run, the objects the run changed, what it read after a yield, the budgets before it.
pub struct GenWatch {
    pub gid: ObjId,
    pub after: State,
    pub own: rustc_hash::FxHashSet<ObjId>,
    pub reads: rustc_hash::FxHashSet<Change>,
    /// the budgets spent before the run (a run again of the code gets them back)
    pub budget: std::rc::Rc<FxHashMap<usize, usize>>,
    /// the birth mark after the run: objects born since are new
    pub born: u64,
    /// the consumer's states where it asked for a later item (`note_resume`), and what it had
    /// changed then of the facts of the objects the generator changes itself
    pub resumed: Option<State>,
    pub own_hits: Vec<Hit>,
}

/// A generator call whose body has not run: it runs at the first consumption, from the state
/// there (`force_gen`), in the call's own site and context, so the generator object is the same.
pub struct LazyGen {
    pub foid: ObjId,
    pub self_v: Option<Value>,
    pub args: Vec<Value>,
    pub kwargs: IndexMap<StrId, Value>,
    pub node: Node,
    pub site: String,
    pub ctx: String,
    pub chain: Vec<u32>,
}

/// The generators made and run so far, and the `@contextmanager` bodies running.
#[derive(Default)]
pub struct Gens {
    /// `__next__` functions whose calls were not decided once (`unroll_next`)
    pub no_unroll_next: FxHashSet<ObjId>,
    /// generators made but not run yet: the call that made each (`force_gen`)
    pub lazy: FxHashMap<ObjId, std::rc::Rc<LazyGen>>,
    /// generators run whole for a consumer that runs code between their items: (generator,
    /// the state after its run, what it was given and yields) — see `with_gen_restart`
    pub watch: Vec<GenWatch>,
    /// what the running `for` statements iterate (`gen_used_up`)
    pub for_iters: Vec<ObjSet>,
    /// running generator activations: whether they yielded, and what they read since (an
    /// object's key, or the whole object) — what code between their items may change for them
    pub reads: Vec<(ObjId, bool, rustc_hash::FxHashSet<Change>)>,
    /// the reads of the last generator run (`force_gen` takes them)
    pub last_reads: Option<rustc_hash::FxHashSet<Change>>,
    /// generators whose consumer changes objects they reach: those objects, changed for the
    /// generator at each of its yields (`consumer_between`)
    pub havoc: FxHashMap<ObjId, Vec<Hit>>,
    /// the call site and context of the generator `force_gen` is running
    pub force_at: Option<(String, String)>,
    /// a generator's effects on older objects, per item: see `set_cursor`
    pub eff: FxHashMap<ObjId, std::rc::Rc<GenEffects>>,
    /// the state at each positioned yield of the running generator activations
    pub pos_states: FxHashMap<ObjId, Vec<Option<State>>>,
    /// positions of a generator's activation that two paths yield (no single state)
    pub pos_conflict: rustc_hash::FxHashSet<(ObjId, usize)>,
    /// the states at the yields of a generator's running activation (copies, read per key
    /// where the caller needs them: joining whole heaps per yield was the cost)
    pub yield_states: FxHashMap<ObjId, YieldStates>,
    /// a `with` over a `@contextmanager` call: its body runs at the generator's yield
    pub cm_hook: Option<CmHook>,
    /// `@contextmanager` generators whose with body is running: (their frame depth, their env
    /// at the yield, the env stack's length then), so a try in the generator around the
    /// yield sees its own names when the body raises
    pub cm_frames: Vec<(usize, IndexMap<StrId, Value>, usize)>,
    /// generators stored into a module's global (watched after the call that started them)
    pub global_held: FxHashSet<ObjId>,
}

impl Gens {
    /// A running generator that yielded reads `o` (one key, or all of it).
    pub(in crate::interp) fn read(&mut self, o: ObjId, k: Option<SlotKey>) {
        for (_, yielded, reads) in self.reads.iter_mut() {
            if *yielded {
                reads.insert((o, k));
            }
        }
    }
}

impl Analyzer {
    /// A generator call: the generator object (the one its run returns), its body not run yet.
    /// Its slot `\u{1}lazy` is 1 while not run, 0 after (both: run on some paths). Two calls at
    /// one site before either is consumed: one run with their arguments joined.
    pub(super) fn make_lazy_gen(&mut self, foid: u32, self_v: Option<Value>, mut args: Vec<Value>, mut kwargs: IndexMap<StrId, Value>, state: &mut State, node: &Node) -> Value {
        let site = self.callee_site(node);
        let ctx = self.receiver_ctx(&self_v, &site);
        let q = self.heap.func_info(foid).qualname.clone();
        let gid = self.heap.alloc_raw(Kind::List, &format!("gen:{}", foid), &ctx, true, &format!("generator {}", q), Payload::None);
        self.heap.obj_mut(gid).iterator = true;
        let mut self_v = self_v;
        if let Some(old) = self.gens.lazy.get(&gid).filter(|_| lazy_state(state, gid).0) {
            self_v = match (&self_v, &old.self_v) {
                (Some(a), Some(b)) => Some(a.join(b)),
                (a, b) => a.clone().or_else(|| b.clone()),
            };
            for (i, a) in old.args.iter().enumerate() {
                match args.get_mut(i) {
                    Some(x) => *x = x.join(a),
                    None => args.push(a.clone()),
                }
            }
            for (k, v) in old.kwargs.iter() {
                let nv = kwargs.get(k).map_or_else(|| v.clone(), |x| x.join(v));
                kwargs.insert(*k, nv);
            }
        }
        // (what the run will read, reachable from the generator for the collector)
        let keep = join_values(self_v.iter().chain(args.iter()).chain(kwargs.values()).cloned().collect::<Vec<Value>>());
        state.set_slot(gid, Ns::Attr, key_str_lit("\u{1}lazyargs"), keep, true);
        state.set_slot(gid, Ns::Attr, key_str_lit("\u{1}lazy"), Value::lit(intern::key_int(1)), true);
        let lg = super::LazyGen { foid, self_v, args, kwargs, node: *node, site, ctx, chain: self.cx.call_chain.clone() };
        self.gens.lazy.insert(gid, Rc::new(lg));
        Value::obj(gid)
    }

    /// Runs the bodies of the generators among `v` not run yet, from this state. `between`:
    /// the consumer runs code between the items (a loop, `next`), which may change what the
    /// generator reads after a yield: watched (`Gens::watch`, `with_gen_restart`).
    pub(in crate::interp) fn force_gen(&mut self, v: &Value, state: &mut State, between: bool) {
        for o in v.pts.iter() {
            let (lazy, run) = lazy_state(state, o);
            if !state.reachable || !lazy {
                continue;
            }
            let Some(lg) = self.gens.lazy.get(&o).cloned() else { continue };
            // (run already on some of the paths joined here: those keep their state)
            let before = if run { Some(state.copy()) } else { None };
            // (its items on the paths where it ran are in `before`: this run's start afresh)
            if run {
                state.drop_obj_ns(o, Ns::Pos);
                state.slots.remove(&o, &(Ns::Elem, STAR));
            }
            self.gens.force_at = Some((lg.site.clone(), lg.ctx.clone()));
            let saved = std::mem::replace(&mut self.cx.call_chain, lg.chain.clone());
            let pre = if between { Some((state.copy(), self.budget.snapshot())) } else { None };
            self.gens.last_reads = None;
            let r = self.call_user(lg.foid, lg.self_v.clone(), Args::new(&lg.args, &lg.kwargs, &[], &[]), state, &lg.node, false);
            self.cx.call_chain = saved;
            if let (Some((pre, budget)), true) = (pre, state.reachable) {
                let fp = state.footprint(&pre);
                let mut own: FxHashSet<ObjId> = fp.rows.iter().map(|k| k.0).chain(fp.slots.iter().map(|k| k.0)).chain(fp.elems.iter().copied()).chain(state.lens.diff_keys(&pre.lens)).collect();
                // (and those whose facts follow its items, installed as a consumer takes them)
                if let Some(ge) = self.gens.eff.get(&o) {
                    own.extend(ge.rows.iter().map(|k| k.0).chain(ge.slots.iter().map(|k| k.0)).chain(ge.elems.iter().copied()).chain(ge.lens.iter().copied()));
                }
                // (what it reads after a yield: through what it was given, its globals, its cells)
                let reads = self.gens.last_reads.take().unwrap_or_default();
                let w = super::GenWatch { gid: o, own, after: state.copy(), reads, budget, born: self.heap.serial(), resumed: None, own_hits: Vec::new() };
                self.gens.watch.push(w);
            }
            if !r.pts.contains(o) && state.reachable {
                // (its body did not run — past its budget, cut at the deadline: unknown code
                // gives any number of unknown items, and returns something unknown)
                let it = self.iterate(&r, &lg.node, state);
                let e = self.elem_value(&it.elem, &lg.node, "gen");
                self.add_elem(o, &e, state);
                self.heap.set_llen(o, state, None);
                state.drop_obj_ns(o, Ns::Pos);
                state.set_slot(o, Ns::Attr, key_str_lit("\u{1}genret"), Value::data("generator result"), true);
            }
            if let Some(mut b) = before {
                let j = state.join(&mut b);
                state.replace_with(j);
                // (how far it has run differs by path: not known, its effects any of its steps')
                if let Some(ge) = self.gens.eff.get(&o).cloned() {
                    crate::interp::gens::gen_copy_facts(state, &ge.join, &ge);
                    state.set_slot(o, Ns::Attr, key_str_lit("\u{1}effpos"), Value::lit(key_str_lit("join")), true);
                }
                self.set_cursor(o, None, state);
            }
            state.slots.remove(&o, &(Ns::Attr, key_str_lit("\u{1}lazyargs")));
            state.set_slot(o, Ns::Attr, key_str_lit("\u{1}lazy"), Value::lit(intern::key_int(0)), true);
        }
    }

    /// The heap a generator's caller sees (see the call): the touched objects older than
    /// `mark` (but the generator's own item list) joined over `pre`, the yield states and `st`.
    pub(super) fn gen_suspension_join(&mut self, st: &mut State, pre: &State, yields: &[State], gid: ObjId, mark: u64) {
        if !st.reachable {
            return;
        }
        let fp = st.footprint(pre);
        let old = |o: ObjId| self.heap.born_before(o, mark) && o != gid;
        let ys: Vec<&State> = yields.iter().filter(|y| y.reachable).collect();
        // Each fact of `st` is joined with `pre`'s and every yield state's. Yield states are
        // copies of one another changed by the body, sharing structure: each is diffed against
        // the one before it (the first against `pre`), and only the keys the diff names are
        // joined. A fact a state does not change equals the previous state's, joined already
        // (a generator can hold thousands of yield states, and joining every touched fact with
        // each was facts × yields).
        let entry = |_: &Analyzer, s: &State, o: ObjId| -> (Option<usize>, usize, u16) { s.lens.get(&o).unwrap_or((None, 0, 0)) };
        // (with the distinct rows already joined: most yield states share their rows)
        let mut rows: rustc_hash::FxHashMap<(ObjId, Ns), (Row, rustc_hash::FxHashSet<Row>)> = rustc_hash::FxHashMap::default();
        for (o, ns) in fp.rows.iter().copied().filter(|(o, _)| old(*o)) {
            let (a, b) = (st.row(o, ns), pre.row(o, ns));
            let j = a.join(&b);
            rows.insert((o, ns), (j, [a, b].into_iter().collect()));
        }
        let mut slots: rustc_hash::FxHashMap<(ObjId, SlotKey), Value> = rustc_hash::FxHashMap::default();
        let mut touched: Vec<ObjId> = Vec::new();
        for (o, sk) in fp.slots.iter().copied().filter(|(o, _)| old(*o)) {
            touched.push(o);
            slots.insert((o, sk), st.slots.get(&o, &sk).unwrap_or_default().join(&pre.slots.get(&o, &sk).unwrap_or_default()));
        }
        let mut elems: rustc_hash::FxHashMap<ObjId, KeyVal> = rustc_hash::FxHashMap::default();
        for o in fp.elems.iter().copied().filter(|o| old(*o)) {
            touched.push(o);
            elems.insert(o, kv_join(&st.elems.get(&o).unwrap_or(DATA), &pre.elems.get(&o).unwrap_or(DATA)));
        }
        touched.sort_unstable();
        touched.dedup();
        let mut lens: rustc_hash::FxHashMap<ObjId, (Option<usize>, usize, u16)> = touched.iter().map(|o| (*o, join_len(&entry(self, st, *o), &entry(self, pre, *o)))).collect();
        let mut prev = pre;
        for y in ys {
            let base = std::mem::replace(&mut prev, y);
            for k in y.rows.diff_keys(&base.rows) {
                if let Some((r, seen)) = rows.get_mut(&k) {
                    let yr = y.row(k.0, k.1);
                    if !seen.contains(&yr) {
                        *r = r.join(&yr);
                        seen.insert(yr);
                    }
                }
            }
            for (o, sk) in y.slots.diff_keys(&base.slots) {
                if let Some(v) = slots.get_mut(&(o, sk)) {
                    *v = v.join(&y.slots.get(&o, &sk).unwrap_or_default());
                }
            }
            for o in y.elems.diff_keys(&base.elems) {
                if let Some(kv) = elems.get_mut(&o) {
                    *kv = kv_join(kv, &y.elems.get(&o).unwrap_or(DATA));
                }
            }
            for o in y.lens.diff_keys(&base.lens) {
                if let Some(l) = lens.get_mut(&o) {
                    *l = join_len(l, &entry(self, y, o));
                }
            }
        }
        for ((o, ns), (r, _)) in rows {
            st.set_row(o, ns, r);
        }
        for ((o, sk), v) in slots {
            st.slots.set(&o, sk, v);
            if matches!(sk.0, Ns::Pos | Ns::Elem) {
                st.clear_last(o);
            }
        }
        for (o, kv) in elems {
            st.elems.insert(o, kv);
        }
        for (o, l) in lens {
            st.set_lens(o, l);
        }
    }

    /// Run `body` from `state`. A generator it ran whole for a consumer that runs code between
    /// the items (`Gens::watch`) did not see what that code changed: when the code changed objects
    /// the generator reaches, `body` runs again, those objects changed for the generator at each
    /// of its yields (`gen_havoc`; at most three rounds, then everything it reaches). `close`:
    /// the generators run in `body` are used up or out of reach after it (a loop without
    /// `break`, a call's end): no later code changes anything for them.
    pub(in crate::interp) fn with_gen_restart(&mut self, state: &mut State, close: bool, body: &mut dyn FnMut(&mut Analyzer, &mut State)) {
        let pre = state.copy();
        let vmark = self.findings.mark();
        let mmark = self.program.modules.len();
        for round in 0..4 {
            let wmark = self.gens.watch.len();
            body(self, state);
            let found = self.gen_interference(wmark, state, round == 3);
            if found.is_empty() || round == 3 {
                if close {
                    self.close_watches(wmark, &pre, state);
                }
                return;
            }
            if super::debug_enabled() {
                // (and what changed for the generators: the objects, the first few of each)
                let what: Vec<String> = found.iter().flat_map(|(_, hs)| hs.iter().take(4)).map(|h| format!("{}{}", self.heap.obj(h.0).label, h.1.and_then(|k| intern::key_as_str(k.1)).map(|k| format!(".{}", k)).unwrap_or_default())).collect();
                eprintln!("restart in {} (frames {}), round {}: {}", self.frame().label, self.cx.frames.len(), round, what.join(", "));
            }
            for (gid, objs) in found {
                self.gens.havoc.entry(gid).or_default().extend(objs);
            }
            self.findings.drop_since(vmark);
            // (the budgets as before the first generator run: the run again spends them)
            if let Some(w) = self.gens.watch.get(wmark) {
                self.budget.restore(&w.budget);
            }
            self.gens.watch.truncate(wmark);
            // (modules it imported stay imported: their facts go on)
            let mods: Vec<Value> = self.program.modules.values().skip(mmark).map(|m| Value::obj(*m)).collect();
            let mut again = pre.copy();
            if !mods.is_empty() {
                let fp = state.footprint(&pre);
                let mut targets: Vec<ObjId> = fp.rows.iter().map(|k| k.0).chain(fp.slots.iter().map(|k| k.0)).chain(fp.elems.iter().copied()).collect();
                targets.sort_unstable();
                targets.dedup();
                let reach = self.reach_among(&mods, &targets, state);
                let keep = Footprint { rows: fp.rows.into_iter().filter(|k| reach.contains(&k.0)).collect(), slots: fp.slots.into_iter().filter(|k| reach.contains(&k.0)).collect(), elems: fp.elems.into_iter().filter(|o| reach.contains(o)).collect() };
                again.join_heap_fp(state, &keep);
            }
            state.replace_with(again);
        }
    }

    /// What code changed from `a` to `b` (a read that narrows a row is no change): a whole
    /// object (unknown code), a key no longer surely there (maybe deleted), one stored. Only
    /// objects with facts in `a` (one without was garbage there: nothing read it since).
    fn changes_between(&self, a: &State, b: &State) -> Vec<Hit> {
        let fp = b.footprint(a);
        self.changes_in(a, b, &fp.rows, &fp.slots, fp.elems.into_iter().chain(b.lens.diff_keys(&a.lens)))
    }

    /// `changes_between` on these facts (and these objects' elements and lengths).
    fn changes_in(&self, a: &State, b: &State, rows: &[(ObjId, Ns)], slots: &[(ObjId, SlotKey)], others: impl Iterator<Item = ObjId>) -> Vec<Hit> {
        let mut changed: Vec<Hit> = Vec::new();
        for &(o, ns) in rows {
            let Some(ra) = a.rows.get(&(o, ns)) else { continue };
            let rb = b.row(o, ns);
            // (unknown code may have changed it — a key surely there now there unless unknown
            // code removed it is that too, not a delete)
            if ra.mutators != rb.mutators || ra.top != rb.top || ra.definite.iter().any(|k| !rb.definite.contains(k) && rb.likely.contains(k)) {
                changed.push((o, None, false));
                continue;
            }
            let lost = ra.definite.iter().filter(|k| !rb.definite.contains(*k)).chain(ra.likely.iter().filter(|k| !rb.likely.contains(*k)));
            changed.extend(lost.map(|k| (o, Some((ns, k)), true)).collect::<Vec<_>>());
            changed.extend(ra.absent.iter().filter(|k| !rb.absent.contains(*k)).map(|k| (o, Some((ns, k)), false)).collect::<Vec<_>>());
        }
        for &(o, sk) in slots {
            let va = a.slots.get(&o, &sk);
            if va == b.slots.get(&o, &sk) || (va.is_none() && !a.slots.has_key(&o) && !a.has_row(o, sk.0)) {
                continue;
            }
            let any = b.slots.get(&o, &sk).is_some_and(|v| v.pts.iter().any(|p| self.is_any(p)));
            let key = (!any && matches!(sk.0, Ns::Item | Ns::Attr) && sk.1 != intern::STAR).then_some(sk);
            changed.push((o, key, false));
        }
        // (a record's length entry marks its birth, not a change)
        let differs = |o: &ObjId| (a.elems.get(o).is_some() && a.elems.get(o) != b.elems.get(o)) || (a.lens.get(o).is_some() && a.lens.get(o) != b.lens.get(o) && self.heap.kind(*o).is_container());
        changed.extend(others.filter(differs).map(|o| (o, None, false)).collect::<Vec<_>>());
        changed
    }

    /// The generators watched since `wmark` are used up or out of reach: no longer watched, but
    /// those that may give more items and that what the code returns or what it was given
    /// (`pre`'s names) hold (a generator started in a call and returned from it, or kept in an
    /// attribute of its receiver for the next call).
    fn close_watches(&mut self, wmark: usize, pre: &State, state: &State) {
        let at = if state.reachable { state } else { state.exits.as_deref().unwrap_or(state) };
        let mut gids: Vec<ObjId> = self.gens.watch[wmark.min(self.gens.watch.len())..].iter().map(|w| w.gid).filter(|g| self.may_resume(*g, at)).collect();
        gids.sort_unstable();
        gids.dedup();
        let kept = if gids.is_empty() {
            FxHashSet::default()
        } else {
            let roots: Vec<Value> = state.ret.iter().chain(pre.env.values()).cloned().collect();
            let mut kept = self.reach_among(&roots, &gids, at);
            // (a generator a global holds is resumed by later calls)
            kept.extend(gids.iter().filter(|g| self.gens.global_held.contains(g)));
            kept
        };
        let tail = self.gens.watch.split_off(wmark.min(self.gens.watch.len()));
        self.gens.watch.extend(tail.into_iter().filter(|w| kept.contains(&w.gid)));
    }

    /// Whether a generator may give more items in `state` (not known to be used up).
    fn may_resume(&self, g: ObjId, state: &State) -> bool {
        if state.slot_opt(g, Ns::Attr, key_str_lit("__cursor")) == Some(Value::data("cursor-end")) {
            return false;
        }
        !matches!((self.cursor_of(g, state), self.heap.llen(g, state)), (Some(c), Some(n)) if c >= n)
    }

    /// The watched generators since `wmark` and what later code changed by `fin` of the
    /// objects they reach from what they were given (in the state after their run): a key
    /// stored, or a whole object; beyond what is already changed for them (`all`: everything
    /// they reach).
    pub(super) fn gen_interference(&self, wmark: usize, fin: &State, all: bool) -> Vec<(ObjId, Vec<Hit>)> {
        (wmark..self.gens.watch.len()).map(|i| (self.gens.watch[i].gid, self.gen_interference_of(i, fin, all))).filter(|(_, h)| !h.is_empty()).collect()
    }

    /// `gen_interference` for one watched generator: the keys / objects it reads after a
    /// yield that later code changed by `fin` or by the time it asked for an item (unknown
    /// code too; of the objects it changes itself, what changed of the facts it gave).
    pub(super) fn gen_interference_of(&self, i: usize, fin: &State, all: bool) -> Vec<Hit> {
        let w = &self.gens.watch[i];
        if !fin.reachable {
            return Vec::new();
        }
        // (what the consumer changed by the time it asked for an item, or by the end)
        let joined;
        let fin = match &w.resumed {
            Some(r) if r.reachable => {
                joined = fin.copy().join(&mut r.copy());
                &joined
            }
            _ => fin,
        };
        let changed = self.changes_between(&w.after, fin);
        let read = |(o, k, _): &Hit| w.reads.contains(&(*o, *k)) || w.reads.contains(&(*o, None)) || (k.is_none() && w.reads.iter().any(|r| r.0 == *o));
        // (an object made since is not the one the generator read, whatever its site)
        let older = |o: ObjId| self.heap.born_before(o, w.born);
        // (the objects it changes itself: as the consumer changed what it was given of them)
        let own = w.own_hits.iter().filter(|c| read(c)).cloned();
        let mut hit: Vec<Hit> = changed.into_iter().filter(|c| c.0 != w.gid && older(c.0) && !w.own.contains(&c.0) && read(c)).chain(own).collect();
        if all && !hit.is_empty() {
            hit = w.reads.iter().map(|r| (r.0, None, false)).collect();
        }
        hit.sort_unstable();
        hit.dedup();
        let have = self.gens.havoc.get(&w.gid);
        hit.retain(|c| !have.is_some_and(|h| h.contains(c) || h.contains(&(c.0, None, false))));
        if !hit.is_empty() && super::debug_enabled() {
            eprintln!("generator {} sees changes of {:?}", self.heap.obj(w.gid).label, hit.iter().map(|(o, k, _)| format!("{}{:?}", self.heap.obj(*o).label, k.map(|k| intern::key_repr(k.1)))).collect::<Vec<_>>());
        }
        hit
    }

    /// A loop used up the generators among `used`: no later code runs between their items. Those
    /// that saw no change by `fin` are no longer watched; the others are left to the run that
    /// made them, to run again.
    pub(in crate::interp) fn gen_used_up(&mut self, used: &ObjSet, fin: &State) {
        let mut i = 0;
        while i < self.gens.watch.len() {
            if used.contains(self.gens.watch[i].gid) && self.gen_interference_of(i, fin, false).is_empty() {
                self.gens.watch.remove(i);
            } else {
                i += 1;
            }
        }
    }

    /// Which of `targets` are reachable from `roots` in `st` (the walk ends when all are found).
    pub(super) fn reach_among(&self, roots: &[Value], targets: &[ObjId], st: &State) -> FxHashSet<ObjId> {
        let mut found: FxHashSet<ObjId> = FxHashSet::default();
        let mut seen: FxHashSet<ObjId> = FxHashSet::default();
        let mut work: Vec<ObjId> = roots.iter().flat_map(|v| v.pts.iter()).collect();
        while let Some(o) = work.pop() {
            if !seen.insert(o) {
                continue;
            }
            self.budget.walk_visit();
            if targets.binary_search(&o).is_ok() {
                found.insert(o);
                if found.len() == targets.len() {
                    break;
                }
            }
            if !matches!(self.heap.kind(o), Kind::Module | Kind::Class | Kind::Function) {
                self.obj_edges(o, st, &mut work);
            }
        }
        found
    }

    /// At a `@contextmanager` generator's yield: the with body runs in the caller's frame with
    /// the target bound to the yielded value, then the generator goes on.
    pub(super) fn run_cm_body(&mut self, v: Value, state: &mut State) {
        let Some(mut h) = self.gens.cm_hook.take() else { return };
        let callee_fr = self.cx.frames.pop().unwrap();
        let caller_env = match &h.env_before {
            Some(e) => e.clone(),
            None => {
                let e = self.cx.env_stack.last().cloned().unwrap_or_default();
                h.env_before = Some(e.clone());
                e
            }
        };
        let callee_env = std::mem::replace(&mut state.env, caller_env);
        let callee_ret = state.ret.take();
        let callee_exits = state.exits.take();
        if let Some(t) = h.target {
            self.bind_target(t, &v, state);
        }
        self.gens.cm_frames.push((self.cx.frames.len() + 1, callee_env.clone(), self.cx.env_stack.len()));
        match h.then.take() {
            Some(f) => f(self, v, state),
            None => self.exec_block(h.body, state),
        }
        h.body_died = !state.reachable && (!h.ran || h.body_died);
        // (the body's `return`s are the caller's: kept for when the with statement ends)
        if let Some(ex) = state.exits.take() {
            let mut ex = ex.copy();
            h.body_exits = Some(match h.body_exits.take() {
                Some(mut prev) => prev.join(&mut ex),
                None => ex,
            });
        }
        if let Some(r) = state.ret.take() {
            h.body_ret = Some(match h.body_ret.take() {
                Some(prev) => prev.join(&r),
                None => r,
            });
        }
        self.gens.cm_frames.pop();
        let after = std::mem::replace(&mut state.env, callee_env);
        // (another yield's run: the caller's names after the with are either run's)
        let after = match h.env_after.take() {
            Some(prev) if h.ran => join_env(&prev, &after),
            _ => after,
        };
        if let Some(top) = self.cx.env_stack.last_mut() {
            *top = after.clone();
        }
        h.env_after = Some(after);
        h.ran = true;
        state.ret = callee_ret;
        state.exits = callee_exits;
        self.cx.frames.push(callee_fr);
        self.gens.cm_hook = Some(h);
    }

    /// After a yield whose consumer runs code before it asks for the next item: what the
    /// generator was given and has yielded may have changed (for the generator only: the
    /// consumer's own run of that code is analysed where it is).
    pub(super) fn consumer_between(&mut self, gid: ObjId, state: &mut State) {
        let Some(changes) = self.gens.havoc.get(&gid).cloned().filter(|c| !c.is_empty()) else { return };
        let site = intern::s(&format!("consumer@{}", self.heap.obj(gid).label.trim_start_matches("generator ")));
        let rid = crate::interp::any_object(&mut self.heap);
        // (an object that code changed in a way not one key: any of its facts, the objects
        // it holds aside: those it changed are listed too)
        let whole: Vec<ObjId> = changes.iter().filter(|c| c.1.is_none()).map(|c| c.0).collect();
        self.change_objects(&whole, rid, site, state);
        // (a key that code stored: any value, maybe stored; one it deleted: maybe deleted)
        for (o, sk, del) in changes.iter().filter_map(|(o, k, d)| k.map(|k| (*o, k, *d))) {
            let r = state.row(o, sk.0);
            if del {
                state.set_row(o, sk.0, r.delete(sk.1, false));
                continue;
            }
            let v = state.slot(o, sk.0, sk.1).join(&Value::obj(rid));
            state.set_slot(o, sk.0, sk.1, v, false);
            state.set_row(o, sk.0, r.store(sk.1, false));
        }
    }

    /// Code `site` may have changed these objects in any way (not the objects they hold): any
    /// key, any value (`rid`), any length.
    pub(in crate::interp) fn change_objects(&mut self, objs: &[ObjId], rid: ObjId, site: intern::StrId, state: &mut State) {
        for &o in objs {
            for ns in [Ns::Item, Ns::Attr] {
                if state.has_row(o, ns) {
                    let r = state.row(o, ns).mutate(site);
                    state.set_row(o, ns, r);
                }
            }
            for ((ns, k), v) in state.slots_of(o) {
                state.set_slot(o, ns, k, v.join(&Value::obj(rid)), false);
            }
            if state.elems.get(&o).is_some() {
                state.elems.insert(o, DATA);
            }
            self.heap.set_llen(o, state, None);
        }
    }

    /// `site`: the yield (and the item of a `yield from`), which with the unrolled loop
    /// positions names the suspension point.
    pub(super) fn yield_value(&mut self, v: Value, state: &mut State, site: (usize, usize)) -> Option<ObjId> {
        let foid = self.frame().func?;
        let ctx = self.frame().ctx.clone();
        let label = format!("generator {}", self.frame().label);
        let gid = self.heap.alloc_raw(Kind::List, &format!("gen:{}", foid), &ctx, true, &label, Payload::None);
        if let Some(r) = self.gens.reads.iter_mut().rev().find(|r| r.0 == gid) {
            r.1 = true;
        }
        // a fresh generator list (no yield yet in this activation) starts empty
        let fresh = !state.has_slot(gid, Ns::Elem, STAR);
        // (lengths that were one of a few: one more each, after this item)
        let alts_before = if fresh { 0 } else { state.lens.get(&gid).filter(|l| l.0.is_none()).map(|l| l.2).unwrap_or(0) };
        state.set_slot(gid, Ns::Elem, STAR, v.clone(), false);
        let cur = state.elems.get(&gid).unwrap_or_else(|| v.kv.clone());
        state.elems.insert(gid, kv_join(&cur, &v.kv));
        // straight-line yields keep their order; in a branch too (a length per path, joined
        // to the shorter known prefix, like appends)
        let in_branch = self.cx.cond_depth > 0;
        let positional = self.frame().single && self.frame().loop_depth == 0 && self.heap.obj(gid).single;
        let ll = if fresh { Some(0) } else { self.heap.llen(gid, state) };
        match (positional, ll) {
            (true, Some(n)) if n < super::UNROLL_MAX => {
                state.set_slot(gid, Ns::Pos, key_int(n as i64), v, true);
                self.heap.set_llen(gid, state, Some(n + 1));
                // (the state at this item: what a consumer that took it sees of the effects)
                // (in a branch: another path may have its own item n — the consumer then
                // takes the generator's joined state there)
                let conflicted = self.gens.pos_conflict.contains(&(gid, n));
                let mut conflict_now = false;
                let ps = self.gens.pos_states.entry(gid).or_default();
                while ps.len() <= n {
                    ps.push(None);
                }
                ps[n] = match (in_branch, ps[n].take(), conflicted) {
                    (_, _, true) => None,
                    (true, Some(_), false) => {
                        conflict_now = true;
                        None
                    }
                    _ => Some(state.copy()),
                };
                if conflict_now {
                    self.gens.pos_conflict.insert((gid, n));
                }
            }
            // (the yields so far keep their positions: a known prefix)
            (_, Some(n)) if n > 0 => {
                self.heap.set_prefix(gid, state, n);
            }
            _ if self.heap.prefix(gid, state).is_some() => {}
            _ => {
                self.heap.set_llen(gid, state, None);
                state.drop_obj_ns(gid, Ns::Pos);
            }
        }
        if alts_before != 0 {
            if let Some((None, p, 0)) = state.lens.get(&gid) {
                let shifted = (alts_before as u32) << 1;
                if shifted <= 0xFFFF {
                    state.set_lens(gid, (None, p, shifted as u16));
                }
            }
        }
        const MAX_YIELD_STATES: usize = 4096;
        let key = (site.0, site.1, self.frame().unroll.clone());
        // (in a loop's fixpoint a later round's state at a suspension point covers the
        // earlier rounds', as the loop's exit does)
        let in_fixpoint = self.frame().loop_depth > 0;
        let here = state.copy();
        let ys = self.gens.yield_states.entry(gid).or_default();
        if let Some(slot) = ys.get_mut(&key).filter(|_| in_fixpoint) {
            *slot = here;
        } else if ys.len() < MAX_YIELD_STATES {
            ys.push(key, here);
        } else if let Some(last) = ys.list.last_mut() {
            // (past the cap: the last one stands for the rest too)
            let mut h = here;
            last.1 = last.1.join(&mut h);
        }
        self.consumer_between(gid, state);
        Some(gid)
    }

    /// A one-shot iterator iterated to its end: later iterations see nothing.
    /// A lazy iterator consumed (to the end, or partly: `full` false): what it takes from its
    /// sources.
    pub fn consume_lazy(&mut self, v: &Value, full: bool, state: &mut State) {
        for lo in v.pts.iter() {
            let Some(srcs) = self.m.lazy_src.get(&lo).cloned() else { continue };
            // (consumed once on a path: a one-shot iterator itself)
            let done = key_str_lit("\u{1}consumed");
            if state.has_slot(lo, Ns::Attr, done) {
                continue;
            }
            state.set_slot(lo, Ns::Attr, done, Value::lit(intern::key_bool(true)), true);
            for (src, n) in srcs {
                match (full, n, self.cursor_of(src, state)) {
                    (true, Some(n), Some(c)) => {
                        let end = match self.heap.llen(src, state) {
                            Some(len) => (c + n).min(len),
                            None => c + n,
                        };
                        self.set_cursor(src, Some(end), state);
                    }
                    (true, None, _) => {
                        self.exhaust(&Value::obj(src), state);
                    }
                    _ => {
                        self.set_cursor(src, None, state);
                    }
                }
            }
        }
    }

    /// Move a one-shot iterator's cursor (None: not known). A generator with tracked effects
    /// gets the facts of the objects it changes as they are after that many items — while
    /// they still are what was installed last (the consumer did not change them meanwhile).
    pub fn set_cursor(&mut self, o: u32, c: Option<usize>, state: &mut State) {
        self.set_cursor_ex(o, c, false, state)
    }

    /// `set_cursor`; `at_end`: used up (its length may not be known).
    pub(in crate::interp) fn set_cursor_ex(&mut self, o: u32, c: Option<usize>, at_end: bool, state: &mut State) {
        self.note_resume(o, state);
        let cv = match c {
            Some(n) => Value::lit(key_int(n as i64)),
            None => Value::data(if at_end { "cursor-end" } else { "cursor" }),
        };
        state.set_slot(o, Ns::Attr, key_str_lit("__cursor"), cv, true);
        let Some(ge) = self.gens.eff.get(&o).cloned() else { return };
        let tk = key_str_lit("\u{1}effpos");
        let Some(expected) = effects_given(state, o, &ge) else { return };
        let equal = gen_facts_equal(state, expected, &ge);
        let ended = at_end || matches!((c, self.heap.llen(o, state)), (Some(k), Some(n)) if k >= n);
        let (target, ntag): (&State, String) = match c {
            // (used up: at its end, also when it yields nothing)
            _ if ended && ge.end.is_some() => (ge.end.as_ref().unwrap(), "end".into()),
            Some(0) => (&ge.pre, "pre".into()),
            Some(k) => match ge.at.get(k - 1).and_then(|x| x.as_ref()) {
                Some(s) => (s, format!("at{}", k - 1)),
                None => (&ge.join, "join".into()),
            },
            None => (&ge.join, "join".into()),
        };
        // (code between the items changed what the generator changes: both changes, joined)
        if !equal {
            gen_join_facts(state, target, &ge);
            state.set_slot(o, Ns::Attr, tk, Value::data("effpos"), true);
            return;
        }
        gen_copy_facts(state, target, &ge);
        state.set_slot(o, Ns::Attr, tk, Value::lit(key_str_lit(&ntag)), true);
    }

    /// An item of generator `o` was taken at a position not known: its effects are those of
    /// some yield or of its end, not those before its first item.
    pub fn gen_took_item(&mut self, o: ObjId, state: &mut State) {
        let Some(ge) = self.gens.eff.get(&o).cloned() else { return };
        let Some(expected) = effects_given(state, o, &ge) else { return };
        if !gen_facts_equal(state, expected, &ge) {
            gen_join_facts(state, &ge.join, &ge);
            state.set_slot(o, Ns::Attr, key_str_lit("\u{1}effpos"), Value::data("effpos"), true);
            return;
        }
        // (every item has its yield's state when the generator yields a known number of items:
        // some yield's or the end's; else any point it may be suspended at)
        let (target, tag) = match ge.taken() {
            Some(t) if self.heap.llen(o, state) == Some(ge.at.len()) => (t, "taken"),
            _ => (&ge.join, "join"),
        };
        gen_copy_facts(state, target, &ge);
        state.set_slot(o, Ns::Attr, key_str_lit("__cursor"), Value::data("cursor"), true);
        state.set_slot(o, Ns::Attr, key_str_lit("\u{1}effpos"), Value::lit(key_str_lit(tag)), true);
    }

    /// The consumer asks a watched generator `o` for an item in `state`: what it changed by
    /// then, the generator sees after its yield (`gen_interference_of`).
    pub(in crate::interp) fn note_resume(&mut self, o: ObjId, state: &State) {
        if !state.reachable || !self.gens.watch.iter().any(|w| w.gid == o) {
            return;
        }
        // (the objects it changes itself: what the consumer changed of the facts it was given)
        let ge = self.gens.eff.get(&o).cloned();
        let own = ge.as_ref().and_then(|ge| effects_given(state, o, ge).map(|given| self.changes_in(given, state, &ge.rows, &ge.slots, ge.elems.iter().chain(&ge.lens).copied()))).unwrap_or_default();
        for w in self.gens.watch.iter_mut().filter(|w| w.gid == o) {
            w.resumed = Some(match w.resumed.take() {
                Some(mut r) => r.join(&mut state.copy()),
                None => state.copy(),
            });
            w.own_hits.extend(own.iter().cloned());
        }
    }

    /// `note_resume` for what the innermost running `for` iterates.
    pub(in crate::interp) fn note_resumes(&mut self, state: &State) {
        if let Some(objs) = self.gens.for_iters.last().filter(|_| !self.gens.watch.is_empty()).cloned() {
            for o in objs.iter() {
                self.note_resume(o, state);
            }
        }
    }

    /// How many items a one-shot iterator has given (None: not known).
    pub fn cursor_of(&self, o: u32, state: &State) -> Option<usize> {
        match state.slot_opt(o, Ns::Attr, key_str_lit("__cursor")) {
            None => Some(0),
            Some(v) => v.kv.as_lit().filter(|_| v.pts.is_empty()).and_then(intern::key_as_int).map(|i| i.max(0) as usize),
        }
    }

    /// `v` is used up (`list(gen)`, a `for` to its end): a lazy generator runs, an iterator's
    /// cursor goes to its end, a generator's effects are all given.
    pub fn exhaust(&mut self, v: &Value, state: &mut State) {
        self.consume_lazy(v, true, state);
        if let Some(o) = v.single() {
            if self.heap.obj(o).iterator && self.heap.obj(o).single {
                match self.heap.llen(o, state) {
                    Some(n) => self.set_cursor(o, Some(n), state),
                    // (used up, its length not known: at its end all the same)
                    None => self.set_cursor_ex(o, None, true, state),
                }
                return;
            }
        }
        // (a generator object that stands for several, or one of several: its run's effects,
        // at its end when it is the one used up, else at any point)
        let one = v.pts.len() == 1 && v.kv.is_bot();
        for o in v.pts.iter().filter(|o| self.gens.eff.contains_key(o)).collect::<Vec<_>>() {
            self.set_cursor_ex(o, None, one, state);
        }
    }
}

/// The facts of a generator's effects its consumer was last given in `state` (`set_cursor_ex`).
fn effects_given<'a>(state: &State, o: ObjId, ge: &'a crate::interp::GenEffects) -> Option<&'a State> {
    let tag = state.slot_opt(o, Ns::Attr, key_str_lit("\u{1}effpos")).and_then(|v| v.kv.as_lit().filter(|_| v.pts.is_empty())).and_then(intern::key_as_str).map(|s| s.to_string());
    match tag.as_deref() {
        Some("pre") => Some(&ge.pre),
        Some("join") => Some(&ge.join),
        Some("taken") => ge.taken(),
        Some("end") => ge.end.as_ref(),
        Some(t) => t.strip_prefix("at").and_then(|k| k.parse::<usize>().ok()).and_then(|k| ge.at.get(k)).and_then(|x| x.as_ref()),
        None => None,
    }
}

/// (not run on some path, run on some path) for a generator `make_lazy_gen` made
fn lazy_state(state: &State, o: ObjId) -> (bool, bool) {
    match state.slot_opt(o, Ns::Attr, key_str_lit("\u{1}lazy")) {
        Some(v) => {
            let ks = kv_literals(&v.kv);
            (ks.contains(intern::key_int(1)), ks.contains(intern::key_int(0)))
        }
        None => (false, false),
    }
}

/// Do two states agree on the facts a generator's effects concern?
fn gen_facts_equal(a: &State, b: &State, ge: &crate::interp::GenEffects) -> bool {
    ge.rows.iter().all(|k| a.rows.get(k) == b.rows.get(k))
        && ge.slots.iter().all(|(o, sk)| a.slots.get(o, sk) == b.slots.get(o, sk))
        && ge.elems.iter().all(|o| a.elems.get(o) == b.elems.get(o))
        && ge.lens.iter().all(|o| a.lens.get(o) == b.lens.get(o))
}

/// Join `src`'s facts about a generator's affected objects into `dst`'s: those the generator
/// changed by then (from what it was given, `ge.pre`); the others are the consumer's as they are.
fn gen_join_facts(dst: &mut State, src: &State, ge: &crate::interp::GenEffects) {
    let pre = &ge.pre;
    for k in &ge.rows {
        if let (Some(a), Some(b)) = (dst.rows.get(k), src.rows.get(k)) {
            if pre.rows.get(k).as_ref() != Some(&b) {
                dst.rows.insert(*k, a.join(&b));
            }
        }
    }
    for (o, sk) in &ge.slots {
        if let (Some(a), Some(b)) = (dst.slots.get(o, sk), src.slots.get(o, sk)) {
            if pre.slots.get(o, sk).as_ref() != Some(&b) {
                dst.slots.set(o, *sk, a.join(&b));
                if matches!(sk.0, Ns::Pos | Ns::Elem) {
                    dst.clear_last(*o);
                }
            }
        }
    }
    for o in &ge.elems {
        if let (Some(a), Some(b)) = (dst.elems.get(o), src.elems.get(o)) {
            if pre.elems.get(o).as_ref() != Some(&b) {
                dst.elems.insert(*o, kv_join(&a, &b));
            }
        }
    }
    for o in &ge.lens {
        let b = src.lens.get(o);
        if pre.lens.get(o) == b {
            continue;
        }
        match (dst.lens.get(o), b) {
            (Some(a), Some(b)) => dst.set_lens(*o, join_len(&a, &b)),
            _ => {
                dst.forget_len(o);
            }
        }
    }
}

/// Install `src`'s facts about a generator's affected objects into `dst`.
pub fn gen_copy_facts(dst: &mut State, src: &State, ge: &crate::interp::GenEffects) {
    for k in &ge.rows {
        match src.rows.get(k) {
            Some(r) => dst.rows.insert(*k, r),
            None => {
                dst.rows.remove(k);
            }
        }
    }
    for (o, sk) in &ge.slots {
        match src.slots.get(o, sk) {
            Some(v) => dst.slots.set(o, *sk, v),
            None => {
                dst.slots.remove(o, sk);
            }
        }
    }
    for o in &ge.elems {
        match src.elems.get(o) {
            Some(v) => dst.elems.insert(*o, v),
            None => {
                dst.elems.remove(o);
            }
        }
    }
    for o in &ge.lens {
        match src.lens.get(o) {
            Some(v) => dst.set_lens(*o, v),
            None => {
                dst.forget_len(o);
            }
        }
    }
    for (o, sk) in &ge.slots {
        if matches!(sk.0, Ns::Pos | Ns::Elem) {
            dst.clear_last(*o);
        }
    }

}

impl Analyzer {
    /// A generator activation begins (its frame pushed): its object of yielded values, new;
    /// what the activation's site held before (a running activation's yields, put back after).
    pub(super) fn gen_activation_begin(&mut self, ygid: ObjId, foid: ObjId, fi: &super::FuncInfo, ctx: &str, state: &mut State) -> Option<YieldStates> {
        // the yielded values of this activation
        let gid = self.alloc(Kind::List, &format!("gen:{}", foid), state, &format!("generator {}", fi.qualname), Payload::None, None, Ns::Item, None, Some(ctx));
        self.heap.set_llen(gid, state, Some(0));
        state.elems.insert(gid, KeyVal::Lits(KeySet::new()));
        // (a new activation: not consumed yet, its own item states; a running one's
        // yields are put back after it)
        // (keyed as `yield_value` finds it: the site's object, not a variant `alloc` made)
        let saved = self.gens.yield_states.remove(&ygid);
        state.slots.remove(&gid, &(Ns::Attr, key_str_lit("__cursor")));
        state.slots.remove(&gid, &(Ns::Attr, key_str_lit("\u{1}effpos")));
        self.gens.pos_states.remove(&gid);
        self.gens.pos_conflict.retain(|(g, _)| *g != gid);
        self.gens.eff.remove(&gid);
        saved
    }

    /// A generator activation's body ran (to `st`): the generator's body runs at the call,
    /// eagerly, but its caller sees the heap only where the body is suspended: before the first
    /// item, at a yield, or at its end. The objects that existed before the call and that the
    /// body changed hold the join of those states (`gen_suspension_join`), and its effects per
    /// item are kept (`GenEffects`).
    pub(super) fn gen_activation_end(&mut self, gid: ObjId, gen_cm: bool, st: &mut State, pre_bind: &State, born_mark: u64) {
        let gen_yields = (!gen_cm).then(|| self.gens.yield_states.get(&gid).map(|v| v.states().map(|s| s.copy()).collect::<Vec<State>>()).unwrap_or_default());
        // (a generator that ran to its end: the state there, for a consumer that uses it up)
        let finite_end = if gen_yields.is_some() && st.reachable { Some(st.copy()) } else { None };
        // a generator whose body never ends (`while True: yield …`): the caller resumes from
        // one of its yields, and the yields go on (their number is unknown)
        if !st.reachable {
            let joined = self.gens.yield_states.remove(&gid).and_then(|v| {
                let mut it = v.list.into_iter().map(|(_, s)| s);
                let mut acc = it.next()?;
                for mut s in it {
                    acc = acc.join(&mut s);
                }
                Some(acc)
            });
            if let Some(mut ys) = joined {
                let mut p = self.heap.prefix(gid, &ys).unwrap_or(0);
                // (the yields joined: every position some yield made is there — the items
                // the endless loop is known to give first)
                while ys.has_slot(gid, Ns::Pos, intern::key_int(p as i64)) {
                    p += 1;
                }
                ys.set_lens(gid, (None, p, 0));
                *st = ys;
            }
        }
        if let Some(ys) = gen_yields {
            self.gen_suspension_join(st, pre_bind, &ys, gid, born_mark);
            // the effects per item: before the first, the objects are as before the call
            if st.reachable {
                let fp = st.footprint(pre_bind);
                let old = |o: ObjId| self.heap.born_before(o, born_mark) && o != gid;
                let rows: Vec<(ObjId, Ns)> = fp.rows.iter().copied().filter(|(o, _)| old(*o)).collect();
                let slots: Vec<(ObjId, SlotKey)> = fp.slots.iter().copied().filter(|(o, _)| old(*o)).collect();
                let elems: Vec<ObjId> = fp.elems.iter().copied().filter(|o| old(*o)).collect();
                let mut lens: Vec<ObjId> = slots.iter().map(|(o, _)| *o).chain(elems.iter().copied()).collect();
                lens.sort_unstable();
                lens.dedup();
                if !rows.is_empty() || !slots.is_empty() || !elems.is_empty() {
                    let at = self.gens.pos_states.remove(&gid).unwrap_or_default();
                    let ge = GenEffects { rows, slots, elems, lens, pre: pre_bind.copy(), at, end: finite_end, join: st.copy(), taken: Default::default() };
                    gen_copy_facts(st, pre_bind, &ge);
                    st.set_slot(gid, Ns::Attr, key_str_lit("\u{1}effpos"), Value::lit(key_str_lit("pre")), true);
                    self.gens.eff.insert(gid, Rc::new(ge));
                }
            }
        }
    }
}
