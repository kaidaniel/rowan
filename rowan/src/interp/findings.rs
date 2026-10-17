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

/// Facts about one name at one line of the main module (hover support): the values it held and
/// the rows of the objects it pointed to. Two sets of values, never joined into one: what holds
/// for any call of the function the line is in (its general run, with open parameters of their
/// annotated types; module code), and what the program's own calls gave (with their lines).
#[derive(Clone, Default)]
pub struct Fact {
    /// object -> (item row, attr row) as they were when the name was read / written: in the
    /// program's calls, and in the general run (the objects a hover describes are the calls'
    /// when there are some: those exist in the program's runs)
    pub rows: IndexMap<ObjId, (Option<Row>, Option<Row>)>,
    pub general_rows: IndexMap<ObjId, (Option<Row>, Option<Row>)>,
    /// the values (`report::render_value`) the general run recorded, distinct texts
    pub general: Vec<String>,
    /// the values the program's calls recorded, distinct texts, each with the lines of the
    /// calls (of the function the line is in) that gave it
    pub calls: Vec<(String, Vec<u32>)>,
    /// the symbols the texts mention, with what is known about each (`report::value_legend`)
    pub legend: Vec<(String, String)>,
    /// display names of the tensor dims the texts mention (`B` for x.shape[0])
    pub t_names: FxHashMap<crate::models::shape::AtomId, String>,
    /// recorded by an assignment (whose value the hover shows, not the reads on the line:
    /// `x = x + 1` is `x = 2`, not `1 | 2`)
    pub written: bool,
    /// a parameter's fact at its `def`: its function, and what the program's calls passed (the
    /// call chain into the function, the objects given; at most `PASSED`)
    pub param_of: Option<ObjId>,
    pub passed: Vec<(Chain, Vec<ObjId>)>,
}

/// The calls' arguments a parameter's fact keeps (`Fact::passed`).
const PASSED: usize = 32;

/// Distinct texts a fact keeps per set: more are not shown.
const FACT_TEXTS: usize = 4;

impl Fact {
    /// The objects a hover describes, with their rows: the program's calls', else the general
    /// run's.
    pub fn shown_rows(&self) -> &IndexMap<ObjId, (Option<Row>, Option<Row>)> {
        if self.rows.is_empty() { &self.general_rows } else { &self.rows }
    }

    /// The rows a recording saw (in a call, or `general`), joined into the fact's.
    fn join_rows(&mut self, rows: Vec<(ObjId, Option<Row>, Option<Row>)>, general: bool) {
        let mine = if general { &mut self.general_rows } else { &mut self.rows };
        for (oid, item, attr) in rows {
            match mine.get_mut(&oid) {
                Some((ci, ca)) => {
                    *ci = join_opt(ci, &item);
                    *ca = join_opt(ca, &attr);
                }
                None => {
                    mine.insert(oid, (item, attr));
                }
            }
        }
    }

    /// Has the set a recording goes to (`call`: the line of the call, None: general) all the
    /// texts it shows?
    fn full(&self, call: Option<u32>) -> bool {
        match call {
            None => self.general.len() >= FACT_TEXTS,
            Some(_) => self.calls.len() >= FACT_TEXTS,
        }
    }

    /// A recording's text (up to `FACT_TEXTS` distinct per set; `call`: the line of the call
    /// it was made in, None: the general run) and its legend (a symbol's descriptions joined).
    fn add_text(&mut self, text: String, call: Option<u32>, legend: Vec<(String, String)>) {
        match call {
            None if !self.general.contains(&text) && self.general.len() < FACT_TEXTS => self.general.push(text),
            None => {}
            Some(line) => match self.calls.iter().position(|(t, _)| *t == text) {
                Some(i) if !self.calls[i].1.contains(&line) => self.calls[i].1.push(line),
                Some(_) => {}
                None if self.calls.len() < FACT_TEXTS => self.calls.push((text, vec![line])),
                None => {}
            },
        }
        for (sym, desc) in legend {
            match self.legend.iter_mut().find(|(s0, _)| *s0 == sym) {
                Some((_, d)) if *d != desc && !d.split(" | ").any(|p| p == desc) => *d = format!("{} | {}", d, desc),
                Some(_) => {}
                None => self.legend.push((sym, desc)),
            }
        }
    }
}

fn join_opt(a: &Option<Row>, b: &Option<Row>) -> Option<Row> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.join(y)),
        (Some(x), None) | (None, Some(x)) => Some(x.clone()),
        _ => None,
    }
}

