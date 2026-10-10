//! Symbolic function summaries (SPECIFICATION.md §7.3, §9); port of `rowan/summary.py`.
//! See that file's module docstring for the design.

use std::cell::Cell;
use std::rc::Rc;

use indexmap::IndexMap;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::domain::{
    chain_extend, classify, join_rows, Chain, Classification, KeyVal, KeysOf, Ns, ObjId, ObjSet, Row, Tier, Value, DATA,
    kv_literals, kv_join, rho, unrho, Rho,
};
use crate::frontend::Node;
use crate::intern::{self, key_str_lit, s, KeyId, StrId, STAR};
use crate::interp::access::{append_slot_of, param_slot_of};
use crate::interp::{debug_enabled, Analyzer, Frame, FrameKind, FuncInfo, FuncNode, Payload, Verdict};
use crate::state::{Kind, State};

pub const MAX_SUMMARY_ROUNDS: usize = 3;
pub const MAX_REL_CHAIN: usize = 1;
pub const MAX_TABULATIONS: usize = 256;
pub const MAX_CALL_TABULATIONS: usize = 256;
pub const MAX_SYMREQ_TARGETS: usize = 8;

pub fn is_code_kind(k: Kind) -> bool {
    matches!(k, Kind::Function | Kind::Class | Kind::Bound | Kind::External | Kind::Partial | Kind::Method)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PhKind {
    Param,
    SelfPh,
    Vararg,
    Kwarg,
    Path,
    Narrow,
}

impl PhKind {
    fn name(self) -> &'static str {
        match self {
            PhKind::Param => "param",
            PhKind::SelfPh => "self",
            PhKind::Vararg => "vararg",
            PhKind::Kwarg => "kwarg",
            PhKind::Path => "path",
            PhKind::Narrow => "narrow",
        }
    }
}

/// Payload of a placeholder object.
#[derive(Debug)]
pub struct PH {
    pub kind: PhKind,
    pub func: ObjId,
    pub name: String,
    pub parent: Option<ObjId>,
    pub ns: Ns,
    pub key: Option<KeyId>,
    /// declared class (annotation), when known
    pub cls: Cell<Option<ObjId>>,
    /// `cls` is the exact class (from the call site's argument signature), not a bound: no
    /// subclass may override the methods resolved through it
    pub exact: Cell<bool>,
}

#[derive(Clone, Debug)]
pub struct SymReq {
    /// a guarded access whose handlers all re-raise: the input still needs the key
    pub reraise: bool,
    pub node: Node,
    pub key_kv: KeyVal,
    pub row: Row,
    pub oid: ObjId,
    pub ns: Ns,
    pub guarded: bool,
    pub rel_chain: Chain,
    pub kind: &'static str,
    pub file: String,
    pub text: String,
    pub cls: Option<ObjId>,
    /// recorded under a condition (relative to the function's entry): a requirement derived
    /// from it is "sometimes"
    pub conditional: bool,
}

