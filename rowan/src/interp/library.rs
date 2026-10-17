//! Followed libraries (`--follow`): installed packages' source from site-packages, run as the
//! program's code is, but each entry of the program's code into them (an import, a call) runs
//! within an allowance of work, or not at all. An entry past its allowance stops and is undone:
//! the state is as before it, and the entry is what it is without following (an unknown module,
//! a call that does not run). Past `LIB_TOTAL` for all entries, entries are not tried. A
//! library's code has no per-function budget: its entries bound its work.

use super::Analyzer;
use crate::domain::{label_type, typed_label, KeyVal, Value};
use crate::intern;
use crate::state::State;

impl Analyzer {
    /// Run `body` as one entry of the program's code into a followed library's code: within its
    /// allowance it is the result; past it (or past all entries' allowance) the state is as
    /// before, the modules and classes it made are forgotten, and it is None.
    pub(super) fn library_entry<T>(&mut self, state: &mut State, body: impl FnOnce(&mut Analyzer, &mut State) -> T) -> Option<T> {
        if self.budget.lib_entry.is_some() {
            return Some(body(self, state));
        }
        if !self.budget.library_open() {
            self.findings.note("installed packages rowan follows ran out of their work: later calls into them are unknown code".to_string());
            self.budget.lib_counts[2] += 1;
            return None;
        }
        self.budget.lib_counts[0] += 1;
        let before = state.copy();
        let (modules, classes) = (self.program.modules.len(), self.program.user_classes.len());
        let w0 = self.library_work();
        self.budget.lib_entry = Some(w0);
        let r = body(self, state);
        self.budget.lib_entry = None;
        self.budget.lib_work += self.library_work() - w0;
        if !std::mem::take(&mut self.budget.lib_abort) {
            return Some(r);
        }
        self.budget.lib_counts[1] += 1;
        *state = before;
        self.program.forget_modules_since(modules);
        self.program.user_classes.truncate(classes);
        self.findings.note("a call into or import of an installed package needs more work than rowan gives it: it is unknown code".to_string());
        None
    }

    /// The work done so far, as an entry's allowance counts it: statements, objects, havoc and
    /// reachability visits, and row joins (a library's merges of large values cost in joins).
    pub(super) fn library_work(&self) -> usize {
        self.hover_work() + crate::domain::row_joins() + crate::state::fp_visits() / 16
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
    /// (`library_entry`); undone, the module is unknown (and so is its parent's attribute).
    pub(super) fn import_library(&mut self, name: &str, node: &crate::frontend::Node, state: &mut State) -> Value {
        if let Some(v) = self.library_entry(state, |an, st| an.import_module(name, node, st)) {
            return v;
        }
        let ext = self.external_module(name);
        if let Some((parent, leaf)) = name.rsplit_once('.') {
            if let Some(&po) = self.program.modules.get(parent) {
                state.set_slot(po, crate::domain::Ns::Attr, crate::intern::key_str_lit(leaf), ext.clone(), true);
            }
        }
        ext
    }
}
