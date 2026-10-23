//! Generators: a generator's body runs whole at its first consumption (`force_gen`), in the
//! creating call's context; a consumer of item *k* sees the effects up to yield *k*
//! (`GenEffects`); code a consumer runs between items is watched (`with_gen_restart`). A
//! `@contextmanager`'s `with` body runs at each yield (`CmHook`).
//!
//! - `iter()` of a generator gives it back and runs nothing. One run on some of the paths
//!   joined here only runs again on the others, its effects joined in; one made anew is younger
//!   than every mark (`Heap::renew`).
//! - A generator expression (`FuncNode::GenExp`) is a generator function with a scope of its
//!   own (its free names are cells) of `iter()` of its first iterable, taken where it is
//!   written; its run is no call site, and its items are positioned as a comprehension's. One
//!   whose items are all taken before other code runs (`Scopes::eager_genexps`: `sum(…)`,
//!   `[*g]`, `f(*g)`, a function's `xs = (…)` its next statement gives whole to one, out of a
//!   `try`; alone given to a library function that takes the items at once, or to unknown
//!   code) runs where it is written, as a comprehension. One given to a call that does not run,
//!   or returned by an entry run, runs there.
//! - `GenEffects` are on objects older than the call and on those it made and changes after a
//!   yield; at a position not known, or one object for several runs, some yield's; joined in
//!   where the consumer changed them too.
//! - Code between items (`note_resume`; unknown code too: a key it made is there unless
//!   removed, and its deletes) that changes what the generator reads after a yield (`Gens::watch`,
//!   also after the call that started it, while held, or while a global holds it) makes it run
//!   again with that change at each yield, and the changes reachable from it, through classes
//!   too (`with_gen_restart`). A `for`'s list is read at each step (`ForWatch::reads`).
//! - A loop or comprehension over a generator with effects, closing facts or raises moves its
//!   cursor per item. Closing is `genclose.rs`, raises `genraise.rs`, consumers that stop early
//!   `stopearly.rs`.
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

    /// One more state at a yield (the first under `key` is what `get_mut` finds).
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

/// What a generator's body does to objects older than its call (and to those it made and
/// changes after a yield), by how far it has run: the
/// facts of those objects before it ran (`pre`), at its k-th yield (`at[k]`), at its end,
/// and joined over all of them.
pub struct GenEffects {
    pub rows: Vec<(ObjId, Ns)>,
    pub slots: Vec<(ObjId, SlotKey)>,
    pub elems: Vec<ObjId>,
    pub lens: Vec<ObjId>,
    /// the objects among them its body made (changed between its suspension points): where a
    /// state does not have them (not made yet), the consumer's facts stay
    pub made: FxHashSet<ObjId>,
    pub pre: State,
    pub at: Vec<Option<State>>,
    pub end: Option<State>,
    pub join: State,
    /// joined over the yields and the end, when each yield has its state in `at`: after an
    /// item was taken, at a position not known
    pub taken: std::cell::OnceCell<Option<State>>,
    /// the facts after it is closed at its k-th yield (GeneratorExit raised there, `genclose.rs`),
    /// and joined over the yields (None: closing it changes nothing)
    pub closed_at: Vec<Option<State>>,
    pub closed: Option<State>,
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
    /// where the consumer's states since the run start in its generator's resume segments
    /// (`Gens::resumes`), and what it had changed when it asked for a later item of the facts
    /// of the objects the generator changes itself
    pub seg: usize,
    pub own_hits: Vec<Hit>,
}

thread_local! {
    static LIVE_KEYS: std::cell::Cell<Option<[intern::KeyId; 2]>> = const { std::cell::Cell::new(None) };
}

/// The keys of the containers a live iterator reads and of the function a `map` calls
/// (`\u{1}src`, `\u{1}fn`: `Analyzer::sync_live`), interned once per thread.
pub fn live_keys() -> [intern::KeyId; 2] {
    LIVE_KEYS.with(|c| {
        c.get().unwrap_or_else(|| {
            let k = [key_str_lit("\u{1}src"), key_str_lit("\u{1}fn")];
            c.set(Some(k));
            k
        })
    })
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
    pub files: Vec<u32>,
}

