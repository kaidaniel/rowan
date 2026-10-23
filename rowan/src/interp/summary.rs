//! Result summaries: what a call of a function that does not run (past its budget, the
//! deadline, a library's allowance) gives back. A function's run for any call (open
//! parameters, from the module's end after any of the program's code ran: `entry.rs`) returns a
//! value; its *shape* (`Shape`) is that value without the heap it was made in: literals and
//! typed unknowns, and for each record born in the run its keys (`Row`), its slots' shapes,
//! a list's elements and length, an instance's class. A skipped call makes the shape anew at
//! the call site (`materialize`): fresh objects with those facts, where today the result is `?`.
//!
//! Sound because the run holds for any call (the general-run premise), and a shape keeps only
//! what no other object shares: an object the run was given, read from elsewhere (`self.cache`,
//! a global), or stored where the program reads it back (the receiver, a global list, a
//! registry, unknown code's object) is *not* the fresh record a shape would make, so it is
//! `any` (the call's unknown result, as without a summary). Keys unknown code may have added or removed
//! keep their row's mutators (reads are `likely`, as for the same code inline).
//!
//! The shapes come from a *source*: an analyzer of its own (`Analyzer::summary_source`) that ran
//! the module's code with every call skipped and is set up for runs on demand (in them every
//! call after a function's first is skipped, `Budget::cap`; installed packages not followed;
//! a quarter of the deadline in all: `main.rs` `summaries_of`). It is made at the first call in
//! the main module that does not run, and in `rowan serve`'s runs on demand (the source costs
//! one more run of the program's module code: a program that skips calls only in other
//! modules, timm's registry decorators, does not pay it; until it is made a skipped call there
//! gives an unknown, and once it is made, every skipped call uses it). A function's shape is made when a call of it first does not run
//! (`shape_for`): its run for any call in the source, with the thread's marks exchanged for the
//! source's around it (`ThreadState`). So the cost is one run per function whose calls are
//! skipped, within a share of the deadline, not a run of every function. Keyed by the `def`'s
//! address, the same in every analyzer of one program (the parse cache).

use std::rc::Rc;

use rustc_hash::{FxHashMap, FxHashSet};

use super::{Analyzer, Payload, site_string};
use crate::domain::{label_base, optional_label, typed_label, KeyVal, Ns, ObjId, Row, Value, DATA};
use crate::frontend::Node;
use crate::intern::{self, s, KeyId, STAR};
use crate::source::SourceKind;
use crate::state::{is_any, Kind, State};

/// Below this many records deep a shape says `any`.
const DEPTH: usize = 4;
/// Past this many records in one shape the rest say `any` (what a skipped call makes stays
/// small).
const RECORDS: usize = 16;

/// The result summaries: per function code, the shape of its result for any call.
#[derive(Default)]
pub struct Summaries {
    /// per function code (its `def`'s address): the shape of what it returns, for any call
    pub shapes: FxHashMap<usize, Shape>,
    /// the pre-pass: each entry run keeps its result's shape in `last`
    pub recording: bool,
    pub last: Option<Shape>,
    /// skipped calls given a shape (`ROWAN_DEBUG`'s `work:` line)
    pub applied: usize,
    /// the program's classes by their `class` statement's address (`class_of`)
    classes: FxHashMap<usize, ObjId>,
    /// the source: the analyzer whose runs for any call give the shapes, with its thread marks,
    /// its functions by their `def`'s address, the time its runs took so far and may take
    pub source: Option<Box<Analyzer>>,
    /// makes the source when a shape is first needed (`set_summary_source`)
    pub factory: Option<Box<dyn FnOnce() -> Option<Box<Analyzer>>>>,
    pub state: crate::state::ThreadState,
    pub by_addr: FxHashMap<usize, ObjId>,
    pub spent: std::time::Duration,
    pub ms: usize,
    /// per function code: the parameter and attribute names a getter's `return` reads
    /// (`getter_result`), None for other functions
    getters: FxHashMap<usize, Option<Rc<(String, Vec<String>)>>>,
}

/// A value without its heap: its scalar part, the records it may be, and whether it may be
/// anything else (the call's unknown result).
#[derive(Clone, Debug)]
pub struct Shape {
    pub kv: KeyVal,
    pub objs: Vec<Rc<ObjShape>>,
    pub any: bool,
}

