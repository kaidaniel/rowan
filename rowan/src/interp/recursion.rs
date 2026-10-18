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

use super::{Analyzer, Args, FuncInfo, FuncNode};
use crate::domain::{KeyVal, ObjId, Value};
use crate::frontend::Node;
use crate::intern::{self, s, StrId};
use crate::state::{Footprint, State};

/// Running activations of one function past which a call with other literal arguments joins
/// one of them (`activation_for`).
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
}

impl Recursion {
    /// Is this activation running (a call of it is recursive)?
    pub(in crate::interp) fn is_active(&self, key: &ActKey) -> bool {
        self.active.contains(key)
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
        if self.recursion.depth(foid) < UNKNOWN_ACTIVE || self.frame().func != Some(foid) || matches!(self.heap.func_info(foid).node, FuncNode::Def(f) if crate::syntax::is_generator(&f.body)) {
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
        if let Some((ret, exit_state, fp)) = self.recursion.table.get(key) {
            let ret = ret.clone();
            state.join_heap_fp(exit_state, fp);
            return ret;
        }
        Value::new()
    }

    /// The rounds of a call's body from `state` (its parameters bound): one, or for a recursive
    /// activation until its result and state are stable, then widened. Its result and end state.
    pub(super) fn run_rounds(&mut self, key: &ActKey, fi: &FuncInfo, gen_id: Option<ObjId>, pre_bind: &State, star_args: &[Value], star_kwargs: &[Value], state: &mut State, node: &Node) -> (Value, State) {
        let fnode = fi.node;
        let mut entry_state = state.copy();
        let mut ret = Value::new();
        let mut st_final: Option<State> = None;
        let mut vmark = self.findings.mark();
        for _round in 0..REC_MAX_ITER {
            // (a round of a recursion before its fixpoint: its accesses' verdicts are superseded)
            self.findings.drop_since(vmark);
            vmark = self.findings.mark();
            self.budget.runs += 1;
            let mut st = entry_state.copy();
            if let Some(g) = gen_id {
                self.gens.reads.push((g, false, Default::default()));
            }
            match fnode {
                FuncNode::Lambda(l) => {
                    let v = self.eval(&l.body, &mut st);
                    // (a body that raised on every path returns nothing)
                    if st.reachable {
                        st.ret = Some(v);
                        st.record_exit();
                    }
                    st.reachable = false;
                }
                FuncNode::Def(f) => self.with_gen_restart(&mut st, true, &mut |an, s| an.exec_block(&f.body, s)),
            }
            if gen_id.is_some() {
                self.gens.last_reads = self.gens.reads.pop().map(|r| r.2);
            }
            ret = st.ret.clone().unwrap_or_default();
            if st.reachable {
                ret = ret.join(&self.none_v);
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
            st.ret = Some(ret.clone());
            st.reachable = completed;
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
            if self.recursion.hit.contains(key) {
                if let Some((pr, pst, _)) = self.recursion.table.get(key) {
                    if *pr == ret && pst.same(&st) {
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

/// A recursive activation's argument, widened: more than two literals become an unknown.
fn widen_value(v: &Value) -> Value {
    if let KeyVal::Lits(ks) = &v.kv {
        if ks.len() > 2 {
            return Value::with_kv(v.pts.clone(), KeyVal::data("recursion"));
        }
    }
    v.clone()
}
