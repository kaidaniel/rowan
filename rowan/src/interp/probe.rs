//! PROTOTYPE (not for main): per-run read sets. A finished run of a function (one activation
//! key, the same arguments) answers a later call whose entry state differs from its own only on
//! objects the run did not read or write (`layered::probe_note` logs every object a state map
//! is asked about), when nothing outside the state the run depended on differs (`context`) and
//! the run neither took a running activation's partial result nor raised out of itself.

use indexmap::IndexMap;
use rustc_hash::{FxHashMap, FxHashSet};
use std::cell::RefCell;

use super::findings::VerdictKey;
use super::recursion::ActKey;
use super::Analyzer;
use crate::domain::Value;
use crate::intern::StrId;
use crate::layered::PROBE_SETS;
use crate::state::State;

pub type ArgsKey = (Option<Value>, Vec<Value>, IndexMap<StrId, Value>, Vec<Value>, Vec<Value>);

struct Rec {
    args: ArgsKey,
    ctx: String,
    entry: State,
    ret: Value,
    exit: State,
    touched: FxHashSet<u32>,
    flags: Vec<(u32, bool, bool)>,
    vkeys: Vec<VerdictKey>,
}

struct Frame {
    depth: usize,
    ok: bool,
    raised: bool,
    vkeys: Vec<VerdictKey>,
}

#[derive(Default)]
struct Data {
    table: FxHashMap<ActKey, Vec<Rec>>,
    frames: Vec<Frame>,
    calls: u64,
    eligible: u64,
    hits: u64,
    stored: u64,
    saved: u64,
    why: [u64; 8],
    inel: [u64; 8],
    only_raised: u64,
    spoil_by: [u64; 8],
    detail: std::collections::BTreeMap<String, u64>,
    norec: u64,
    spoiled: u64,
}

thread_local! {
    static D: RefCell<Data> = Default::default();
    static DETAIL: RefCell<std::collections::BTreeMap<String, u64>> = Default::default();
}

fn det(k: String) {
    DETAIL.with(|m| *m.borrow_mut().entry(k).or_insert(0) += 1);
}

pub fn on() -> bool {
    crate::layered::probe_on()
}

/// Every running run from frame depth `from` up depended on something outside its entry.
pub fn spoil(from: usize) {
    spoil_why(from, 7)
}

pub fn spoil_why(from: usize, why: usize) {
    D.with(|d| {
        let mut d = d.borrow_mut();
        if d.frames.iter().any(|f| f.depth > from && f.ok) {
            d.spoil_by[why] += 1;
        }
        for f in d.frames.iter_mut() {
            if f.depth > from {
                f.ok = false;
            }
        }
    });
}

/// A raise reached a try of frame depth `from` (none: 0).
pub fn raised(from: usize) {
    D.with(|d| {
        for f in d.borrow_mut().frames.iter_mut() {
            if f.depth > from {
                f.raised = true;
            }
        }
    });
}

pub fn inelig(i: usize) {
    D.with(|d| d.borrow_mut().inel[i] += 1);
}

pub fn spoil_all() {
    spoil(0)
}

pub fn spoil_all_why(why: usize) {
    spoil_why(0, why)
}

/// A verdict the running runs record (or find recorded).
pub fn verdict_key(k: &VerdictKey) {
    D.with(|d| {
        if let Some(f) = d.borrow_mut().frames.last_mut() {
            f.vkeys.push(k.clone());
        }
    });
}

impl Analyzer {
    /// What outside the state a run's outcome depends on.
    pub(super) fn memo_context(&self) -> Option<String> {
        if self.cx.called_by_model || self.budget.hard_cut || self.budget.lib_entry.is_some() || self.findings.wants_general() {
            inelig(0);
            return None;
        }
        let g = &self.gens;
        if !g.reads.is_empty() || !g.forcing.is_empty() || !g.watch.is_empty() || !g.refilling.is_empty() || self.gens.force_at.is_some() || self.gens.cm_hook.is_some() {
            inelig(if !g.reads.is_empty() { 1 } else if !g.watch.is_empty() { 2 } else if !g.for_iters.is_empty() { 3 } else { 4 });
            return None;
        }
        if !self.recursion.collecting.is_empty() {
            inelig(5);
            return None;
        }
        let u = &self.unwind;
        let tries: Vec<(usize, Vec<(Vec<String>, bool)>, bool)> = u
            .tries
            .iter()
            .map(|t| {
                (t.frames, t.hclasses.iter().map(|(h, b)| {
                    let mut v: Vec<String> = h.iter().cloned().collect();
                    v.sort();
                    (v, *b)
                }).collect(), t.runs)
            })
            .collect();
        let m = &self.mode;
        let fr = self.frame();
        Some(format!(
            "{:?}|{}|{}|{}|{}|{}|{}|{}|{:?}|{}|{}|{:?}|{:?}|{}|{}|{}|{:?}|{}|{}|{}|{}|{:?}|{}|{}",
            tries,
            u.handling.len(),
            u.handling_values.len(),
            u.closing.len(),
            u.finally_stack.len(),
            u.catching_depth,
            u.cur_exc.is_some(),
            m.verdict_guarded,
            (m.recv_scalar, m.one_of_several, m.bypass_setattr, m.plain_get),
            m.trace.as_ref().map(|t| t.0).unwrap_or(0),
            m.key_alias.is_some(),
            m.wide_read,
            (m.nonempty_comp, m.nonempty_gen, m.bulk_genexp, m.keep_deferred),
            self.cx.recent_stack.len(),
            self.cx.frames.len(),
            fr.single,
            (fr.single_nl, fr.loop_depth),
            fr.unroll.len(),
            self.findings.quiet,
            self.findings.quiet_records,
            self.entry.recording,
            self.cx.call_chain,
            self.cx.model_depth,
            self.heap.from_main,
        ))
    }

