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
    cut_tick: u32,
}

impl Budget {
    /// A statement runs in function code `code` (0: module or class code, not budgeted).
    pub fn charge(&mut self, code: usize) {
        self.statements += 1;
        if code != 0 {
            *self.spent.entry(code).or_default() += 1;
        }
    }

    /// Has function code `code` run past its budget?
    pub fn exhausted(&self, code: usize) -> bool {
        self.spent.get(&code).copied().unwrap_or(0) > limit("budget", FN_BUDGET)
    }

    /// The budgets spent so far (a generator run again from here gets them back: `restore`).
    pub fn snapshot(&self) -> std::rc::Rc<FxHashMap<usize, usize>> {
        std::rc::Rc::new(self.spent.clone())
    }

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

    pub fn elapsed_ms(&self) -> u128 {
        self.started.elapsed().as_millis()
    }

    /// Past the deadline (`DEADLINE_MS`): calls of user code are unknown code from there on.
    pub fn over_hard_deadline(&self) -> bool {
        self.started.elapsed().as_millis() as usize > limit("ms", DEADLINE_MS)
    }

    /// Has the deadline cut the analysis? Checked every 64 statements (`cut_body`).
    pub fn tick(&mut self) -> bool {
        if !self.hard_cut {
            self.cut_tick = self.cut_tick.wrapping_add(1);
            if self.cut_tick % 64 != 0 || !self.over_hard_deadline() {
                return false;
            }
            self.hard_cut = true;
        }
        true
    }
}