/// One record of a shape: a dict, list, tuple, set, instance or namespace born in the run.
#[derive(Debug)]
pub struct ObjShape {
    kind: Kind,
    /// an instance's class, by its `class` statement's address
    class: Option<usize>,
    ty: Option<&'static str>,
    /// its keys (a dict's items, an instance's attributes)
    row: Row,
    /// the values under its present keys
    slots: Vec<(KeyId, Shape)>,
    /// a list / tuple / set: its elements (None: nothing known of them), and the values at its
    /// positions when its length is known
    elem: Option<Shape>,
    positions: Option<Vec<Shape>>,
}

impl Shape {
    /// Nothing known: the call's unknown result.
    pub fn any() -> Shape {
        Shape { kv: KeyVal::Bot, objs: Vec::new(), any: true }
    }

    /// Does it say anything a `?` does not?
    pub fn informative(&self) -> bool {
        !self.any || !self.kv.is_bot() || !self.objs.is_empty()
    }
}

impl Analyzer {
    /// The shape of what function code `code`'s calls that do not run give back: the source's
    /// run of it for any call, the first time (`Summaries`); None when it says nothing a `?`
    /// does not.
    pub(in crate::interp) fn shape_for(&mut self, code: usize) -> Option<Shape> {
        if let Some(sh) = self.summaries.shapes.get(&code) {
            return sh.informative().then(|| sh.clone());
        }
        if self.summaries.source.is_none() {
            // (made now, with this analysis's thread marks and counters kept apart)
            let factory = self.summaries.factory.take()?;
            let mut mine = crate::state::ThreadState::default();
            mine.swap();
            let made = factory();
            mine.swap();
            let mut src = made?;
            self.summaries.ms = src.summaries.ms;
            self.summaries.by_addr = std::mem::take(&mut src.summaries.by_addr);
            self.summaries.state = std::mem::take(&mut src.summaries.state);
            self.summaries.source = Some(src);
        }
        let mut src = self.summaries.source.take()?;
        let sh = match self.summaries.by_addr.get(&code).copied() {
            Some(oid) if self.summaries.spent.as_millis() < self.summaries.ms as u128 => {
                let t = std::time::Instant::now();
                self.summaries.state.swap();
                src.summaries.last = None;
                src.general_run_of(oid);
                let sh = src.summaries.last.take();
                self.summaries.state.swap();
                self.summaries.spent += t.elapsed();
                if super::debug_level() == 2 {
                    eprintln!("  summary of {}: {:?}", src.heap.func_info(oid).qualname, sh);
                }
                sh.unwrap_or_else(Shape::any)
            }
            _ => Shape::any(),
        };
        self.summaries.source = Some(src);
        self.summaries.shapes.insert(code, sh.clone());
        sh.informative().then_some(sh)
    }

    /// A skipped call of `foid`'s result from its function's shape (`unknown`: the call's
    /// unknown result, which it stays without one): its records at the main module's call sites
    /// (what the editor and the report show), elsewhere its scalars by type beside the unknown
    /// result, as records there would cost every later walk and join.
    pub(in crate::interp) fn summarized(&mut self, foid: ObjId, unknown: Value, node: &Node, state: &mut State) -> Value {
        let code = self.heap.func_info(foid).node.addr();
        // (the source is made for the file the editor shows and the report is about, at the
        // first call there that does not run; until then a call elsewhere stays unknown)
        if self.summaries.source.is_none() && node.module != self.program.main_idx {
            return unknown;
        }
        let Some(sh) = self.shape_for(code) else { return unknown };
        self.summaries.applied += 1;
        let records = node.module == self.program.main_idx;
        self.materialize(&sh, &unknown, node, state, "", records)
    }