    /// A finished run answering this call: its result and end state, made from this entry.
    pub(super) fn memo_lookup(&mut self, key: &ActKey, args: &ArgsKey, ctx: &str, entry: &State) -> Option<(Value, State)> {
        let found = D.with(|d| {
            let mut d = d.borrow_mut();
            d.eligible += 1;
            if !d.table.contains_key(key) {
                d.norec += 1;
                return None;
            }
            let recs = d.table.get(key)?;
            let mut why = 0usize;
            let mut out = None;
            for r in recs.iter().rev() {
                if r.args != *args {
                    why = why.max(1);
                    det("args".to_string());
                    continue;
                }
                if r.ctx != ctx {
                    why = why.max(1);
                    for (i, (x, y)) in r.ctx.split('|').zip(ctx.split('|')).enumerate() {
                        if x != y {
                            det(format!("ctx{}", i));
                            break;
                        }
                    }
                    continue;
                }
                let (a, b) = (&r.entry, entry);
                let same_fields = a.env == b.env && a.narrowed == b.narrowed && a.ret == b.ret && a.reachable == b.reachable && a.exits.is_none() && b.exits.is_none() && std::sync::Arc::ptr_eq(&a.facts, &b.facts) && a.name_keys == b.name_keys && a.lasts == b.lasts && a.tf == b.tf && a.unbound == b.unbound && a.path == b.path && a.sym_lens == b.sym_lens;
                if !same_fields {
                    why = why.max(2);
                    let names = ["env", "narrowed", "ret", "reachable", "exits", "facts", "name_keys", "lasts", "tf", "unbound", "path", "sym_lens"];
                    let diffs = [a.env != b.env, a.narrowed != b.narrowed, a.ret != b.ret, a.reachable != b.reachable, !(a.exits.is_none() && b.exits.is_none()), !std::sync::Arc::ptr_eq(&a.facts, &b.facts), a.name_keys != b.name_keys, a.lasts != b.lasts, a.tf != b.tf, a.unbound != b.unbound, a.path != b.path, a.sym_lens != b.sym_lens];
                    for (n, x) in names.iter().zip(diffs) {
                        if x {
                            det(n.to_string());
                        }
                    }
                    continue;
                }
                let rows = b.rows.diff_keys(&a.rows);
                let slots = b.slots.diff_keys(&a.slots);
                let elems = b.elems.diff_keys(&a.elems);
                let lens = b.lens.diff_keys(&a.lens);
                let t = &r.touched;
                if rows.iter().any(|(o, _)| t.contains(o)) || slots.iter().any(|(o, _)| t.contains(o)) || elems.iter().any(|o| t.contains(o)) || lens.iter().any(|o| t.contains(o)) {
                    why = why.max(3);
                    continue;
                }
                let reqs = b.reqs.diff_keys(&a.reqs);
                if !reqs.is_empty() {
                    let ran: FxHashSet<_> = r.exit.reqs.diff_keys(&a.reqs).into_iter().collect();
                    if reqs.iter().any(|k| ran.contains(k)) {
                        why = why.max(4);
                        continue;
                    }
                }
                out = Some((r, rows, slots, elems, lens, reqs));
                break;
            }
            let Some((r, rows, slots, elems, lens, reqs)) = out else {
                d.why[why] += 1;
                return None;
            };
            Some((r.ret.clone(), r.exit.copy(), r.touched.clone(), r.flags.clone(), r.vkeys.clone(), rows, slots, elems, lens, reqs))
        })?;
        let (ret, mut exit, touched, flags, vkeys, rows, slots, elems, lens, reqs) = found;
        // (the heap's strength of what the run changed is as it was)
        if flags.iter().any(|(o, single, merged)| self.heap.obj(*o).single != *single || crate::state::is_merged(*o) != *merged) {
            D.with(|d| d.borrow_mut().why[5] += 1);
            return None;
        }
        // (every verdict it recorded is still recorded)
        if !vkeys.iter().all(|k| self.findings.has_key(k)) {
            D.with(|d| d.borrow_mut().why[6] += 1);
            return None;
        }
        // the end state: the run's, with what differs at this entry (none of which it touched)
        for k in rows {
            match entry.rows.get(&k) {
                Some(v) => exit.rows.insert(k, v),
                None => {
                    exit.rows.remove(&k);
                }
            }
        }
        for (o, sk) in slots {
            match entry.slots.get(&o, &sk) {
                Some(v) => exit.slots.set(&o, sk, v),
                None => {
                    exit.slots.remove(&o, &sk);
                }
            }
        }
        for o in elems {
            match entry.elems.get(&o) {
                Some(v) => exit.elems.insert(o, v),
                None => {
                    exit.elems.remove(&o);
                }
            }
        }
        for o in lens {
            match entry.lens.get(&o) {
                Some(v) => exit.lens.insert(o, v),
                None => {
                    exit.lens.remove(&o);
                }
            }
        }
        for (s, k) in reqs {
            match entry.reqs.get(&s, &k) {
                Some(v) => exit.reqs.set(&s, k, v),
                None => {
                    exit.reqs.remove(&s, &k);
                }
            }
        }
        // (the caller's run read what this one did)
        PROBE_SETS.with(|s| {
            if let Some(top) = s.borrow_mut().last_mut() {
                top.extend(touched.iter().copied());
            }
        });
        D.with(|d| {
            let mut d = d.borrow_mut();
            d.hits += 1;
            if let Some(f) = d.frames.last_mut() {
                f.vkeys.extend(vkeys);
            }
        });
        Some((ret, exit))
    }
}

