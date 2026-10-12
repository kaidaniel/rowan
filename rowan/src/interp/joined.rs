//! Joined summaries: what a call of a user function does once the function has used up its
//! budget of per-call-site runs (`Analyzer::tab_budget`).
//!
//! The function's body is run once on the *join* of the inputs of its calls so far: the
//! arguments bound to each parameter, and every fact of the state the run read (the read
//! journal of the persistent maps, `layered::journal_open`), each joined over the calls'
//! states. A call whose arguments and read facts are below those replays the run: the facts
//! the run wrote (the write journal) take the run's values, and the run's result, verdicts,
//! requirements and raises are the call's. A call that brings more joins it into
//! the inputs and runs the body again. The facts a run writes are inputs too (a path that
//! does not write one keeps the caller's value). By monotonicity the run on the larger inputs
//! ends above where a run on the call's own inputs would, at every fact: a replay puts the
//! run's values of the written facts in place of the caller's. There are no placeholders:
//! models see ordinary values.
//!
//! Objects the runs make are named by the function alone (`j<function>`): several calls'
//! results may be one object, which becomes a merged object (no strong store into it) when a
//! replay meets it live. Inside a run an earlier object of a site counts as live whenever the
//! state has facts about it (no reachability walk: a walk's result is not a journaled input).

use std::rc::Rc;

use indexmap::IndexMap;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::domain::{chain_extend, kv_join, KeyVal, Ns, ObjId, ReqInfo, Row, Value};
use crate::frontend::Node;
use crate::intern::{KeyId, SourceId, StrId};
use crate::layered::{self, Pair, T_ELEMS, T_LENS, T_REQS, T_ROWS, T_SLOTS, WHOLE};
use crate::state::{join_len, SlotKey, State};

use super::{Analyzer, ExcNames, Payload, Verdict};
use crate::state::Kind;

pub type FactKey = (u8, u64);
type Len = (Option<usize>, usize, u16);

/// Runs of one summary before its inputs that keep growing are widened (as at a loop head:
/// a counter, a list appended to by every call).
const WIDEN_AFTER: usize = 2;

/// Summaries kept per function (variants for calls that do not match each other).
const MAX_VARIANTS: usize = 4;

/// Runs per call site and callee before the callee's joined summary answers there: the number
/// of per-call-site runs is at most this many times the program's call sites (an unrolled
/// loop's iterations counting as sites of their own).
pub const SITE_RUNS: usize = 2;

/// One fact of a state, as a summary's input.
#[derive(Clone, PartialEq, Debug)]
pub enum Fact {
    Row(Option<Row>),
    Slot(Option<Value>),
    /// all of an object's slots (a listing of them was read)
    Slots(Vec<(SlotKey, Value)>),
    Elem(Option<KeyVal>),
    Len(Option<Len>),
}

/// What a run on the joined inputs did (see the module comment).
pub struct JSum {
    /// parameter -> the join of the arguments bound to it
    pub args: IndexMap<StrId, Value>,
    /// the closure cells of the function object the runs were of (another closure of the same
    /// `def` has its own, matched by name)
    pub cells: IndexMap<StrId, ObjId>,
    /// the facts the runs read, joined over the calls' states
    pub input: FxHashMap<FactKey, Fact>,
    /// the facts the runs wrote; their values in `exit`
    pub writes: Vec<FactKey>,
    pub exit: State,
    /// the run's entry state (inputs installed): an object whose slots are still its subtree
    /// there needs no comparison of them
    pub entry: State,
    /// the states where the body may raise to its caller, joined
    pub raise_state: Option<State>,
    /// the exceptions it may raise (objects of known classes), joined
    pub raise_vals: Value,
    pub ret: Value,
    pub raises: bool,
    pub raise_classes: Option<Vec<ExcNames>>,
    /// some path returns
    pub completes: bool,
    /// verdicts, row checks (golden harness) and tensor-shape notes of the run, chains
    /// relative to the call
    pub verdicts: Vec<Verdict>,
    pub row_checks: Vec<super::RowCheck>,
    pub shape_notes: Vec<(bool, crate::models::torch::ShapeNote)>,
    /// objects the runs made
    pub born: FxHashSet<ObjId>,
    /// runs so far
    pub runs: usize,
}

/// A run in progress: its previous round's summary (what a recursive call inside it uses),
/// whether an inner call used it, whether it used another run's (then its own summary is not
/// kept), and what the inner calls brought (renamed onto the run's objects).
pub struct JsFrame {
    pub code: usize,
    pub partial: Option<Rc<JSum>>,
    pub hit: bool,
    pub provisional: bool,
    pub args: IndexMap<StrId, Value>,
    pub input: FxHashMap<FactKey, Fact>,
}

/// The most rounds a recursive function's summary iterates before it is widened.
const MAX_ROUNDS: usize = 4;

/// A run on joined inputs: the bindings of the parameters and the allocation context.
pub struct JsRun {
    pub bound: IndexMap<StrId, Value>,
    pub ctx: String,
}

fn row_key(h: u64) -> (ObjId, Ns) {
    ((h >> 3) as ObjId, Ns::from_index(h & 7))
}

fn slot_key(h: u64) -> (ObjId, SlotKey) {
    let lo = h as u32;
    ((h >> 32) as ObjId, (Ns::from_index((lo >> 29) as u64), KeyId(lo & ((1 << 29) - 1))))
}

pub fn fact_of(st: &State, k: FactKey) -> Fact {
    let (tag, h) = k;
    match tag {
        T_ROWS => Fact::Row(st.rows.get_h(h).map(|(_, v)| v.clone())),
        T_SLOTS if h as u32 == WHOLE => Fact::Slots(st.slots.whole_h(h >> 32).into_iter().map(|(Pair(_, ik), v)| (ik, v)).collect()),
        T_SLOTS => Fact::Slot(st.slots.get_h(h).map(|(_, v)| v.clone())),
        T_ELEMS => Fact::Elem(st.elems.get_h(h).map(|(_, v)| v.clone())),
        _ => Fact::Len(st.lens.get_h(h).map(|(_, v)| *v)),
    }
}

fn opt_leq<T: PartialEq>(a: &Option<T>, b: &Option<T>, join: impl Fn(&T, &T) -> T) -> bool {
    match (a, b) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(x), Some(y)) => x == y || join(x, y) == *y,
    }
}

fn opt_join<T: Clone + PartialEq>(a: &Option<T>, b: &Option<T>, join: impl Fn(&T, &T) -> T) -> Option<T> {
    match (a, b) {
        (None, x) | (x, None) => x.clone(),
        (Some(x), Some(y)) => Some(if x == y { x.clone() } else { join(x, y) }),
    }
}

