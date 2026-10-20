//! The bounds on work: each function code may run `FN_BUDGET` statements in all its calls
//! (past them its calls are unknown calls, `skipped_call`), and a safety-net deadline
//! (`DEADLINE_MS`) past which calls of user code are unknown calls and running bodies stop.
//! Also the deterministic work counters (`ROWAN_DEBUG=1`'s `work:` line, the golden `#@work`
//! and `#@tabulations` bounds).
//!
//! For a call memo: `spent` and `out` are flow-insensitive state a run changes (a hit would
//! have to charge the statements its run charged); the counters only measure.

use rustc_hash::{FxHashMap, FxHashSet};

use super::{limit, DEADLINE_MS, FN_BUDGET};
use crate::domain::ObjId;

/// The work one call may fan out to before the calls under it do not run, in function budgets
/// (`ROWAN_LIMITS=fanout=N`): a function's run on demand, an entry into a followed library.
const FANOUT: usize = 4;

/// When the analysis started (the deadline counts from it).
#[derive(Clone, Copy)]
pub struct Clock(std::time::Instant);

impl Default for Clock {
    fn default() -> Self {
        Clock(std::time::Instant::now())
    }
}

impl Clock {
    pub fn elapsed(&self) -> std::time::Duration {
        self.0.elapsed()
    }
}

/// The analysis's work bounds and counters (see the module doc).
#[derive(Default)]
pub struct Budget {
    /// statements run per function code (`FuncNode::addr`: every closure object made from one
    /// `def` shares the budget)
    spent: FxHashMap<usize, usize>,
    /// functions some call of which their budget refused: their own lines then need a reporting
    /// run with open parameters (it covers the calls that did not run)
    pub out: FxHashSet<ObjId>,
    /// every body run of a call (the golden `#@tabulations <= N` bound)
    pub runs: usize,
    /// statements executed (debug output, the `#@work` bound)
    pub statements: usize,
    /// objects visited by reachability walks (liveness, GC): part of the work measure
    pub walked: std::cell::Cell<usize>,
    /// when the analysis started (see `DEADLINE_MS`)
    started: Clock,
    /// the hard deadline passed: function bodies stop, the calls they are in become unknown calls
    pub hard_cut: bool,
    /// set from outside (`rowan serve`: the file changed, the request timed out): the
    /// analysis is stale, its deadline has passed
    pub cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    /// a function's run on demand: the work done when it began (`Analyzer::hover_work`), and
    /// the frames it runs in (its own calls run; past `past_hover_work`, its callees' calls are
    /// unknown)
    pub hover: Option<(usize, usize)>,
    /// on demand, a callee's call was past `past_hover_work` (the run's note says so)
    pub hover_cut: std::cell::Cell<bool>,
    /// calls run no body (`skips_calls`): the main module's `if __name__ == "__main__":` block
    /// while the module code is set up for runs on demand (`Analyzer::main_block_unrun`)
    pub skip_calls: bool,
    /// the program's code entered a followed library's (`--follow`): the work done then
    pub lib_entry: Option<usize>,
    /// the work done in followed libraries' code, entries that ended
    pub lib_work: usize,
    /// the running entry went past its allowance: its later calls did not run (`library_entry`)
    pub lib_cut: std::cell::Cell<bool>,
    /// calls that did not run (`skipped_call`), all along
    pub skipped: usize,
    /// entries into followed libraries: run, cut (the `ROWAN_DEBUG` `loaded:` line)
    pub lib_counts: [usize; 2],
    /// `ROWAN_DEBUG` only: per function code, its name, body runs, statements of its own, and
    /// the work units and microseconds of its outermost runs (callees included; `tally_lines`)
    pub tally: FxHashMap<usize, (String, [usize; 4])>,
    /// `ROWAN_DEBUG` only: the function codes running, how many times each
    pub tally_active: FxHashMap<usize, usize>,
}

impl Budget {
    /// A statement runs in function code `code` (0: module or class code, not budgeted).
    pub fn charge(&mut self, code: usize) {
        self.statements += 1;
        if code != 0 {
            *self.spent.entry(code).or_default() += 1;
            if let Some((_, t)) = self.tally.get_mut(&code) {
                t[1] += 1;
            }
        }
    }

    /// `ROWAN_DEBUG` only: a body run of function code `code` (named `name`) begins; it returns
    /// what `tally_end` needs.
    pub fn tally_begin(&mut self, code: usize, name: impl FnOnce() -> String) -> (usize, std::time::Instant) {
        let e = self.tally.entry(code).or_insert_with(|| (name(), [0; 4]));
        e.1[0] += 1;
        *self.tally_active.entry(code).or_default() += 1;
        (self.work_units(), std::time::Instant::now())
    }