pub struct RowCheck {
    /// the module's path relative to the main file's directory ("" for the main file)
    pub file: String,
    pub line: u32,
    pub name: String,
    pub row: Option<Row>,
    /// the name's value: its known literals, whether it may be anything else, its objects
    pub value: ValueInfo,
    /// tensor facts for the golden harness: {"shape", "dim", "chain", "entry"}
    pub tensor: Option<serde_json::Value>,
    /// made where the function runs for any call (`in_general_run`), and in a
    /// requirement-only run (whose lines checks of the program's calls do not count)
    pub general: bool,
    pub quiet: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ValueInfo {
    pub lits: Vec<KeyId>,
    pub unknown: bool,
    pub objects: Vec<String>,
    /// the types of its unknown parts known to have one (`int(?)`), and whether some has none
    pub types: Vec<&'static str>,
    pub untyped: bool,
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
    /// the golden harness checks values for any call (`#@general`)
    pub general_checks: bool,
    pub facts: IndexMap<(u32, StrId), Fact>,
    /// the next hover fact is an assignment's
    pub fact_write: bool,
    /// a requirement-only entry run (`param F.x` schemas of a called function): no verdicts,
    /// row checks, facts or notes
    pub quiet: bool,
    /// no record-access verdicts (shapes still reported): the own run of a called function
    /// of a torch / numpy module
    pub quiet_records: bool,
    /// the state the main module ended in (the report's rows)
    pub final_state: Option<State>,
    /// what functions' runs for any call read of their own parameters (`param F.x`
    /// requirements: a hovered parameter's contract)
    pub contracts: Option<State>,
}

impl Findings {
    /// Are values for any call asked for (hover facts, `#@general` checks)? Functions whose run
    /// with verdicts is not one for any call then get a run for any call of their own.
    pub(in crate::interp) fn wants_general(&self) -> bool {
        self.facts_on || self.general_checks
    }

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
        let cls = match self.cx.picked.last() {
            _ if cls.tier != Tier::Missing => cls,
            _ if self.mode.recv_scalar => {
                weakened = Classification::with_note(Tier::Possibly, format!("{}, unless the value is a plain one such as a str", cls.note));
                &weakened
            }
            // (in one of several functions a value picks: where the program picks this one)
            Some((call, f)) => {
                weakened = Classification::with_note(Tier::Possibly, format!("{}, where `{}` calls {}", cls.note, call, f));
                &weakened
            }
            None => cls,
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

    /// Does a fact recorded here hold for any call (module code, or the frame of the function
    /// whose general run is going on), or for one call of the program's?
    pub(in crate::interp) fn in_general_run(&self) -> bool {
        match self.frame().func {
            None => true,
            f => self.entry.general.is_some() && self.entry.general == f,
        }
    }

    pub(in crate::interp) fn record_fact(&mut self, node: &Node, name: StrId, v: &Value, state: &State) {
        if !self.findings.facts_on || node.addr == 0 {
            return;
        }
        if node.module != self.heap.module_info(self.program.main_module).idx {
            return;
        }
        // the set the value goes to: the general one, or that of the call (its line)
        let general = self.in_general_run();
        if self.no_facts_here(general) {
            return;
        }
        let call = if general { None } else { Some(self.cx.call_chain.last().copied().unwrap_or(0)) };
        // (an assignment's value is what the hover shows, not the reads on the line: `x = x + 1`)
        let writing = std::mem::take(&mut self.findings.fact_write);
        if !writing && self.findings.facts.get(&(node.line, name)).is_some_and(|e| e.written) {
            return;
        }
        let rows = self.fact_rows(v, state);
        let scope = self.frame().func.unwrap_or(0);
        // (a fact that has all the texts it shows only joins the value and rows: rendering a
        // large value at every run of the line is what costs — not in torch programs, whose
        // hover is the assignment's own value)
        if !self.m.torch_on {
            if let Some(e) = self.findings.facts.get_mut(&(node.line, name)) {
                if e.full(call) && (e.written || !writing) {
                    e.join_rows(rows, general);
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
        let render = |an: &Analyzer| crate::interp::render::in_scope(scope, || (crate::interp::render::render_value(an, state, v, 0, &mut Vec::new()), crate::interp::render::value_legend(an, state, v)));
        let (text, mut legend) = match pctx {
            Some(ctx) => crate::models::shape::with_ctx(ctx, || render(self)),
            None => render(self),
        };
        legend.extend(tlegend);
        let e = self.findings.facts.entry((node.line, name)).or_default();
        if writing && !e.written {
            e.written = true;
            e.general.clear();
            e.calls.clear();
            e.legend.clear();
            e.rows.clear();
            e.general_rows.clear();
        }
        // (the dims are named as the general run names them)
        if general && self.m.torch_on {
            e.t_names = tnames;
        } else {
            for (a, n) in tnames {
                e.t_names.entry(a).or_insert(n);
            }
        }
        e.add_text(text, call, legend);
        e.join_rows(rows, general);
    }

    /// Does this run record no hover facts on the running function's lines? A requirement-only
    /// run records only its function's own lines; on demand, what the program's calls give is
    /// not known (only values for any call); a run with an instance made with defaults, neither.
    fn no_facts_here(&self, general: bool) -> bool {
        ((self.findings.quiet || self.entry.on_demand) && !general) || (self.entry.no_facts.is_some() && self.entry.no_facts == self.frame().func)
    }

    /// The facts of the running function's parameters at its `def`, as a call binds them: their
    /// values, and for the program's calls what each passed (`Fact::passed`).
    pub(in crate::interp) fn record_param_facts(&mut self, fi: &super::FuncInfo, bound: &IndexMap<StrId, Value>, state: &State) {
        let (Some(params), Some(foid)) = (fi.node.params(), self.frame().func) else { return };
        let module = fi.node.node().module;
        if !self.findings.facts_on || module != self.heap.module_info(self.program.main_module).idx {
            return;
        }
        let general = self.in_general_run();
        if self.no_facts_here(general) {
            return;
        }
        for p in crate::scopes::all_params(params) {
            let name = s(p.name.id.as_str());
            let Some(v) = bound.get(&name) else { continue };
            let node = frontend::node_in(module, p);
            self.record_fact(&node, name, v, state);
            let Some(f) = self.findings.facts.get_mut(&(node.line, name)) else { continue };
            f.param_of = Some(foid);
            if !general && f.passed.len() < PASSED && !f.passed.iter().any(|(c, _)| *c == self.cx.call_chain) {
                f.passed.push((self.cx.call_chain.clone(), v.pts.to_vec()));
            }
        }
    }

    /// The rows a hover shows for the objects of `v`: item rows of records, attribute rows of
    /// instances (and of what has either).
    fn fact_rows(&self, v: &Value, state: &State) -> Vec<(ObjId, Option<Row>, Option<Row>)> {
        let mut rows: Vec<(ObjId, Option<Row>, Option<Row>)> = Vec::new();
        for oid in v.pts.iter() {
            let k = self.heap.kind(oid);
            let item = if matches!(k, Kind::Dict | Kind::Frame | Kind::Unknown | Kind::Namespace | Kind::Module | Kind::Class) || state.has_row(oid, Ns::Item) { Some(state.row(oid, Ns::Item)) } else { None };
            let attr = if matches!(k, Kind::Instance | Kind::Namespace | Kind::Unknown | Kind::Module | Kind::Class) || state.has_row(oid, Ns::Attr) { Some(state.row(oid, Ns::Attr)) } else { None };
            rows.push((oid, item, attr));
        }
        rows
    }

    /// What a value is, for the golden harness's `#@value` checks: literals it may be, whether
    /// it may also be something unknown, and the (known) objects it may be.
    pub(in crate::interp) fn value_info(&self, v: &Value, state: &State) -> ValueInfo {
        let mut info = ValueInfo::default();
        let typed = |info: &mut ValueInfo, t: Option<&'static str>| match t {
            Some(t) if !info.types.contains(&t) => info.types.push(t),
            Some(_) => {}
            None => info.untyped = true,
        };
        if v.kv.is_literal() {
            info.lits = crate::domain::kv_literals(&v.kv).iter().collect();
        } else if !v.kv.is_bot() {
            info.unknown = true;
            typed(&mut info, v.kv.data_site().and_then(|l| crate::domain::label_type(&intern::str_get(l))));
        }
        for o in v.pts.iter() {
            match self.heap.kind(o) {
                Kind::Unknown | Kind::External => {
                    info.unknown = true;
                    typed(&mut info, self.heap.obj(o).ty);
                }
                // (an unknown's attribute nothing is known of: perhaps a method, perhaps any value)
                Kind::Method if matches!(&self.heap.obj(o).payload, crate::interp::Payload::Method(q, _) if q.starts_with("unknown.")) => {
                    info.unknown = true;
                    info.untyped = true;
                }
                // a tuple of known literals is that literal (as Python prints it)
                Kind::Tuple | Kind::Pair if self.heap.literal_tuple(o, state, 0).is_some() => info.lits.push(self.heap.literal_tuple(o, state, 0).unwrap()),
                _ => info.objects.push(self.heap.obj(o).label.to_string()),
            }
        }
        if v.pts.is_empty() && v.kv.is_bot() {
            info.unknown = true;
            info.untyped = true;
        }
        info
    }

    /// Row-check support for the golden harness: after a statement at a requested line.
    pub(in crate::interp) fn after_stmt(&mut self, stmt: &ast::Stmt, state: &mut State) {
        let general = self.in_general_run();
        if self.findings.row_check_requests.is_empty() || (self.findings.quiet && !general) {
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
            for oid in v.pts.iter() {
                // a UserDict's keys are those of its `data`
                if self.heap.kind(oid) == Kind::Instance && self.heap.instance_class(oid).map(|c| self.heap.mro(c).iter().any(|m| self.m.userdict_classes.contains(m))).unwrap_or(false) {
                    let d = state.slot(oid, Ns::Attr, key_str_lit("data"));
                    if let Some(dd) = d.single().filter(|x| self.heap.kind(*x) == Kind::Dict) {
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
            self.findings.row_checks.push(RowCheck { file: file.clone(), line: ln, name, row, value, tensor, general, quiet: self.findings.quiet });
        }
    }
}