/// `a` ⊑ `b`: a run that read `b` covers a state holding `a` (a missing fact is below any).
pub fn fact_leq(a: &Fact, b: &Fact) -> bool {
    match (a, b) {
        (Fact::Row(x), Fact::Row(y)) => opt_leq(x, y, |p, q| p.join(q)),
        (Fact::Slot(x), Fact::Slot(y)) => opt_leq(x, y, |p, q| p.join(q)),
        (Fact::Elem(x), Fact::Elem(y)) => opt_leq(x, y, kv_join),
        (Fact::Len(x), Fact::Len(y)) => opt_leq(x, y, join_len),
        (Fact::Slots(xs), Fact::Slots(ys)) => xs.iter().all(|(k, v)| ys.iter().any(|(k2, w)| k2 == k && (v == w || v.join(w) == *w))),
        _ => false,
    }
}

pub fn fact_join(a: &Fact, b: &Fact) -> Fact {
    match (a, b) {
        (Fact::Row(x), Fact::Row(y)) => Fact::Row(opt_join(x, y, |p, q| p.join(q))),
        (Fact::Slot(x), Fact::Slot(y)) => Fact::Slot(opt_join(x, y, |p, q| p.join(q))),
        (Fact::Elem(x), Fact::Elem(y)) => Fact::Elem(opt_join(x, y, kv_join)),
        (Fact::Len(x), Fact::Len(y)) => Fact::Len(opt_join(x, y, join_len)),
        (Fact::Slots(xs), Fact::Slots(ys)) => {
            let mut out = xs.clone();
            for (k, w) in ys {
                match out.iter_mut().find(|(k2, _)| k2 == k) {
                    Some((_, v)) => *v = v.join(w),
                    None => out.push((*k, w.clone())),
                }
            }
            Fact::Slots(out)
        }
        _ => b.clone(),
    }
}

thread_local! {
    pub static JS_STATS: std::cell::RefCell<FxHashMap<usize, (String, usize, usize, usize)>> = std::cell::RefCell::new(FxHashMap::default());
}

/// A renaming of a summary's objects (those its arguments and cells reach) to a call's.
pub type Sigma = FxHashMap<ObjId, ObjId>;

fn key_owner((tag, h): FactKey) -> ObjId {
    match tag {
        T_ROWS => (h >> 3) as ObjId,
        T_SLOTS | T_REQS => (h >> 32) as ObjId,
        _ => h as ObjId,
    }
}

fn rename_key(k: FactKey, sg: &Sigma) -> FactKey {
    let (tag, h) = k;
    let Some(&n) = sg.get(&key_owner(k)).filter(|_| tag != T_REQS) else { return k };
    let n = n as u64;
    match tag {
        T_ROWS => (tag, (n << 3) | (h & 7)),
        T_SLOTS => (tag, (n << 32) | (h & 0xffff_ffff)),
        _ => (tag, n),
    }
}

fn rename_value(v: &Value, sg: &Sigma) -> Value {
    if sg.is_empty() {
        return v.clone();
    }
    let hit = v.pts.iter().any(|o| sg.contains_key(&o)) || matches!(&v.kv, KeyVal::KeysOf(ko) if ko.objs.iter().any(|o| sg.contains_key(&o)));
    if !hit {
        return v.clone();
    }
    let map = |o: ObjId| *sg.get(&o).unwrap_or(&o);
    let kv = match &v.kv {
        KeyVal::KeysOf(ko) => KeyVal::KeysOf(crate::domain::KeysOf { objs: ko.objs.map(|o| map(o)), ..ko.clone() }),
        other => other.clone(),
    };
    Value::with_kv(v.pts.map(|o| map(o)), kv)
}

fn rename_fact(f: &Fact, sg: &Sigma) -> Fact {
    if sg.is_empty() {
        return f.clone();
    }
    match f {
        Fact::Slot(Some(v)) => Fact::Slot(Some(rename_value(v, sg))),
        Fact::Slots(vs) => Fact::Slots(vs.iter().map(|(k, v)| (*k, rename_value(v, sg))).collect()),
        Fact::Elem(Some(kv)) => Fact::Elem(Some(rename_value(&Value::kv_only(kv.clone()), sg).kv)),
        other => other.clone(),
    }
}

/// A state holding only `keys`' facts of `st`.
fn restrict(st: &State, keys: &[FactKey]) -> State {
    layered::journal_paused(|| {
        let mut out = State::new();
        for &(tag, h) in keys {
            match tag {
                T_ROWS => {
                    if let Some((k, v)) = st.rows.get_h(h) {
                        out.rows.insert(*k, v.clone());
                    }
                }
                T_SLOTS => {
                    if let Some((Pair(o, sk), v)) = st.slots.get_h(h) {
                        out.slots.set(o, *sk, v.clone());
                    }
                }
                T_ELEMS => {
                    if let Some((k, v)) = st.elems.get_h(h) {
                        out.elems.insert(*k, v.clone());
                    }
                }
                T_LENS => {
                    if let Some((k, v)) = st.lens.get_h(h) {
                        out.lens.insert(*k, *v);
                    }
                }
                T_REQS => {
                    if let Some((Pair(src, k), v)) = st.reqs.get_h(h) {
                        out.reqs.set(src, *k, v.clone());
                    }
                }
                _ => {}
            }
        }
        out
    })
}

/// A value that keeps growing, widened: its literals unknown (shapes keep their own joins).
fn widen_value(v: &Value) -> Value {
    match &v.kv {
        KeyVal::Data(d) if crate::intern::str_get(*d).starts_with('\u{1}') => v.clone(),
        KeyVal::Bot => v.clone(),
        _ => Value::with_kv(v.pts.clone(), crate::domain::DATA),
    }
}

/// `new` (a join with `old`), widened where it differs from `old`.
fn fact_widen(new: &Fact, old: &Fact) -> Fact {
    if new == old {
        return new.clone();
    }
    match new {
        Fact::Slot(Some(v)) => Fact::Slot(Some(widen_value(v))),
        Fact::Slots(vs) => Fact::Slots(vs.iter().map(|(k, v)| (*k, widen_value(v))).collect()),
        Fact::Elem(Some(KeyVal::Data(d))) if crate::intern::str_get(*d).starts_with('\u{1}') => new.clone(),
        Fact::Elem(Some(_)) => Fact::Elem(Some(crate::domain::DATA)),
        Fact::Len(Some(_)) => Fact::Len(Some((None, 0, 0))),
        _ => new.clone(),
    }
}