    /// `ROWAN_DEBUG` only: the body run `tally_begin` began ended (an outermost one adds its work).
    pub fn tally_end(&mut self, code: usize, (w0, t0): (usize, std::time::Instant)) {
        let n = self.tally_active.get_mut(&code).unwrap();
        *n -= 1;
        if *n == 0 {
            let (w, us) = (self.work_units() - w0, t0.elapsed().as_micros() as usize);
            let t = &mut self.tally.get_mut(&code).unwrap().1;
            t[2] += w;
            t[3] += us;
        }
    }

    /// `ROWAN_DEBUG` only: the function codes that did the most work, by their own statements
    /// and by their outermost runs' time.
    pub fn tally_lines(&self, n: usize) -> Vec<String> {
        let mut v: Vec<&(String, [usize; 4])> = self.tally.values().collect();
        let mut out = vec![format!("functions by statements of their own (runs, statements, units incl., ms incl.):")];
        v.sort_by_key(|(_, t)| std::cmp::Reverse(t[1]));
        out.extend(v.iter().take(n).map(|(q, t)| format!("  {:>7} {:>8} {:>9} {:>7} {}", t[0], t[1], t[2], t[3] / 1000, q)));
        out.push("functions by time of their outermost runs:".to_string());
        v.sort_by_key(|(_, t)| std::cmp::Reverse(t[3]));
        out.extend(v.iter().take(n).map(|(q, t)| format!("  {:>7} {:>8} {:>9} {:>7} {}", t[0], t[1], t[2], t[3] / 1000, q)));
        out
    }

    /// Has function code `code` run past its budget?
    pub fn exhausted(&self, code: usize) -> bool {
        self.spent.get(&code).copied().unwrap_or(0) > limit("budget", FN_BUDGET)
    }

    /// The budgets spent so far (a generator run again from here gets them back: `restore`).
    pub fn snapshot(&self) -> std::rc::Rc<FxHashMap<usize, usize>> {
        std::rc::Rc::new(self.spent.clone())
    }

    /// The budgets spent as at a `snapshot`.
    pub fn restore(&mut self, spent: &FxHashMap<usize, usize>) {
        self.spent = spent.clone();
    }

    /// Statements run by budgeted code in all.
    pub fn spent_total(&self) -> usize {
        self.spent.values().sum()
    }

    /// The work done so far: statements run and objects visited by reachability walks (16 to a
    /// unit): a deterministic cost measure (`ROWAN_DEBUG=1`, the golden `#@work` bound).
    pub fn work_units(&self) -> usize {
        self.statements + self.walked.get() / 16
    }

    /// One more object visited by a reachability walk.
    pub fn walk_visit(&self) {
        self.walked.set(self.walked.get() + 1);
    }

    pub fn now(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }

    /// Milliseconds since the analysis (or the run on demand) started.
    pub fn elapsed_ms(&self) -> u128 {
        self.started.elapsed().as_millis()
    }

    /// Past the deadline (`DEADLINE_MS`): calls of user code are unknown code from there on.
    pub fn over_hard_deadline(&self) -> bool {
        self.started.elapsed().as_millis() as usize > limit("ms", DEADLINE_MS) || self.cancel.as_ref().is_some_and(|c| c.load(std::sync::atomic::Ordering::Relaxed))
    }

    /// A fresh budget for one function's run on demand (`general_run_of`): no statements
    /// spent, no calls refused, the deadline counted from now (the counters go on).
    pub fn fresh(&mut self) {
        self.spent.clear();
        self.out.clear();
        self.started = Clock::default();
        self.hard_cut = false;
        self.hover_cut.set(false);
        self.lib_work = 0;
    }

    /// A call made with `frames` frames running, on demand, the work done so far `work`: is it
    /// one of a callee's, past the work a function's run on demand may do (`fanout_limit`)?
    pub fn past_hover_work(&self, frames: usize, work: usize) -> bool {
        let past = self.hover.is_some_and(|(w0, depth)| frames > depth && work - w0 > Self::fanout_limit());
        if past {
            self.hover_cut.set(true);
        }
        self.skip_calls || past
    }

    /// The work a function's run on demand, or an entry into a followed library, may do before
    /// the calls under it are unknown: `FANOUT` budgets.
    pub fn fanout_limit() -> usize {
        limit("fanout", FANOUT).saturating_mul(limit("budget", FN_BUDGET))
    }

    /// A call of user code within an entry into a followed library, with the work done so far
    /// `work`: past the work one entry may do (`fanout_limit`), it does not run (`lib_cut`).
    pub fn past_library_work(&self, work: usize) -> bool {
        let past = self.lib_entry.is_some_and(|w0| work.saturating_sub(w0) > Self::fanout_limit());
        if past {
            self.lib_cut.set(true);
        }
        past
    }

    /// Has the deadline cut the analysis? Checked at every statement (`cut_body`): one statement
    /// may take long (a loop's round over values of many objects).
    pub fn tick(&mut self) -> bool {
        if !self.hard_cut && self.over_hard_deadline() {
            self.hard_cut = true;
        }
        self.hard_cut
    }
}