    /// A skipped call of a getter (`return self._link`, `return p.name`, `return self.a.b`: its
    /// body one `return` of attributes of a parameter): that attribute as the call site's
    /// object has it, the same value and object, where reading it runs no code of the
    /// program's (a property, a descriptor, `__getattr__`); None for other functions.
    pub(in crate::interp) fn getter_result(&mut self, foid: ObjId, self_v: Option<&Value>, positional: &[String], a: &super::Args, node: &Node, state: &mut State) -> Option<Value> {
        let fi = self.heap.func_info(foid);
        let code = fi.node.addr();
        let path = match self.summaries.getters.get(&code) {
            Some(p) => p.clone(),
            None => {
                let p = getter_path(&fi.node).map(Rc::new);
                self.summaries.getters.insert(code, p.clone());
                p
            }
        }?;
        let (param, names) = (&path.0, &path.1);
        let first = fi.node.params()?.posonlyargs.iter().chain(fi.node.params()?.args.iter()).next().map(|p| p.parameter.name.id.as_str());
        let mut v = match self_v {
            Some(r) if first == Some(param.as_str()) && !fi.is_static.get() => r.clone(),
            // (a static method through an instance: its parameters not matched here)
            Some(_) if fi.is_static.get() => return None,
            _ if !a.star.is_empty() || !a.star_kw.is_empty() => return None,
            _ => positional.iter().position(|x| x == param).and_then(|i| a.pos.get(i)).or_else(|| a.kw.get(&s(param)))?.clone(),
        };
        for name in names.iter() {
            if v.pts.iter().any(|o| self.attr_runs_user_code(o, name, false)) {
                return None;
            }
            v = self.load_attr(&v, name, node, state, false);
        }
        Some(v)
    }

    /// The shape of `ret`, a run's result: `pre` the state the run started from, `born` the
    /// heap's serial then (objects made since are the run's own), `st` its end.
    pub(in crate::interp) fn shape_of_result(&self, ret: &Value, pre: &State, st: &State, born: u64) -> Shape {
        let escaped = self.escaped(pre, st, born);
        let mut seen = Vec::new();
        self.shape_of(ret, st, born, &escaped, &mut seen, &mut 0)
    }

    /// The objects born in the run that an object from before it holds at its end (stored into
    /// a global, the receiver, a registry, unknown code's object), and what they hold: another
    /// call sees them too, so no fresh record stands for them.
    fn escaped(&self, pre: &State, st: &State, born: u64) -> FxHashSet<ObjId> {
        let old = |o: ObjId| is_any(o) || self.heap.born_before(o, born);
        let mut work: Vec<ObjId> = Vec::new();
        for (o, (ns, k)) in st.slots.diff_keys(&pre.slots) {
            if old(o) {
                if let Some(v) = st.slot_opt(o, ns, k) {
                    v.push_objs(&mut work);
                }
            }
        }
        for o in st.elems.diff_keys(&pre.elems) {
            if old(o) {
                if let Some(KeyVal::KeysOf(ko)) = st.elems.get_ref(&o) {
                    work.extend(ko.objs.iter());
                }
            }
        }
        let mut out: FxHashSet<ObjId> = FxHashSet::default();
        while let Some(o) = work.pop() {
            if old(o) || !out.insert(o) {
                continue;
            }
            self.obj_edges(o, st, &mut work);
        }
        out
    }

    /// The shape of `v` at `st`: its scalar part cleaned of the run's names, the records born
    /// in the run that nothing else holds, `any` for the rest.
    fn shape_of(&self, v: &Value, st: &State, born: u64, escaped: &FxHashSet<ObjId>, seen: &mut Vec<ObjId>, count: &mut usize) -> Shape {
        let mut sh = Shape { kv: clean_kv(&v.kv), objs: Vec::new(), any: false };
        for o in v.pts.iter() {
            if is_any(o) || self.heap.born_before(o, born) || escaped.contains(&o) || seen.contains(&o) || seen.len() >= DEPTH || *count >= RECORDS {
                sh.any = true;
                continue;
            }
            *count += 1;
            seen.push(o);
            match self.obj_shape(o, st, born, escaped, seen, count) {
                Some(os) => sh.objs.push(Rc::new(os)),
                None => sh.any = true,
            }
            seen.pop();
        }
        sh
    }

