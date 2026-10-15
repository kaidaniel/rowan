//! Call memo probe (measurement only): how often a body run's whole entry equals the previous
//! run's at the same call, and what the earlier run changed outside `State`.

use indexmap::IndexMap;
use rustc_hash::FxHashMap;

use super::Analyzer;
use crate::domain::{ObjId, Value};
use crate::intern::StrId;
use crate::state::State;

#[derive(Clone, PartialEq)]
pub struct MemoKey {
    self_v: Option<Value>,
    args: Vec<Value>,
    kwargs: IndexMap<StrId, Value>,
    star_args: Vec<Value>,
    star_kwargs: Vec<Value>,
    chain: Vec<u32>,
    depths: (i32, i32, usize, usize, u32, usize),
    frame: (bool, bool, u32, String),
    extra: Option<String>,
    fresh: Vec<ObjId>,
    comp_known: Vec<bool>,
    modes: (bool, bool, bool, bool, u32, bool, usize, bool),
    tries: Vec<Vec<String>>,
    envs: Vec<IndexMap<StrId, Value>>,
    frames: Vec<(String, Vec<ObjId>, ObjId)>,
    roots: (Vec<Value>, Vec<Vec<(ObjId, crate::state::Kind, String, String)>>, Vec<Vec<(StrId, ObjId)>>, usize, Option<Value>),
}

/// Counters of things outside `State`.
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Outside {
    pub serial: u64,
    pub heap_muts: u64,
    pub verdicts: usize,
    pub rec_calls: u64,
    pub gen_ops: u64,
    pub raise_recs: u64,
    pub outer_raise_recs: u64,
    pub modules: usize,
    pub tables: usize,
    pub recent: usize,
    pub out: usize,
    pub runs: usize,
}

struct Rec {
    key: MemoKey,
    entry: State,
    exit: Option<(State, Value)>,
    end: Outside,
    delta: Outside,
    stmts: usize,
}

#[derive(Default)]
pub struct MemoProbe {
    last: FxHashMap<(ObjId, String), Rec>,
    pub runs: usize,
    pub run_stmts: usize,
    pub hits: usize,
    pub hit_stmts: usize,
    /// hits whose earlier run changed nothing outside State but allocations / verdicts
    pub clean: usize,
    pub clean_stmts: usize,
    /// ...and nothing outside changed between the earlier run's end and this call (but allocs/verdicts)
    pub quiet: usize,
    pub quiet_stmts: usize,
    /// ...and the exit equals the earlier run's exit
    pub exact: usize,
    pub inexact: usize,
    pub in_try: usize,
    pub in_loop: usize,
    pub tainted_rec: usize,
    pub tainted_gen: usize,
    pub tainted_raise: usize,
    pub tainted_heap: usize,
    pub tainted_mod: usize,
    pub tainted_runs: usize,
    pub between_heap: usize,
    pub between_alloc: usize,
    pub gen_ops: u64,
    pub rec_calls: u64,
    pub raise_recs: u64,
    pub outer_raise_recs: u64,
    pub raise_log: Vec<usize>,
    pub verdict_adds: usize,
    pub facts: usize,
    pub tainted_tables: usize,
    pub tainted_recent: usize,
    pub between_tables: usize,
    pub between_locs: FxHashMap<String, usize>,
    hit_depth: [usize; 3],
    pub saved: [usize; 3],
}

pub struct ProbeTok {
    slot: (ObjId, String),
    key: MemoKey,
    entry: State,
    start: Outside,
    stmts0: usize,
    hit: Option<(bool, bool, Option<(State, Value)>, usize)>,
}

