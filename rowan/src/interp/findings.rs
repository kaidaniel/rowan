//! What the analysis finds, for the report, the golden harness and the hovers: the access
//! verdicts (deduplicated, dropped again for runs a later run supersedes), notes, per-function
//! result summaries, `#@row` checks and the hover facts. `Findings` only collects: the
//! `Analyzer` methods here (`verdict`, `record_fact`, `after_stmt`) decide what to record.
//!
//! For a call memo: everything here is output a run *appends* (and a superseded round drops,
//! `drop_since`); a memo hit would replay the verdicts, notes, facts and row checks its run
//! appended (the `quiet` flags are per-run modes, restored by the code that sets them).

use indexmap::IndexMap;
use rustc_hash::{FxHashMap, FxHashSet};
use ruff_python_ast as ast;

use super::{Analyzer, Payload, ViewKind};
use crate::domain::{Chain, Classification, Ns, ObjId, Row, Tier, Value};
use crate::frontend::{self, Node};
use crate::intern::{self, key_str_lit, s, KeyId, StrId};
use crate::state::{Kind, State};


#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Verdict {
    pub line: u32,
    pub col: u32,
    pub key: KeyId,
    pub tier: Tier,
    pub obj: String,
    /// call sites (lines) active when the access ran
    pub chain: Chain,
    pub note: String,
    pub providers: Vec<crate::domain::SourceId>,
    /// unknown code that may have added or removed the key (its call sites)
    pub mutators: Vec<String>,
    pub ns: Ns,
    pub text: String,
    pub oid: ObjId,
    /// module the access is in (empty = main file)
    pub file: String,
    /// inside a try whose handler catches the failure: the program handles it (no diagnostic)
    pub guarded: bool,
}

impl Verdict {
    pub(in crate::interp) fn dedupe_key(&self) -> (String, u32, u32, String, Tier, ObjId, Chain) {
        (self.file.clone(), self.line, self.col, intern::key_repr(self.key), self.tier, self.oid, self.chain.clone())
    }
}

/// Facts about one name at one line of the main module, joined over every execution of
/// the statement (hover support): the value it held and the rows of the objects it pointed to.
#[derive(Clone)]
pub struct Fact {
    /// object -> (item row, attr row) as they were when the name was read / written
    pub rows: IndexMap<ObjId, (Option<Row>, Option<Row>)>,
    /// the value rendered (`report::render_value`) at each recording, distinct texts
    pub texts: Vec<String>,
    /// the symbols the texts mention, with what is known about each (`report::value_legend`)
    pub legend: Vec<(String, String)>,
    /// display names of the tensor dims the texts mention (`B` for x.shape[0])
    pub t_names: FxHashMap<crate::models::shape::AtomId, String>,
    /// the text from the function's own analysis (open parameters), when there is one: the
    /// hover leads with it; `texts` then holds what calls gave
    pub primary: Option<String>,
    /// recorded by an assignment (whose value the hover shows, not the reads on the line)
    pub written: bool,
}

pub struct RowCheck {
    /// the module's path relative to the main file's directory ("" for the main file)
    pub file: String,
    pub line: u32,
    pub name: String,
    pub row: Option<Row>,
    pub ns: Ns,
    /// the name's value: its known literals, whether it may be anything else, its objects
    pub value: ValueInfo,
    /// tensor facts for the golden harness: {"shape", "dim", "chain", "entry"}
    pub tensor: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default)]
pub struct ValueInfo {
    pub lits: Vec<KeyId>,
    pub unknown: bool,
    pub objects: Vec<String>,
}