    /// The shape of one record born in the run (None: one a shape cannot stand for).
    fn obj_shape(&self, o: ObjId, st: &State, born: u64, escaped: &FxHashSet<ObjId>, seen: &mut Vec<ObjId>, count: &mut usize) -> Option<ObjShape> {
        let kind = self.heap.kind(o);
        let obj = self.heap.obj(o);
        let (ns, class) = match (kind, &obj.payload) {
            (Kind::Dict, Payload::None) => (Ns::Item, None),
            (Kind::List | Kind::Tuple | Kind::Set, Payload::None) => (Ns::Item, None),
            (Kind::Namespace, Payload::None) => (Ns::Attr, None),
            (Kind::Instance, Payload::Cls(c)) => (Ns::Attr, Some(self.heap.class_info_opt(*c)?.node as *const _ as usize)),
            _ => return None,
        };
        if obj.iterator || obj.derived_from.is_some() {
            return None;
        }
        let mut out = ObjShape { kind, class, ty: obj.ty, row: Row::new(), slots: Vec::new(), elem: None, positions: None };
        if matches!(kind, Kind::List | Kind::Tuple | Kind::Set) {
            // (its items: scalars in `elems`, objects in the element slot; nothing there: not known)
            let kv = st.elems.get(&o);
            let objs = st.slot_opt(o, Ns::Elem, STAR).map(|v| v.pts.clone()).unwrap_or_default();
            out.elem = kv.map(|kv| self.shape_of(&Value::with_kv(objs, kv), st, born, escaped, seen, count));
            if obj.single && kind != Kind::Set {
                if let Some(n) = self.heap.llen(o, st) {
                    let vals: Vec<Shape> = (0..n).map(|i| self.shape_of(&st.slot(o, Ns::Pos, intern::key_int(i as i64)), st, born, escaped, seen, count)).collect();
                    out.positions = Some(vals);
                }
            }
            return Some(out);
        }
        let row = st.row(o, ns);
        // (open through a parameter, an instance of a class not all seen, a name not bound, keys
        // from data: what it holds depends on the call)
        if row.top || st.has_slot(o, ns, STAR) || row.sources.iter().any(|(src, _)| !matches!(intern::source_get(src).kind, SourceKind::Call | SourceKind::External)) {
            return None;
        }
        for k in row.present.iter() {
            let v = st.slot_opt(o, ns, k).map(|v| self.shape_of(&v, st, born, escaped, seen, count)).unwrap_or_else(Shape::any);
            out.slots.push((k, v));
        }
        out.row = row;
        Some(out)
    }

    /// A value from a shape, made at `node`: `unknown` is the call's unknown result (the shape's
    /// `any`, and the records past `RECORDS` or `DEPTH`); its records only where `records` (the
    /// main module's call sites), else the unknown result beside its scalars by type (`int(?)`:
    /// records made at every call site would cost every later walk and join).
    pub(in crate::interp) fn materialize(&mut self, sh: &Shape, unknown: &Value, node: &Node, state: &mut State, path: &str, records: bool) -> Value {
        // (elsewhere its scalars by type only: literal sets cost every join they reach)
        let mut v = Value::kv_only(if records { sh.kv.clone() } else { type_only(&sh.kv) });
        if sh.any || (!records && !sh.objs.is_empty()) {
            v = v.join(unknown);
        }
        if !records {
            return v;
        }
        for (i, os) in sh.objs.iter().enumerate() {
            let o = self.materialize_obj(os, unknown, node, state, &format!("{}.{}", path, i));
            v.pts = v.pts.insert(o);
        }
        v
    }

    /// One record of a shape, allocated at `node` (its site: the call's and the record's place
    /// in the shape): its row, slots, elements and length as the shape says; a merged object
    /// (the site allocated many times) joins them in.
    fn materialize_obj(&mut self, os: &ObjShape, unknown: &Value, node: &Node, state: &mut State, path: &str) -> ObjId {
        let site = format!("{}#sum{}", site_string(node), path);
        let (payload, label, ns) = match os.class.and_then(|c| self.class_of(c)) {
            Some(c) => (Payload::Cls(c), format!("{}()", self.heap.class_info(c).name), Ns::Attr),
            None if os.kind == Kind::Instance => return unknown.single().unwrap_or_else(|| crate::interp::any_object(&mut self.heap)),
            None => (Payload::None, format!("{}@L{}", os.kind.name(), node.line), if os.kind == Kind::Namespace { Ns::Attr } else { Ns::Item }),
        };
        let seq = matches!(os.kind, Kind::List | Kind::Tuple | Kind::Set);
        let oid = self.alloc(os.kind, &site, state, &label, payload, (!seq).then(|| os.row.clone()), ns, None, None);
        self.heap.obj_mut(oid).ty = os.ty;
        let single = self.heap.obj(oid).single;
        if seq {
            match (&os.positions, &os.elem) {
                (Some(ps), _) if single => {
                    let vals: Vec<Value> = ps.iter().enumerate().map(|(i, p)| self.materialize(p, unknown, node, state, &format!("{}.{}", path, i), true)).collect();
                    self.set_positions(oid, vals, state);
                }
                (_, Some(e)) => {
                    let v = self.materialize(e, unknown, node, state, &format!("{}.e", path), true);
                    self.add_elem(oid, &v, state);
                }
                _ => {}
            }
            return oid;
        }
        for (k, sh) in &os.slots {
            let v = self.materialize(sh, unknown, node, state, &format!("{}.{}", path, k.0), true);
            state.set_slot(oid, ns, *k, v, single);
        }
        oid
    }