/// `st[k] := st[k] ⊔ f`.
fn install(st: &mut State, k: FactKey, f: &Fact) {
    let (_, h) = k;
    match f {
        Fact::Row(Some(r)) => {
            let nr = match st.rows.get_h(h) {
                Some((_, cur)) if cur == r => return,
                Some((_, cur)) => cur.join(r),
                None => r.clone(),
            };
            st.rows.insert(row_key(h), nr);
        }
        Fact::Slot(Some(v)) => install_slot(st, slot_key(h), v),
        Fact::Slots(vs) => {
            for (sk, v) in vs {
                install_slot(st, ((h >> 32) as ObjId, *sk), v);
            }
        }
        Fact::Elem(Some(kv)) => {
            let o = h as ObjId;
            let n = match st.elems.get_h(h) {
                Some((_, cur)) if cur == kv => return,
                Some((_, cur)) => kv_join(cur, kv),
                None => kv.clone(),
            };
            st.elems.insert(o, n);
        }
        Fact::Len(Some(l)) => {
            let o = h as ObjId;
            let n = match st.lens.get_h(h) {
                Some((_, cur)) if cur == l => return,
                Some((_, cur)) => join_len(cur, l),
                None => *l,
            };
            st.lens.insert(o, n);
        }
        _ => {}
    }
}

fn install_slot(st: &mut State, (o, sk): (ObjId, SlotKey), v: &Value) {
    let h = Pair(o, sk);
    let n = match st.slots.get_h(crate::layered::PKey::key64(&h)) {
        Some((_, cur)) if cur == v => return,
        Some((_, cur)) => cur.join(v),
        None => v.clone(),
    };
    st.slots.set(&o, sk, n);
}

impl Analyzer {
    /// Inside a run on joined inputs (no call cache, no reachability walks).
    pub fn js_active(&self) -> bool {
        !self.js_frames.is_empty()
    }

    /// May this call site run `foid`'s body for itself (`SITE_RUNS`, `ROWAN_LIMITS=tab=N`)?
    pub fn site_budget(&mut self, foid: ObjId, node: &Node) -> bool {
        if self.over_deadline() {
            return false;
        }
        let mut site = if node.addr != 0 { node.addr } else { ((node.module as usize) << 32) | node.line as usize };
        // (each iteration of an unrolled loop is straight-line code: its calls sites of their own)
        for i in &self.frame().unroll {
            site = site.wrapping_mul(31).wrapping_add(*i + 1);
        }
        let n = self.site_runs.entry((site, self.func_info(foid).node.addr())).or_default();
        if *n >= super::limit("tab", SITE_RUNS) {
            return false;
        }
        *n += 1;
        true
    }

    /// A call of `foid` past its budget (see the module comment).
    #[allow(clippy::too_many_arguments)]
    pub fn joined_call(&mut self, foid: ObjId, self_v: Option<Value>, args: Vec<Value>, kwargs: IndexMap<StrId, Value>, star_args: Vec<Value>, star_kwargs: Vec<Value>, state: &mut State, node: &Node) -> Value {
        let fi = self.func_info(foid);
        let code = fi.node.addr();
        let bound = self.compute_bindings(&fi, &self_v, &args, &kwargs, &star_args, &star_kwargs, state, node);
        if !state.reachable {
            return Value::new();
        }
        let call = self.js_call_ctx(&self_v, node);
        let variants = self.jsums.get(&code).cloned().unwrap_or_default();
        if std::env::var("ROWAN_JSDBG").is_ok() {
            let qn = fi.qualname.clone();
            for js in &variants {
                JS_STATS.with(|m| { m.borrow_mut().entry(js.args.keys().map(|k| k.0 as usize).sum::<usize>()).or_insert((String::new(), 0, 0, 0)).0 = qn.clone(); });
            }
        }
        for js in &variants {
            if let (sg, true) = self.js_match(js, &bound, &fi.closure, state) {
                return self.js_replay(js, sg, state, node, &call);
            }
        }
        // a recursive call inside the function's own run: the previous round's summary, what
        // this call brings (renamed onto the run's objects) folded into the next round's inputs
        if let Some(i) = self.js_frames.iter().rposition(|f| f.code == code) {
            for f in self.js_frames.iter_mut().skip(i + 1) {
                f.provisional = true;
            }
            self.js_frames[i].hit = true;
            let Some(p) = self.js_frames[i].partial.clone() else {
                // (the first round: no summary yet, the call's result is the bottom one)
                state.reachable = false;
                return Value::new();
            };
            let (sg, _) = self.js_match(&p, &bound, &fi.closure, state);
            let inv: Sigma = sg.iter().map(|(t, a)| (*a, *t)).collect();
            let mut pend_input: Vec<(FactKey, Fact)> = Vec::new();
            // (not inputs of this round: folded into the next one's under the run's objects)
            for k in p.input.keys() {
                let kk = rename_key(*k, &sg);
                pend_input.push((*k, rename_fact(&fact_of(state, kk), &inv)));
            }
            let fr = &mut self.js_frames[i];
            for (n, v) in &bound {
                let v = rename_value(v, &inv);
                let nv = match fr.args.get(n) {
                    Some(cur) => cur.join(&v),
                    None => v,
                };
                fr.args.insert(*n, nv);
            }
            for (k, f) in pend_input {
                let nf = match fr.input.get(&k) {
                    Some(g) => fact_join(g, &f),
                    None => f,
                };
                fr.input.insert(k, nf);
            }
            // (the objects its runs make stand for those of every level of the recursion: not
            // copied, merged — no stores into them replace what is there)
            for o in p.born.iter().copied() {
                if self.obj(o).single {
                    self.obj_mut(o).single = false;
                    crate::state::mark_merged(o);
                }
            }
            let empty = FxHashSet::default();
            return self.js_replay_with(&p, sg, &empty, state, node, &call);
        }
        // none covers the call: a variant of its own while there are few, else the call's
        // inputs folded into the last one (renamed onto the call's objects) and the body run again
        let base = if variants.len() < MAX_VARIANTS { None } else { variants.last().cloned() };
        let (js, keep) = self.js_run(foid, base.as_deref(), bound, state, node);
        // (one computed against another run's partial summary is this call's only)
        if keep {
            let vs = self.jsums.entry(code).or_default();
            if base.is_some() {
                vs.pop();
            }
            vs.push(js.clone());
        }
        self.js_replay(&js, Sigma::default(), state, node, &call)
    }