/// The findings collected so far (see the module doc).
#[derive(Default)]
pub struct Findings {
    /// the access verdicts in the order recorded, without repeats (`keys`)
    verdicts: Vec<Verdict>,
    keys: FxHashSet<(String, u32, u32, String, Tier, ObjId, Chain)>,
    notes: Vec<String>,
    notes_set: FxHashSet<String>,
    /// per (function, call context or "entry"): the last run's result and its records' rows
    pub summaries: IndexMap<(String, String), (Value, Vec<(ObjId, Row)>)>,
    /// `#@row NAME:` checks requested by the golden harness: (file relative to the main
    /// file's directory, "" for the main file; line; name)
    pub row_check_requests: Vec<(String, u32, String)>,
    /// keys a check names as absent (`absent={k}`): an open row lists only the absent keys it
    /// has seen; the check also asks whether reading k would surely fail there
    pub row_absent_probes: Vec<(String, u32, String, KeyId)>,
    pub row_checks: Vec<RowCheck>,
    /// golden rows: the joined value of (line, name, call chain, entry run) so far
    pub t_row_values: FxHashMap<(u32, String, Vec<u32>, bool), Value>,
    /// collect `facts` (the `--facts` mode)
    pub facts_on: bool,
    pub facts: IndexMap<(u32, StrId), Fact>,
    /// the next hover fact is an assignment's
    pub t_fact_write: bool,
    /// a requirement-only entry run (`param F.x` schemas of a called function): no verdicts,
    /// row checks, facts or notes
    pub quiet: bool,
    /// no record-access verdicts (shapes still reported): the own run of a called function
    /// of a torch / numpy module
    pub quiet_records: bool,
    /// the state the main module ended in (the report's rows)
    pub final_state: Option<State>,
}

impl Findings {
    pub(in crate::interp) fn verdicts(&self) -> &[Verdict] {
        &self.verdicts
    }

    /// A mark to drop the verdicts recorded after it (`drop_since`).
    pub(in crate::interp) fn mark(&self) -> usize {
        self.verdicts.len()
    }

    /// Record the verdict `make` builds, unless one with the same key is recorded.
    pub(in crate::interp) fn add(&mut self, key: (String, u32, u32, String, Tier, ObjId, Chain), make: impl FnOnce() -> Verdict) {
        if self.keys.contains(&key) {
            return;
        }
        self.keys.insert(key);
        self.verdicts.push(make());
    }

    /// Forget the verdicts recorded since `mark` (a run whose states a later run supersedes).
    pub(in crate::interp) fn drop_since(&mut self, mark: usize) {
        for v in self.verdicts.drain(mark..) {
            self.keys.remove(&v.dedupe_key());
        }
    }

    pub(in crate::interp) fn notes(&self) -> &[String] {
        &self.notes
    }

    /// A note for the report (once each; none in a quiet run).
    pub(in crate::interp) fn note(&mut self, msg: String) {
        if self.quiet {
            return;
        }
        if !self.notes_set.contains(&msg) {
            self.notes_set.insert(msg.clone());
            self.notes.push(msg);
        }
    }
}

impl Analyzer {
    pub fn verdict(&mut self, node: &Node, key: KeyId, cls: &Classification, oid: ObjId, ns: Ns, file_override: Option<&str>, text_override: Option<&str>) {
        self.gens.read(oid, Some((ns, key)));
        if matches!(cls.tier, Tier::Map) || self.findings.quiet || self.findings.quiet_records {
            return;
        }
        let weakened;
        let cls = if self.mode.recv_scalar && cls.tier == Tier::Missing {
            weakened = Classification::with_note(Tier::Possibly, format!("{}, unless the value is a plain one such as a str", cls.note));
            &weakened
        } else {
            cls
        };
        // a key written as `True` that reached the dict's `1`: reported as written
        let key = match self.mode.key_alias {
            Some((c, o)) if c == key => o,
            _ => key,
        };
        let text = match text_override {
            Some(t) if !t.is_empty() => t.to_string(),
            _ => self.node_text(node),
        };
        let fname = match file_override {
            Some(f) => f.to_string(),
            None => self.file_of_frame(),
        };
        let chain = self.cx.call_chain.clone();
        let dk = (fname.clone(), node.line, node.col, intern::key_repr(key), cls.tier, oid, chain.clone());
        let (obj, guarded) = (self.heap.obj(oid).label.to_string(), self.mode.verdict_guarded);
        self.findings.add(dk, || Verdict {
            line: node.line,
            col: node.col,
            key,
            tier: cls.tier,
            obj,
            chain,
            note: cls.note.clone(),
            providers: cls.providers.iter().map(|(s, _)| *s).collect(),
            mutators: { let mut m: Vec<String> = cls.mutators.iter().map(|m| intern::str_get(*m).to_string()).collect(); m.sort(); m },
            ns,
            text,
            oid,
            file: fname,
            guarded,
        });
    }