impl Analyzer {
    pub(in crate::interp) fn outside(&self) -> Outside {
        Outside {
            serial: self.heap.serial(),
            heap_muts: self.heap.muts,
            verdicts: self.findings.mark(),
            rec_calls: self.memo.rec_calls,
            gen_ops: self.memo.gen_ops,
            raise_recs: self.memo.raise_recs,
            outer_raise_recs: self.memo.outer_raise_recs,
            modules: self.program.modules.len(),
            tables: self.m.size_fp() + self.heap.cls_slots.len() + self.heap.prop_setters.len() + self.heap.copy_root.len() + self.heap.keys.member_key.len() + self.heap.keys.identity_keys.borrow().len() + self.program.user_classes.len() + self.program.type_only.len(),
            recent: self.cx.recent_stack.iter().map(|r| r.len()).sum(),
            out: self.budget.out.len(),
            runs: self.budget.runs,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::interp) fn probe_begin(&mut self, foid: ObjId, site: &str, self_v: &Option<Value>, args: &[Value], kwargs: &IndexMap<StrId, Value>, star_args: &[Value], star_kwargs: &[Value], state: &State) -> ProbeTok {
        let fr = self.frame();
        let mut fresh: Vec<ObjId> = self.cx.fresh_init.iter().copied().collect();
        fresh.sort_unstable();
        let key = MemoKey {
            self_v: self_v.clone(),
            args: args.to_vec(),
            kwargs: kwargs.clone(),
            star_args: star_args.to_vec(),
            star_kwargs: star_kwargs.to_vec(),
            chain: self.cx.call_chain.clone(),
            depths: (self.cx.cond_depth, self.cx.branch_depth, self.cx.recent_stack.len(), self.cx.env_stack.len(), self.cx.model_depth, self.cx.frames.len()),
            frame: (fr.single, fr.single_nl, fr.loop_depth, fr.ctx.clone()),
            extra: self.cx.alloc_extra.clone(),
            fresh,
            comp_known: self.cx.comp_known.clone(),
            modes: (self.mode.verdict_guarded, self.mode.recv_scalar, self.mode.key_alias.is_some(), self.mode.trace.is_some(), self.unwind.catching_depth, self.unwind.caught_load, self.unwind.handling.len(), self.gens.reads.is_empty()),
            envs: self.cx.env_stack.clone(),
            frames: self.cx.frames.iter().map(|f| (f.ctx.clone(), f.cells.values().copied().collect(), f.module)).collect(),
            roots: (
                self.unwind.tries.iter().flat_map(|t| t.recs.iter().map(|r| r.2.clone())).chain(self.unwind.handling_values.iter().cloned()).collect(),
                self.cx.recent_stack.clone(),
                self.cx.comp_cells.iter().map(|c| { let mut v: Vec<(StrId, ObjId)> = c.iter().map(|(a, b)| (*a, *b)).collect(); v.sort_unstable(); v }).collect(),
                self.unwind.finally_stack.len() + self.gens.cm_frames.len() * 1000,
                self.unwind.cur_exc.clone(),
            ),
            tries: self.unwind.tries.iter().map(|t| {
                let mut v: Vec<String> = t.caught.iter().cloned().collect();
                v.sort();
                v
            }).collect(),
        };
        let slot = (foid, site.to_string());
        let now = self.outside();
        let mut hit = None;
        if let Some(r) = self.memo.last.get(&slot) {
            if r.key == key && r.entry.same(state) {
                let clean = r.delta.heap_muts == 0 && r.delta.rec_calls == 0 && r.delta.gen_ops == 0 && r.delta.modules == 0 && r.delta.tables == 0;
                let quiet = clean && now.heap_muts == r.end.heap_muts && now.gen_ops == r.end.gen_ops && now.modules == r.end.modules && now.tables == r.end.tables;
                self.memo.tainted_tables += (r.delta.tables > 0) as usize;
                self.memo.tainted_recent += (r.delta.recent > 0) as usize;
                self.memo.between_tables += (now.tables != r.end.tables) as usize;
                self.memo.between_heap += (now.heap_muts != r.end.heap_muts) as usize;
                if now.heap_muts != r.end.heap_muts {
                    let l = self.heap.last_mut.map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
                    *self.memo.between_locs.entry(l).or_default() += 1;
                }
                self.memo.between_alloc += (now.serial != r.end.serial) as usize;
                self.memo.tainted_rec += (r.delta.rec_calls > 0) as usize;
                self.memo.tainted_gen += (r.delta.gen_ops > 0) as usize;
                self.memo.tainted_raise += (r.delta.outer_raise_recs > 0) as usize;
                self.memo.tainted_heap += (r.delta.heap_muts > 0) as usize;
                self.memo.tainted_mod += (r.delta.modules > 0) as usize;
                hit = Some((clean, quiet, r.exit.as_ref().map(|(s, v)| (s.copy(), v.clone())), r.stmts));
            }
        }
        if let Some((clean, quiet, _, _)) = &hit {
            for (i, f) in [true, *clean, *quiet].into_iter().enumerate() {
                if f {
                    self.memo.hit_depth[i] += 1;
                }
            }
        }
        ProbeTok { slot, key, entry: state.copy(), start: now, stmts0: self.budget.statements, hit }
    }

    pub(in crate::interp) fn probe_end(&mut self, tok: ProbeTok, state: &State, ret: &Value) {
        let end = self.outside();
        let stmts = self.budget.statements - tok.stmts0;
        let s = tok.start;
        let delta = Outside {
            serial: end.serial - s.serial,
            heap_muts: end.heap_muts - s.heap_muts,
            verdicts: end.verdicts.saturating_sub(s.verdicts),
            rec_calls: end.rec_calls - s.rec_calls,
            gen_ops: end.gen_ops - s.gen_ops,
            raise_recs: end.raise_recs - s.raise_recs,
            outer_raise_recs: end.outer_raise_recs - s.outer_raise_recs,
            modules: end.modules - s.modules,
            tables: end.tables.abs_diff(s.tables),
            recent: end.recent.abs_diff(s.recent),
            out: end.out - s.out,
            runs: end.runs - s.runs,
        };
        let m = &mut self.memo;
        m.runs += 1;
        m.run_stmts += stmts;
        if let Some((clean, quiet, _, _)) = &tok.hit {
            for (i, f) in [true, *clean, *quiet].into_iter().enumerate() {
                if f {
                    m.hit_depth[i] -= 1;
                    if m.hit_depth[i] == 0 {
                        m.saved[i] += stmts;
                    }
                }
            }
        }
        if let Some((clean, quiet, prev_exit, _pstmts)) = tok.hit {
            m.hits += 1;
            m.hit_stmts += stmts;
            if !tok.key.tries.is_empty() {
                m.in_try += 1;
            }
            if tok.key.depths.2 > 0 || tok.key.frame.2 > 0 {
                m.in_loop += 1;
            }
            if clean {
                m.clean += 1;
                m.clean_stmts += stmts;
            }
            if quiet {
                m.quiet += 1;
                m.quiet_stmts += stmts;
                match prev_exit {
                    Some((ps, pv)) if ps.same(state) && pv == *ret => m.exact += 1,
                    _ => m.inexact += 1,
                }
            }
        }
        let tainted = delta.heap_muts > 0 || delta.rec_calls > 0 || delta.gen_ops > 0 || delta.tables > 0 || delta.modules > 0;
        m.tainted_runs += tainted as usize;
        self.memo.last.insert(tok.slot, Rec { key: tok.key, entry: tok.entry, exit: Some((state.copy(), ret.clone())), end, delta, stmts });
    }

    pub(in crate::interp) fn probe_line(&self) -> String {
        let m = &self.memo;
        format!(
            "memo probe: journal: verdict adds {} facts {} raise recs {} heap muts {}; saved {:?} between {:?}; runs {} ({} stmts), key hits {} ({} stmts), clean {} ({}), quiet {} ({}), exact {} inexact {}; hits in try {} in loop {}; prev run tainted: rec {} gen {} raise {} heap {} mod {} tables {} recent {}; tainted runs {}; between: heap {} alloc {} tables {}",
            m.verdict_adds, m.facts, m.raise_recs, self.heap.muts, m.saved, { let mut v: Vec<_> = m.between_locs.iter().collect(); v.sort_by_key(|x| std::cmp::Reverse(*x.1)); v.truncate(8); v }, m.runs, m.run_stmts, m.hits, m.hit_stmts, m.clean, m.clean_stmts, m.quiet, m.quiet_stmts, m.exact, m.inexact, m.in_try, m.in_loop, m.tainted_rec, m.tainted_gen, m.tainted_raise, m.tainted_heap, m.tainted_mod, m.tainted_tables, m.tainted_recent, m.tainted_runs, m.between_heap, m.between_alloc, m.between_tables
        )
    }
}