    /// May template object `t` stand for the call's object `a`: of one kind (and class),
    /// each one runtime object.
    fn js_matchable(&self, t: ObjId, a: ObjId) -> bool {
        let (kt, ka) = (self.kind(t), self.kind(a));
        kt == ka
            && matches!(kt, Kind::Instance | Kind::Dict | Kind::List | Kind::Tuple | Kind::Set | Kind::Namespace | Kind::Frame | Kind::Cell | Kind::Unknown | Kind::Series)
            && self.obj(t).single
            && self.obj(a).single
            && !crate::state::is_merged(t)
            && !crate::state::is_merged(a)
            && match (&self.obj(t).payload, &self.obj(a).payload) {
                (Payload::Cls(x), Payload::Cls(y)) => x == y,
                (Payload::Str(x), Payload::Str(y)) => x == y,
                (Payload::None, Payload::None) => true,
                _ => false,
            }
    }

    /// The renaming of the summary's objects to the call's (from the arguments and cells, then
    /// along the slots the runs read; injective: objects the summary keeps apart stay apart),
    /// and whether the call is covered: its arguments and the facts the runs read, renamed,
    /// below the summary's inputs.
    fn js_match(&self, js: &JSum, bound: &IndexMap<StrId, Value>, cells: &IndexMap<StrId, ObjId>, state: &State) -> (Sigma, bool) {
        let mut sg = Sigma::default();
        let mut image: FxHashSet<ObjId> = FxHashSet::default();
        let unify = |an: &Analyzer, sg: &mut Sigma, image: &mut FxHashSet<ObjId>, w: &Value, v: &Value| -> bool {
            match (w.single(), v.single()) {
                (Some(t), Some(a)) if t != a && !sg.contains_key(&t) && !image.contains(&a) && an.js_matchable(t, a) => {
                    sg.insert(t, a);
                    image.insert(a);
                    true
                }
                _ => false,
            }
        };
        let mut ok = true;
        for (n, c) in &js.cells {
            match cells.get(n) {
                Some(c2) if c2 == c => {}
                Some(c2) => {
                    unify(self, &mut sg, &mut image, &Value::obj(*c), &Value::obj(*c2));
                }
                None => ok = false,
            }
        }
        for (n, v) in bound {
            match js.args.get(n) {
                Some(w) => {
                    unify(self, &mut sg, &mut image, w, v);
                }
                None => ok = false,
            }
        }
        // (the objects the read slots of matched objects hold, matched in turn)
        loop {
            let mut grew = false;
            for (k, f) in &js.input {
                if k.0 != T_SLOTS || !sg.contains_key(&key_owner(*k)) {
                    continue;
                }
                let actual = fact_of(state, rename_key(*k, &sg));
                match (f, &actual) {
                    (Fact::Slot(Some(w)), Fact::Slot(Some(v))) => grew |= unify(self, &mut sg, &mut image, w, v),
                    (Fact::Slots(ws), Fact::Slots(vs)) => {
                        for (sk, w) in ws {
                            if let Some((_, v)) = vs.iter().find(|(k2, _)| k2 == sk) {
                                grew |= unify(self, &mut sg, &mut image, w, v);
                            }
                        }
                    }
                    _ => {}
                }
            }
            if !grew {
                break;
            }
        }
        if !ok {
            return (sg, false);
        }
        for (n, v) in bound {
            let w = rename_value(&js.args[n], &sg);
            if !(*v == w || v.join(&w) == w) {
                return (sg, false);
            }
        }
        let mut compared = 0;
        let mut same: FxHashMap<ObjId, bool> = FxHashMap::default();
        let covered = js.input.iter().all(|(k, f)| {
            let kk = rename_key(*k, &sg);
            layered::journal_read(kk);
            // (an object not renamed whose slots are the run's entry ones: its slot facts hold)
            if k.0 == T_SLOTS && kk == *k {
                let o = key_owner(*k);
                if *same.entry(o).or_insert_with(|| state.slots.same_sub(&js.entry.slots, o as u64)) {
                    return true;
                }
            }
            compared += 1;
            fact_leq(&fact_of(state, kk), &rename_fact(f, &sg))
        });
        // (work: a fact compared is a visit)
        self.walked.set(self.walked.get() + compared);
        if std::env::var("ROWAN_JSDBG").is_ok() {
            JS_STATS.with(|m| {
                let mut m = m.borrow_mut();
                let e = m.entry(js.cells.len() * 0 + js.args.keys().map(|k| k.0 as usize).sum::<usize>()).or_insert((String::new(), 0usize, 0usize, 0usize));
                e.1 += 1;
                e.2 += compared;
                e.3 = e.3.max(js.input.len());
            });
        }
        self.count_visit(0, compared);
        (sg, covered)
    }

    /// The context the objects a replay makes are allocated in: the call site's, as a run of
    /// the body there would have them (`receiver_ctx` of the site chained to the frame's).
    fn js_call_ctx(&self, self_v: &Option<Value>, node: &Node) -> (String, bool) {
        let mut site = if node.addr != 0 { format!("{}L{}:{}", crate::frontend::site_prefix(node.module), node.line, node.col) } else { format!("{}L{}", crate::frontend::site_prefix(node.module), node.line) };
        let fr = self.frame();
        for i in &fr.unroll {
            site.push_str(&format!("#{}", i));
        }
        if fr.kind != super::FrameKind::Module && !fr.ctx.is_empty() && !fr.ctx.starts_with("entry") {
            site = if fr.ctx.matches('/').count() < super::limit("chain", 2) {
                format!("{}/{}", fr.ctx, site)
            } else {
                let mut h = rustc_hash::FxHasher::default();
                std::hash::Hash::hash(&fr.ctx, &mut h);
                format!("h{:x}/{}", std::hash::Hasher::finish(&h), site)
            };
        }
        let (ctx, single) = self.receiver_ctx_pub(self_v, &site);
        (ctx, single && fr.single && fr.loop_depth == 0)
    }