/// The generators made and run so far, and the `@contextmanager` bodies running.
#[derive(Default)]
pub struct Gens {
    /// per watched generator, the consumer's states where it asked for a later item
    /// (`note_resume`), joined per segment: a watch starts one, and its run sees those from its
    /// own on (a generator run again and again — a merged one, forced at each consumer — has a
    /// watch per run: each consumer state is joined once, not once per watch)
    pub resumes: FxHashMap<ObjId, Vec<Option<State>>>,
    /// `__next__` functions whose calls were not decided once (`unroll_next`)
    pub no_unroll_next: FxHashSet<ObjId>,
    /// generators made but not run yet: the call that made each (`force_gen`)
    pub lazy: FxHashMap<ObjId, std::rc::Rc<LazyGen>>,
    /// generator expressions a call that does not run is taking the items of
    /// (`force_given_genexps`): one built over itself (`g = (… for r in g)`) not again
    pub forcing: Vec<ObjId>,
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
    /// consumers that stop early running a generator made for them alone (`stopearly.rs`)
    pub stop_hooks: Vec<super::stopearly::StopHook>,
    /// `@contextmanager` generators whose with body is running: (their frame depth, their env
    /// at the yield, the env stack's length then, their possibly-unbound names then), so a try
    /// in the generator around the yield sees its own names when the body raises
    pub cm_frames: Vec<(usize, IndexMap<StrId, Value>, usize, std::sync::Arc<Vec<StrId>>)>,
    /// generators stored into a module's global (watched after the call that started them)
    pub global_held: FxHashSet<ObjId>,
    /// the generator runs whole in progress: where GeneratorExit raised at their yields lands
    /// (`genclose.rs`)
    pub bounds: Vec<super::genclose::GenBound>,
    /// generators with closing facts asked for items, and the frame depth where they last were
    pub closable: FxHashMap<ObjId, usize>,
    /// the loops running over a generator made for them alone: (their loop level, their frame
    /// depth, the generator)
    pub fresh_loops: Vec<(usize, usize, ObjId)>,
    /// what a generator's suspended frame holds of generators with closing facts
    pub holds: FxHashMap<ObjId, Value>,
    /// generators run whole whose body surely raises before its end, and the running tries then
    /// (`Analyzer::try_mark`)
    pub raising: FxHashMap<ObjId, (usize, u64)>,
    /// the raises of generators run whole for the running `for` loops, taken where they ask
    /// for items (`genraise.rs`)
    pub raises: Vec<super::genraise::GenRaises>,
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
        // (a generator expression allocates as the code it is written in: its frame's context)
        let genexp = self.heap.func_info(foid).node.is_genexp();
        let ctx = if genexp { self.frame().ctx.clone() } else { self.receiver_ctx(&self_v, &site) };
        let label = if genexp { "generator expression".to_string() } else { format!("generator {}", self.heap.func_info(foid).qualname) };
        let gid = self.heap.alloc_raw(Kind::List, &format!("gen:{}", foid), &ctx, true, &label, Payload::None);
        // (the one made here before, suspended with closing facts, is this object no more: it
        // may be dropped now, as when its name is bound again — `genclose.rs`)
        if self.may_close(gid, state) {
            self.close_gen(gid, state, super::genclose::Close::Maybe);
        }
        self.start_iterator(gid, state);
        let mut self_v = self_v;
        let pending = self.gens.lazy.get(&gid).filter(|_| lazy_state(state, gid).0).cloned();
        // (a new generator, not one made here and not run yet: younger than every mark, as a
        // strong allocation is)
        if pending.is_none() && self.heap.obj(gid).single {
            self.heap.renew(gid);
        }
        if let Some(old) = pending {
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
        let lg = super::LazyGen { foid, self_v, args, kwargs, node: *node, site, ctx, chain: self.cx.call_chain.clone(), files: self.cx.call_files.clone() };
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
            let saved_files = std::mem::replace(&mut self.cx.call_files, lg.files.clone());
            let pre = if between { Some((state.copy(), self.budget.snapshot())) } else { None };
            self.gens.last_reads = None;
            let r = self.call_user(lg.foid, lg.self_v.clone(), Args::new(&lg.args, &lg.kwargs, &[], &[]), state, &lg.node, false);
            self.cx.call_chain = saved;
            self.cx.call_files = saved_files;
            if let (Some((pre, budget)), true) = (pre, state.reachable) {
                let fp = state.footprint(&pre);
                let mut own: FxHashSet<ObjId> = fp.rows.iter().map(|k| k.0).chain(fp.slots.iter().map(|k| k.0)).chain(fp.elems.iter().copied()).chain(state.lens.diff_keys(&pre.lens)).collect();
                // (and those whose facts follow its items, installed as a consumer takes them)
                if let Some(ge) = self.gens.eff.get(&o) {
                    own.extend(ge.rows.iter().map(|k| k.0).chain(ge.slots.iter().map(|k| k.0)).chain(ge.elems.iter().copied()).chain(ge.lens.iter().copied()));
                }
                // (what it reads after a yield: through what it was given, its globals, its cells)
                let reads = self.gens.last_reads.take().unwrap_or_default();
                let segs = self.gens.resumes.entry(o).or_default();
                segs.push(None);
                let w = super::GenWatch { gid: o, own, after: state.copy(), reads, budget, born: self.heap.serial(), seg: segs.len() - 1, own_hits: Vec::new() };
                self.gens.watch.push(w);
            }
            self.gen_body_not_run(o, &r, &lg.node, state);
            if let Some(mut b) = before {
                let j = state.join(&mut b);
                state.replace_with(j);
                // (how far it has run differs by path: not known, its effects any of its steps'
                // joined in — the paths it ran on keep what their consumer changed since)
                if let Some(ge) = self.gens.eff.get(&o).cloned() {
                    gen_join_facts(state, &ge.join, &ge, &ge.pre);
                    state.set_slot(o, Ns::Attr, key_str_lit("\u{1}effpos"), Value::data("effpos"), true);
                }
                self.set_cursor(o, None, state);
            }
            state.slots.remove(&o, &(Ns::Attr, key_str_lit("\u{1}lazyargs")));
            state.set_slot(o, Ns::Attr, key_str_lit("\u{1}lazy"), Value::lit(intern::key_int(0)), true);
        }
    }

    /// Generator `o`'s run gave `r`: when that is not `o`, its body did not run (past its budget,
    /// cut at the deadline): unknown code gives any number of unknown items, and returns
    /// something unknown.
    pub(super) fn gen_body_not_run(&mut self, o: ObjId, r: &Value, node: &Node, state: &mut State) {
        if r.pts.contains(o) || !state.reachable {
            return;
        }
        let it = self.iterate(r, node, state);
        let e = self.elem_value(&it.elem, node, "gen");
        self.add_elem(o, &e, state);
        self.heap.set_llen(o, state, None);
        state.drop_obj_ns(o, Ns::Pos);
        state.set_slot(o, Ns::Attr, key_str_lit("\u{1}genret"), Value::data("generator result"), true);
    }

    /// The generator expressions among a call's arguments run, as for a library function: a
    /// call that does not run takes their items as unknown code does.
    pub(super) fn force_given_genexps(&mut self, a: &Args, state: &mut State) {
        for v in a.pos.iter().chain(a.kw.values()).chain(a.star.iter()) {
            let genexps = v.pts.filter(|o| !self.gens.forcing.contains(&o) && self.gens.lazy.get(&o).is_some_and(|lg| self.heap.func_info(lg.foid).node.is_genexp()));
            if !genexps.is_empty() {
                let depth = self.gens.forcing.len();
                self.gens.forcing.extend(genexps.iter());
                self.force_gen(&Value::objs(genexps), state, true);
                self.gens.forcing.truncate(depth);
            }
        }
    }

    /// The heap a generator's caller sees (see the call): the touched objects older than
    /// `mark` (but the generator's own item list) joined over `pre`, the yield states and `st`.
    pub(super) fn gen_suspension_join(&mut self, st: &mut State, pre: &State, yields: &[State], gid: ObjId, mark: u64, made: &Footprint) {
        if !st.reachable {
            return;
        }
        let ys: Vec<&State> = yields.iter().filter(|y| y.reachable).collect();
        let mut fp = gen_footprint(st, pre, &ys);
        let old = |o: ObjId| self.heap.born_before(o, mark) && o != gid;
        fp.rows.retain(|k| old(k.0));
        fp.slots.retain(|k| old(k.0));
        fp.elems.retain(|o| old(*o));
        // (and the facts of what it made that change between its suspension points: joined
        // over those where the object is there)
        fp.rows.extend(made.rows.iter().copied());
        fp.slots.extend(made.slots.iter().copied());
        fp.elems.extend(made.elems.iter().copied());
        // Each fact of `st` is joined with `pre`'s and every yield state's. Yield states are
        // copies of one another changed by the body, sharing structure: each is diffed against
        // the one before it (the first against `pre`), and only the keys the diff names are
        // joined. A fact a state does not change equals the previous state's, joined already
        // (a generator can hold thousands of yield states, and joining every touched fact with
        // each was facts × yields).
        let entry = |_: &Analyzer, s: &State, o: ObjId| -> (Option<usize>, usize, u16) { s.lens.get(&o).unwrap_or((None, 0, 0)) };
        // (with the distinct rows already joined: most yield states share their rows)
        // (an object the run made: joined over the states it is there in — None: in none yet)
        let there = |s: &State, o: ObjId| old(o) || made_there(s, o);
        let in_end: FxHashSet<ObjId> = fp.rows.iter().map(|k| k.0).chain(fp.slots.iter().map(|k| k.0)).chain(fp.elems.iter().copied()).filter(|o| there(st, *o)).collect();
        let mut rows: rustc_hash::FxHashMap<(ObjId, Ns), (Option<Row>, rustc_hash::FxHashSet<Row>)> = rustc_hash::FxHashMap::default();
        for (o, ns) in fp.rows.iter().copied() {
            let a = st.row(o, ns);
            let b = if old(o) { pre.row(o, ns) } else { a.clone() };
            let j = in_end.contains(&o).then(|| a.join(&b));
            rows.insert((o, ns), (j, [a, b].into_iter().collect()));
        }
        let mut slots: rustc_hash::FxHashMap<(ObjId, SlotKey), Option<Value>> = rustc_hash::FxHashMap::default();
        let mut touched: Vec<ObjId> = Vec::new();
        let end = st.copy();
        let at = |o: ObjId| if old(o) { pre } else { &end };
        for (o, sk) in fp.slots.iter().copied() {
            touched.push(o);
            slots.insert((o, sk), in_end.contains(&o).then(|| st.slots.get(&o, &sk).unwrap_or_default().join(&at(o).slots.get(&o, &sk).unwrap_or_default())));
        }
        let mut elems: rustc_hash::FxHashMap<ObjId, Option<KeyVal>> = rustc_hash::FxHashMap::default();
        for o in fp.elems.iter().copied() {
            touched.push(o);
            elems.insert(o, in_end.contains(&o).then(|| kv_join(&st.elems.get(&o).unwrap_or(DATA), &at(o).elems.get(&o).unwrap_or(DATA))));
        }
        touched.sort_unstable();
        touched.dedup();
        let mut lens: rustc_hash::FxHashMap<ObjId, Option<(Option<usize>, usize, u16)>> = touched.iter().map(|o| (*o, in_end.contains(o).then(|| join_len(&entry(self, st, *o), &entry(self, at(*o), *o))))).collect();
        let mut prev = pre;
        for y in ys {
            let base = std::mem::replace(&mut prev, y);
            for k in y.rows.diff_keys(&base.rows) {
                if let (Some((r, seen)), true) = (rows.get_mut(&k), y.rows.get(&k).is_some()) {
                    let yr = y.row(k.0, k.1);
                    if r.is_none() || !seen.contains(&yr) {
                        join_opt(r, yr.clone(), |a, b| a.join(b));
                        seen.insert(yr);
                    }
                }
            }
            for (o, sk) in y.slots.diff_keys(&base.slots) {
                if let (Some(v), true) = (slots.get_mut(&(o, sk)), there(y, o)) {
                    join_opt(v, y.slots.get(&o, &sk).unwrap_or_default(), |a, b| a.join(b));
                }
            }
            for o in y.elems.diff_keys(&base.elems) {
                if let (Some(kv), true) = (elems.get_mut(&o), there(y, o)) {
                    join_opt(kv, y.elems.get(&o).unwrap_or(DATA), kv_join);
                }
            }
            for o in y.lens.diff_keys(&base.lens) {
                if let (Some(l), true) = (lens.get_mut(&o), there(y, o)) {
                    join_opt(l, entry(self, y, o), join_len);
                }
            }
        }
        let rows = rows.into_iter().filter_map(|(k, (r, _))| Some((k, r?))).collect();
        let slots = slots.into_iter().filter_map(|(k, v)| Some((k, v?))).collect();
        let elems = elems.into_iter().filter_map(|(k, v)| Some((k, v?))).collect();
        let lens = lens.into_iter().filter_map(|(k, v)| Some((k, v?))).collect();
        set_facts(st, rows, slots, elems, lens);
    }

    /// Run `body` from `state`. A generator it ran whole for a consumer that runs code between
    /// the items (`Gens::watch`) did not see what that code changed: when the code changed objects
    /// the generator reaches, `body` runs again, those objects changed for the generator at each
    /// of its yields (`gen_interference`, `Gens::havoc`; at most three rounds, then everything it reaches). `close`:
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
            self.prune_resumes();
            // (modules it imported stay imported: their facts go on)
            let mods: Vec<Value> = self.program.modules.values().skip(mmark).map(|m| Value::obj(*m)).collect();
            let mut again = pre.copy();
            if !mods.is_empty() {
                let fp = state.footprint(&pre);
                let mut targets: Vec<ObjId> = fp.rows.iter().map(|k| k.0).chain(fp.slots.iter().map(|k| k.0)).chain(fp.elems.iter().copied()).collect();
                targets.sort_unstable();
                targets.dedup();
                let reach = self.reach_among(&mods, &targets, state, false);
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
        // (an object `b` has no facts of is garbage there: nothing changed it since)
        for &(o, ns) in rows.iter().filter(|k| made_there(b, k.0)) {
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
        for &(o, sk) in slots.iter().filter(|k| made_there(b, k.0) || b.slots.has_key(&k.0)) {
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
        changed.extend(others.filter(|o| made_there(b, *o) && differs(o)).map(|o| (o, None, false)).collect::<Vec<_>>());
        changed
    }

    /// The generators watched since `wmark` are used up or out of reach: no longer watched, but
    /// those that may give more items and that what the code returns or what it was given
    /// (`pre`'s names) hold (a generator started in a call and returned from it, or kept in an
    /// attribute of its receiver for the next call).
    fn close_watches(&mut self, wmark: usize, pre: &State, state: &State) {
        let at = if state.reachable { state } else { state.exits.as_deref().unwrap_or(state) };
        let held = self.gens.bounds.last().filter(|b| b.depth == self.cx.frames.len());
        let mut gids: Vec<ObjId> = self.gens.watch[wmark.min(self.gens.watch.len())..].iter().map(|w| w.gid).filter(|g| self.may_resume(*g, at) || held.is_some_and(|b| b.holds.contains(g))).collect();
        gids.sort_unstable();
        gids.dedup();
        let kept = if gids.is_empty() {
            FxHashSet::default()
        } else {
            let roots: Vec<Value> = state.ret.iter().chain(pre.env.values()).cloned().collect();
            let mut kept = self.reach_among(&roots, &gids, at, false);
            // (a generator a global holds is resumed by later calls; one the frame of a
            // generator run whole holds, as that one is resumed: `genclose.rs`)
            let held = self.gens.bounds.last().filter(|b| b.depth == self.cx.frames.len());
            kept.extend(gids.iter().filter(|g| self.gens.global_held.contains(g) || held.is_some_and(|b| b.holds.contains(g))));
            kept
        };
        let tail = self.gens.watch.split_off(wmark.min(self.gens.watch.len()));
        self.gens.watch.extend(tail.into_iter().filter(|w| kept.contains(&w.gid)));
        self.prune_resumes();
    }

    /// The resume segments of generators no longer watched go.
    fn prune_resumes(&mut self) {
        let watched: FxHashSet<ObjId> = self.gens.watch.iter().map(|w| w.gid).collect();
        self.gens.resumes.retain(|g, _| watched.contains(g));
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
        let watches = wmark..self.gens.watch.len();
        let suffixes = self.resumed_suffixes(watches.clone());
        watches
            .map(|i| {
                let w = &self.gens.watch[i];
                (w.gid, self.gen_interference_with(i, fin, all, suffixes.get(&(w.gid, w.seg)).and_then(|r| r.as_ref())))
            })
            .filter(|(_, h)| !h.is_empty())
            .collect()
    }

    /// For watches `among`, by (generator, resume segment), the consumer's states from that
    /// segment on, joined (one pass from the last segment per generator).
    fn resumed_suffixes(&self, among: impl Iterator<Item = usize>) -> FxHashMap<(ObjId, usize), Option<State>> {
        let mut from: FxHashMap<ObjId, usize> = FxHashMap::default();
        for i in among {
            let w = &self.gens.watch[i];
            let f = from.entry(w.gid).or_insert(w.seg);
            *f = (*f).min(w.seg);
        }
        let mut out: FxHashMap<(ObjId, usize), Option<State>> = FxHashMap::default();
        for (gid, first) in from {
            let segs = self.gens.resumes.get(&gid).map(|v| v.as_slice()).unwrap_or_default();
            let mut acc: Option<State> = None;
            for (i, s) in segs.iter().enumerate().skip(first).rev() {
                if let Some(s) = s {
                    acc = Some(match acc {
                        Some(mut a) => a.join(&mut s.copy()),
                        None => s.copy(),
                    });
                }
                out.insert((gid, i), acc.as_ref().map(|a| a.copy()));
            }
        }
        out
    }

    /// `gen_interference` for one watched generator (`resumed`: the consumer's states since its
    /// run): the keys / objects it reads after a yield that later code changed by `fin` or by the
    /// time it asked for an item (unknown code too; of the objects it changes itself, what
    /// changed of the facts it gave).
    fn gen_interference_with(&self, i: usize, fin: &State, all: bool, resumed: Option<&State>) -> Vec<Hit> {
        let w = &self.gens.watch[i];
        if !fin.reachable {
            return Vec::new();
        }
        // (what the consumer changed by the time it asked for an item, or by the end)
        let joined;
        let fin = match resumed {
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
        let (mut hit, unread): (Vec<Hit>, Vec<Hit>) = changed.into_iter().filter(|c| c.0 != w.gid && older(c.0) && !w.own.contains(&c.0)).partition(read);
        hit.extend(own);
        // (what changed that the run again may read through what changed for it: the generator
        // reads on where a changed value leads, so it is changed for it now, not a round later)
        if !hit.is_empty() && !unread.is_empty() {
            let roots: Vec<Value> = hit.iter().map(|(o, k, _)| match k {
                Some(sk) => fin.slots.get(o, sk).unwrap_or_default(),
                None => Value::obj(*o),
            }).collect();
            let mut targets: Vec<ObjId> = unread.iter().map(|c| c.0).collect();
            targets.sort_unstable();
            targets.dedup();
            let reach = self.reach_among(&roots, &targets, fin, true);
            hit.extend(unread.into_iter().filter(|c| reach.contains(&c.0)));
        }
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
        let suffixes = self.resumed_suffixes((0..self.gens.watch.len()).filter(|i| used.contains(self.gens.watch[*i].gid)));
        while i < self.gens.watch.len() {
            let (gid, seg) = (self.gens.watch[i].gid, self.gens.watch[i].seg);
            if !used.contains(gid) {
                i += 1;
                continue;
            }
            if self.gen_interference_with(i, fin, false, suffixes.get(&(gid, seg)).and_then(|r| r.as_ref())).is_empty() {
                self.gens.watch.remove(i);
            } else {
                i += 1;
            }
        }
        self.prune_resumes();
    }

    /// Which of `targets` are reachable from `roots` in `st` (the walk ends when all are found;
    /// modules and functions end a path, classes too unless `classes`: their attributes).
    pub(super) fn reach_among(&self, roots: &[Value], targets: &[ObjId], st: &State, classes: bool) -> FxHashSet<ObjId> {
        let work: Vec<ObjId> = roots.iter().flat_map(|v| v.pts.iter()).collect();
        let mut edges = |o: ObjId, work: &mut Vec<ObjId>| match self.heap.kind(o) {
            Kind::Module | Kind::Function => {}
            Kind::Class if !classes => {}
            Kind::Class => {
                self.obj_edges(o, st, work);
                work.extend(self.heap.class_info_opt(o).map(|ci| ci.bases.clone()).unwrap_or_default());
            }
            _ => self.obj_edges(o, st, work),
        };
        self.find_among(work, targets, usize::MAX, true, &mut edges).unwrap_or_default()
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
        self.gens.cm_frames.push((self.cx.frames.len() + 1, callee_env.clone(), self.cx.env_stack.len(), state.unbound.clone()));
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
        let mut alts_before = if fresh { 0 } else { state.lens.get(&gid).filter(|l| l.0.is_none()).map(|l| l.2).unwrap_or(0) };
        state.set_slot(gid, Ns::Elem, STAR, v.clone(), false);
        let cur = state.elems.get(&gid).unwrap_or_else(|| v.kv.clone());
        state.elems.insert(gid, kv_join(&cur, &v.kv));
        // straight-line yields keep their order; in a branch too (a length per path, joined
        // to the shorter known prefix, like appends)
        let in_branch = self.cx.cond_depth > 0;
        // (a generator expression's positioned as a comprehension's: its object is one generator)
        let positional = (self.frame().single || self.heap.func_info(foid).node.is_genexp()) && self.frame().loop_depth == 0 && self.heap.obj(gid).single;
        let ll = if fresh { Some(0) } else { self.heap.llen(gid, state) };
        // (the position of this yield, for the state GeneratorExit raised here leaves)
        let mut pos: Option<usize> = None;
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
                pos = self.gens.pos_states.get(&gid).and_then(|ps| ps[n].as_ref()).map(|_| n);
            }
            // (the yields so far keep their positions: a known prefix)
            (_, Some(n)) if n > 0 => {
                self.heap.set_prefix(gid, state, n);
            }
            // one of a few lengths, every position known: the item at each one's end (as the
            // appends of `list.append` do)
            (true, None) if alts_before != 0 && (0..15 - alts_before.leading_zeros() as usize).all(|p| state.has_slot(gid, Ns::Pos, key_int(p as i64))) => {
                for n in (0..16).filter(|i| alts_before & (1 << i) != 0) {
                    state.set_slot(gid, Ns::Pos, key_int(n as i64), v.clone(), false);
                }
                // (the lengths, one more each, below)
                let p = self.heap.prefix(gid, state).unwrap_or(0);
                self.heap.set_prefix(gid, state, p);
            }
            // (else the lengths it may have are not known: positions are read under the
            // shortest one only, and this item may be at any of them)
            _ if self.heap.prefix(gid, state).is_some() => {
                alts_before = 0;
                let p = self.heap.prefix(gid, state).unwrap_or(0);
                self.heap.set_prefix(gid, state, p);
            }
            _ => {
                alts_before = 0;
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
        self.record_yield_state(gid, site, state);
        self.consumer_between(gid, state);
        self.close_point(pos, state);
        Some(gid)
    }

    /// The state at a yield of generator `gid`'s running activation (`site`: the suspension
    /// point with the unrolled loop positions), for its caller (`gen_suspension_join`).
    fn record_yield_state(&mut self, gid: ObjId, site: (usize, usize), state: &State) {
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
    }

    /// A lazy iterator consumed (to the end, or partly: `full` false): what it takes from its
    /// sources.
    pub fn consume_lazy(&mut self, v: &Value, full: bool, state: &mut State) {
        for lo in v.pts.iter() {
            let Some(srcs) = self.m.lazy_src.get(&lo).cloned() else { continue };
            // (consumed once on a path: a one-shot iterator itself)
            // (false from where it is made: consumed on some of the paths joined here only,
            // where its sources stand is not known)
            let done = key_str_lit("\u{1}consumed");
            let was = state.slot_opt(lo, Ns::Attr, done).map(|v| kv_literals(&v.kv));
            if was.as_ref().is_some_and(|ks| !ks.contains(intern::key_bool(false))) {
                continue;
            }
            let some = was.is_some_and(|ks| ks.contains(intern::key_bool(true)));
            state.set_slot(lo, Ns::Attr, done, Value::lit(intern::key_bool(true)), true);
            for (src, n) in srcs {
                // (used up: so are the sources it takes all of, on the paths it was used up on
                // before too)
                if let (true, None) = (full, n) {
                    self.exhaust(&Value::obj(src), state);
                    continue;
                }
                match (full && !some, n, self.cursor_of(src, state)) {
                    (true, Some(n), Some(c)) => match self.heap.llen(src, state) {
                        // (asked for more than it has: used up)
                        Some(len) if c + n > len => self.set_cursor_ex(src, Some(len), true, state),
                        // (just its last item: a generator may be suspended there or ended)
                        Some(len) if c + n == len && self.gens.eff.contains_key(&src) => self.set_cursor(src, None, state),
                        _ => self.set_cursor(src, Some(c + n), state),
                    },
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

    /// `set_cursor`; `at_end`: used up (its length may not be known). A generator whose
    /// cursor stands after its last item is suspended at its last yield until asked for more.
    pub(in crate::interp) fn set_cursor_ex(&mut self, o: u32, c: Option<usize>, at_end: bool, state: &mut State) {
        self.note_resume(o, state);
        // (asked for the items up to there: its raises there, `genraise.rs`)
        if !self.gens.raises.is_empty() {
            let from = self.cursor_of(o, state);
            self.replay_gen_raises(o, from, c.filter(|_| !at_end), at_end, state);
        }
        let cv = match c {
            Some(n) => Value::lit(key_int(n as i64)),
            None if at_end => Value::data("cursor-end"),
            // (one with closing facts that may not have started: that kept, `may_be_unstarted`)
            None if self.closes_on_drop(o) && self.may_be_unstarted(o, state) => Value::lit(key_int(0)).join(&Value::data("cursor")),
            None => Value::data("cursor"),
        };
        state.set_slot(o, Ns::Attr, key_str_lit("__cursor"), cv, true);
        // (where its frame is suspended, it holds what its names do: `genclose.rs`)
        self.note_closable(o);
        self.note_ended(o, at_end, state);
        self.set_holds(o, at_end || c == Some(0), state);
        let Some(ge) = self.gens.eff.get(&o).cloned() else { return };
        let tk = key_str_lit("\u{1}effpos");
        let Some(expected) = effects_given(state, o, &ge) else { return };
        let equal = gen_facts_equal(state, expected, &ge);
        let ended = at_end;
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
            gen_join_facts(state, target, &ge, expected);
            state.set_slot(o, Ns::Attr, tk, Value::data("effpos"), true);
            return;
        }
        gen_copy_facts(state, target, &ge);
        state.set_slot(o, Ns::Attr, tk, Value::lit(key_str_lit(&ntag)), true);
    }

    /// An item of generator `o` was taken at a position not known: its effects are those of
    /// some yield or of its end, not those before its first item.
    pub fn gen_took_item(&mut self, o: ObjId, state: &mut State) {
        if !self.gens.raises.is_empty() {
            let from = self.cursor_of(o, state);
            self.replay_gen_raises(o, from, None, false, state);
        }
        self.note_closable(o);
        self.note_ended(o, false, state);
        self.set_holds(o, false, state);
        let Some(ge) = self.gens.eff.get(&o).cloned() else { return };
        let Some(expected) = effects_given(state, o, &ge) else { return };
        if !gen_facts_equal(state, expected, &ge) {
            gen_join_facts(state, &ge.join, &ge, expected);
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
    /// then, the generator sees after its yield (`gen_interference`).
    pub(in crate::interp) fn note_resume(&mut self, o: ObjId, state: &State) {
        if !state.reachable || !self.gens.watch.iter().any(|w| w.gid == o) {
            return;
        }
        // (the objects it changes itself: what the consumer changed of the facts it was given)
        let ge = self.gens.eff.get(&o).cloned();
        let own = ge.as_ref().and_then(|ge| effects_given(state, o, ge).map(|given| self.changes_in(given, state, &ge.rows, &ge.slots, ge.elems.iter().chain(&ge.lens).copied()))).unwrap_or_default();
        if let Some(Some(last)) = self.gens.resumes.get_mut(&o).and_then(|segs| segs.last_mut()) {
            last.replace_with(last.copy().join(&mut state.copy()));
        } else if let Some(last) = self.gens.resumes.get_mut(&o).and_then(|segs| segs.last_mut()) {
            *last = Some(state.copy());
        }
        for w in self.gens.watch.iter_mut().filter(|w| w.gid == o) {
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

    /// `o` is a one-shot iterator at its start on this path: its cursor 0 (not left unset, so
    /// a join with a path that took items from it leaves where it stands not known).
    pub fn start_iterator(&mut self, o: ObjId, state: &mut State) {
        self.heap.obj_mut(o).iterator = true;
        state.set_slot(o, Ns::Attr, key_str_lit("__cursor"), Value::lit(key_int(0)), true);
        // (a new one: not closed, not used up — `genclose.rs`)
        self.reset_closing(o, state);
    }

    /// One-shot iterator `o` is used up: it gives nothing more, whatever its containers hold
    /// (a live one's `\u{1}done`).
    pub fn end_iterator(&mut self, o: ObjId, state: &mut State) {
        match self.heap.llen(o, state) {
            Some(n) => self.set_cursor_ex(o, Some(n), true, state),
            None => self.set_cursor_ex(o, None, true, state),
        }
        if self.m.live_iters.contains_key(&o) {
            state.set_slot(o, Ns::Attr, key_str_lit("\u{1}done"), Value::lit(intern::key_bool(true)), true);
        }
    }

    /// Live iterator `o` (`Tables::live_iters`) reads `srcs` as they are at each step: what
    /// they hold now (`live_marks`), and how many items it had (`\u{1}n0`: a reversed list's
    /// first index + 1; a dict's or set's size, which may not change while it runs).
    pub fn watch_live(&mut self, o: ObjId, srcs: &Value, state: &mut State) {
        state.set_slot(o, Ns::Attr, key_str_lit("\u{1}src"), srcs.clone(), true);
        state.set_slot(o, Ns::Attr, key_str_lit("\u{1}done"), Value::lit(intern::key_bool(false)), true);
        let n0 = self.heap.llen(o, state).map_or_else(|| Value::data("n0"), |n| Value::lit(key_int(n as i64)));
        state.set_slot(o, Ns::Attr, key_str_lit("\u{1}n0"), n0, true);
        let (all, keys) = self.live_marks(srcs, state);
        state.set_slot(o, Ns::Attr, key_str_lit("\u{1}srcfp"), all, true);
        state.set_slot(o, Ns::Attr, key_str_lit("\u{1}keysfp"), keys, true);
    }

    /// Is `o` a container code may change while an iterator reads it (a list, dict, set, view)?
    pub fn changing_container(&self, o: ObjId) -> bool {
        match self.heap.kind(o) {
            Kind::List => !self.heap.obj(o).iterator,
            Kind::Dict => !crate::models::pandas::is_series_obj(self, o),
            Kind::Set | Kind::View => true,
            _ => false,
        }
    }

    /// What `srcs` hold on this path, as two hashes: everything, and their keys (all but a
    /// dict's values; a view's dict's).
    fn live_marks(&self, srcs: &Value, state: &State) -> (Value, Value) {
        use std::hash::{Hash, Hasher};
        let mut objs: Vec<ObjId> = srcs.pts.iter().collect();
        for s in srcs.pts.iter() {
            if let Payload::View(_, r) = &self.heap.obj(s).payload {
                objs.extend(r.pts.iter());
            }
        }
        let (mut all, mut keys) = (rustc_hash::FxHasher::default(), rustc_hash::FxHasher::default());
        for o in objs {
            let head = (o, state.lens.get(&o), state.elems.get(&o), state.rows.get(&(o, Ns::Item)));
            head.hash(&mut all);
            head.hash(&mut keys);
            let dict = self.heap.kind(o) == Kind::Dict;
            for (sk, v) in state.slots_of(o) {
                (sk, &v).hash(&mut all);
                if !(dict && sk.0 == Ns::Item) {
                    (sk, &v).hash(&mut keys);
                }
            }
        }
        (Value::lit(key_int(all.finish() as i64)), Value::lit(key_int(keys.finish() as i64)))
    }

    /// The live iterators among `v` (`Tables::live_iters`) are about to give items: those whose
    /// containers changed since they took their items take them again (from where they stand);
    /// a dict's or set's whose keys changed raises RuntimeError (surely when `v` is it alone and
    /// its size surely differs), its items not known. Not `consume`: `v` is given to a call,
    /// which may not take items (those are left to where items are taken).
    pub fn sync_live(&mut self, v: &Value, state: &mut State, consume: bool) {
        if self.m.live_iters.is_empty() {
            return;
        }
        for o in v.pts.iter() {
            let Some(call) = self.m.live_iters.get(&o).cloned() else { continue };
            let Some(srcs) = state.slot_opt(o, Ns::Attr, key_str_lit("\u{1}src")) else { continue };
            if !state.reachable || self.at_end(o, state) {
                continue;
            }
            let (all, keys) = self.live_marks(&srcs, state);
            if state.slot_opt(o, Ns::Attr, key_str_lit("\u{1}srcfp")).as_ref() == Some(&all) {
                continue;
            }
            // (a dict's, set's or deque's iterator: its size may not change)
            let keyed = srcs.pts.iter().any(|s| matches!(self.heap.kind(s), Kind::Dict | Kind::Set | Kind::View) || self.m.deques.contains_key(&s));
            let keys_changed = keyed && state.slot_opt(o, Ns::Attr, key_str_lit("\u{1}keysfp")).as_ref() != Some(&keys);
            if keys_changed && !consume {
                continue;
            }
            let old = (state.slot(o, Ns::Elem, STAR), state.elems.get(&o));
            crate::models::builtins::refill_live(self, state, o, &call, &srcs);
            state.set_slot(o, Ns::Attr, key_str_lit("\u{1}srcfp"), all, true);
            state.set_slot(o, Ns::Attr, key_str_lit("\u{1}keysfp"), keys, true);
            if !keys_changed {
                continue;
            }
            let n0 = state.slot_opt(o, Ns::Attr, key_str_lit("\u{1}n0")).and_then(|v| v.kv.as_lit()).and_then(intern::key_as_int);
            let now = self.heap.llen(o, state).map(|n| n as i64);
            if keys_changed && v.single() == Some(o) && v.kv.is_bot() && n0.is_some() && now.is_some() && n0 != now {
                self.raise_here("RuntimeError", state);
                return;
            }
            if keys_changed {
                self.may_raise_here("RuntimeError", state);
            }
            state.drop_obj_ns(o, Ns::Pos);
            self.heap.set_llen(o, state, None);
            let e = state.slot(o, Ns::Elem, STAR).join(&old.0);
            state.set_slot(o, Ns::Elem, STAR, e, true);
            if let Some(kv) = old.1 {
                let j = kv_join(&state.elems.get(&o).unwrap_or(DATA), &kv);
                state.elems.insert(o, j);
            }
        }
    }

    /// Is a one-shot iterator used up on this path (its length not known; a live one, at all)?
    pub fn at_end(&self, o: ObjId, state: &State) -> bool {
        self.heap.obj(o).iterator && (state.slot_opt(o, Ns::Attr, key_str_lit("__cursor")) == Some(Value::data("cursor-end")) || state.slot_opt(o, Ns::Attr, key_str_lit("\u{1}done")) == Some(Value::lit(intern::key_bool(true))))
    }

    /// The items of one-shot iterator `o` from where it stands, of `items` all it gives from
    /// its start (None: where it stands not known); an object that is no iterator: all.
    pub fn items_from_cursor<T>(&self, o: ObjId, items: Vec<T>, state: &State) -> Option<Vec<T>> {
        if !self.heap.obj(o).iterator {
            return Some(items);
        }
        match self.cursor_of(o, state) {
            Some(c) => Some(items.into_iter().skip(c).collect()),
            None if self.at_end(o, state) => Some(Vec::new()),
            None => None,
        }
    }

    /// A list's or tuple's positions still to come, from where a one-shot iterator stands:
    /// (the first, how many); None: not known.
    pub fn positions_left(&self, o: ObjId, state: &State) -> Option<(usize, usize)> {
        if !matches!(self.heap.kind(o), Kind::List | Kind::Tuple) {
            return None;
        }
        let n = self.heap.llen(o, state);
        if !self.heap.obj(o).iterator {
            return n.map(|n| (0, n));
        }
        match self.cursor_of(o, state) {
            Some(c) => n.map(|n| (c.min(n), n.saturating_sub(c))),
            None if self.at_end(o, state) => Some((0, 0)),
            None => None,
        }
    }

    /// Where a one-shot iterator of known length may stand on the paths joined here (each one's
    /// cursor: a number, or at its end), None where some path's is not known.
    pub fn cursor_alternatives(&self, o: ObjId, state: &State) -> Option<Vec<usize>> {
        let n = self.heap.llen(o, state)?;
        let Some(v) = state.slot_opt(o, Ns::Attr, key_str_lit("__cursor")) else { return Some(vec![0]) };
        if !v.pts.is_empty() {
            return None;
        }
        let (mut out, lits) = match &v.kv {
            KeyVal::Data(l, h) if &*intern::str_get(*l) == "cursor-end" && !h.contains(STAR) => (vec![n], h.clone()),
            kv if kv.is_literal() => (Vec::new(), kv_literals(kv)),
            _ => return None,
        };
        for k in lits.iter() {
            out.push(intern::key_as_int(k).filter(|c| *c >= 0)?.min(n as i64) as usize);
        }
        out.sort_unstable();
        out.dedup();
        Some(out)
    }

    /// How many items a one-shot iterator has given (None: not known).
    pub fn cursor_of(&self, o: u32, state: &State) -> Option<usize> {
        match state.slot_opt(o, Ns::Attr, key_str_lit("__cursor")) {
            None => Some(0),
            Some(v) => v.kv.as_lit().filter(|_| v.pts.is_empty()).and_then(intern::key_as_int).map(|i| i.max(0) as usize),
        }
    }

    /// `v` is used up at once by a call that runs no code between its items (`list(gen)`,
    /// `sum`): as `exhaust`; a generator whose body surely raises before its end, run where
    /// the tries running now recorded its raises, raises there (nothing goes on after it).
    pub fn exhaust_whole(&mut self, v: &Value, state: &mut State) {
        if v.single().is_some_and(|o| self.gens.raising.get(&o) == Some(&self.try_mark())) {
            self.consume_lazy(v, true, state);
            state.reachable = false;
            return;
        }
        self.exhaust(v, state);
    }

    /// `v` is used up (`list(gen)`, a `for` to its end): a lazy generator runs, an iterator's
    /// cursor goes to its end, a generator's effects are all given.
    pub fn exhaust(&mut self, v: &Value, state: &mut State) {
        self.consume_lazy(v, true, state);
        if let Some(o) = v.single() {
            if self.heap.obj(o).iterator && self.heap.obj(o).single {
                // (used up, its length not known: at its end all the same; a live one stays used
                // up whatever its containers gain)
                self.end_iterator(o, state);
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
pub(super) fn effects_given<'a>(state: &State, o: ObjId, ge: &'a crate::interp::GenEffects) -> Option<&'a State> {
    let tag = state.slot_opt(o, Ns::Attr, key_str_lit("\u{1}effpos")).and_then(|v| v.kv.as_lit().filter(|_| v.pts.is_empty())).and_then(intern::key_as_str).map(|s| s.to_string());
    match tag.as_deref() {
        Some("pre") => Some(&ge.pre),
        Some("join") => Some(&ge.join),
        Some("taken") => ge.taken(),
        Some("end") => ge.end.as_ref(),
        Some("closed") => ge.closed.as_ref(),
        Some(t) if t.starts_with("closed") => t["closed".len()..].parse::<usize>().ok().and_then(|k| ge.closed_at.get(k)).and_then(|x| x.as_ref()),
        Some(t) => t.strip_prefix("at").and_then(|k| k.parse::<usize>().ok()).and_then(|k| ge.at.get(k)).and_then(|x| x.as_ref()),
        None => None,
    }
}

/// (not run on some path, run on some path) for a generator `make_lazy_gen` made
/// A fact joined with what a state has, or that, where nothing was joined yet.
fn join_opt<T>(acc: &mut Option<T>, v: T, j: impl Fn(&T, &T) -> T) {
    *acc = Some(match acc.take() {
        Some(a) => j(&a, &v),
        None => v,
    });
}

/// Set these facts in `st` (elements changed: the last one not known).
fn set_facts(st: &mut State, rows: Vec<((ObjId, Ns), Row)>, slots: Vec<((ObjId, SlotKey), Value)>, elems: Vec<(ObjId, KeyVal)>, lens: Vec<(ObjId, (Option<usize>, usize, u16))>) {
    for ((o, ns), r) in rows {
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

/// `pre` with the facts of what the generator made (`ge.made`) as `st` has them.
fn made_as_after(pre: &State, st: &State, ge: &GenEffects) -> State {
    let mut out = pre.copy();
    for k in ge.rows.iter().filter(|k| ge.made.contains(&k.0)) {
        if let Some(r) = st.rows.get(k) {
            out.rows.insert(*k, r);
        }
    }
    for (o, sk) in ge.slots.iter().filter(|k| ge.made.contains(&k.0)) {
        if let Some(v) = st.slots.get(o, sk) {
            out.slots.set(o, *sk, v);
        }
    }
    for o in ge.elems.iter().chain(ge.lens.iter()).filter(|o| ge.made.contains(o)) {
        if let Some(v) = st.elems.get(o) {
            out.elems.insert(*o, v);
        }
        if let Some(l) = st.lens.get(o) {
            out.set_lens(*o, l);
        }
    }
    out
}

/// Has `s` facts of object `o` (one a generator's run made: made by then)?
fn made_there(s: &State, o: ObjId) -> bool {
    s.lens.get(&o).is_some() || s.elems.get(&o).is_some() || s.rows.get(&(o, Ns::Attr)).is_some() || s.rows.get(&(o, Ns::Item)).is_some()
}

/// The facts of objects the run made (`made`) that change from one of the states `ss` (in
/// order) to the next where the object is there in both (a key added to it too), sorted.
fn changed_made(ss: &[&State], made: &dyn Fn(ObjId) -> bool) -> Footprint {
    let there = made_there;
    let mut out = Footprint::default();
    for w in ss.windows(2) {
        let (a, b) = (w[0], w[1]);
        let changed = |o: ObjId| made(o) && there(a, o);
        out.rows.extend(b.rows.diff_keys(&a.rows).into_iter().filter(|k| changed(k.0)));
        out.slots.extend(b.slots.diff_keys(&a.slots).into_iter().filter(|k| changed(k.0)));
        out.elems.extend(b.elems.diff_keys(&a.elems).into_iter().filter(|o| changed(*o)));
    }
    out.rows.sort_unstable();
    out.rows.dedup();
    out.slots.sort_unstable();
    out.slots.dedup();
    out.elems.sort_unstable();
    out.elems.dedup();
    out
}

/// What a generator's run changed of the facts it had at the call (`pre`): at its end (`st`) or
/// at any of its yields (`ys`, in order: each diffed against the one before), sorted.
fn gen_footprint(st: &State, pre: &State, ys: &[&State]) -> Footprint {
    let mut fp = st.footprint(pre);
    let mut prev = pre;
    for y in ys {
        fp.rows.extend(y.rows.diff_keys(&prev.rows));
        fp.slots.extend(y.slots.diff_keys(&prev.slots));
        fp.elems.extend(y.elems.diff_keys(&prev.elems));
        prev = y;
    }
    fp.rows.sort_unstable();
    fp.rows.dedup();
    fp.slots.sort_unstable();
    fp.slots.dedup();
    fp.elems.sort_unstable();
    fp.elems.dedup();
    fp
}

/// Whether generator `o` may still be lazy (not run) on this path, and whether it may have run
/// (its `\u{1}lazy` slot: 1 lazy, 0 run; both where paths that differ were joined).
pub(super) fn lazy_state(state: &State, o: ObjId) -> (bool, bool) {
    match state.slot_opt(o, Ns::Attr, key_str_lit("\u{1}lazy")) {
        Some(v) => {
            let ks = kv_literals(&v.kv);
            (ks.contains(intern::key_int(1)), ks.contains(intern::key_int(0)))
        }
        None => (false, false),
    }
}

/// Do two states agree on the facts a generator's effects concern?
pub(super) fn gen_facts_equal(a: &State, b: &State, ge: &crate::interp::GenEffects) -> bool {
    ge.rows.iter().all(|k| a.rows.get(k) == b.rows.get(k))
        && ge.slots.iter().all(|(o, sk)| a.slots.get(o, sk) == b.slots.get(o, sk))
        && ge.elems.iter().all(|o| a.elems.get(o) == b.elems.get(o))
        && ge.lens.iter().all(|o| a.lens.get(o) == b.lens.get(o))
}

/// Join `src`'s facts about a generator's affected objects into `dst`'s: those the generator
/// changed by then from the facts the consumer was given (`pre`: `effects_given`, else those
/// before its first item); the others are the consumer's as they are.
pub(super) fn gen_join_facts(dst: &mut State, src: &State, ge: &crate::interp::GenEffects, pre: &State) {
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
    // (an object it made, not made yet in `src`: the consumer cannot reach it, its facts stay)
    let made_yet = |o: &ObjId| !ge.made.contains(o) || made_there(src, *o);
    for k in ge.rows.iter().filter(|k| made_yet(&k.0)) {
        match src.rows.get(k) {
            Some(r) => dst.rows.insert(*k, r),
            None => {
                dst.rows.remove(k);
            }
        }
    }
    for (o, sk) in ge.slots.iter().filter(|k| made_yet(&k.0)) {
        match src.slots.get(o, sk) {
            Some(v) => dst.slots.set(o, *sk, v),
            None => {
                dst.slots.remove(o, sk);
            }
        }
    }
    for o in ge.elems.iter().filter(|o| made_yet(o)) {
        match src.elems.get(o) {
            Some(v) => dst.elems.insert(*o, v),
            None => {
                dst.elems.remove(o);
            }
        }
    }
    // (elements changed: the last one is not known; the lengths are `src`'s, with their
    // alternatives)
    for (o, sk) in &ge.slots {
        if matches!(sk.0, Ns::Pos | Ns::Elem) {
            dst.clear_last(*o);
        }
    }
    for o in ge.lens.iter().filter(|o| made_yet(o)) {
        match src.lens.get(o) {
            Some(v) => dst.set_lens(*o, v),
            None => {
                dst.forget_len(o);
            }
        }
    }

}

impl Analyzer {
    /// The facts of what generator `gid`'s run made (since `mark`) that change between its
    /// suspension points (`ys`, then its end `st`), of the objects its items reach (what its
    /// consumer sees); not of iterators it made (their state is their cursor's, `genclose.rs`).
    fn made_changed(&self, ys: &[State], st: &State, gid: ObjId, mark: u64) -> Footprint {
        let mut ss: Vec<&State> = ys.iter().filter(|y| y.reachable).collect();
        ss.extend(st.reachable.then_some(st));
        let born = |o: ObjId| !self.heap.born_before(o, mark) && o != gid && !self.heap.obj(o).iterator;
        let mut made = changed_made(&ss, &born);
        if made.rows.is_empty() && made.slots.is_empty() && made.elems.is_empty() {
            return made;
        }
        let mut targets: Vec<ObjId> = made.rows.iter().map(|k| k.0).chain(made.slots.iter().map(|k| k.0)).chain(made.elems.iter().copied()).collect();
        targets.sort_unstable();
        targets.dedup();
        let items: Vec<Value> = ss.iter().flat_map(|y| y.slots_of(gid).into_iter().map(|(_, v)| v)).collect();
        let seen = self.reach_among(&items, &targets, st, false);
        made.rows.retain(|k| seen.contains(&k.0));
        made.slots.retain(|k| seen.contains(&k.0));
        made.elems.retain(|o| seen.contains(o));
        made
    }

    /// A generator activation begins (its frame pushed): its object of yielded values, new;
    /// what the activation's site held before (a running activation's yields, put back after).
    pub(super) fn gen_activation_begin(&mut self, ygid: ObjId, foid: ObjId, fi: &super::FuncInfo, ctx: &str, state: &mut State) -> Option<YieldStates> {
        // (a new activation: not consumed yet, its own item states; a running one's
        // yields are put back after it)
        // (keyed as `yield_value` finds it: the site's object, not a variant `alloc` made)
        let saved = self.gens.yield_states.remove(&ygid);
        // the yielded values of this activation (a generator expression's: none yet, those of
        // an earlier activation at this site are not this one's)
        let gid = match saved {
            None if fi.node.is_genexp() => {
                state.drop_obj_ns(ygid, Ns::Pos);
                state.drop_obj_ns(ygid, Ns::Elem);
                ygid
            }
            _ => self.alloc(Kind::List, &format!("gen:{}", foid), state, &format!("generator {}", fi.qualname), Payload::None, None, Ns::Item, None, Some(ctx)),
        };
        self.heap.set_llen(gid, state, Some(0));
        state.elems.insert(gid, KeyVal::Lits(KeySet::new()));
        state.slots.remove(&gid, &(Ns::Attr, key_str_lit("__cursor")));
        state.slots.remove(&gid, &(Ns::Attr, key_str_lit("\u{1}effpos")));
        self.gens.pos_states.remove(&gid);
        self.gens.pos_conflict.retain(|(g, _)| *g != gid);
        self.gens.eff.remove(&gid);
        saved
    }

    /// A generator activation ended: what its site held before it (`gen_activation_begin`), back.
    pub(super) fn gen_yields_restore(&mut self, saved: Option<(ObjId, Option<YieldStates>)>) {
        match saved {
            Some((gid, Some(v))) => {
                self.gens.yield_states.insert(gid, v);
            }
            Some((gid, None)) => {
                self.gens.yield_states.remove(&gid);
            }
            None => {}
        }
    }

    /// A generator activation's body ran (to `st`): the generator's body runs at the call,
    /// eagerly, but its caller sees the heap only where the body is suspended: before the first
    /// item, at a yield, or at its end. The objects that existed before the call and that the
    /// body changed hold the join of those states (`gen_suspension_join`), and its effects per
    /// item are kept (`GenEffects`).
    pub(super) fn gen_activation_end(&mut self, gid: ObjId, gen_cm: bool, st: &mut State, pre_bind: &State, born_mark: u64, closes: Option<super::genclose::GenBound>) {
        let gen_yields = (!gen_cm).then(|| self.gens.yield_states.get(&gid).map(|v| v.states().map(|s| s.copy()).collect::<Vec<State>>()).unwrap_or_default());
        // (a generator that ran to its end: the state there, for a consumer that uses it up)
        let finite_end = if gen_yields.is_some() && st.reachable { Some(st.copy()) } else { None };
        // (a run whole that ends nowhere and has no endless loop raises before its end: using it
        // up raises there, where the tries that recorded its raises run — `exhaust`)
        match (&closes, st.reachable) {
            (Some(b), false) if !b.endless && !self.budget.hard_cut && !self.budget.lib_abort.get() => {
                let m = self.try_mark();
                self.gens.raising.insert(gid, m);
            }
            _ => {
                self.gens.raising.remove(&gid);
            }
        }
        // a generator whose body never ends (`while True: yield …`): the caller resumes from
        // one of its yields, and the yields go on (their number is unknown)
        if !st.reachable {
            // (one that raises before its end, every yield after as many items: that many)
            let raises = self.gens.raising.contains_key(&gid);
            let lens: Vec<Option<usize>> = self.gens.yield_states.get(&gid).into_iter().flat_map(|v| v.states()).filter(|s| s.reachable).map(|s| self.heap.llen(gid, s)).collect();
            let exact = lens.first().copied().flatten().filter(|n| raises && lens.iter().all(|m| *m == Some(*n)));
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
                match exact {
                    Some(n) => self.heap.set_llen(gid, &mut ys, Some(n)),
                    None => ys.set_lens(gid, (None, p, 0)),
                }
                *st = ys;
            }
        }
        let (closed_at, closed, marked_only) = self.closed_states(gid, closes, finite_end.as_ref());
        if let Some(ys) = gen_yields {
            let made = self.made_changed(&ys, st, gid, born_mark);
            self.gen_suspension_join(st, pre_bind, &ys, gid, born_mark, &made);
            // the effects per item: before the first, the objects are as before the call
            // (those of an item undone by the end too)
            if st.reachable {
                let mut ys: Vec<&State> = ys.iter().filter(|y| y.reachable).collect();
                // (and what closing it changes)
                ys.extend(closed.iter());
                let fp = gen_footprint(st, pre_bind, &ys);
                let old = |o: ObjId| self.heap.born_before(o, born_mark) && o != gid;
                // (of what it made, one runtime object each: a merged one keeps its facts joined
                // over the suspension points at every item)
                let one = |o: &ObjId| self.heap.obj(*o).single;
                let rows: Vec<(ObjId, Ns)> = fp.rows.iter().copied().filter(|(o, _)| old(*o)).chain(made.rows.iter().copied().filter(|k| one(&k.0))).collect();
                let slots: Vec<(ObjId, SlotKey)> = fp.slots.iter().copied().filter(|(o, _)| old(*o)).chain(made.slots.iter().copied().filter(|k| one(&k.0))).collect();
                let elems: Vec<ObjId> = fp.elems.iter().copied().filter(|o| old(*o)).chain(made.elems.iter().copied().filter(one)).collect();
                let made: FxHashSet<ObjId> = made.rows.iter().map(|k| k.0).chain(made.slots.iter().map(|k| k.0)).chain(made.elems.iter().copied()).filter(one).collect();
                let mut lens: Vec<ObjId> = slots.iter().map(|(o, _)| *o).chain(elems.iter().copied()).collect();
                lens.sort_unstable();
                lens.dedup();
                if !rows.is_empty() || !slots.is_empty() || !elems.is_empty() {
                    let at = self.gens.pos_states.remove(&gid).unwrap_or_default();
                    let mut ge = GenEffects { rows, slots, elems, lens, made, pre: State::new(), at, end: finite_end, join: st.copy(), taken: Default::default(), closed_at, closed };
                    // (before its first item, what it made is as after the run: joined over its
                    // suspension points)
                    ge.pre = made_as_after(pre_bind, st, &ge);
                    // (closing it at each of its yields changes nothing there: no closing facts)
                    let same = |p: usize, c: &State| ge.at.get(p).and_then(|a| a.as_ref()).is_some_and(|a| gen_facts_equal(c, a, &ge));
                    if marked_only && ge.closed_at.iter().enumerate().all(|(p, c)| c.as_ref().is_none_or(|c| same(p, c))) {
                        ge.closed = None;
                        ge.closed_at.clear();
                    }
                    gen_copy_facts(st, &ge.pre, &ge);
                    st.set_slot(gid, Ns::Attr, key_str_lit("\u{1}effpos"), Value::lit(key_str_lit("pre")), true);
                    self.gens.eff.insert(gid, Rc::new(ge));
                }
            }
        }
    }
}
