//! Followed libraries (`--follow`): installed packages' source from site-packages (`main.rs`
//! `site_packages_for`; not the packages rowan has models of, `has_module_models`), run as the
//! program's code is, each function code within its budget (past it, its calls are calls that
//! do not run, `skipped_call`). Each entry of the program's code into them (an import, a call)
//! also has an allowance of work (`fanout_limit`): past it, the entry's calls of code (the
//! library's, and the program's it calls back) do not run either, while its module code goes on
//! (an import loads its modules: names known, expensive functions unknown). It bounds what one
//! entry fans out to (a framework's call reaches hundreds of functions, each with a budget).
//!
//! **The library's share.** All entries together get their own share of work and time
//! (`LIB_SHARE` allowances, half the deadline), apart from the program's: the time spent in
//! them does not count against the program's deadline (`Budget::over_hard_deadline`), and the
//! program's own code keeps its budgets. When the share runs out inside an entry, the entry
//! stops at once (every statement under it, `cut_body`; every call, `call_user`) and is undone:
//! the state as before it, the modules it loaded forgotten, and it is what it is without
//! following: an import an unknown module, a call one that does not run (`skipped_call`). From
//! then on no library code runs: imports of library modules not loaded are unknown modules,
//! calls into loaded ones do not run.

use super::Analyzer;
use crate::domain::{label_type, typed_label, KeyVal, Value};
use crate::intern;
use crate::state::State;

/// The work all entries into followed libraries may do in all, in entry allowances
/// (`fanout_limit`); their time, at most half the deadline.
const LIB_SHARE: usize = 64;

impl Analyzer {
    /// Run `body` as one entry of the program's code into a followed library's code, within its
    /// allowance (`Budget::past_library_work`) and the library's share: None when the share
    /// ran out (then, or before), the state as before it.
    pub(super) fn library_entry<T>(&mut self, what: impl Fn() -> String, state: &mut State, body: impl FnOnce(&mut Analyzer, &mut State) -> T) -> Option<T> {
        if self.budget.lib_entry.is_some() {
            return Some(body(self, state));
        }
        if self.budget.lib_out {
            return None;
        }
        self.budget.lib_counts[0] += 1;
        let (w0, s0) = (self.library_work(), self.share_work());
        let before = state.copy();
        let mark = self.program.mark();
        self.budget.lib_entry = Some(w0);
        self.budget.lib_share0 = s0;
        self.budget.lib_since = Some(std::time::Instant::now());
        let r = body(self, state);
        self.budget.lib_entry = None;
        if let Some(t) = self.budget.lib_since.take() {
            self.budget.lib_time += t.elapsed();
        }
        self.budget.lib_work += self.library_work() - w0;
        self.budget.lib_share += self.share_work() - s0;
        let cut = self.budget.lib_cut.take();
        let out = self.budget.lib_abort.take();
        if super::debug_enabled() {
            eprintln!("library entry {}: work {}{}", what(), self.library_work() - w0, if out { ", undone: the library's share ran out" } else if cut { ", cut" } else { "" });
        }
        if out {
            self.budget.lib_out = true;
            state.replace_with(before);
            self.program.forget_since(mark);
            self.budget.lib_counts[1] += 1;
            self.findings.note("installed packages used up their share of the analysis: later calls into them are unknown code, packages not loaded by then are not followed".to_string());
            return None;
        }
        if cut {
            self.budget.lib_counts[1] += 1;
            self.findings.note("a call into or import of an installed package needs more work than rowan gives it: some of its calls are unknown code".to_string());
        }
        Some(r)
    }

    /// Inside an entry into a followed library: has the library's share run out (or the
    /// analysis been cancelled)? Then the entry stops (`cut_body`) and is undone.
    pub(super) fn library_share_spent(&self) -> bool {
        if self.budget.lib_entry.is_none() {
            return false;
        }
        if self.budget.lib_abort.get() {
            return true;
        }
        let ms = super::limit("ms", super::DEADLINE_MS) as u128 / 2;
        let work = self.budget.lib_share + self.share_work() - self.budget.lib_share0;
        let spent = self.budget.library_ms() > ms || work > LIB_SHARE.saturating_mul(self.budget.fanout_limit()) || self.budget.cancelled();
        if spent {
            self.budget.lib_abort.set(true);
        }
        spent
    }

    /// The work done so far, as an entry's allowance counts it: statements, objects, havoc and
    /// reachability visits, and row joins (a library's merges of large values cost in joins).
    /// Not definitions (functions, classes, modules): the statement making one pays for it, and
    /// an import makes one per `def` of the code it loads.
    pub(super) fn library_work(&self) -> usize {
        self.hover_work() - self.heap.defs + crate::domain::row_joins() + crate::state::fp_visits() / 16
    }

    /// The work the library's share counts: `library_work` and the objects reachability and
    /// closure walks visit (16 to a unit), what calls that do not run cost.
    fn share_work(&self) -> usize {
        self.library_work() + self.budget.walked.get() / 16
    }

    /// The result of a call into a library (an entry): an unknown scalar is named by the call as
    /// written (`parser.parse(text)`), as a call rowan does not follow names its result.
    pub(super) fn named_by_call(&self, r: Value, node: &crate::frontend::Node) -> Value {
        let (true, KeyVal::Data(l, _)) = (r.pts.is_empty(), &r.kv) else { return r };
        let l = intern::str_get(*l);
        let text = crate::frontend::text_at(node).split_whitespace().collect::<Vec<_>>().join(" ");
        if l.starts_with('\u{1}') || text.is_empty() || text.len() > 60 {
            return r;
        }
        let name = format!("expr:{}:{}", self.frame().func.unwrap_or(0), text);
        Value::data(&match label_type(&l) {
            Some(t) => typed_label(t, &name),
            None => name,
        })
    }

    /// `import name` of a followed library from the program's code: one entry
    /// (`library_entry`); past the library's share, a module rowan does not read.
    pub(super) fn import_library(&mut self, name: &str, node: &crate::frontend::Node, state: &mut State) -> Value {
        if let Some(v) = self.library_entry(|| format!("import {}", name), state, |an, st| an.import_module(name, node, st)) {
            return v;
        }
        // (a submodule of a package loaded before: its attribute too, as an import makes it)
        let v = self.external_module(name);
        if let Some(&po) = name.rsplit_once('.').and_then(|(parent, _)| self.program.modules.get(parent)) {
            state.set_slot(po, crate::domain::Ns::Attr, intern::key_str_lit(name.rsplit('.').next().unwrap_or(name)), v.clone(), true);
        }
        v
    }
}