    /// Run the body on this call's inputs, joined with those of `base` renamed onto the call's
    /// objects (a summary folded into: its objects do not grow with the calls'). A recursive
    /// function runs again with what its inner calls brought until its summary is stable.
    /// Also returns whether the summary may be kept (it did not use another run's partial one).
    fn js_run(&mut self, foid: ObjId, base: Option<&JSum>, mut bound: IndexMap<StrId, Value>, state: &State, node: &Node) -> (Rc<JSum>, bool) {
        let fi = self.func_info(foid);
        let code = fi.node.addr();
        let mut input: FxHashMap<FactKey, Fact> = FxHashMap::default();
        let mut born: FxHashSet<ObjId> = base.map(|p| p.born.clone()).unwrap_or_default();
        let runs = base.map(|p| p.runs).unwrap_or(0) + 1;
        if let Some(p) = base {
            let (sg, _) = self.js_match(p, &bound, &fi.closure, state);
            for (n, v) in &p.args {
                let v = rename_value(v, &sg);
                let nv = match bound.get(n) {
                    Some(cur) => cur.join(&v),
                    None => v.clone(),
                };
                // (an argument that grows at every call: widened)
                let nv = if runs > WIDEN_AFTER && nv != v { widen_value(&nv) } else { nv };
                bound.insert(*n, nv);
            }
            for (k, old) in &p.input {
                let (k2, old) = (rename_key(*k, &sg), rename_fact(old, &sg));
                let f = fact_join(&old, &fact_of(state, k2));
                // (this call's facts joined in, those that grew widened)
                let f = if runs > WIDEN_AFTER { fact_widen(&f, &old) } else { f };
                input.insert(k2, f);
            }
        }
        // the caller's context is not the run's
        let saved_chain = std::mem::take(&mut self.call_chain);
        let saved_loops = std::mem::take(&mut self.loop_stack);
        let saved_finally = std::mem::take(&mut self.finally_stack);
        let saved_guards = std::mem::take(&mut self.guards);
        let saved_may_raise = std::mem::take(&mut self.may_raise);
        let saved_raise_states = std::mem::take(&mut self.raise_states);
        let saved_try_marks = std::mem::take(&mut self.try_marks);
        let saved_try_sum = std::mem::take(&mut self.try_sum_depth);
        let saved_raise_values = std::mem::take(&mut self.raise_values);
        let saved_raise_cls = std::mem::take(&mut self.raise_cls);
        let saved_handling = std::mem::take(&mut self.handling);
        let saved_handling_values = std::mem::take(&mut self.handling_values);
        let saved_recent = std::mem::take(&mut self.recent_stack);
        let saved_cond = std::mem::replace(&mut self.cond_depth, 0);
        let saved_branch = std::mem::replace(&mut self.branch_depth, 0);
        let saved_catching = std::mem::replace(&mut self.catching_depth, 0);
        let saved_exh = self.exh_log.take();
        let saved_cur_exc = self.cur_exc.take();
        // (its notes are relative to the call: each replay places them)
        let saved_entry = self.entry_run.take();
        let mut partial: Option<Rc<JSum>> = None;
        let mut rounds = 0;
        let (js, keep) = loop {
            rounds += 1;
            if super::debug_enabled() {
                eprintln!("jrun {} ({} input facts, round {}) at L{}", fi.qualname, input.len(), rounds, node.line);
                if std::env::var("ROWAN_JSDBG").is_ok() {
                    let mut by: FxHashMap<String, usize> = FxHashMap::default();
                    for k in input.keys() {
                        let o = key_owner(*k);
                        *by.entry(format!("{}:{}", self.obj(o).label, self.obj(o).ctx)).or_default() += 1;
                    }
                    let mut v: Vec<(String, usize)> = by.into_iter().collect();
                    v.sort_by(|a, b| b.1.cmp(&a.1));
                    eprintln!("   owners: {:?}", &v[..v.len().min(6)]);
                }
            }
            self.js_frames.push(JsFrame { code, partial: partial.clone(), hit: false, provisional: false, args: IndexMap::new(), input: FxHashMap::default() });
            let mut js = self.js_run_once(foid, &bound, &mut input, &mut born, runs, state, node);
            let fr = self.js_frames.pop().unwrap();
            if !fr.hit {
                break (js, !fr.provisional);
            }
            // a recursive function: again with what its inner calls brought (renamed onto this
            // run's objects), until its summary is stable
            let mut grew = false;
            for (n, v) in fr.args {
                let old = bound.get(&n).cloned().unwrap_or_default();
                let mut nv = old.join(&v);
                if rounds > WIDEN_AFTER && nv != old {
                    nv = widen_value(&nv);
                }
                if nv != old {
                    if std::env::var("ROWAN_JSDBG").is_ok() && rounds >= 9 {
                        eprintln!("     fold changes arg {}: {:?} -> {:?}", crate::intern::str_get(n), old, nv);
                    }
                    grew = true;
                    bound.insert(n, nv);
                }
            }
            for (k, f) in fr.input {
                let old = input.get(&k).cloned();
                let mut nf = match &old {
                    Some(g) => fact_join(g, &f),
                    None => f,
                };
                if let Some(o) = old.as_ref().filter(|_| rounds > WIDEN_AFTER) {
                    nf = fact_widen(&nf, o);
                }
                if old.as_ref() != Some(&nf) {
                    if std::env::var("ROWAN_JSDBG").is_ok() && rounds >= 9 {
                        eprintln!("     fold changes {:?} {}: {:?} -> {:?}", k.0, self.obj(key_owner(k)).label, old, nf);
                    }
                    grew = true;
                    input.insert(k, nf);
                }
            }
            let stable = !grew && partial.as_ref().is_some_and(|p| js_same(p, &js));
            if std::env::var("ROWAN_JSDBG").is_ok() {
                if let Some(p) = &partial {
                    let diff: Vec<String> = js.writes.iter().filter(|k| fact_of(&p.exit, **k) != fact_of(&js.exit, **k)).take(3).map(|k| format!("{:?} {}: {:?} -> {:?}", k.0, self.obj(key_owner(*k)).label, fact_of(&p.exit, *k), fact_of(&js.exit, *k))).collect();
                    eprintln!("   round {}: grew={} ret_same={} writes_same={} diff={:?}", rounds, grew, p.ret == js.ret, p.writes == js.writes, diff);
                }
            }
            if stable || rounds >= MAX_ROUNDS {
                if !stable {
                    // not stable: what it wrote may hold anything
                    let site = crate::intern::s(&format!("recursion@{}", fi.qualname));
                    js.ret = js.ret.join(&Value::data(&format!("recursion@{}", fi.qualname)));
                    for (tag, h) in js.writes.clone() {
                        match tag {
                            T_ROWS => {
                                if let Some((k, r)) = js.exit.rows.get_h(h).map(|(k, r)| (*k, r.mutate(site))) {
                                    js.exit.rows.insert(k, r);
                                }
                            }
                            T_SLOTS => {
                                if let Some((Pair(o, sk), v)) = js.exit.slots.get_h(h).map(|(k, v)| (k.clone(), widen_value(v))) {
                                    js.exit.slots.set(&o, sk, v);
                                }
                            }
                            T_LENS => {
                                if let Some(o) = js.exit.lens.get_h(h).map(|(k, _)| *k) {
                                    js.exit.lens.insert(o, (None, 0, 0));
                                }
                            }
                            _ => {}
                        }
                    }
                }
                break (js, !fr.provisional);
            }
            partial = Some(Rc::new(js));
        };
        self.call_chain = saved_chain;
        self.loop_stack = saved_loops;
        self.finally_stack = saved_finally;
        self.guards = saved_guards;
        self.may_raise = saved_may_raise;
        self.raise_states = saved_raise_states;
        self.try_marks = saved_try_marks;
        self.try_sum_depth = saved_try_sum;
        self.raise_values = saved_raise_values;
        self.raise_cls = saved_raise_cls;
        self.handling = saved_handling;
        self.handling_values = saved_handling_values;
        self.recent_stack = saved_recent;
        self.cond_depth = saved_cond;
        self.branch_depth = saved_branch;
        self.catching_depth = saved_catching;
        self.exh_log = saved_exh;
        self.cur_exc = saved_cur_exc;
        self.entry_run = saved_entry;
        (Rc::new(js), keep)
    }