pub fn begin(depth: usize) {
    D.with(|d| {
        let mut d = d.borrow_mut();
        d.calls += 1;
        d.frames.push(Frame { depth, ok: true, raised: false, vkeys: Vec::new() });
    });
    PROBE_SETS.with(|s| s.borrow_mut().push(FxHashSet::default()));
}

/// The run ended: kept for later calls when nothing outside its entry decided it.
#[allow(clippy::too_many_arguments)]
pub fn end(an: &Analyzer, key: ActKey, args: ArgsKey, ctx: Option<String>, entry: State, ret: &Value, exit: &State, stmts: usize) {
    let touched = PROBE_SETS.with(|s| {
        let mut s = s.borrow_mut();
        let t = s.pop().unwrap_or_default();
        if let Some(p) = s.last_mut() {
            p.extend(t.iter().copied());
        }
        t
    });
    D.with(|d| {
        let mut d = d.borrow_mut();
        let f = d.frames.pop().unwrap();
        if let Some(p) = d.frames.last_mut() {
            p.ok &= f.ok;
            p.raised |= f.raised;
            p.vkeys.extend(f.vkeys.iter().cloned());
        }
        let Some(ctx) = ctx else { return };
        if !f.ok {
            d.spoiled += 1;
            return;
        }
        if f.raised {
            d.only_raised += 1;
            return;
        }
        d.stored += 1;
        let _ = stmts;
        let flags = touched.iter().filter(|o| an.heap.has_obj(**o)).map(|o| (*o, an.heap.obj(*o).single, crate::state::is_merged(*o))).collect();
        let recs = d.table.entry(key).or_default();
        recs.push(Rec { args, ctx, entry, ret: ret.clone(), exit: exit.copy(), touched, flags, vkeys: f.vkeys });
        if recs.len() > 2 {
            recs.remove(0);
        }
    });
}

pub fn saved(stmts: usize) {
    D.with(|d| d.borrow_mut().saved += stmts as u64);
}

pub fn report() -> String {
    D.with(|d| {
        let d = d.borrow();
        format!("memo: runs {}, eligible {}, stored {}, hits {}, misses by (-, args/ctx, fields, touched, reqs, flags, verdicts) {:?}; inelig (general/model/lib, genreads, genwatch, foriters, gens other, rec collecting) {:?}; not stored: spoiled {}, only raised {}; spoils by (partial, lazygen, forcegen, allocwalk, callrecent, close, -, other) {:?}; no record {}; detail {:?}", d.calls, d.eligible, d.stored, d.hits, d.why, d.inel, d.spoiled, d.only_raised, d.spoil_by, d.norec, DETAIL.with(|m| m.borrow().clone()))
    })
}