    /// A verdict on an access the program guards (a try handling the failure): recorded for
    /// the golden harness and the hover, never a diagnostic.
    pub fn verdict_guarded(&mut self, node: &Node, key: KeyId, cls: &Classification, oid: ObjId, ns: Ns) {
        let saved = std::mem::replace(&mut self.mode.verdict_guarded, true);
        self.verdict(node, key, cls, oid, ns, None, None);
        self.mode.verdict_guarded = saved;
    }

    pub(in crate::interp) fn node_text(&self, node: &Node) -> String {
        if node.addr == 0 {
            return String::new();
        }
        // (an expression written over several lines, on one: the report's column and a hover
        // keep their layout)
        let t = frontend::text_at(node);
        if t.contains('\n') { t.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ") } else { t }
    }

    pub(in crate::interp) fn record_fact(&mut self, node: &Node, name: StrId, v: &Value, state: &State) {
        if !self.findings.facts_on || node.addr == 0 || self.findings.quiet {
            return;
        }
        if node.module != self.heap.module_info(self.program.main_module).idx {
            return;
        }
        let mut rows: Vec<(ObjId, Option<Row>, Option<Row>)> = Vec::new();
        for oid in v.pts.iter() {
            let k = self.heap.kind(oid);
            let item = if matches!(k, Kind::Dict | Kind::Frame | Kind::Unknown | Kind::Namespace | Kind::Module | Kind::Class) || state.has_row(oid, Ns::Item) { Some(state.row(oid, Ns::Item)) } else { None };
            let attr = if matches!(k, Kind::Instance | Kind::Namespace | Kind::Unknown | Kind::Module | Kind::Class) || state.has_row(oid, Ns::Attr) { Some(state.row(oid, Ns::Attr)) } else { None };
            rows.push((oid, item, attr));
        }
        let join_opt = |a: &Option<Row>, b: &Option<Row>| -> Option<Row> {
            match (a, b) {
                (Some(x), Some(y)) => Some(x.join(y)),
                (Some(x), None) | (None, Some(x)) => Some(x.clone()),
                _ => None,
            }
        };
        let scope = self.frame().func.unwrap_or(0);
        // (a fact that has all the texts it shows (4) only joins the value and rows: rendering a
        // large value at every run of the line is what costs — not in torch programs, whose
        // hover is the assignment's own value)
        if !self.m.torch_on {
            if let Some(e) = self.findings.facts.get_mut(&(node.line, name)) {
                if e.texts.len() >= 4 {
                    for (oid, item, attr) in rows {
                        match e.rows.get_mut(&oid) {
                            Some((ci, ca)) => {
                                *ci = join_opt(ci, &item);
                                *ca = join_opt(ca, &attr);
                            }
                            None => {
                                e.rows.insert(oid, (item, attr));
                            }
                        }
                    }
                    return;
                }
            }
        }
        // tensors: dims named by locals / letters, printed in the function's own terms
        let (pctx, tnames, tlegend) = if self.m.torch_on {
            let mut ctx = crate::models::torch::t_print_ctx(self, state);
            let (names, legend) = crate::models::shape::with_ctx(ctx.clone(), || crate::models::torch::t_hover_names(self, v, state));
            ctx.atom_names = names.clone();
            (Some(ctx), names, legend)
        } else {
            (None, FxHashMap::default(), Vec::new())
        };
        let render = |an: &Analyzer| crate::report::in_scope(scope, || (crate::report::render_value(an, state, v, 0, &mut Vec::new()), crate::report::value_legend(an, state, v)));
        let (text, mut legend) = match pctx {
            Some(ctx) => crate::models::shape::with_ctx(ctx, || render(self)),
            None => render(self),
        };
        legend.extend(tlegend);
        let writing = std::mem::take(&mut self.findings.t_fact_write);
        let e = self.findings.facts.entry((node.line, name)).or_insert_with(|| Fact { rows: IndexMap::new(), texts: Vec::new(), legend: Vec::new(), t_names: FxHashMap::default(), primary: None, written: false });
        // in torch programs an assignment's value is what the hover shows (`y = f(y)`)
        if self.m.torch_on {
            if writing && !e.written {
                e.written = true;
                e.texts.clear();
                e.legend.clear();
                e.primary = None;
            } else if !writing && e.written {
                return;
            }
        }
        // the function analysed on its own (not a call from elsewhere): the general answer
        let own = self.entry.run.is_some() && self.cx.call_chain.len() <= 1 && self.m.torch_on;
        if own {
            e.t_names = tnames;
            e.primary = Some(match &e.primary {
                Some(p) if *p != text && !p.split(" | ").any(|x| x == text) => format!("{} | {}", p, text),
                Some(p) => p.clone(),
                None => text.clone(),
            });
        } else {
            for (a, n) in tnames {
                e.t_names.entry(a).or_insert(n);
            }
        }
        if !e.texts.contains(&text) && e.texts.len() < 4 {
            e.texts.push(text);
        }
        for (sym, desc) in legend {
            match e.legend.iter_mut().find(|(s0, _)| *s0 == sym) {
                Some((_, d)) if *d != desc && !d.split(" | ").any(|p| p == desc) => *d = format!("{} | {}", d, desc),
                Some(_) => {}
                None => e.legend.push((sym, desc)),
            }
        }
        for (oid, item, attr) in rows {
            match e.rows.get_mut(&oid) {
                Some((ci, ca)) => {
                    *ci = join_opt(ci, &item);
                    *ca = join_opt(ca, &attr);
                }
                None => {
                    e.rows.insert(oid, (item, attr));
                }
            }
        }
    }

    /// What a value is, for the golden harness's `#@value` checks: literals it may be, whether
    /// it may also be something unknown, and the (known) objects it may be.
    pub(in crate::interp) fn value_info(&self, v: &Value, state: &State) -> ValueInfo {
        let mut info = ValueInfo::default();
        if v.kv.is_literal() {
            info.lits = crate::domain::kv_literals(&v.kv).iter().collect();
        } else if !v.kv.is_bot() {
            info.unknown = true;
        }
        for o in v.pts.iter() {
            match self.heap.kind(o) {
                Kind::Unknown | Kind::External => info.unknown = true,
                // a tuple of known literals is that literal (as Python prints it)
                Kind::Tuple | Kind::Pair if self.heap.literal_tuple(o, state, 0).is_some() => info.lits.push(self.heap.literal_tuple(o, state, 0).unwrap()),
                _ => info.objects.push(self.heap.obj(o).label.to_string()),
            }
        }
        if v.pts.is_empty() && v.kv.is_bot() {
            info.unknown = true;
        }
        info
    }

    /// Row-check support for the golden harness: after a statement at a requested line.
    pub(in crate::interp) fn after_stmt(&mut self, stmt: &ast::Stmt, state: &mut State) {
        if self.findings.row_check_requests.is_empty() || self.findings.quiet {
            return;
        }
        let n = frontend::node(stmt);
        let file = self.program.rel_file(n.module);
        if !state.reachable && !matches!(stmt, ast::Stmt::Return(_) | ast::Stmt::Break(_) | ast::Stmt::Continue(_)) {
            // the statement raised on this path: nothing to observe after it
            return;
        }
        // (a simple statement spanning several lines: a check on any of its lines)
        let simple = matches!(stmt, ast::Stmt::Assign(_) | ast::Stmt::AnnAssign(_) | ast::Stmt::AugAssign(_) | ast::Stmt::Expr(_) | ast::Stmt::Return(_) | ast::Stmt::Delete(_));
        let last = if simple { n.end_line } else { n.line };
        let reqs: Vec<(u32, String)> = self.findings.row_check_requests.iter().filter(|(f, ln, _)| *ln >= n.line && *ln <= last && *f == file).map(|(_, ln, name)| (*ln, name.clone())).collect();
        for (ln, name) in reqs {
            // (an observation, not a read of the program: no position, so no unbound-name check)
            let v = self.load_name(s(&name), &Node::none(), state);
            let mut rows: Vec<Row> = Vec::new();
            let mut ns_used = Ns::Item;
            for oid in v.pts.iter() {
                // a UserDict's keys are those of its `data`
                if self.heap.kind(oid) == Kind::Instance && self.heap.instance_class(oid).map(|c| self.heap.mro(c).iter().any(|m| self.m.userdict_classes.contains(m))).unwrap_or(false) {
                    let d = state.slot(oid, Ns::Attr, key_str_lit("data"));
                    if let Some(dd) = d.single().filter(|x| self.heap.kind(*x) == Kind::Dict) {
                        ns_used = Ns::Item;
                        rows.push(state.row(dd, Ns::Item));
                        continue;
                    }
                }
                // a mapping proxy shows its dict
                if let Payload::View(ViewKind::Proxy, inner) = self.heap.obj(oid).payload.clone() {
                    for dd in inner.pts.iter() {
                        rows.push(state.row(dd, Ns::Item));
                    }
                    continue;
                }
                // (a numpy structured array is keyed by its fields)
                // (an instance of a dict subclass: its items)
                let dict_sub = self.heap.kind(oid) == Kind::Instance && self.heap.instance_class(oid).is_some_and(|c| self.heap.mro(c).iter().any(|x| self.heap.class_info_opt(*x).map(|ci| ci.lib.dict_base).unwrap_or(false)));
                let ns = if matches!(self.heap.kind(oid), Kind::Instance | Kind::Namespace) && !self.m.np_struct.contains(&oid) && !dict_sub { Ns::Attr } else { Ns::Item };
                ns_used = ns;
                rows.push(state.row(oid, ns));
            }
            let must: Vec<KeyId> = state.name_keys.iter().filter(|(n, _)| *intern::str_get(*n) == *name).map(|(_, k)| *k).collect();
            let mut row = if rows.is_empty() { None } else { Some(must.iter().fold(crate::domain::join_rows(rows), |r, k| r.store(*k, true))) };
            if let Some(r) = row.as_mut() {
                for (_, _, _, k) in self.findings.row_absent_probes.iter().filter(|(f, l, nm, _)| *f == file && *l == ln && *nm == name) {
                    if crate::domain::classify(r, *k).tier == crate::domain::Tier::Missing {
                        *r = r.delete(*k, true);
                    }
                }
            }
            let value = self.value_info(&v, state);
            let tensor = if self.m.torch_on {
                // a line run several times in one analysis (a loop body): the join of its values
                let (chain, entry) = (self.cx.call_chain.clone(), self.entry.run.is_some());
                let prev = self.findings.t_row_values.get(&(ln, name.clone(), chain.clone(), entry)).cloned();
                let joined = match prev {
                    Some(p) => p.join(&v),
                    None => v.clone(),
                };
                self.findings.t_row_values.insert((ln, name.clone(), chain, entry), joined.clone());
                Some(crate::models::torch::t_check_json(self, &joined, state))
            } else {
                None
            };
            self.findings.row_checks.push(RowCheck { file: file.clone(), line: ln, name, row, ns: ns_used, value, tensor });
        }
    }
}