    /// One run of the body from the state with `input` installed (the entry values of the
    /// facts it reads and writes are added to `input`).
    #[allow(clippy::too_many_arguments)]
    fn js_run_once(&mut self, foid: ObjId, bound: &IndexMap<StrId, Value>, input: &mut FxHashMap<FactKey, Fact>, born: &mut FxHashSet<ObjId>, runs: usize, state: &State, node: &Node) -> JSum {
        let fi = self.func_info(foid);
        let mut run = state.copy_ref();
        self.walked.set(self.walked.get() + input.len());
        self.count_visit(1, input.len());
        for (k, f) in input.iter() {
            install(&mut run, *k, f);
        }
        run.ret = None;
        run.exits = None;
        // (path facts beyond the maps — tests' outcomes, keys stored through names, lists'
        // last items, tensor-dim facts — are not journaled: the run starts without them)
        run.facts = Default::default();
        run.name_keys = Default::default();
        run.lasts = Default::default();
        run.tf = Default::default();
        let entry = run.copy();
        let vmark = self.verdicts.len();
        let (rmark, qmark, emark) = (self.row_checks.len(), self.shape_reqs.len(), self.shape_errors.len());
        self.js_born.push(FxHashSet::default());
        self.js_raise.push(None);
        self.js_raise_vals.push(Value::new());
        self.raise_stack.push(false);
        self.esc_cls.push(Some(Vec::new()));
        layered::journal_open();
        let jr = JsRun { bound: bound.clone(), ctx: format!("j{}", foid) };
        let ret = self.call_user_jrun(foid, &mut run, node, jr);
        let journal = layered::journal_close();
        let raises = self.raise_stack.pop().unwrap_or(false);
        let raise_classes = self.esc_cls.pop().flatten();
        let raise_state = self.js_raise.pop().flatten().map(|(_, acc)| acc);
        let raise_vals = self.js_raise_vals.pop().unwrap_or_default();
        let made = self.js_born.pop().unwrap_or_default();
        if let Some(parent) = self.js_born.last_mut() {
            parent.extend(made.iter().copied());
        }
        born.extend(made);
        let verdicts: Vec<Verdict> = self.verdicts.drain(vmark..).collect();
        for v in &verdicts {
            self.verdict_keys.remove(&v.dedupe_key());
        }
        let row_checks: Vec<super::RowCheck> = self.row_checks.drain(rmark..).collect();
        let mut shape_notes: Vec<(bool, crate::models::torch::ShapeNote)> = self.shape_reqs.drain(qmark..).map(|n| (false, n)).collect();
        shape_notes.extend(self.shape_errors.drain(emark..).map(|n| (true, n)));
        // the facts it read, and those it wrote (a path that does not write one keeps the
        // caller's value: the run's exit covers the call's only from a larger entry), as they
        // were at its entry, are its inputs
        // (only what this run read and wrote: installed values a run does not look at constrain
        // nothing it computed)
        let mut writes: Vec<FactKey> = journal.writes.into_iter().collect();
        writes.sort_unstable();
        let mut used: FxHashMap<FactKey, Fact> = FxHashMap::default();
        for k in journal.reads.iter().chain(writes.iter().filter(|k| k.0 != T_REQS)) {
            used.insert(*k, fact_of(&entry, *k));
        }
        *input = used;
        let completes = run.reachable;
        // (kept: the written facts of the exit and of the raise states, the slot subtrees of the
        // objects read at entry — not whole states, which would pin the heap as it was)
        let exit = restrict(&run, &writes);
        let raise_state = raise_state.map(|rs| restrict(&rs, &writes));
        let mut entry_slots = State::new();
        let owners: FxHashSet<ObjId> = input.keys().filter(|k| k.0 == T_SLOTS).map(|k| key_owner(*k)).collect();
        for o in owners {
            entry_slots.slots.graft(&entry.slots, &o);
        }
        JSum { args: bound.clone(), cells: fi.closure.clone(), input: input.clone(), writes, exit, entry: entry_slots, raise_state, raise_vals, ret, raises, raise_classes, completes, verdicts, row_checks, shape_notes, born: born.clone(), runs }
    }

    /// The summary's written facts as `from` has them, in place of `state`'s: the run started
    /// from inputs above the call's (written facts included), so by monotonicity its values
    /// cover what the call writes and what it leaves (a fact `from` lacks stays as it is).
    fn js_apply(&mut self, js: &JSum, from: &State, state: &mut State, inst: &mut Inst) {
        let mut touched: Vec<ObjId> = Vec::new();
        // (facts of objects the run made: those the result or the other facts reach, on their
        // copies; the others are garbage)
        let mut born_facts: Vec<(u8, u64)> = Vec::new();
        for &(tag, h) in &js.writes {
            let owner = match tag {
                T_ROWS => (h >> 3) as ObjId,
                T_SLOTS => (h >> 32) as ObjId,
                T_ELEMS | T_LENS => h as ObjId,
                _ => 0,
            };
            if tag != T_REQS && inst.born.contains(&owner) {
                born_facts.push((tag, h));
                continue;
            }
            self.js_apply_one(tag, h, from, state, inst, &mut touched);
        }
        // (a copy made while applying a fact reaches further copies: until none is new)
        let mut done: FxHashSet<(u8, u64)> = FxHashSet::default();
        loop {
            let ready: Vec<(u8, u64)> = born_facts.iter().copied().filter(|k| !done.contains(k)).filter(|(tag, h)| {
                let owner = match *tag {
                    T_ROWS => (h >> 3) as ObjId,
                    T_SLOTS => (h >> 32) as ObjId,
                    _ => *h as ObjId,
                };
                inst.map.contains_key(&owner)
            }).collect();
            if ready.is_empty() {
                break;
            }
            for (tag, h) in ready {
                done.insert((tag, h));
                self.js_apply_one(tag, h, from, state, inst, &mut touched);
            }
        }
        if !touched.is_empty() {
            // (keys may be gone now from records stored through names; tests on them are stale)
            state.drop_name_keys(None);
            touched.sort_unstable();
            touched.dedup();
            self.kill_facts_on(&touched, state);
        }
    }