    /// The class object whose `class` statement is at `addr` (the last defined).
    fn class_of(&mut self, addr: usize) -> Option<ObjId> {
        if let Some(c) = self.summaries.classes.get(&addr) {
            return Some(*c);
        }
        let found = self.program.user_classes.iter().rev().copied().find(|c| self.heap.class_info_opt(*c).is_some_and(|ci| ci.node as *const _ as usize == addr))?;
        self.summaries.classes.insert(addr, found);
        Some(found)
    }
}

/// A scalar without the run's names: literals as they are, an unknown's type kept, the rest
/// unnamed (`?`).
fn clean_kv(kv: &KeyVal) -> KeyVal {
    match kv {
        KeyVal::Lit(_) | KeyVal::Lits(_) | KeyVal::Bot => kv.clone(),
        KeyVal::Data(l, hint) => match label_base(&intern::str_get(*l)) {
            Some((ty, opt)) => KeyVal::Data(s(&if opt { optional_label(ty, "") } else { typed_label(ty, "") }), hint.clone()),
            None => KeyVal::Data(intern::EMPTY_STR, hint.clone()),
        },
        KeyVal::KeysOf(_) => DATA,
    }
}

/// A scalar as its type alone (`int(?)`, `None`, `None | str(?)`), or unnamed where its
/// literals are of several types.
fn type_only(kv: &KeyVal) -> KeyVal {
    let (mut ty, mut none) = (None::<&'static str>, false);
    for k in crate::domain::kv_literals(kv).iter() {
        if k == intern::key_none() {
            none = true;
            continue;
        }
        match crate::domain::TYPE_NAMES.iter().copied().find(|t| crate::domain::key_type(k, t)) {
            Some(t) if ty.is_none_or(|x| x == t) => ty = Some(t),
            _ => return DATA,
        }
    }
    if let KeyVal::Data(l, _) = kv {
        match label_base(&intern::str_get(*l)) {
            Some((t, opt)) if ty.is_none_or(|x| x == t) => {
                ty = Some(t);
                none |= opt;
            }
            _ => return DATA,
        }
    }
    match (ty, none) {
        (Some(t), false) => KeyVal::Data(s(&typed_label(t, "")), Default::default()),
        (Some(t), true) => KeyVal::Data(s(&optional_label(t, "")), Default::default()),
        (None, true) => KeyVal::Lit(intern::key_none()),
        (None, false) if kv.is_bot() => KeyVal::Bot,
        (None, false) => DATA,
    }
}

/// A getter's body (after a docstring): one `return p.a.b` of attributes of parameter `p`,
/// with the names in the order read; not async, not a generator.
fn getter_path(f: &super::FuncNode) -> Option<(String, Vec<String>)> {
    use ruff_python_ast as ast;
    let super::FuncNode::Def(d) = f else { return None };
    let body: &[ast::Stmt] = match d.body.first() {
        Some(ast::Stmt::Expr(e)) if e.value.is_string_literal_expr() => &d.body[1..],
        _ => &d.body,
    };
    let ([ast::Stmt::Return(r)], false) = (body, d.is_async) else { return None };
    let mut e = r.value.as_deref()?;
    let mut names = Vec::new();
    while let ast::Expr::Attribute(a) = e {
        names.push(a.attr.id.to_string());
        e = &a.value;
    }
    let ast::Expr::Name(n) = e else { return None };
    let params = d.parameters.posonlyargs.iter().chain(d.parameters.args.iter()).chain(d.parameters.kwonlyargs.iter());
    if names.is_empty() || !params.into_iter().any(|p| p.parameter.name.id == n.id) {
        return None;
    }
    names.reverse();
    Some((n.id.to_string(), names))
}