pub type SymKey = (Node, KeyVal, ObjId, Ns, bool, Chain, &'static str, Option<ObjId>);

#[derive(Clone, Debug)]
pub struct Event {
    pub kind: &'static str,
    pub ph: ObjId,
}

/// (function, code held by its cells, classes of instance cells, classes of the arguments)
pub type SummaryKey = (ObjId, Vec<(ObjId, Vec<ObjId>)>, Vec<(ObjId, ObjId)>, Vec<(String, ObjId)>);

/// How many distinct argument-class signatures one function is summarised for; beyond
/// that the unspecialised summary is used.
pub const MAX_SIGNATURES: usize = 8;

/// What a row check inside a function body saw during the summary run (relative to the
/// summary's entry); resolved at every instantiation.
#[derive(Clone, Debug)]
pub struct Obs {
    pub file: String,
    pub line: u32,
    pub name: String,
    pub rows: Vec<(ObjId, Ns, Row)>,
    /// keys the name's object surely has (stored through the name)
    pub must: Vec<KeyId>,
    /// the name's value (relative to the summary's entry)
    pub value: Value,
}

/// A hover fact recorded in a summary body (relative to its entry); resolved per call.
#[derive(Clone, Debug)]
pub struct FactObs {
    pub line: u32,
    pub name: StrId,
    pub value: Value,
    pub rows: Vec<(ObjId, Option<Row>, Option<Row>)>,
    /// the contents of the objects the value reaches, as they were at that line
    pub slots: Vec<(ObjId, Ns, KeyId, Value)>,
    /// the function the line is in (whose names expressions refer to)
    pub scope: u32,
}

pub struct Summary {
    pub foid: ObjId,
    /// hover facts inside the body (--facts)
    pub fact_obs: Vec<FactObs>,
    /// row checks inside the body (golden harness)
    pub obs: Vec<Obs>,
    /// debug: the summary's context and its key (typed / code cells, signature)
    pub desc: String,
    pub ph_by_name: IndexMap<StrId, ObjId>,
    pub ph_self: Option<ObjId>,
    pub exit: State,
    pub ret: Value,
    pub symreqs: Vec<SymReq>,
    pub rel_verdicts: Vec<Verdict>,
    pub notes: Vec<String>,
    pub templates: FxHashSet<ObjId>,
    pub events: Vec<Event>,
    pub rounds: usize,
    pub partial: Cell<bool>,
    pub rows_fp: Vec<(ObjId, Ns, Row)>,
    pub slots_fp: Vec<((ObjId, Ns, KeyId), Value)>,
    pub elems_fp: Vec<(ObjId, KeyVal)>,
    /// the body may raise an uncaught exception (or calls unknown code)
    pub raises: bool,
}

impl Summary {
    /// Exit facts that differ from the entry state `base` and are not the identity.
    pub fn compute_footprint(&mut self, an: &Analyzer, base: &State) {
        let is_path_of = |v: &Value, oid: ObjId, ns: Ns, key: KeyId| -> bool {
            let Some(ph) = v.single() else { return false };
            if !an.has_obj(ph) || an.kind(ph) != Kind::Ph {
                return false;
            }
            let p = an.ph_info(ph);
            if p.kind != PhKind::Path {
                return false;
            }
            p.parent == Some(oid) && p.ns == ns && p.key == Some(key) && (matches!(v.kv, KeyVal::Param(x) if x == ph) || v.kv.is_bot())
        };
        let mut rk = self.exit.rows.diff_keys(&base.rows);
        rk.sort_by_key(|(o, n)| (*o, *n));
        self.rows_fp.clear();
        for (oid, ns) in rk {
            let Some(expr) = self.exit.rows.get(&(oid, ns)) else { continue };
            if expr != Row::entry(rho(oid, ns)) {
                self.rows_fp.push((oid, ns, expr));
            }
        }
        let mut sk = self.exit.slots.diff_keys(&base.slots);
        sk.sort_by_key(|(o, (n, k))| (*o, *n, *k));
        self.slots_fp.clear();
        for (oid, (ns, k)) in sk {
            let Some(v) = self.exit.slots.get(&oid, &(ns, k)) else { continue };
            if is_path_of(&v, oid, ns, k) {
                continue;
            }
            self.slots_fp.push(((oid, ns, k), v));
        }
        let mut ek = self.exit.elems.diff_keys(&base.elems);
        ek.sort_unstable();
        self.elems_fp.clear();
        for oid in ek {
            let Some(kv) = self.exit.elems.get(&oid) else { continue };
            if let KeyVal::Param(ph) = &kv {
                if is_path_of(&Value::with_kv(ObjSet::one(*ph), kv.clone()), oid, Ns::Item, STAR) {
                    continue;
                }
            }
            self.elems_fp.push((oid, kv));
        }
        self.collect_garbage(an);
    }

    /// Objects the body allocated that the caller can never reach are dropped from the footprint.
    fn collect_garbage(&mut self, an: &Analyzer) {
        let mut roots: Vec<ObjId> = self.ret.pts.to_vec();
        if let KeyVal::KeysOf(ko) = &self.ret.kv {
            roots.extend(ko.objs.iter());
        }
        for (oid, _, _) in &self.rows_fp {
            if !self.templates.contains(oid) {
                roots.push(*oid);
            }
        }
        for ((oid, _, _), _) in &self.slots_fp {
            if !self.templates.contains(oid) {
                roots.push(*oid);
            }
        }
        for (oid, _) in &self.elems_fp {
            if !self.templates.contains(oid) {
                roots.push(*oid);
            }
        }
        let mut reach: FxHashSet<ObjId> = FxHashSet::default();
        let mut work = roots;
        fn push_value(v: &Value, work: &mut Vec<ObjId>) {
            work.extend(v.pts.iter());
            match &v.kv {
                KeyVal::KeysOf(ko) => work.extend(ko.objs.iter()),
                KeyVal::Param(p) => work.push(*p),
                _ => {}
            }
        }
        while let Some(o) = work.pop() {
            if !reach.insert(o) {
                continue;
            }
            for (_, v) in self.exit.slots.for_key(&o) {
                push_value(&v, &mut work);
            }
            if let Some(kv) = self.exit.elems.get(&o) {
                push_value(&Value::kv_only(kv), &mut work);
            }
            if !an.has_obj(o) {
                continue;
            }
            let ob = an.obj(o);
            if let Some(d) = ob.derived_from {
                work.push(d);
            }
            match &ob.payload {
                Payload::Func(fi) => work.extend(fi.closure.values().copied()),
                Payload::Ph(p) => {
                    if let Some(par) = p.parent {
                        work.push(par);
                    }
                }
                Payload::Cls(c) => work.push(*c),
                Payload::Method(_, v) | Payload::View(_, v) | Payload::Enumerate(v, _) => push_value(v, &mut work),
                Payload::Bound(f, v) | Payload::Super(f, v) => {
                    work.push(*f);
                    push_value(v, &mut work);
                }
                Payload::Zip(a, b) | Payload::Pair(a, b) => {
                    push_value(a, &mut work);
                    push_value(b, &mut work);
                }
                Payload::Partial(f, args, kw) => {
                    push_value(f, &mut work);
                    for a in args {
                        push_value(a, &mut work);
                    }
                    for v in kw.values() {
                        push_value(v, &mut work);
                    }
                }
                Payload::Groupby(fs, _, _, _) => work.extend(fs.iter().copied()),
                Payload::Locidx(_, o2) => work.push(*o2),
                _ => {}
            }
        }
        let dead: FxHashSet<ObjId> = self.templates.iter().copied().filter(|t| !reach.contains(t)).collect();
        if dead.is_empty() {
            return;
        }
        self.templates.retain(|t| !dead.contains(t));
        self.rows_fp.retain(|(o, _, _)| !dead.contains(o));
        self.slots_fp.retain(|((o, _, _), _)| !dead.contains(o));
        self.elems_fp.retain(|(o, _)| !dead.contains(o));
    }
}

impl Analyzer {
    // ------------------------------------------------------------ placeholders
    /// the allocation context of the summary of `func` being computed (or its plain one)
    pub fn sum_ctx(&self, func: ObjId) -> String {
        match self.sum_stack.iter().rposition(|f| *f == func) {
            Some(i) => self.sum_ctxs[i].clone(),
            None => format!("sum{}", func),
        }
    }

    pub fn new_placeholder(&mut self, kind: PhKind, func: ObjId, name: &str, parent: Option<ObjId>, ns: Ns, key: Option<KeyId>, cls: Option<ObjId>, tag: &str) -> ObjId {
        let site = if kind == PhKind::Path {
            format!("ph:path:{}:{}:{}:{}{}", func, parent.map(|p| p.to_string()).unwrap_or_else(|| "None".into()), ns, key.map(intern::key_repr).unwrap_or_else(|| "None".into()), tag)
        } else {
            format!("ph:{}:{}:{}", kind.name(), func, name)
        };
        let label = if name.is_empty() { format!("ρ{}", key.map(intern::key_repr).unwrap_or_else(|| "None".into())) } else { format!("ρ{}", name) };
        let payload = Payload::Ph(Rc::new(PH { kind, func, name: name.to_string(), parent, ns, key, cls: Cell::new(cls), exact: Cell::new(false) }));
        let pctx = self.sum_ctx(func);
        let oid = self.alloc_raw(Kind::Ph, &site, &pctx, true, &label, payload.clone());
        self.obj_mut(oid).payload = payload;
        if !self.symbols.contains_key(&oid) {
            let sym = match (kind, parent, key) {
                (PhKind::Path, Some(p), Some(k)) => self.sym_access(p, ns, k),
                (_, _, _) if !name.is_empty() => Some(name.to_string()),
                _ => None,
            };
            if let Some(sym) = sym {
                let scope = match parent {
                    Some(p) if kind == PhKind::Path => self.symbol_scope.get(&p).copied().unwrap_or(func),
                    _ => func,
                };
                self.set_symbol(oid, sym, scope);
            }
        }
        if kind == PhKind::Path {
            let d = parent.map(|p| self.obj(p).depth + 1).unwrap_or(1);
            self.obj_mut(oid).depth = d;
            self.obj_mut(oid).iter_elem = key == Some(STAR);
        }
        oid
    }

    /// `isinstance(x, C)` on placeholder x: the placeholder standing for x when it is a C.
    pub fn narrowed_placeholder(&mut self, parent: ObjId, cls: ObjId, state: &mut State) -> ObjId {
        let func = *self.sum_stack.last().unwrap();
        let site = format!("ph:narrow:{}:{}:{}", func, parent, cls);
        let label = format!("{}:{}", self.obj(parent).label, self.obj(cls).label);
        let payload = Payload::Ph(Rc::new(PH { kind: PhKind::Narrow, func, name: String::new(), parent: Some(parent), ns: Ns::Item, key: None, cls: Cell::new(Some(cls)), exact: Cell::new(false) }));
        let pctx = self.sum_ctx(func);
        let oid = self.alloc_raw(Kind::Ph, &site, &pctx, true, &label, payload.clone());
        self.obj_mut(oid).payload = payload;
        if let Some(sym) = self.symbols.get(&parent).cloned() {
            let scope = self.symbol_scope.get(&parent).copied().unwrap_or(0);
            if !self.symbols.contains_key(&oid) {
                self.set_symbol(oid, sym, scope);
            }
        }
        let d = self.obj(parent).depth;
        self.obj_mut(oid).depth = d;
        if !state.has_row(oid, Ns::Item) {
            state.set_row(oid, Ns::Item, Row::entry(rho(oid, Ns::Item)));
            state.set_row(oid, Ns::Attr, Row::entry(rho(oid, Ns::Attr)));
        }
        oid
    }

    /// Placeholder for the (unknown) content of an entry object's slot, inside a summary.
    pub fn path_placeholder(&mut self, parent: ObjId, ns: Ns, key: KeyId, state: &mut State, cls: Option<ObjId>, tag: &str) -> ObjId {
        let func = *self.sum_stack.last().unwrap();
        let cls = if cls.is_none() && ns == Ns::Cell { self.cell_classes.get(&parent).copied() } else { cls };
        let oid = self.new_placeholder(PhKind::Path, func, "", Some(parent), ns, Some(key), cls, tag);
        if !state.has_row(oid, Ns::Item) {
            state.set_row(oid, Ns::Item, Row::entry(rho(oid, Ns::Item)));
            state.set_row(oid, Ns::Attr, Row::entry(rho(oid, Ns::Attr)));
        }
        oid
    }

    pub fn is_ph(&self, oid: ObjId) -> bool {
        self.kind(oid) == Kind::Ph
    }

    /// Inside a summary: was this object around before the function started?
    pub fn is_entry_object(&self, oid: ObjId) -> bool {
        !self.sum_stack.is_empty() && (oid < *self.sum_obj_mark.last().unwrap() || self.kind(oid) == Kind::Ph)
    }

    /// Presence of literal key k in records from outside the function (`os.environ`, a
    /// global) that the summary leaves undecided: decided per call site.
    pub fn entry_key_events(&mut self, recs: &[ObjId], k: crate::intern::KeyId, st: &crate::state::State) {
        for o in recs {
            if self.kind(*o) != Kind::Ph && self.is_entry_object(*o) {
                let r = st.row(*o, crate::domain::Ns::Item);
                if !r.definite.contains(k) && !r.lacks(k) {
                    self.record_event("concrete", *o);
                }
            }
        }
    }

    /// The symbolic analysis just lost information about entry object `ph`.
    pub fn record_event(&mut self, kind: &'static str, ph: ObjId) {
        if !self.sum_stack.is_empty() && (self.kind(ph) == Kind::Ph || self.is_entry_object(ph)) {
            if crate::interp::debug_level() == 2 {
                let top = *self.sum_stack.last().unwrap();
                let par = if self.kind(ph) == Kind::Ph { self.ph_info(ph).parent.map(|p| self.obj(p).label.clone()).unwrap_or_default() } else { String::new() };
                eprintln!("  event {} on {} (parent {}) in {} [{}]", kind, self.obj(ph).label, par, self.func_info(top).qualname, self.frames.last().map(|f| f.label.clone()).unwrap_or_default());
            }
            self.sum_events.last_mut().unwrap().insert((kind, ph), Event { kind, ph });
        }
    }

    /// A "concrete" event on a placeholder and on the parameters it hangs below.
    pub fn concrete_root(&mut self, oid: ObjId) {
        self.record_event("concrete", oid);
        let mut p = if self.kind(oid) == Kind::Ph { self.ph_info(oid).parent } else { None };
        let mut n = 0;
        while let Some(par) = p {
            if self.kind(par) != Kind::Ph || n > 4 {
                break;
            }
            self.record_event("concrete", par);
            p = self.ph_info(par).parent;
            n += 1;
        }
    }

    /// A "concrete" event on every placeholder in `v` and in the containers it holds (to
    /// `depth` levels): a model whose result depends on their contents.
    pub fn concrete_events(&mut self, v: &Value, state: &State, depth: usize) {
        // the keys of a placeholder or outside record (`sorted(d)`, a `for k in d` variable)
        if let crate::domain::KeyVal::KeysOf(ko) = &v.kv {
            for o in ko.objs.iter() {
                if self.kind(o) == Kind::Ph || self.is_entry_object(o) {
                    self.record_event("concrete", o);
                }
            }
        }
        for oid in v.pts.iter() {
            if self.kind(oid) == Kind::Ph {
                self.record_event("concrete", oid);
                // a path below a parameter (`df["c"]`): its root decides too
                let mut p = self.ph_info(oid).parent;
                let mut n = 0;
                while let Some(par) = p {
                    if self.kind(par) != Kind::Ph || n > 4 {
                        break;
                    }
                    self.record_event("concrete", par);
                    p = self.ph_info(par).parent;
                    n += 1;
                }
            } else if matches!(self.kind(oid), Kind::List | Kind::Tuple | Kind::Dict | Kind::Set) && self.is_entry_object(oid) && !crate::interp::disabled("entryconcrete") {
                // a container from outside the function (a global): its contents as the call
                // site has them
                self.record_event("concrete", oid);
            } else if depth > 0 && matches!(self.kind(oid), Kind::List | Kind::Tuple | Kind::Dict | Kind::Set) && !self.is_entry_object(oid) {
                for (_, x) in state.slots_of(oid) {
                    self.concrete_events(&x, state, depth - 1);
                }
            }
        }
    }

    /// Concrete events on the parameters a model's arguments hold (placeholders, `Param`
    /// values, containers of them): a model call decided per call site.
    pub fn concrete_args<'a>(&mut self, vals: impl IntoIterator<Item = &'a Value>, state: &State) {
        if self.sum_stack.is_empty() {
            return;
        }
        for v in vals {
            self.concrete_events(v, state, 1);
            if let Some(ph) = v.kv.as_param() {
                self.record_event("concrete", ph);
            }
        }
    }

    /// Record a deferred access; the same access seen again is merged by joining its rows.
    pub fn defer(&mut self, mut sr: SymReq) {
        if sr.rel_chain.len() > MAX_REL_CHAIN {
            sr.rel_chain.truncate(MAX_REL_CHAIN);
        }
        let key: SymKey = (sr.node, sr.key_kv.clone(), sr.oid, sr.ns, sr.guarded, sr.rel_chain.clone(), sr.kind, sr.cls);
        let reqs = self.sum_reqs.last_mut().unwrap();
        match reqs.get_mut(&key) {
            None => {
                reqs.insert(key, sr);
            }
            Some(prev) => {
                if prev.row != sr.row {
                    prev.row = prev.row.join(&sr.row);
                }
            }
        }
    }

    pub fn record_symreq(&mut self, node: &Node, key_kv: KeyVal, row: &Row, oid: ObjId, ns: Ns, guarded: bool, kind: &'static str) {
        let fname = self.file_of_frame();
        let text = self.node_text(node);
        let guard = match &self.obj(oid).payload {
            Payload::Ph(p) if self.kind(oid) == Kind::Ph => p.cls.get(),
            _ => None,
        };
        let conditional = self.branch_depth > 0;
        let reraise = guarded && (self.catching_depth > 0 || self.guard_reraises(ns));
        let sr = SymReq { reraise, node: *node, key_kv, row: row.clone(), oid, ns, guarded, rel_chain: self.call_chain.clone(), kind, file: fname, text, cls: guard, conditional };
        self.defer(sr);
    }

    // ------------------------------------------------------------- summarise
    /// Compute (or fetch) the summary of function object foid.  None while it is being
    /// computed (recursion): the caller then behaves as a ⊥ summary.
    pub fn summarise(&mut self, foid: ObjId, state: &mut State) -> Option<Rc<Summary>> {
        self.summarise_sig(foid, state, Vec::new())
    }

    /// `sig`: parameter name -> class, for parameters bound to instances of one class at the
    /// call site; the placeholders carry those classes so methods resolve by class.
    pub fn summarise_sig(&mut self, foid: ObjId, state: &mut State, sig: Vec<(String, ObjId)>) -> Option<Rc<Summary>> {
        let (sig, sig_idx) = {
            let n = self.sig_count.entry(foid).or_default();
            if sig.is_empty() {
                (sig, 0)
            } else if let Some(i) = n.iter().position(|x| *x == sig) {
                (sig, i + 1)
            } else if n.len() < MAX_SIGNATURES {
                n.push(sig.clone());
                (sig, n.len())
            } else {
                (Vec::new(), 0)
            }
        };
        let _ = sig_idx;
        let fi = self.func_info(foid);
        let fnode = fi.node;
        let mut code_cells: IndexMap<ObjId, Value> = IndexMap::new();
        let mut typed_cells: Vec<(ObjId, ObjId)> = Vec::new();
        let mut cells: Vec<ObjId> = fi.closure.values().copied().collect();
        let mut seen_cells: FxHashSet<ObjId> = FxHashSet::default();
        let mut i = 0;
        while i < cells.len() {
            let cid = cells[i];
            i += 1;
            if !seen_cells.insert(cid) {
                continue;
            }
            for o in state.slot(cid, Ns::Cell, key_str_lit("v")).pts.iter() {
                if self.kind(o) == Kind::Function {
                    cells.extend(self.func_info(o).closure.values().copied());
                }
            }
        }
        let mut uniq: Vec<ObjId> = Vec::new();
        for c in cells {
            if !uniq.contains(&c) {
                uniq.push(c);
            }
        }
        for cid in uniq {
            let cv = state.slot(cid, Ns::Cell, key_str_lit("v"));
            if !cv.pts.is_empty() && cv.pts.iter().all(|o| is_code_kind(self.kind(o))) {
                code_cells.insert(cid, cv);
            } else if !cv.pts.is_empty() {
                let mut classes: Vec<ObjId> = Vec::new();
                let all_inst = cv.pts.iter().all(|o| self.kind(o) == Kind::Instance);
                for o in cv.pts.iter() {
                    if self.kind(o) == Kind::Instance {
                        if let Some(c) = self.instance_class(o) {
                            if !classes.contains(&c) {
                                classes.push(c);
                            }
                        }
                    }
                }
                if classes.len() == 1 && all_inst {
                    typed_cells.push((cid, classes[0]));
                }
            }
        }
        typed_cells.sort();
        let mut cc: Vec<(ObjId, Vec<ObjId>)> = code_cells.iter().map(|(c, v)| (*c, v.pts.to_vec())).collect();
        cc.sort();
        let skey: SummaryKey = (foid, cc.clone(), typed_cells.clone(), sig.clone());
        // placeholders and the objects a summary allocates are keyed by this context: one per
        // key variant (code cells, typed cells, signature), so two summaries of one function
        // share nothing
        let ctx = {
            let rest = (cc, typed_cells.clone(), sig.clone());
            let vs = self.key_variants.entry(foid).or_default();
            let vi = match vs.iter().position(|x| *x == rest) {
                Some(i) => i,
                None => {
                    vs.push(rest);
                    vs.len() - 1
                }
            };
            if vi == 0 { format!("sum{}", foid) } else { format!("sum{}v{}", foid, vi) }
        };
        if let Some(done) = self.summary_table.get(&skey) {
            return Some(done.clone());
        }
        if let Some(pos) = self.sum_stack.iter().position(|f| *f == foid) {
            self.sum_hit.insert(foid);
            // every summary being computed above foid now depends on foid's partial summary
            for d in self.sum_deps.iter_mut().skip(pos + 1) {
                d.insert(foid);
            }
            return self.sum_partial.get(&foid).cloned();
        }
        if let Some((rc, deps)) = self.summary_provisional.get(&skey).cloned() {
            // computed against partial summaries of functions still being summarised: valid
            // while they are on the stack, and the current computation inherits the dependency
            if deps.iter().all(|d| self.sum_stack.contains(d)) {
                for d in &deps {
                    let pos = self.sum_stack.iter().position(|f| f == d).unwrap();
                    for dd in self.sum_deps.iter_mut().skip(pos + 1) {
                        dd.insert(*d);
                    }
                }
                return Some(rc);
            }
            self.summary_provisional.remove(&skey);
        }
        // save interpreter context
        let saved_frames = std::mem::take(&mut self.frames);
        let saved_chain = std::mem::take(&mut self.call_chain);
        let saved_loops = std::mem::take(&mut self.loop_stack);
        let saved_guards = std::mem::take(&mut self.guards);
        let saved_may_raise = std::mem::take(&mut self.may_raise);
        let saved_cond = self.cond_depth;
        let saved_branch = self.branch_depth;
        self.branch_depth = 0;
        let saved_exh = self.exh_log.take();
        let saved_active = std::mem::take(&mut self.active);
        let saved_abf = std::mem::take(&mut self.active_by_func);
        let saved_rec_entry = std::mem::take(&mut self.rec_entry);
        let saved_rec_widened = std::mem::take(&mut self.rec_widened);
        let saved_rec_hit = std::mem::take(&mut self.rec_hit);
        let saved_recent = std::mem::take(&mut self.recent_stack);
        if let Some(f0) = saved_frames.first() {
            self.frames.push(Frame { unroll: Vec::new(), kind: f0.kind, scope: f0.scope, module: f0.module, ctx: f0.ctx.clone(), single: f0.single, single_nl: f0.single_nl, func: f0.func, cells: f0.cells.clone(), class_oid: f0.class_oid, overlays: Vec::new(), loop_depth: f0.loop_depth, label: f0.label.clone(), symbolic: f0.symbolic });
        }
        self.cond_depth = 0;
        self.sum_stack.push(foid);
        self.sum_ctxs.push(ctx.clone());
        self.sum_reqs.push(IndexMap::new());
        self.sum_events.push(IndexMap::new());
        self.sum_obs.push(Vec::new());
        self.sum_fact_obs.push(Vec::new());
        self.raise_stack.push(false);
        self.sum_obj_mark.push(self.objects.len() as ObjId);
        self.sum_deps.push(FxHashSet::default());
        let verdict_mark = self.verdicts.len();
        let notes_mark = self.notes.len();
        let mut summary: Option<Rc<Summary>> = None;
        for rnd in 0..MAX_SUMMARY_ROUNDS {
            if rnd >= 1 && self.opts.scc {
                // callees summarised against the previous round's partial summary are redone
                self.summary_provisional.retain(|_, (_, deps)| !deps.contains(&foid));
            }
            for v in self.verdicts[verdict_mark..].iter() {
                self.verdict_keys.remove(&v.dedupe_key());
            }
            self.verdicts.truncate(verdict_mark);
            for n in self.notes[notes_mark..].iter() {
                self.notes_set.remove(n);
            }
            self.notes.truncate(notes_mark);
            *self.sum_reqs.last_mut().unwrap() = IndexMap::new();
            *self.sum_obs.last_mut().unwrap() = Vec::new();
            *self.sum_fact_obs.last_mut().unwrap() = Vec::new();
            *self.sum_events.last_mut().unwrap() = IndexMap::new();
            *self.raise_stack.last_mut().unwrap() = false;
            let obj_mark = *self.sum_obj_mark.last().unwrap();
            let mut st = State::new();
            st.symbolic = true;
            // module bindings and the contents of module-level literal containers are entry facts
            let module_oids: Vec<ObjId> = self.modules.values().copied().collect();
            // (nested ones too: `TABLE[1][0]`; bounded)
            let mut seen_c: FxHashSet<ObjId> = FxHashSet::default();
            for moid in module_oids {
                for (k, v) in state.obj_slots(moid, Ns::Attr) {
                    st.slots.set(&moid, (Ns::Attr, k), v.clone());
                    let mut work: Vec<ObjId> = v.pts.iter().collect();
                    while let Some(x) = work.pop() {
                        if seen_c.len() > 4096 || !seen_c.insert(x) {
                            continue;
                        }
                        let xk = self.kind(x);
                        if matches!(xk, Kind::List | Kind::Tuple | Kind::Set) || (xk == Kind::Dict && self.module_done) {
                            if !crate::interp::disabled("nestedglobals") {
                                for (_, v2) in state.slots_of(x) {
                                    work.extend(v2.pts.iter());
                                }
                            }
                        }
                        if matches!(xk, Kind::List | Kind::Tuple | Kind::Set) {
                            for ((ns2, k2), v2) in state.slots_of(x) {
                                st.slots.set(&x, (ns2, k2), v2);
                            }
                            if let Some(e) = state.elems.get(&x) {
                                st.elems.insert(x, e);
                            }
                        } else if xk == Kind::Dict && self.module_done {
                            for ns2 in [Ns::Item, Ns::Attr] {
                                if let Some(r) = state.rows.get(&(x, ns2)) {
                                    st.set_row(x, ns2, r);
                                }
                            }
                            for ((ns2, k2), v2) in state.slots_of(x) {
                                st.slots.set(&x, (ns2, k2), v2);
                            }
                            if let Some(e) = state.elems.get(&x) {
                                st.elems.insert(x, e);
                            }
                        }
                    }
                }
            }
            for (cid, cv) in code_cells.iter() {
                st.slots.set(cid, (Ns::Cell, key_str_lit("v")), cv.clone());
            }
            for (cid, c) in &typed_cells {
                self.cell_classes.insert(*cid, *c);
            }
            let mut fr = Frame::new(FrameKind::Function, fi.scope, fi.module, &ctx, true, &fi.qualname);
            fr.func = Some(foid);
            fr.symbolic = true;
            self.frames.push(fr);
            let mut ph_by_name: IndexMap<StrId, ObjId> = IndexMap::new();
            let params = fnode.params();
            let mut positional: Vec<&ruff_python_ast::Parameter> = Vec::new();
            let mut kwonly: Vec<&ruff_python_ast::Parameter> = Vec::new();
            if let Some(p) = params {
                positional.extend(p.posonlyargs.iter().chain(p.args.iter()).map(|x| &x.parameter));
                kwonly.extend(p.kwonlyargs.iter().map(|x| &x.parameter));
            }
            let mut self_ph: Option<ObjId> = None;
            let sig_cls = |name: &str| -> Option<ObjId> { sig.iter().find(|(n, _)| n == name).map(|(_, c)| *c) };
            if fi.cls.is_some() && !fi.is_static.get() && !positional.is_empty() {
                let sigc = sig_cls(positional[0].name.id.as_str());
                let self_cls = sigc.or(fi.cls);
                let sp = self.new_placeholder(PhKind::SelfPh, foid, positional[0].name.id.as_str(), None, Ns::Item, None, self_cls, "");
                if sigc.is_some() {
                    self.ph_info(sp).exact.set(true);
                }
                self_ph = Some(sp);
                ph_by_name.insert(s(positional[0].name.id.as_str()), sp);
                positional.remove(0);
            }
            let mut args: Vec<Value> = Vec::new();
            for p in &positional {
                let cls = match &p.annotation {
                    Some(ann) => self.annotation_class(ann, &fi, state),
                    None => sig_cls(p.name.id.as_str()),
                };
                let ph = self.new_placeholder(PhKind::Param, foid, p.name.id.as_str(), None, Ns::Item, None, cls, "");
                if p.annotation.is_none() && cls.is_some() {
                    self.ph_info(ph).exact.set(true);
                }
                ph_by_name.insert(s(p.name.id.as_str()), ph);
                args.push(Value::with_kv(ObjSet::one(ph), KeyVal::Param(ph)));
            }
            let mut kwargs: IndexMap<StrId, Value> = IndexMap::new();
            for p in &kwonly {
                let cls = match &p.annotation {
                    Some(ann) => self.annotation_class(ann, &fi, state),
                    None => sig_cls(p.name.id.as_str()),
                };
                let ph = self.new_placeholder(PhKind::Param, foid, p.name.id.as_str(), None, Ns::Item, None, cls, "");
                if p.annotation.is_none() && cls.is_some() {
                    self.ph_info(ph).exact.set(true);
                }
                ph_by_name.insert(s(p.name.id.as_str()), ph);
                kwargs.insert(s(p.name.id.as_str()), Value::with_kv(ObjSet::one(ph), KeyVal::Param(ph)));
            }
            for ph in ph_by_name.values() {
                st.set_row(*ph, Ns::Item, Row::entry(rho(*ph, Ns::Item)));
                st.set_row(*ph, Ns::Attr, Row::entry(rho(*ph, Ns::Attr)));
            }
            {
                let mi = self.module_info(fi.module);
                let mut cells: Vec<String> = mi.scopes.get(fi.scope).cells.iter().cloned().collect();
                cells.sort();
                for name in cells {
                    let cid = self.alloc_raw(Kind::Cell, &format!("cell:{}.{}", fi.qualname, name), &ctx, true, &format!("cell {}", name), Payload::None);
                    self.frame_mut().cells.insert(s(&name), cid);
                }
                for (n, c) in fi.closure.iter() {
                    self.frame_mut().cells.insert(*n, *c);
                }
            }
            st.env = IndexMap::new();
            let self_v = self_ph.map(|sp| Value::with_kv(ObjSet::one(sp), KeyVal::Param(sp)));
            let fake = Node::synthetic(fnode.node().module, fnode.node().line);
            // the entry snapshot is taken before the parameters are bound, so that cells holding
            // captured parameters and the *args / **kwargs containers are part of the footprint
            // and get instantiated at each call site
            let base_early = Some(st.copy());
            self.bind_params(&fi, &self_v, &args, &kwargs, &[], &[], &mut st, &fake);
            if let Some(p) = params {
                if let Some(va) = &p.vararg {
                    let ph = self.new_placeholder(PhKind::Vararg, foid, va.name.id.as_str(), None, Ns::Item, None, None, "");
                    ph_by_name.insert(s(va.name.id.as_str()), ph);
                    self.store_name(s(va.name.id.as_str()), Value::with_kv(ObjSet::one(ph), KeyVal::Param(ph)), &mut st);
                }
                if let Some(kw) = &p.kwarg {
                    let ph = self.new_placeholder(PhKind::Kwarg, foid, kw.name.id.as_str(), None, Ns::Item, None, None, "");
                    ph_by_name.insert(s(kw.name.id.as_str()), ph);
                    st.set_row(ph, Ns::Item, Row::entry(rho(ph, Ns::Item)));
                    self.store_name(s(kw.name.id.as_str()), Value::with_kv(ObjSet::one(ph), KeyVal::Param(ph)), &mut st);
                }
            }
            let base = match base_early {
                Some(b) => b,
                None => st.copy(),
            };
            // run the body
            match fnode {
                FuncNode::Lambda(l) => {
                    let v = self.eval(&l.body, &mut st);
                    st.ret = Some(v);
                    st.record_exit();
                    st.reachable = false;
                }
                FuncNode::Def(f) => self.exec_block(&f.body, &mut st),
            }
            let mut ret = st.ret.clone().unwrap_or_default();
            if st.reachable {
                ret = ret.join(&self.none_v);
            }
            if let Some(ex) = st.exits.take() {
                let mut exs = ex.copy_ref();
                st = if st.reachable { st.join(&mut exs) } else { exs };
                st.exits = None;
            }
            st.reachable = true;
            if let Some(gen) = fi.generator.get() {
                if let FuncNode::Def(f) = fnode {
                    if crate::interp::is_generator(&f.body) {
                        ret = Value::obj(gen);
                    }
                }
            }
            self.frames.pop();
            // bound methods / method values / views made in the body hold receivers that are
            // templates themselves: cloned with them (their payloads are remapped)
            let old_filter = crate::interp::disabled("boundtmpl");
            let templates: FxHashSet<ObjId> = (obj_mark..self.objects.len() as ObjId).filter(|o| !matches!(self.kind(*o), Kind::Ph | Kind::External | Kind::Super) && !(old_filter && matches!(self.kind(*o), Kind::Method | Kind::Bound | Kind::View))).collect();
            let rel_full: Vec<Verdict> = self.verdicts[verdict_mark..].to_vec();
            let mut seen_rel: FxHashSet<(String, u32, u32, String, Tier, ObjId, Chain)> = FxHashSet::default();
            let mut rel_list: Vec<Verdict> = Vec::new();
            for mut v in rel_full {
                self.verdict_keys.remove(&v.dedupe_key());
                if v.chain.len() > MAX_REL_CHAIN {
                    v.chain.truncate(MAX_REL_CHAIN);
                }
                let dk = v.dedupe_key();
                if seen_rel.insert(dk) {
                    rel_list.push(v);
                }
            }
            self.verdicts.truncate(verdict_mark);
            let body_notes: Vec<String> = self.notes[notes_mark..].to_vec();
            for n in &body_notes {
                self.notes_set.remove(n);
            }
            self.notes.truncate(notes_mark);
            let symreqs: Vec<SymReq> = self.sum_reqs.last().unwrap().values().cloned().collect();
            let events: Vec<Event> = self.sum_events.last().unwrap().values().cloned().collect();
            let desc = {
                let mut d = ctx.clone();
                for (c, k) in &typed_cells {
                    d.push_str(&format!(" cell{}:{}", c, self.obj(*k).label));
                }
                for (c, v) in code_cells.iter() {
                    d.push_str(&format!(" cell{}:code{:?}", c, v.pts.to_vec()));
                }
                for (n, c) in &sig {
                    d.push_str(&format!(" {}:{}", n, self.obj(*c).label));
                }
                d
            };
            let mut summ = Summary {
                foid,
                desc,
                ph_by_name: ph_by_name.clone(),
                ph_self: self_ph,
                exit: st,
                ret,
                symreqs,
                rel_verdicts: rel_list,
                obs: self.sum_obs.last().cloned().unwrap_or_default(),
                fact_obs: self.sum_fact_obs.last().cloned().unwrap_or_default(),
                notes: body_notes,
                templates,
                events,
                rounds: rnd + 1,
                partial: Cell::new(false),
                rows_fp: Vec::new(),
                slots_fp: Vec::new(),
                elems_fp: Vec::new(),
                raises: self.raise_stack.last().copied().unwrap_or(false),
            };
            summ.compute_footprint(self, &base);
            if self.sum_hit.contains(&foid) {
                let prev = self.sum_partial.get(&foid).cloned();
                if let Some(prev) = &prev {
                    if rnd >= 1 {
                        if !prev.exit.same(&summ.exit) {
                            let mut pe = prev.exit.copy_ref();
                            summ.exit = summ.exit.join(&mut pe);
                        }
                        summ.ret = summ.ret.join(&prev.ret);
                        summ.compute_footprint(self, &base);
                    }
                }
                let converged = match &prev {
                    Some(p) => p.ret == summ.ret && p.exit.same(&summ.exit) && p.symreqs.len() == summ.symreqs.len(),
                    None => false,
                };
                summ.partial.set(true);
                let rc = Rc::new(summ);
                self.sum_partial.insert(foid, rc.clone());
                summary = Some(rc.clone());
                if converged {
                    rc.partial.set(false);
                    break;
                }
                if debug_enabled() && rnd >= 2 {
                    eprintln!("[rowan] round {} for {}: not converged", rnd + 1, fi.qualname);
                }
                continue;
            }
            summary = Some(Rc::new(summ));
            break;
        }
        // not converged within the rounds: widened, but final for the callers from here on
        if let Some(s) = &summary {
            if !crate::interp::disabled("partialclear") {
                s.partial.set(false);
            }
        }
        self.sum_stack.pop();
        self.sum_ctxs.pop();
        self.sum_reqs.pop();
        self.sum_obs.pop();
        self.sum_fact_obs.pop();
        self.sum_events.pop();
        self.raise_stack.pop();
        self.sum_obj_mark.pop();
        let deps = self.sum_deps.pop().unwrap_or_default();
        self.sum_hit.remove(&foid);
        self.sum_partial.remove(&foid);
        if self.opts.scc {
            // foid's summary is final now: everything computed against its partial one is stale
            self.summary_provisional.retain(|_, (_, d)| !d.contains(&foid));
        }
        self.frames = saved_frames;
        self.call_chain = saved_chain;
        self.loop_stack = saved_loops;
        self.guards = saved_guards;
        self.may_raise = saved_may_raise;
        self.cond_depth = saved_cond;
        self.branch_depth = saved_branch;
        self.exh_log = saved_exh;
        self.active = saved_active;
        self.active_by_func = saved_abf;
        self.rec_entry = saved_rec_entry;
        self.rec_widened = saved_rec_widened;
        self.rec_hit = saved_rec_hit;
        self.recent_stack = saved_recent;
        match summary {
            Some(rc) => {
                rc.partial.set(false);
                if self.opts.scc && !deps.is_empty() {
                    self.summary_provisional.insert(skey, (rc.clone(), deps));
                } else {
                    self.summary_table.insert(skey, rc.clone());
                }
                Some(rc)
            }
            None => None,
        }
    }

    pub fn annotation_class(&mut self, ann: &'static ruff_python_ast::Expr, fi: &FuncInfo, state: &mut State) -> Option<ObjId> {
        let node = Analyzer::strip_annotation(ann)?;
        if !matches!(node, ruff_python_ast::Expr::Name(_) | ruff_python_ast::Expr::Attribute(_)) {
            return None;
        }
        let mi = self.module_info(fi.module);
        self.frames.push(Frame::new(FrameKind::Module, mi.scopes.module(), fi.module, "", true, "<ann>"));
        let v = self.eval(node, state);
        self.frames.pop();
        let found = v.pts.iter().find(|o| self.kind(*o) == Kind::Class);
        found
    }

    // ------------------------------------------------------- fallback decision
    /// Actual value a placeholder stands for at this call (parameters and paths).
    pub fn binding_of(&mut self, summary: &Summary, ph: ObjId, bind: &IndexMap<ObjId, Value>, state: &mut State, memo: &mut FxHashMap<ObjId, Value>) -> Value {
        if let Some(v) = memo.get(&ph) {
            return v.clone();
        }
        memo.insert(ph, Value::new());
        let v: Value = if let Some(b) = bind.get(&ph) {
            b.clone()
        } else if self.kind(ph) != Kind::Ph {
            Value::obj(ph)
        } else {
            let p = self.ph_info(ph);
            match p.kind {
                PhKind::Narrow => {
                    let pv = self.binding_of(summary, p.parent.unwrap(), bind, state, memo);
                    let c = p.cls.get().unwrap();
                    Value::with_kv(pv.pts.filter(|o| self.may_be_instance(o, c)), pv.kv.clone())
                }
                PhKind::Path => {
                    let parents: Vec<ObjId> = match p.parent {
                        None => Vec::new(),
                        Some(par) if self.kind(par) == Kind::Ph => self.binding_of(summary, par, bind, state, memo).pts.to_vec(),
                        Some(par) if summary.templates.contains(&par) => Vec::new(),
                        Some(par) => vec![par],
                    };
                    let mut v = Value::new();
                    for par in parents {
                        if summary.templates.contains(&par) {
                            continue;
                        }
                        let key = p.key.unwrap_or(STAR);
                        if key == STAR {
                            let it = self.iterate(&Value::obj(par), &Node::none(), state);
                            v = v.join(&it.elem.flat());
                        } else if let Some(kph) = param_slot_of(key) {
                            // `d[k]` with `k` a parameter: the slots named by the literals `k`
                            // is bound to at this call site, else every slot
                            let kv = self.binding_of(summary, kph, bind, state, memo).kv;
                            let lits = kv_literals(&kv);
                            if lits.is_empty() {
                                v = v.join(&state.all_slots(par, p.ns));
                            } else {
                                for k in lits.iter() {
                                    v = v.join(&state.slot(par, p.ns, k));
                                }
                                v = v.join(&state.slot(par, p.ns, STAR));
                            }
                        } else if intern::with_key(key, |k| matches!(k, intern::Key::Tuple(_))) {
                            v = v.join(&state.all_slots(par, p.ns));
                        } else {
                            v = v.join(&state.slot(par, p.ns, key));
                        }
                    }
                    v
                }
                _ => Value::new(),
            }
        };
        // a value holding this summary's own placeholders (a cell holding a parameter): the
        // bindings of those placeholders
        let v = if !crate::interp::disabled("bindsub") && (v.pts.iter().any(|o| bind.contains_key(&o)) || v.kv.as_param().map(|p| bind.contains_key(&p)).unwrap_or(false)) {
            let mut out = Value::with_kv(v.pts.filter(|o| !bind.contains_key(&o)), if v.kv.as_param().map(|p| bind.contains_key(&p)).unwrap_or(false) { crate::domain::BOT } else { v.kv.clone() });
            for o in v.pts.iter() {
                if let Some(b) = bind.get(&o) {
                    out = out.join(b);
                }
            }
            if let Some(p) = v.kv.as_param() {
                if let Some(b) = bind.get(&p) {
                    out = out.join(b);
                }
            }
            out
        } else {
            v
        };
        memo.insert(ph, v.clone());
        v
    }

    /// A callee summary instantiated inside another summary: the callee's events whose
    /// placeholders stand for this summary's placeholders become events of this summary, so
    /// that the outer call site tabulates (and then analyses the callee concretely).
    pub fn propagate_events(&mut self, summary: &Summary, bind: &IndexMap<ObjId, Value>, state: &mut State) {
        let mut memo: FxHashMap<ObjId, Value> = FxHashMap::default();
        for ev in &summary.events {
            let v = self.binding_of(summary, ev.ph, bind, state, &mut memo);
            let mut phs: Vec<ObjId> = v.pts.iter().filter(|o| self.kind(*o) == Kind::Ph).collect();
            for o in v.pts.iter() {
                if let Some(src) = self.obj(o).copied_from {
                    if self.kind(src) == Kind::Ph && !phs.contains(&src) {
                        phs.push(src);
                    }
                }
            }
            if let Some(p) = v.kv.as_param() {
                if !phs.contains(&p) {
                    phs.push(p);
                }
            }
            if phs.is_empty() && v.empty() {
                let mut ph = ev.ph;
                while self.kind(ph) == Kind::Ph {
                    let p = self.ph_info(ph);
                    let Some(par) = p.parent else { break };
                    if !matches!(p.kind, PhKind::Path | PhKind::Narrow) {
                        break;
                    }
                    let pv = self.binding_of(summary, par, bind, state, &mut memo);
                    let ops: Vec<ObjId> = pv.pts.iter().filter(|o| self.kind(*o) == Kind::Ph).collect();
                    if !ops.is_empty() {
                        phs = ops;
                        break;
                    }
                    ph = par;
                }
            }
            for p in phs {
                self.record_event(ev.kind, p);
            }
        }
    }

    pub fn should_tabulate(&mut self, summary: &Summary, bind: &IndexMap<ObjId, Value>, state: &mut State) -> bool {
        let Some(ev) = self.tabulation_reason(summary, bind, state) else {
            if debug_enabled() {
                eprintln!("summary used for {} ({} events{}) [{}]", self.func_info(summary.foid).qualname, summary.events.len(), if summary.partial.get() { ", partial" } else { "" }, summary.desc);
            }
            return false;
        };
        let kkey: &'static str = if ev.kind == "call" || ev.kind == "dispatch" { ev.kind } else { "lit" };
        let key = (summary.foid, kkey);
        let n = self.tab_count.get(&key).copied().unwrap_or(0);
        let budget = if kkey == "call" || kkey == "dispatch" { crate::interp::limit("call", MAX_CALL_TABULATIONS) } else { crate::interp::limit("tab", MAX_TABULATIONS) };
        let over_time = self.over_deadline();
        if n >= budget || over_time {
            if debug_enabled() {
                eprintln!("budget: summary used for {} ({} on {}; {})", self.func_info(summary.foid).qualname, ev.kind, self.obj(ev.ph).label, if over_time { "deadline" } else { kkey });
            }
            return false;
        }
        if debug_enabled() {
            eprintln!("tabulate {} because of {} on {} [{}]", self.func_info(summary.foid).qualname, ev.kind, self.obj(ev.ph).label, summary.desc);
        }
        self.tab_count.insert(key, n + 1);
        true
    }

    fn class_resolves(&self, oid: ObjId, name: &str) -> bool {
        if self.kind(oid) != Kind::Instance {
            return false;
        }
        match self.instance_class(oid) {
            // (a field's class-level default is shadowed by the instance's attribute)
            Some(c) => self.class_lookup(c, name).is_some() && !self.dataclass_fields_mro(c).iter().any(|(f, _)| f == name),
            None => false,
        }
    }

    pub fn tabulation_reason(&mut self, summary: &Summary, bind: &IndexMap<ObjId, Value>, state: &mut State) -> Option<Event> {
        if summary.partial.get() {
            return None;
        }
        let mut memo: FxHashMap<ObjId, Value> = FxHashMap::default();
        for ev in &summary.events {
            let mut v = self.binding_of(summary, ev.ph, bind, state, &mut memo);
            if v.empty() && ev.kind == "call" {
                let mut ph = ev.ph;
                loop {
                    if !v.empty() || self.kind(ph) != Kind::Ph {
                        break;
                    }
                    let p = self.ph_info(ph);
                    if !matches!(p.kind, PhKind::Path | PhKind::Narrow) || p.parent.is_none() {
                        break;
                    }
                    let key = p.key;
                    ph = p.parent.unwrap();
                    let pv = self.binding_of(summary, ph, bind, state, &mut memo);
                    let useful = pv.pts.filter(|o| {
                        self.kind(o) != Kind::Ph
                            && (self.kind(o) != Kind::Instance || key.and_then(intern::key_as_str).map(|k| self.class_resolves(o, &k)).unwrap_or(false))
                    });
                    v = if !useful.is_empty() { Value::with_kv(useful, pv.kv.clone()) } else { Value::new() };
                }
            }
            if v.empty() && matches!(ev.kind, "truth" | "narrow" | "iter") && self.kind(ev.ph) == Kind::Ph {
                let p = self.ph_info(ev.ph);
                if p.kind == PhKind::Path {
                    if let Some(par) = p.parent {
                        let pv = self.binding_of(summary, par, bind, state, &mut memo);
                        if pv.pts.iter().any(|o| self.kind(o) != Kind::Ph) {
                            return Some(ev.clone());
                        }
                    }
                }
            }
            if v.empty() {
                continue;
            }
            let concrete: Vec<ObjId> = v.pts.iter().filter(|o| self.kind(*o) != Kind::Ph && !self.obj(*o).copied_from.map(|s| self.kind(s) == Kind::Ph).unwrap_or(false)).collect();
            match ev.kind {
                "call" => {
                    if !concrete.is_empty() {
                        return Some(ev.clone());
                    }
                }
                "dispatch" => {
                    if concrete.iter().any(|o| matches!(self.kind(*o), Kind::Function | Kind::Bound | Kind::Class | Kind::Method | Kind::Partial | Kind::Instance | Kind::External)) {
                        return Some(ev.clone());
                    }
                }
                "iter" => {
                    for o in &concrete {
                        let k = self.kind(*o);
                        // known items, or concrete element objects (copies and dispatch on
                        // them are precise then); not open input rows
                        if matches!(k, Kind::List | Kind::Tuple | Kind::Set) {
                            let elems = state.slot(*o, Ns::Elem, STAR);
                            if self.llen(*o, state).is_some() || elems.pts.iter().any(|e| matches!(self.kind(e), Kind::Dict | Kind::Instance | Kind::List | Kind::Tuple)) {
                                return Some(ev.clone());
                            }
                        }
                        if k == Kind::Dict && self.literal_dict_items(*o, state).is_some() {
                            return Some(ev.clone());
                        }
                        if matches!(k, Kind::View | Kind::Zip | Kind::Pair | Kind::Enumerate) {
                            return Some(ev.clone());
                        }
                    }
                    if v.pts.is_empty() && v.kv.is_literal() {
                        return Some(ev.clone());
                    }
                }
                "narrow" | "truth" => {
                    if v.kv.is_literal() || !concrete.is_empty() {
                        return Some(ev.clone());
                    }
                }
                // an unknown key tested against closed dicts: narrows to their keys anywhere
                "keynarrow" => {
                    if !v.empty() && !v.pts.iter().any(|o| self.kind(o) == Kind::Ph) && v.kv.as_param().is_none() {
                        return Some(ev.clone());
                    }
                }
                // an instance that holds an attribute of the name of a class data attribute
                "clsattr" => {
                    for o in &concrete {
                        if self.kind(*o) != Kind::Instance {
                            continue;
                        }
                        let row = state.row(*o, Ns::Attr);
                        let Some(cls) = self.instance_class(*o) else { continue };
                        if row.open() || row.present.iter().any(|k| intern::key_as_str(k).is_some_and(|n| self.class_lookup(cls, &n).is_some())) {
                            return Some(ev.clone());
                        }
                    }
                }
                "isinstance" | "concrete" | "guarded" => {
                    if !concrete.is_empty() {
                        return Some(ev.clone());
                    }
                    // a literal (a column name, a key) decides a model's answer too
                    if ev.kind == "concrete" && v.pts.is_empty() && v.kv.is_literal() && !crate::interp::disabled("concretelit") {
                        return Some(ev.clone());
                    }
                    // so does a text a model knows the structure of (a csv header, a dumped value)
                    if ev.kind == "concrete" && v.pts.is_empty() && !crate::interp::disabled("concretetext") {
                        if let KeyVal::Data(d) = &v.kv {
                            let t = intern::str_get(*d);
                            if ["csvhdr:", "pyjsontext:", "pickletext:", "jsontext:", "pydjson:"].iter().any(|p| t.starts_with(p)) {
                                return Some(ev.clone());
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }

    pub fn may_be_instance(&self, oid: ObjId, cls: ObjId) -> bool {
        if self.kind(oid) == Kind::Instance {
            return match self.instance_class(oid) {
                Some(c) => self.is_subclass(c, cls),
                None => false,
            };
        }
        true
    }

    fn indexes_without_getitem(&self, oid: ObjId) -> bool {
        if self.kind(oid) != Kind::Instance {
            return false;
        }
        let Some(c) = self.instance_class(oid) else { return false };
        let Some(ci) = self.class_info_opt(c) else { return false };
        if ci.typed_dict || ci.named_tuple || ci.open_base {
            return false;
        }
        self.class_lookup(c, "__getitem__").is_none()
    }

    // --------------------------------------------------------------- instantiate
    pub fn instantiate(&mut self, summary: &Rc<Summary>, bind: &IndexMap<ObjId, Value>, state: &mut State, node: &Node, call_ctx: &str, ctx_single: bool) -> Value {
        self.work += 1 + summary.templates.len();
        let t0 = std::time::Instant::now();
        let r = self.instantiate_inner(summary, bind, state, node, call_ctx, ctx_single);
        if debug_enabled() {
            let dt = t0.elapsed().as_secs_f64();
            if dt > 0.5 {
                eprintln!("[rowan] slow instantiation {} dt={:.1}s rows={} symreqs={} templates={}", self.func_info(summary.foid).qualname, dt, summary.rows_fp.len(), summary.symreqs.len(), summary.templates.len());
            }
        }
        r
    }

    fn instantiate_inner(&mut self, summary: &Rc<Summary>, bind: &IndexMap<ObjId, Value>, state: &mut State, node: &Node, call_ctx: &str, ctx_single: bool) -> Value {
        let mut pre = state.copy();
        let mut chain = self.call_chain.clone();
        if summary.partial.get() && !chain.is_empty() {
            chain.pop();
        }
        let in_summary = !self.sum_stack.is_empty();
        let mut inst = Inst {
            summary: summary.clone(),
            bind: bind.clone(),
            call_ctx: call_ctx.to_string(),
            ctx_single,
            obj_mark: self.objects.len() as ObjId,
            in_summary,
            clones: IndexMap::new(),
            collided: FxHashSet::default(),
            resolved: FxHashMap::default(),
            born: FxHashMap::default(),
            kv_cache: FxHashMap::default(),
            erow_cache: FxHashMap::default(),
            conc_cache: FxHashMap::default(),
            new_clones: FxHashSet::default(),
        };
        // 1. clones for all templates
        let mut templates: Vec<ObjId> = summary.templates.iter().copied().collect();
        templates.sort_unstable();
        for t in &templates {
            inst.clone_obj(self, state, &pre, *t);
        }
        // 2. rows
        let mut targets_count: FxHashMap<(ObjId, Ns), usize> = FxHashMap::default();
        let mut plan: Vec<(ObjId, Ns, Row, ObjSet)> = Vec::new();
        for (oid, ns, expr) in &summary.rows_fp {
            let tg = inst.resolve(self, state, &pre, *oid);
            for a in tg.iter() {
                *targets_count.entry((a, *ns)).or_insert(0) += 1;
            }
            plan.push((*oid, *ns, expr.clone(), tg));
        }
        let mut written: FxHashSet<(ObjId, Ns)> = FxHashSet::default();
        for (oid, ns, expr, tg) in plan {
            if tg.is_empty() {
                continue;
            }
            let is_template = summary.templates.contains(&oid);
            let many = tg.len() > 1;
            let mut per_target: Vec<(ObjId, Row)> = Vec::new();
            if !many || !expr.mentions(oid) {
                let c0 = inst.concretise(self, state, &pre, &expr, None);
                for a in tg.iter() {
                    per_target.push((a, c0.clone()));
                }
            } else {
                for (focus, members) in inst.focus_groups(self, state, &pre, oid, &tg) {
                    let c = inst.concretise(self, state, &pre, &expr, Some(focus));
                    for a in members {
                        per_target.push((a, c.clone()));
                    }
                }
            }
            for (a, concrete) in per_target {
                // keys known absent before the call stay so unless the callee added them
                let concrete = if pre.has_row(a, ns) && !crate::interp::disabled("keepabsent") {
                    let before = pre.row(a, ns);
                    let lost = before.absent.difference(&concrete.present).difference(&concrete.absent);
                    if lost.is_empty() || !expr.mentions(oid) && !expr.is_entry() {
                        concrete
                    } else {
                        let mut d = concrete.data();
                        d.absent = d.absent.union(&lost);
                        Row::from_data(d)
                    }
                } else {
                    concrete
                };
                let strong = targets_count.get(&(a, ns)).copied().unwrap_or(0) == 1
                    && (inst.new_clones.contains(&a)
                        || (is_template && (self.strongable(a) || !expr.symbolic()))
                        || (!many && !is_template && (self.strongable(a) || self.fresh_init.contains(&a))));
                if strong {
                    state.set_row(a, ns, concrete);
                } else if written.contains(&(a, ns)) {
                    let j = state.row(a, ns).join(&concrete);
                    state.set_row(a, ns, j);
                } else if pre.has_row(a, ns) {
                    state.set_row(a, ns, pre.row(a, ns).join(&concrete));
                } else {
                    state.set_row(a, ns, concrete);
                }
                written.insert((a, ns));
            }
        }
        inst.erow_cache.clear();
        inst.conc_cache.clear();
        // 3. slots
        let mut appends: Vec<(ObjId, i64, Value)> = Vec::new();
        for ((oid, ns, k), v) in &summary.slots_fp {
            if let Some(i) = append_slot_of(*k) {
                if *ns == Ns::Pos {
                    appends.push((*oid, i, v.clone()));
                    continue;
                }
            }
            let tg = inst.resolve(self, state, &pre, *oid);
            if tg.is_empty() {
                continue;
            }
            let nv = inst.remap_value(self, state, &pre, v);
            let is_template = summary.templates.contains(oid);
            let keys: Vec<KeyId> = match param_slot_of(*k) {
                Some(ph) => {
                    let kv = inst.kv_of(self, state, &pre, ph);
                    let lits = kv_literals(&kv);
                    if lits.is_empty() {
                        vec![STAR]
                    } else {
                        intern::sorted_by_str(lits.iter())
                    }
                }
                None => vec![*k],
            };
            for a in tg.iter() {
                // a store under an unknown key (STAR: the parameter's value was not a
                // literal) may have hit any key: never strong, it would wipe the others
                let strong = *ns != Ns::Elem && ((is_template && !inst.collided.contains(&a)) || (tg.len() == 1 && self.strongable(a) && targets_count.get(&(a, *ns)).copied().unwrap_or(1) == 1 && keys.len() == 1));
                for kk in &keys {
                    state.set_slot(a, *ns, *kk, nv.clone(), strong);
                }
            }
        }
        // 3b. appends through placeholders, in order: positions of the actual lists
        appends.sort_by_key(|(o, i, _)| (*o, if *i < 0 { i64::MAX } else { *i }));
        for (oid, i, v) in &appends {
            let tg = inst.resolve(self, state, &pre, *oid);
            let nv = inst.remap_value(self, state, &pre, v);
            for a in tg.iter() {
                // through a placeholder of the summary being built: an append of its own (in
                // order, positional while this call is straight-line code)
                if self.kind(a) == Kind::Ph && !crate::interp::disabled("phappend") {
                    let positional = *i >= 0 && tg.len() == 1 && self.frame().loop_depth == 0 && self.cond_depth == 0;
                    let j = if positional { (0..16).find(|j| !state.has_slot(a, Ns::Pos, crate::interp::access::append_slot_key(*j))).unwrap_or(-1) } else { -1 };
                    let k = crate::interp::access::append_slot_key(j);
                    let strong = j >= 0;
                    state.set_slot(a, Ns::Pos, k, nv.clone(), strong);
                    continue;
                }
                if !matches!(self.kind(a), Kind::List | Kind::Tuple | Kind::Set) {
                    continue;
                }
                let ll = self.llen(a, state);
                if *i >= 0 && tg.len() == 1 && self.strongable(a) {
                    if let Some(n) = ll {
                        if n < 16 {
                            state.set_slot(a, Ns::Pos, intern::key_int(n as i64), nv.clone(), true);
                            self.set_llen(a, state, Some(n + 1));
                            continue;
                        }
                    }
                }
                if ll.is_some() {
                    state.drop_obj_ns(a, Ns::Pos);
                }
                self.set_llen(a, state, None);
            }
        }
        // 3c. the elements the body appended last
        if !summary.exit.lasts.is_empty() && !crate::interp::disabled("lastelem") {
            for (oid, v) in summary.exit.lasts.iter() {
                let tg = inst.resolve(self, state, &pre, *oid);
                if tg.len() != 1 {
                    continue;
                }
                let a = tg.first().unwrap();
                if self.kind(a) == Kind::List && self.obj(a).single {
                    let nv = inst.remap_value(self, state, &pre, v);
                    state.set_last(a, nv);
                }
            }
        }
        // 4. elems
        for (oid, kv) in &summary.elems_fp {
            let tg = inst.resolve(self, state, &pre, *oid);
            for a in tg.iter() {
                let nk = inst.remap_value(self, state, &pre, &Value::kv_only(kv.clone())).kv;
                if (inst.clones.values().any(|c| *c == a) || summary.templates.contains(oid)) && !inst.collided.contains(&a) {
                    state.elems.insert(a, nk);
                } else {
                    let nv = match state.elems.get(&a) {
                        None => nk,
                        Some(cur) => kv_join(&cur, &nk),
                    };
                    state.elems.insert(a, nv);
                }
            }
        }
        // 5. requirements on concrete sources created in the callee
        for (src, d) in summary.exit.reqs.items() {
            for (k, info) in d.iter() {
                let site = info.sites.first().unwrap_or_else(|| s(&format!("L{}", node.line)));
                state.require(src, *k, site, info.alternatives.clone(), info.caveats.clone(), info.alt_keys.clone());
                if !info.always {
                    let tgt = state.reqs.writable(&src);
                    if let Some(cur) = tgt.get(k).cloned() {
                        tgt.insert(*k, crate::domain::ReqInfo { always: false, ..cur });
                    }
                }
            }
        }
        // 6. symbolic requirements → verdicts / requirements now
        let mut failed: Vec<(ObjId, Ns, KeyId)> = Vec::new();
        let mut sure_fail = false;
        for sr in &summary.symreqs {
            let tg = inst.resolve(self, state, &pre, sr.oid);
            if tg.is_empty() {
                continue;
            }
            let kv = match &sr.key_kv {
                KeyVal::Param(ph) => inst.kv_of(self, state, &pre, *ph),
                KeyVal::KeysOf(_) => inst.remap_value(self, state, &pre, &Value::kv_only(sr.key_kv.clone())).kv,
                other => other.clone(),
            };
            let mut tg = tg;
            if let Some(c) = sr.cls {
                tg = tg.filter(|a| self.may_be_instance(a, c));
            }
            tg = tg.filter(|a| self.is_record_like(a));
            if sr.ns == Ns::Attr {
                if let KeyVal::Lit(k) = &sr.key_kv {
                    if let Some(name) = intern::key_as_str(*k) {
                        tg = tg.filter(|a| !self.class_resolves(a, &name));
                    }
                }
            } else if sr.ns == Ns::Item {
                tg = tg.filter(|a| !self.indexes_without_getitem(a));
            }
            if tg.is_empty() {
                continue;
            }
            let many = tg.len() > 1;
            let sr_chain = chain_extend(&chain, &sr.rel_chain);
            let mut groups: Vec<(Row, Vec<ObjId>)> = Vec::new();
            let mut add = |row: Row, members: Vec<ObjId>| {
                if let Some(g) = groups.iter_mut().find(|(r, _)| *r == row) {
                    g.1.extend(members);
                } else {
                    groups.push((row, members));
                }
            };
            if !many {
                let r = inst.concretise(self, state, &pre, &sr.row, None);
                add(r, vec![tg.first().unwrap()]);
            } else if sr.row.is_entry() && sr.row.refs() == intern::IdSet::one(rho(sr.oid, sr.ns)) {
                for a in tg.iter() {
                    let mut r0 = erow_of(&pre, state, &inst.born, a, sr.ns);
                    if r0.symbolic() && !in_summary {
                        r0 = inst.concretise(self, state, &pre, &r0, None);
                    }
                    add(r0, vec![a]);
                }
            } else if !sr.row.mentions(sr.oid) {
                let r = inst.concretise(self, state, &pre, &sr.row, None);
                add(r, tg.to_vec());
            } else {
                for (focus, members) in inst.focus_groups(self, state, &pre, sr.oid, &tg) {
                    let r = inst.concretise(self, state, &pre, &sr.row, Some(focus));
                    add(r, members);
                }
            }
            // (an access of the callee raises into a try around this call)
            let sr_guarded = sr.guarded || (self.is_guarded(sr.ns) && !crate::interp::disabled("callguard"));
            // (re-raised by the callee's handlers, and not caught here, or re-raised here too)
            let here = !self.is_guarded(sr.ns) || self.guard_reraises(sr.ns) || self.catching_depth > 0;
            let sr_reraise = if sr.guarded { sr.reraise && here } else { self.is_guarded(sr.ns) && (self.guard_reraises(sr.ns) || self.catching_depth > 0) };
            let ngroups = groups.len();
            let mut all_missing = ngroups > 0;
            for (row_c, members) in groups {
                let tiers = self.emit_access(&sr.node, &kv, &row_c, members[0], sr.ns, sr_guarded, &sr_chain, &sr.file, state, &sr.text, sr.cls, sr.conditional, sr_reraise);
                if tiers.is_empty() || tiers.iter().any(|(_, t)| *t != Tier::Missing) {
                    all_missing = false;
                }
                for (k, tier) in tiers {
                    if tier == Tier::Missing {
                        for a in &members {
                            failed.push((*a, sr.ns, k));
                        }
                    }
                    // an access of the callee that may fail here: the call may raise (a try
                    // around it reaches its handler)
                    if !sr.guarded && matches!(tier, Tier::Missing | Tier::Possibly | Tier::Input | Tier::LikelyMissing) && !crate::interp::disabled("symreqraise") {
                        let exc = if sr.ns == Ns::Attr { "AttributeError" } else { "KeyError" };
                        // (the exception, with its key, for a handler's `as e`)
                        if !self.raise_states.is_empty() && !crate::interp::disabled("excvalue") {
                            let e = self.exception_object(exc, vec![Value::lit(k)], &sr.node, state);
                            self.cur_exc = Some(e);
                        }
                        self.may_raise_here(exc, &mut pre.copy());
                        self.cur_exc = None;
                    }
                }
            }
            // an access on every path of the callee that surely fails, under a try around the
            // call that catches it: the call raises there (as a guarded access does)
            if all_missing && !sr.guarded && !sr.conditional && self.is_guarded(sr.ns) && !crate::interp::disabled("surefail") {
                sure_fail = true;
            }
        }
        if sure_fail && !in_summary {
            state.reachable = false;
        }
        // an access that definitely fails ends the call: its fall-through narrowing never happened
        for (a, ns, k) in failed {
            if pre.row(a, ns).present.contains(k) {
                continue;
            }
            let row = state.row(a, ns);
            if row.present.contains(k) {
                let mut d = row.data();
                d.present = d.present.remove(k);
                d.definite = d.definite.remove(k);
                d.likely = d.likely.remove(k);
                // (what was known absent stays so)
                if pre.row(a, ns).absent.contains(k) {
                    d.absent = d.absent.insert(k);
                }
                state.set_row(a, ns, Row::from_data(d));
            }
        }
        // 7. verdicts decided inside the body
        for v in &summary.rel_verdicts {
            let full = chain_extend(&chain, &v.chain);
            let mut nv = v.clone();
            nv.chain = full;
            let dk = nv.dedupe_key();
            if !self.verdict_keys.contains(&dk) && !self.quiet {
                self.verdict_keys.insert(dk);
                self.verdicts.push(nv);
            }
        }
        for n in &summary.notes {
            self.note(n.clone());
        }
        // 8. row checks inside the body: resolved for this call (inside another summary they
        // become that summary's, relative to its entry)
        for ob in &summary.obs {
            let value = inst.remap_value(self, state, &pre, &ob.value);
            let mut rows: Vec<(ObjId, Ns, Row)> = Vec::new();
            for (o, ns, expr) in &ob.rows {
                let tg = inst.resolve(self, state, &pre, *o);
                let c = inst.concretise(self, state, &pre, expr, None);
                for a in tg.iter() {
                    let ns2 = if matches!(self.kind(a), Kind::Instance | Kind::Namespace) { Ns::Attr } else { *ns };
                    let row = if ns2 == *ns { c.clone() } else { state.row(a, ns2) };
                    rows.push((a, ns2, row));
                }
            }
            if in_summary {
                if let Some(top) = self.sum_obs.last_mut() {
                    top.push(Obs { file: ob.file.clone(), line: ob.line, name: ob.name.clone(), rows, must: ob.must.clone(), value });
                }
            } else {
                let ns = rows.first().map(|r| r.1).unwrap_or(Ns::Item);
                let row = if rows.is_empty() { None } else { Some(ob.must.iter().fold(crate::domain::join_rows(rows.into_iter().map(|r| r.2)), |r, k| r.store(*k, true))) };
                let value = self.value_info(&value, state);
                if self.quiet {
                    continue;
                }
                self.row_checks.push(crate::interp::RowCheck { file: ob.file.clone(), line: ob.line, name: ob.name.clone(), row, ns, value, tensor: None });
            }
        }
        // hover facts inside the body, for this call
        for fo in &summary.fact_obs {
            let v = inst.remap_value(self, state, &pre, &fo.value);
            let mut rows: Vec<(ObjId, Option<Row>, Option<Row>)> = Vec::new();
            for (o, item, attr) in &fo.rows {
                let tg = inst.resolve(self, state, &pre, *o);
                let ci = item.as_ref().map(|r| inst.concretise(self, state, &pre, r, None));
                let ca = attr.as_ref().map(|r| inst.concretise(self, state, &pre, r, None));
                for a in tg.iter() {
                    rows.push((a, ci.clone(), ca.clone()));
                }
            }
            let mut slots: Vec<(ObjId, Ns, KeyId, Value)> = Vec::new();
            for (o, ns, k, sv) in &fo.slots {
                let tg = inst.resolve(self, state, &pre, *o);
                let nv = inst.remap_value(self, state, &pre, sv);
                for a in tg.iter() {
                    slots.push((a, *ns, *k, nv.clone()));
                }
            }
            if in_summary {
                if let Some(top) = self.sum_fact_obs.last_mut() {
                    top.push(FactObs { line: fo.line, name: fo.name, value: v, rows, slots, scope: fo.scope });
                }
            } else {
                self.record_resolved_fact(fo.line, fo.name, &v, rows, slots, fo.scope, state);
            }
        }
        let ret = inst.remap_value(self, state, &pre, &summary.ret);
        if !in_summary {
            let fi = self.func_info(summary.foid);
            let rows: Vec<(ObjId, Row)> = ret.pts.iter().filter(|o| self.is_record_like(*o)).map(|o| (o, state.row(o, Ns::Item))).collect();
            self.summaries.entry((fi.qualname.clone(), call_ctx.to_string())).or_insert((ret.clone(), rows));
        }
        ret
    }

    /// Classify an access against a (concrete or symbolic) row and record its consequences.
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_arguments)]
    pub fn emit_access(&mut self, node: &Node, kv: &KeyVal, row: &Row, oid: ObjId, ns: Ns, guarded: bool, chain: &Chain, file: &str, state: &mut State, text: &str, guard_cls: Option<ObjId>, conditional: bool, reraise: bool) -> Vec<(KeyId, Tier)> {
        let mut out: Vec<(KeyId, Tier)> = Vec::new();
        let conditional = conditional || self.branch_depth > 0;
        if row.symbolic() {
            if !self.sum_stack.is_empty() {
                // a callee's access deferred again, now under a try of this function: it may
                // raise into the handler; decided where the record is known
                let guarded = guarded || (self.is_guarded(ns) && !crate::interp::disabled("symreqraise"));
                if guarded && !crate::interp::disabled("symreqraise") && self.is_guarded(ns) {
                    let exc = if ns == Ns::Attr { "AttributeError" } else { "KeyError" };
                    self.may_raise_here(exc, state);
                    self.record_event("guarded", oid);
                }
                self.defer(SymReq { reraise, node: *node, key_kv: kv.clone(), row: row.clone(), oid, ns, guarded, rel_chain: chain.clone(), kind: "load", file: file.to_string(), text: text.to_string(), cls: guard_cls, conditional });
            }
            return out;
        }
        let site = format!("L{}", node.line);
        let saved_chain = std::mem::replace(&mut self.call_chain, chain.clone());
        if kv.is_literal() {
            for k in intern::sorted_by_str(kv_literals(kv).iter()) {
                let cls = classify(row, k);
                out.push((k, cls.tier));
                if guarded {
                    if !crate::interp::disabled("guardedverdicts") {
                        let saved = std::mem::replace(&mut self.verdict_guarded, true);
                        self.verdict(node, k, &cls, oid, ns, Some(file), Some(text));
                        self.verdict_guarded = saved;
                    }
                    if reraise && cls.tier == Tier::Input && !crate::interp::disabled("catchreq") {
                        self.require_providers_k(&cls, row, &site, true, state, Some(k));
                    }
                    continue;
                }
                self.verdict(node, k, &cls, oid, ns, Some(file), Some(text));
                if cls.tier == Tier::Proven && ns == Ns::Item {
                    if let Some(src) = self.td_param_src.get(&oid).copied() {
                        let fresh = !state.reqs.inner(&src).map(|d| d.contains_key(&k)).unwrap_or(false);
                        state.require(src, k, intern::s(&site), intern::IdSet::new(), intern::IdSet::new(), intern::IdSet::new());
                        if conditional && fresh {
                            let tgt = state.reqs.writable(&src);
                            if let Some(cur) = tgt.get(&k).cloned() {
                                tgt.insert(k, crate::domain::ReqInfo { always: false, ..cur });
                            }
                        }
                    }
                }
                if cls.tier == Tier::Input {
                    self.require_providers_k(&cls, row, &site, true, state, Some(k));
                    if conditional {
                        for (src, terms) in &cls.providers {
                            let tgt = state.reqs.writable(src);
                            for t in terms.iter() {
                                if let Some(cur) = tgt.get(&t).cloned() {
                                    tgt.insert(t, crate::domain::ReqInfo { always: false, ..cur });
                                }
                            }
                        }
                    }
                }
            }
        } else if let KeyVal::KeysOf(_) = kv {
        } else if !guarded && row.closed() && !row.is_map() && !row.present.is_empty() && matches!(self.kind(oid), Kind::Dict | Kind::Frame) {
            let k = key_str_lit(&format!("<data {}>", kv));
            self.verdict(node, k, &Classification::with_note(Tier::DataKey, "data-derived key into a closed record"), oid, ns, Some(file), Some(text));
        }
        self.call_chain = saved_chain;
        out
    }
}

/// The entry row of `a`: before the call; an object made while instantiating (a path below
/// an open input) has the row it was made with (`born`), not the one the call's effects gave it.
fn erow_of(pre: &State, state: &State, born: &FxHashMap<(ObjId, Ns), Row>, a: ObjId, ns: Ns) -> Row {
    if pre.has_row(a, ns) {
        pre.row(a, ns)
    } else if let Some(r) = born.get(&(a, ns)) {
        r.clone()
    } else {
        state.row(a, ns)
    }
}

fn group_by_site(an: &Analyzer, tg: &ObjSet) -> Vec<Vec<ObjId>> {
    let mut by: Vec<((Kind, String), Vec<ObjId>)> = Vec::new();
    for a in tg.iter() {
        let key = (an.kind(a), an.obj(a).site.clone());
        if let Some(e) = by.iter_mut().find(|(k, _)| *k == key) {
            e.1.push(a);
        } else {
            by.push((key, vec![a]));
        }
    }
    by.into_iter().map(|(_, m)| m).collect()
}

/// The entry rows substituted for one placeholder when a symbolic row is concretised for
/// a particular target (or group of targets sharing their entry rows).
#[derive(Clone, PartialEq, Eq, Hash)]
struct Focus {
    ph: ObjId,
    item: Row,
    attr: Row,
}

impl Focus {
    fn of_target(pre: &State, state: &State, born: &FxHashMap<(ObjId, Ns), Row>, ph: ObjId, a: ObjId) -> Focus {
        Focus { ph, item: erow_of(pre, state, born, a, Ns::Item), attr: erow_of(pre, state, born, a, Ns::Attr) }
    }
}

/// The per-instantiation context of `_instantiate` (its closures and caches).
struct Inst {
    summary: Rc<Summary>,
    bind: IndexMap<ObjId, Value>,
    call_ctx: String,
    ctx_single: bool,
    /// number of objects before this instantiation: clones with a larger id are new
    obj_mark: ObjId,
    in_summary: bool,
    clones: IndexMap<ObjId, ObjId>,
    /// clones shared by several templates (same allocation site and context, e.g. the levels
    /// of a recursive structure): their facts combine instead of overwriting each other
    collided: FxHashSet<ObjId>,
    resolved: FxHashMap<ObjId, ObjSet>,
    kv_cache: FxHashMap<ObjId, KeyVal>,
    erow_cache: FxHashMap<Rho, Row>,
    conc_cache: FxHashMap<(Row, Option<Focus>), Row>,
    new_clones: FxHashSet<ObjId>,
    /// rows of objects made while resolving paths, as made
    born: FxHashMap<(ObjId, Ns), Row>,
}

impl Inst {
    fn kv_of(&mut self, an: &mut Analyzer, state: &mut State, pre: &State, ph: ObjId) -> KeyVal {
        if let Some(kv) = self.kv_cache.get(&ph) {
            return kv.clone();
        }
        let kv = if let Some(v) = self.bind.get(&ph) {
            v.kv.clone()
        } else if an.has_obj(ph) && an.kind(ph) == Kind::Ph {
            let p = an.ph_info(ph);
            match p.kind {
                PhKind::Path => self.path_value(an, state, pre, &p).kv,
                PhKind::Narrow => self.kv_of(an, state, pre, p.parent.unwrap()),
                _ => DATA,
            }
        } else {
            DATA
        };
        self.kv_cache.insert(ph, kv.clone());
        kv
    }

    fn resolve(&mut self, an: &mut Analyzer, state: &mut State, pre: &State, oid: ObjId) -> ObjSet {
        if let Some(r) = self.resolved.get(&oid) {
            return r.clone();
        }
        self.resolved.insert(oid, ObjSet::new()); // cycle guard
        let out: ObjSet = if !an.has_obj(oid) {
            ObjSet::new()
        } else if self.summary.templates.contains(&oid) {
            ObjSet::one(self.clone_obj(an, state, pre, oid))
        } else if an.kind(oid) == Kind::Ph {
            if let Some(v) = self.bind.get(&oid) {
                v.pts.clone()
            } else {
                let p = an.ph_info(oid);
                match p.kind {
                    PhKind::Path => self.path_value(an, state, pre, &p).pts,
                    PhKind::Narrow => {
                        let c = p.cls.get().unwrap();
                        self.resolve(an, state, pre, p.parent.unwrap()).filter(|o| an.may_be_instance(o, c))
                    }
                    _ => ObjSet::new(),
                }
            }
        } else {
            ObjSet::one(oid)
        };
        self.resolved.insert(oid, out.clone());
        out
    }

    fn remap_value(&mut self, an: &mut Analyzer, state: &mut State, pre: &State, v: &Value) -> Value {
        let mut pts: Vec<ObjId> = Vec::new();
        for o in v.pts.iter() {
            pts.extend(self.resolve(an, state, pre, o).iter());
        }
        let kv = match &v.kv {
            KeyVal::Param(ph) => self.kv_of(an, state, pre, *ph),
            KeyVal::KeysOf(ko) => {
                let mut objs: Vec<ObjId> = Vec::new();
                for o in ko.objs.iter() {
                    objs.extend(self.resolve(an, state, pre, o).iter());
                }
                if objs.is_empty() {
                    DATA
                } else {
                    KeyVal::KeysOf(KeysOf { objs: ObjSet::from_vec(objs), ns: ko.ns, exclude: ko.exclude.clone(), chain: ko.chain, exhaustive_depth: -1 })
                }
            }
            other => other.clone(),
        };
        Value::with_kv(ObjSet::from_vec(pts), kv)
    }

    fn entry_row_of(&mut self, an: &mut Analyzer, state: &mut State, pre: &State, r: Rho) -> Row {
        if let Some(h) = self.erow_cache.get(&r) {
            return h.clone();
        }
        let (oid, ns) = unrho(r);
        let targets = self.resolve(an, state, pre, oid);
        // (a placeholder standing for no object here: whatever value it is, its keys are not
        // known — an empty closed row would claim there are none)
        let out = if targets.is_empty() {
            if an.has_obj(oid) && an.kind(oid) == Kind::Ph && !crate::interp::disabled("unresolvedtop") {
                Row::top()
            } else {
                Row::new()
            }
        } else {
            join_rows(targets.iter().map(|t| erow_of(pre, state, &self.born, t, ns)))
        };
        self.erow_cache.insert(r, out.clone());
        out
    }

    /// `focus`: the row as it is for one target (or a pair of rows standing for a group).
    fn concretise(&mut self, an: &mut Analyzer, state: &mut State, pre: &State, row: &Row, focus: Option<Focus>) -> Row {
        let focus = focus.filter(|f| row.mentions(f.ph));
        let ck = (row.clone(), focus.clone());
        if let Some(h) = self.conc_cache.get(&ck) {
            return h.clone();
        }
        // pre-compute entry rows (focus-aware) for every ρ the row refers to, transitively
        let mut erows: FxHashMap<Rho, Row> = FxHashMap::default();
        let mut pending: Vec<Rho> = row.refs().to_vec();
        while let Some(r) = pending.pop() {
            if erows.contains_key(&r) {
                continue;
            }
            let e = match &focus {
                Some(f) if r == rho(f.ph, Ns::Item) => f.item.clone(),
                Some(f) if r == rho(f.ph, Ns::Attr) => f.attr.clone(),
                _ => self.entry_row_of(an, state, pre, r),
            };
            pending.extend(e.refs().iter());
            erows.insert(r, e);
        }
        // key values of every placeholder a parameter store may refer to
        let mut kvs: FxHashMap<ObjId, KeyVal> = FxHashMap::default();
        let mut phs: Vec<ObjId> = row.pstores.iter().map(|(r, _, _)| r).collect();
        for e in erows.values() {
            phs.extend(e.pstores.iter().map(|(r, _, _)| r));
        }
        for ph in phs {
            if !kvs.contains_key(&ph) {
                let kv = self.kv_of(an, state, pre, ph);
                kvs.insert(ph, kv);
            }
        }
        let mut erow = |r: Rho| -> Row { erows.get(&r).cloned().unwrap_or_default() };
        let mut kv_of = |ph: ObjId| -> KeyVal { kvs.get(&ph).cloned().unwrap_or(DATA) };
        let bot_is_data = true;
        let mut hit = if row.is_entry() { erow(row.entry_rho().unwrap()) } else { row.concretise_opt(&mut erow, &mut kv_of, bot_is_data) };
        if hit.symbolic() && !self.in_summary {
            hit = hit.concretise_opt(&mut erow, &mut kv_of, bot_is_data);
        }
        self.conc_cache.insert(ck, hit.clone());
        hit
    }

    /// Targets of placeholder `ph` partitioned for concretisation.  With site resolution
    /// (AGENTS.md next step 3) targets of one allocation site that share their entry rows
    /// form one group with one exact representative row; otherwise the prototype's scheme:
    /// one group per target up to `MAX_SYMREQ_TARGETS`, then one joined group per site.
    fn focus_groups(&mut self, an: &Analyzer, state: &State, pre: &State, ph: ObjId, tg: &ObjSet) -> Vec<(Focus, Vec<ObjId>)> {
        if an.opts.site_resolution {
            let mut groups: Vec<((Kind, String, Focus), Vec<ObjId>)> = Vec::new();
            for a in tg.iter() {
                let f = Focus::of_target(pre, state, &self.born, ph, a);
                let key = (an.kind(a), an.obj(a).site.clone(), f);
                if let Some(g) = groups.iter_mut().find(|(k, _)| *k == key) {
                    g.1.push(a);
                } else {
                    groups.push((key, vec![a]));
                }
            }
            return groups.into_iter().map(|((_, _, f), m)| (f, m)).collect();
        }
        if tg.len() <= MAX_SYMREQ_TARGETS {
            return tg.iter().map(|a| (Focus::of_target(pre, state, &self.born, ph, a), vec![a])).collect();
        }
        let mut out = Vec::new();
        for members in group_by_site(an, tg) {
            if members.len() == 1 {
                out.push((Focus::of_target(pre, state, &self.born, ph, members[0]), members));
            } else {
                let ri = join_rows(members.iter().map(|a| erow_of(pre, state, &self.born, *a, Ns::Item)));
                let ra = join_rows(members.iter().map(|a| erow_of(pre, state, &self.born, *a, Ns::Attr)));
                out.push((Focus { ph, item: ri, attr: ra }, members));
            }
        }
        out
    }

    fn fresh_clone(&self, nid: ObjId) -> bool {
        nid >= self.obj_mark
    }

    fn clone_obj(&mut self, an: &mut Analyzer, state: &mut State, pre: &State, t: ObjId) -> ObjId {
        if let Some(&n) = self.clones.get(&t) {
            return n;
        }
        let (tkind, tsite, tctx, tlabel, tsingle, tll, titer, tdepth, tderived, payload) = {
            let o = an.obj(t);
            (o.kind, o.site.clone(), o.ctx.clone(), o.label.clone(), o.single, an.llen(t, &self.summary.exit), o.iter_elem, o.depth, o.derived_from, o.payload.clone())
        };
        let tsite = tsite.strip_suffix("#recent").map(|x| x.to_string()).unwrap_or(tsite);
        // templates allocated at one line in different inner contexts (two objects built by
        // one constructor inside the body) stay distinct in the clone
        let tsite = if tctx.is_empty() || crate::interp::disabled("clonectx") { tsite } else { format!("{}|{}", tsite, tctx) };
        let ns = if matches!(tkind, Kind::Instance | Kind::Namespace) { Ns::Attr } else { Ns::Item };
        let nid = an.alloc_in_frame(tkind, &tsite, state, &tlabel, Payload::None, None, ns, tsingle, &self.call_ctx, self.ctx_single);
        if let Some(o) = an.origins.get(&t).cloned() {
            an.origins.insert(nid, o);
        }
        if let Some(sym) = an.symbols.get(&t).cloned() {
            let scope = an.symbol_scope.get(&t).copied().unwrap_or(0);
            an.set_symbol(nid, sym, scope);
        }
        if let Some(f) = an.dd_factories.get(&t).cloned() {
            an.dd_factories.insert(nid, f);
        }
        an.copy_side_tables(t, nid);
        let existed = !self.fresh_clone(nid);
        if !existed {
            self.new_clones.insert(nid);
        }
        if self.clones.values().any(|c| *c == nid) {
            self.collided.insert(nid);
            self.new_clones.remove(&nid);
        }
        self.clones.insert(t, nid);
        an.set_llen(nid, state, tll);
        an.obj_mut(nid).iter_elem = titer;
        an.obj_mut(nid).depth = tdepth;
        if let Some(d) = tderived {
            let dd = self.resolve(an, state, pre, d);
            an.obj_mut(nid).derived_from = if dd.len() == 1 { dd.first() } else { None };
        }
        let np = match payload {
            Payload::Func(fi) => {
                let mut closure: IndexMap<StrId, ObjId> = IndexMap::new();
                for (n, c) in fi.closure.iter() {
                    let r = self.resolve(an, state, pre, *c);
                    closure.insert(*n, r.first().unwrap_or(*c));
                }
                Payload::Func(Rc::new(fi.clone_with_closure(closure)))
            }
            Payload::Bound(f, sv) => {
                let fr = self.resolve(an, state, pre, f);
                let sv2 = self.remap_value(an, state, pre, &sv);
                Payload::Bound(fr.first().unwrap_or(f), sv2)
            }
            Payload::Method(q, v) => Payload::Method(q, self.remap_value(an, state, pre, &v)),
            Payload::View(k, v) => Payload::View(k, self.remap_value(an, state, pre, &v)),
            Payload::Zip(a, b) => {
                let a2 = self.remap_value(an, state, pre, &a);
                let b2 = self.remap_value(an, state, pre, &b);
                Payload::Zip(a2, b2)
            }
            Payload::Enumerate(v, s0) => Payload::Enumerate(self.remap_value(an, state, pre, &v), s0),
            Payload::Pair(a, b) => {
                let a2 = self.remap_value(an, state, pre, &a);
                let b2 = self.remap_value(an, state, pre, &b);
                Payload::Pair(a2, b2)
            }
            Payload::Partial(f, args, kw) => {
                let f2 = self.remap_value(an, state, pre, &f);
                let args2: Vec<Value> = args.iter().map(|a| self.remap_value(an, state, pre, a)).collect();
                let kw2: IndexMap<StrId, Value> = kw.iter().map(|(k, v)| (*k, self.remap_value(an, state, pre, v))).collect();
                Payload::Partial(f2, args2, kw2)
            }
            Payload::Groupby(fs, ks, ai, sel) => {
                let fs2: Vec<ObjId> = fs
                    .iter()
                    .map(|f| {
                        let r = self.resolve(an, state, pre, *f);
                        r.first().filter(|_| r.len() == 1).unwrap_or(*f)
                    })
                    .collect();
                Payload::Groupby(fs2, ks, ai, sel)
            }
            Payload::Locidx(name, o) => {
                let r = self.resolve(an, state, pre, o);
                Payload::Locidx(name, r.first().filter(|_| r.len() == 1).unwrap_or(o))
            }
            other => other,
        };
        an.obj_mut(nid).payload = np;
        nid
    }

    /// Actual value of an entry object's slot that a path placeholder stands for.
    fn path_value(&mut self, an: &mut Analyzer, state: &mut State, pre: &State, ph: &PH) -> Value {
        let parents = match ph.parent {
            Some(p) => self.resolve(an, state, pre, p),
            None => ObjSet::new(),
        };
        let mut acc = Value::new();
        let key = ph.key.unwrap_or(STAR);
        for p in parents.iter() {
            // the content of a dict under an unknown key: its values (iterating it gives keys)
            if key == STAR && ph.ns == Ns::Item && an.kind(p) == Kind::Dict && !crate::interp::disabled("dictstar") {
                acc = acc.join(&an.record_values(p, state));
                continue;
            }
            if key == STAR {
                let it = an.iterate(&Value::obj(p), &Node::none(), state);
                let e = it.elem.flat();
                for o in e.pts.iter() {
                    if !pre.has_row(o, Ns::Item) && !pre.has_row(o, Ns::Attr) {
                        for ns in [Ns::Item, Ns::Attr] {
                            self.born.entry((o, ns)).or_insert_with(|| state.row(o, ns));
                        }
                    }
                }
                acc = acc.join(&e);
                continue;
            }
            if let Some(kph) = param_slot_of(key) {
                let kv = self.kv_of(an, state, pre, kph);
                let lits = kv_literals(&kv);
                if matches!(an.kind(p), Kind::List | Kind::Tuple) && ph.ns == Ns::Item {
                    // `xs[i]` with `i` a parameter: the positions it may name, else every
                    // element (a store through it then reaches all of them, weakly)
                    let ints: Vec<i64> = lits.iter().filter_map(intern::key_as_int).collect();
                    if !lits.is_empty() && ints.len() == lits.len() && an.llen(p, pre).is_some() {
                        let n = an.llen(p, pre).unwrap() as i64;
                        for i in ints {
                            let j = if i < 0 { n + i } else { i };
                            if 0 <= j && j < n {
                                acc = acc.join(&pre.slot(p, Ns::Pos, intern::key_int(j)));
                            }
                        }
                    } else {
                        acc = acc.join(&pre.all_slots(p, Ns::Pos));
                        acc = acc.join(&pre.slot(p, Ns::Elem, STAR));
                    }
                    continue;
                }
                if !lits.is_empty() {
                    for k in lits.iter() {
                        acc = acc.join(&pre.slot(p, ph.ns, k));
                    }
                    acc = acc.join(&pre.slot(p, ph.ns, STAR));
                } else {
                    acc = acc.join(&pre.all_slots(p, ph.ns));
                }
                if acc.empty() {
                    let row = pre.row(p, ph.ns);
                    if row.open() {
                        let v = an.path_placeholder_value(p, ph.ns, STAR, &row, state);
                        acc = acc.join(&v);
                    }
                }
                continue;
            }
            let mut v = pre.slot(p, ph.ns, key);
            // a module name read as a deferred load, met while building another summary: the
            // load stays deferred (the summary must not mutate the module's actual list)
            if ph.ns == Ns::Attr && an.kind(p) == Kind::Module && !an.sum_stack.is_empty() && !crate::interp::disabled("globalrd2") {
                if let Some(name) = intern::key_as_str(key) {
                    if an.deferred_global(p, &name, &v) {
                        acc = acc.join(&an.path_placeholder_value(p, Ns::Attr, key, &Row::new(), state));
                        continue;
                    }
                }
            }
            if ph.ns == Ns::Attr && an.kind(p) == Kind::Class && v.empty() && !crate::interp::disabled("clsattrpath") {
                // a class's attribute through a parameter: its class table, or its name
                if let Some(name) = intern::key_as_str(key) {
                    if matches!(&*name, "__name__" | "__qualname__") && an.class_lookup(p, &name).is_none() {
                        let n = an.class_info_opt(p).map(|ci| ci.name.clone()).unwrap_or_default();
                        acc = acc.join(&Value::lit(intern::key_str_lit(&n)));
                        continue;
                    }
                    let cv = an.class_attr_value(p, &name);
                    if !cv.empty() {
                        acc = acc.join(&cv);
                        continue;
                    }
                }
            }
            if matches!(an.kind(p), Kind::List | Kind::Tuple) && ph.ns == Ns::Item {
                // `xs[i]` through a placeholder: the positional slot, else the element slot
                v = match intern::key_as_int(key) {
                    Some(i) if i >= 0 => pre.slot(p, Ns::Pos, key),
                    _ => Value::new(),
                };
                if v.empty() {
                    v = pre.slot(p, Ns::Elem, STAR);
                }
            }
            let star = pre.slot(p, ph.ns, STAR);
            if !star.pts.is_empty() {
                v = v.join(&Value::with_kv(star.pts, DATA));
            }
            if v.empty() {
                // (an object made while resolving, e.g. the element of an unknown iterable,
                // has its row in the current state only)
                let row = if pre.has_row(p, ph.ns) || !state.has_row(p, ph.ns) { pre.row(p, ph.ns) } else { state.row(p, ph.ns) };
                if ph.ns == Ns::Cell {
                    // a data cell not bound at this (symbolic) call site: still an entry fact
                    // of the summary being built, resolved when *it* is instantiated
                    v = if !an.sum_stack.is_empty() && an.is_entry_object(p) { an.path_placeholder_value(p, ph.ns, key, &Row::new(), state) } else { Value::new() };
                } else if row.open() || an.kind(p) == Kind::Ph {
                    v = an.path_placeholder_value(p, ph.ns, key, &row, state);
                    for o in v.pts.iter() {
                        if !pre.has_row(o, Ns::Item) && !pre.has_row(o, Ns::Attr) {
                            for ns in [Ns::Item, Ns::Attr] {
                                self.born.entry((o, ns)).or_insert_with(|| state.row(o, ns));
                            }
                        }
                    }
                }
            }
            if ph.ns == Ns::Attr && an.kind(p) == Kind::Instance {
                if let (Some(c), Some(name)) = (an.instance_class(p), intern::key_as_str(key)) {
                    if an.field_is_any(c, &name) {
                        let label = format!("{}.{}", an.obj(p).label, name);
                        v = v.join(&an.dynamic_value(&label, state));
                    }
                }
            }
            acc = acc.join(&v);
        }
        if let Some(c) = ph.cls.get() {
            if !acc.pts.is_empty() {
                let kept = acc.pts.filter(|o| an.may_be_instance(o, c));
                if kept != acc.pts {
                    acc = Value::with_kv(kept, acc.kv.clone());
                }
            }
        }
        acc
    }
}