    fn js_apply_one(&mut self, tag: u8, h: u64, from: &State, state: &mut State, inst: &mut Inst, touched: &mut Vec<ObjId>) {
        {
            match tag {
                T_ROWS => {
                    let Some(((o, ns), r)) = from.rows.get_h(h).map(|(k, r)| (*k, r.clone())) else { return };
                    let o = inst.obj(self, state, o);
                    if state.rows.get(&(o, ns)).is_some_and(|cur| cur == r) {
                        return;
                    }
                    touched.push(o);
                    state.set_row(o, ns, r);
                }
                T_SLOTS => {
                    let Some((Pair(o, sk), v)) = from.slots.get_h(h).map(|(k, v)| (k.clone(), v.clone())) else { return };
                    let o = inst.obj(self, state, o);
                    let v = inst.value(self, state, &v);
                    if state.slots.get(&o, &sk).is_some_and(|cur| cur == v) {
                        return;
                    }
                    touched.push(o);
                    state.set_slot(o, sk.0, sk.1, v, true);
                }
                T_ELEMS => {
                    let Some((o, kv)) = from.elems.get_h(h).map(|(k, v)| (*k, v.clone())) else { return };
                    let o = inst.obj(self, state, o);
                    let kv = inst.value(self, state, &Value::kv_only(kv)).kv;
                    if state.elems.get(&o).is_some_and(|cur| cur == kv) {
                        return;
                    }
                    state.clear_last(o);
                    touched.push(o);
                    state.elems.insert(o, kv);
                }
                T_LENS => {
                    let Some((o, l)) = from.lens.get_h(h).map(|(k, v)| (*k, *v)) else { return };
                    let o0 = o;
                    let o = inst.obj(self, state, o);
                    if std::env::var("ROWAN_JSDBG").is_ok() && self.obj(o).label.contains("L216") { eprintln!("  lens write {}->{} {:?} (was {:?})", o0, o, l, state.lens.get(&o)); }
                    state.lens.insert(o, l);
                }
                T_REQS if !inst.no_reqs => {
                    let Some((Pair(src, k), info)) = from.reqs.get_h(h).map(|(k, v)| (k.clone(), v.clone())) else { return };
                    let src: SourceId = src;
                    let nv: ReqInfo = match state.reqs.get(&src, &k) {
                        Some(cur) => cur.seq(&info),
                        None => info,
                    };
                    state.reqs.set(&src, k, nv);
                }
                _ => {}
            }
        }
    }

    /// The call's effects: the summary's written facts, result, raises and verdicts, the
    /// objects the run made copied into the call's context.
    fn js_replay(&mut self, js: &JSum, sg: Sigma, state: &mut State, node: &Node, call: &(String, bool)) -> Value {
        self.js_replay_with(js, sg, &js.born, state, node, call)
    }

    /// `js_replay`, copying the objects of `born` only.
    fn js_replay_with(&mut self, js: &JSum, sg: Sigma, born: &FxHashSet<ObjId>, state: &mut State, node: &Node, call: &(String, bool)) -> Value {
        let mut inst = Inst { no_reqs: false, born, ctx: call.0.clone(), single: call.1, map: sg };
        // (the result's copies first: the facts of the objects it reaches are applied to them)
        let ret = inst.value(self, state, &js.ret);
        if js.raises {
            let mut hs = state.copy();
            // (the exceptions, copied like the run's other objects — before the facts are
            // applied, so theirs are; each of a known class goes to the handlers of its class)
            let exc = inst.value(self, &mut hs, &js.raise_vals);
            if let Some(rs) = &js.raise_state {
                self.js_apply(js, rs, &mut hs, &mut inst);
            }

            match js.raise_classes.clone().filter(|cs| !cs.is_empty()) {
                Some(cs) => {
                    for c in cs {
                        let name = c.0.first().cloned().unwrap_or_default();
                        let of_c: Vec<ObjId> = exc.pts.iter().filter(|o| self.exc_value_names(&Value::obj(*o)).is_some_and(|n| n.0.first() == c.0.first())).collect();
                        self.cur_exc = if of_c.is_empty() { None } else { Some(Value::objs(crate::domain::ObjSet::from_vec(of_c))) };
                        self.cur_exc_cls = Some(c.clone());
                        self.may_raise_here(&name, &mut hs.copy());
                        self.cur_exc_cls = None;
                        self.cur_exc = None;
                    }
                }
                None => {
                    self.note_raise();
                    if !self.raise_states.is_empty() {
                        self.cur_exc = if exc.pts.is_empty() { None } else { Some(exc.clone()) };
                        self.record_raise_state(&mut hs);
                        self.cur_exc = None;
                    }
                    self.note_summary_raise(&hs);
                    self.js_note_exc(Some(&exc));
                }
            }
        }
        // (under a try that handles a failed read, the run's reads are guarded: no
        // requirement and no diagnostic, as a run of the body here would have them)
        let guarded = (self.is_guarded(Ns::Item) || self.is_guarded(Ns::Attr)) && !(self.catching_depth > 0 || self.caught_load || self.guard_reraises(Ns::Item) || self.guard_reraises(Ns::Attr));
        inst.no_reqs = guarded;
        self.js_apply(js, &js.exit, state, &mut inst);
        if std::env::var("ROWAN_JSDBG").is_ok() {
            let mut m: Vec<String> = inst.map.iter().map(|(t, n)| format!("{}:{}->{}", t, self.obj(*t).label, n)).collect();
            m.sort();
            eprintln!("replay at L{} ctx={} writes={} map={:?} ret={:?} completes={} raises={} lens_writes={:?}", node.line, call.0, js.writes.len(), m, ret.pts.to_vec(), js.completes, js.raises, js.writes.iter().filter(|k| k.0 == T_LENS).map(|k| k.1).collect::<Vec<_>>());
            for o in ret.pts.iter().chain(inst.map.values().copied()) {
                eprintln!("   o{} attr={:?}", o, state.rows.get(&(o, Ns::Attr)).map(|r| r.details()));
            }
        }
        let chain = chain_extend(&self.call_chain, &[node.line]);
        for v in &js.verdicts {
            let mut nv = v.clone();
            nv.chain = chain_extend(&chain, &v.chain);
            nv.guarded |= guarded;
            let dk = nv.dedupe_key();
            if !self.verdict_keys.contains(&dk) && !self.quiet {
                self.verdict_keys.insert(dk);
                self.verdicts.push(nv);
            }
        }
        if !self.quiet {
            let (cchain, centry) = self.t_note_ctx_pub();
            let prefix = chain_extend(&cchain, &[node.line]);
            for rc in &js.row_checks {
                let mut rc = rc.clone();
                if let Some(t) = rc.tensor.as_mut() {
                    let rel: Vec<u32> = t.get("chain").and_then(|c| serde_json::from_value(c.clone()).ok()).unwrap_or_default();
                    t["chain"] = serde_json::json!(chain_extend(&prefix, &rel));
                    t["entry"] = serde_json::json!(centry);
                }
                self.row_checks.push(rc);
            }
            for (err, n) in &js.shape_notes {
                let mut n = n.clone();
                n.chain = chain_extend(&prefix, &n.chain);
                n.entry = centry;
                let list = if *err { &mut self.shape_errors } else { &mut self.shape_reqs };
                if !list.iter().any(|m| m.line == n.line && m.chain == n.chain && m.text == n.text) {
                    list.push(n);
                }
            }
        }
        if !js.completes {
            state.reachable = false;
        }
        ret
    }
}

