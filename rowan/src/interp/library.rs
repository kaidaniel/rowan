//! Followed libraries (`--follow`): installed packages' source from site-packages, run as the
//! program's code is, each function code within its budget (past it, its calls are calls that
//! do not run, `skipped_call`). Each entry of the program's code into them (an import, a call)
//! also has an allowance of work (`LIB_ENTRY`): past it, the entry's calls of code (the
//! library's, and the program's it calls back) do not run either, while its module code goes on
//! (an import loads its modules: names known, expensive functions unknown). It bounds what one
//! entry fans out to (a framework's call reaches hundreds of functions, each with a budget).

use super::Analyzer;
use crate::domain::{label_type, typed_label, KeyVal, Value};
use crate::intern;
use crate::state::State;

impl Analyzer {
    /// Run `body` as one entry of the program's code into a followed library's code, within its
    /// allowance (`Budget::past_library_work`).
    pub(super) fn library_entry<T>(&mut self, what: impl Fn() -> String, state: &mut State, body: impl FnOnce(&mut Analyzer, &mut State) -> T) -> T {
        if self.budget.lib_entry.is_some() {
            return body(self, state);
        }
        self.budget.lib_counts[0] += 1;
        let w0 = self.library_work();
        self.budget.lib_entry = Some(w0);
        let r = body(self, state);
        self.budget.lib_entry = None;
        self.budget.lib_work += self.library_work() - w0;
        let cut = self.budget.lib_cut.take();
        if super::debug_enabled() {
            eprintln!("library entry {}: work {}{}", what(), self.library_work() - w0, if cut { ", cut" } else { "" });
        }
        if cut {
            self.budget.lib_counts[1] += 1;
            self.findings.note("a call into or import of an installed package needs more work than rowan gives it: some of its calls are unknown code".to_string());
        }
        r
    }

    /// The work done so far, as an entry's allowance counts it: statements, objects, havoc and
    /// reachability visits, and row joins (a library's merges of large values cost in joins).
    /// Not definitions (functions, classes, modules): the statement making one pays for it, and
    /// an import makes one per `def` of the code it loads.
    pub(super) fn library_work(&self) -> usize {
        self.hover_work() - self.heap.defs + crate::domain::row_joins() + crate::state::fp_visits() / 16
    }

    /// The result of a call into a library (an entry): an unknown scalar is named by the call as
    /// written (`parser.parse(text)`), as a call rowan does not follow names its result.
    pub(super) fn named_by_call(&self, r: Value, node: &crate::frontend::Node) -> Value {
        let (true, KeyVal::Data(l)) = (r.pts.is_empty(), &r.kv) else { return r };
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
    /// (`library_entry`).
    pub(super) fn import_library(&mut self, name: &str, node: &crate::frontend::Node, state: &mut State) -> Value {
        self.library_entry(|| format!("import {}", name), state, |an, st| an.import_module(name, node, st))
    }
}