/// The copies a replay makes of the objects its run made (made when first reached).
struct Inst<'a> {
    /// requirements are not the call's (a guarded call)
    no_reqs: bool,
    born: &'a FxHashSet<ObjId>,
    ctx: String,
    single: bool,
    map: FxHashMap<ObjId, ObjId>,
}

impl Inst<'_> {
    fn obj(&mut self, an: &mut Analyzer, state: &mut State, t: ObjId) -> ObjId {
        if let Some(n) = self.map.get(&t) {
            return *n;
        }
        if !self.born.contains(&t) || matches!(an.kind(t), Kind::Class | Kind::Module | Kind::External | Kind::Super) {
            return t;
        }
        let (kind, site, tctx, label, tsingle, iter_elem, iterator, depth, derived, copied, payload) = {
            let o = an.obj(t);
            (o.kind, o.site.to_string(), o.ctx.to_string(), o.label.to_string(), o.single, o.iter_elem, o.iterator, o.depth, o.derived_from, o.copied_from, o.payload.clone())
        };
        let site = site.strip_suffix("#recent").map(|x| x.to_string()).unwrap_or(site);
        // (objects of one line made in different inner contexts of the run stay apart: by the
        // innermost call site of its context — a copy of a copy is named as the copy was)
        let base = site.split('|').next().unwrap_or(&site).to_string();
        let inner = tctx.rsplit('/').next().unwrap_or("");
        let site = format!("{}|{}", base, inner);
        let ns = if matches!(kind, Kind::Instance | Kind::Namespace) { Ns::Attr } else { Ns::Item };
        let n = an.alloc_in_frame(kind, &site, state, &label, Payload::None, None, ns, tsingle, &self.ctx, self.single);
        self.map.insert(t, n);
        if let Some(o) = an.origins.get(&t).cloned() {
            an.origins.insert(n, o);
        }
        if let Some(sym) = an.symbols.get(&t).cloned() {
            let scope = an.symbol_scope.get(&t).copied().unwrap_or(0);
            an.set_symbol(n, sym, scope);
        }
        if let Some(f) = an.dd_factories.get(&t).cloned() {
            an.dd_factories.insert(n, f);
        }
        an.copy_side_tables(t, n);
        let derived = derived.map(|d| self.obj(an, state, d));
        let copied = copied.map(|d| self.obj(an, state, d));
        let payload = match payload {
            Payload::Func(fi) => {
                let closure: IndexMap<StrId, ObjId> = fi.closure.iter().map(|(k, c)| (*k, self.obj(an, state, *c))).collect();
                Payload::Func(Rc::new(fi.clone_with_closure(closure)))
            }
            Payload::Bound(f, v) => Payload::Bound(self.obj(an, state, f), self.value(an, state, &v)),
            Payload::Method(q, v) => Payload::Method(q, self.value(an, state, &v)),
            Payload::View(k, v) => Payload::View(k, self.value(an, state, &v)),
            Payload::Zip(a, b) => Payload::Zip(self.value(an, state, &a), self.value(an, state, &b)),
            Payload::Enumerate(v, s0) => Payload::Enumerate(self.value(an, state, &v), s0),
            Payload::Pair(a, b) => Payload::Pair(self.value(an, state, &a), self.value(an, state, &b)),
            Payload::Partial(f, args, kw) => {
                let f2 = self.value(an, state, &f);
                let args2: Vec<Value> = args.iter().map(|a| self.value(an, state, a)).collect();
                let kw2: IndexMap<StrId, Value> = kw.iter().map(|(k, v)| (*k, self.value(an, state, v))).collect();
                Payload::Partial(f2, args2, kw2)
            }
            Payload::Groupby(fs, ks, ai, sel) => Payload::Groupby(fs.iter().map(|f| self.obj(an, state, *f)).collect(), ks, ai, sel),
            Payload::Locidx(name, o) => Payload::Locidx(name, self.obj(an, state, o)),
            Payload::Super(c, v) => Payload::Super(c, self.value(an, state, &v)),
            other => other,
        };
        let o = an.obj_mut(n);
        o.payload = payload;
        o.iter_elem = iter_elem;
        o.iterator = iterator;
        o.depth = depth;
        o.derived_from = derived;
        o.copied_from = copied;
        n
    }

    fn value(&mut self, an: &mut Analyzer, state: &mut State, v: &Value) -> Value {
        if self.map.is_empty() && self.born.is_empty() {
            return v.clone();
        }
        let hit = v.pts.iter().any(|o| self.born.contains(&o) || self.map.contains_key(&o)) || matches!(&v.kv, KeyVal::KeysOf(ko) if ko.objs.iter().any(|o| self.born.contains(&o) || self.map.contains_key(&o)));
        if !hit {
            return v.clone();
        }
        let pts: Vec<ObjId> = v.pts.iter().map(|o| self.obj(an, state, o)).collect();
        let kv = match &v.kv {
            KeyVal::KeysOf(ko) => {
                let objs: Vec<ObjId> = ko.objs.iter().map(|o| self.obj(an, state, o)).collect();
                KeyVal::KeysOf(crate::domain::KeysOf { objs: crate::domain::ObjSet::from_vec(objs), ..ko.clone() })
            }
            other => other.clone(),
        };
        Value::with_kv(crate::domain::ObjSet::from_vec(pts), kv)
    }
}

/// The same summary (a recursive function's rounds): result, completion and written facts.
fn js_same(a: &JSum, b: &JSum) -> bool {
    a.ret == b.ret && a.completes == b.completes && a.raises == b.raises && a.writes == b.writes && a.writes.iter().all(|k| fact_of(&a.exit, *k) == fact_of(&b.exit, *k))
}
