//! Deferred definitions: `@d1 … @dn def f` in a module's code (not the main module's: its
//! values are what the editor shows), where every decorator is the program's function whose
//! own code is *plain* inert (`inert::inert`: straight-line, it makes a wrapper with
//! `functools.wraps`, stores attributes on it or on `f`, reads nothing but the names it is
//! bound to and what every function has, gives back a function). Its application changes
//! nothing but `f` and what it makes, cannot raise, and reads only the bindings of its names:
//! it can run whenever `f` is first read instead of at the `def`, from those bindings as they
//! were at the `def` (`Deferred::names`). The name is bound to a placeholder (an unknown
//! object, `Payload::Deferred`) until a read of it (`force`: a name, a module's attribute, a
//! call or an attribute of the value, a module's namespace as a whole); unknown code that may
//! reach it may run the decorators' code and `f`'s (`deferred_code`). A definition nobody
//! reads costs nothing; one read gives what running it at the `def` gives.

use std::rc::Rc;

use ruff_python_ast as ast;

use super::effects::Loc;
use super::heap::Heap;
use super::{Analyzer, Args, FrameKind, Payload};
use crate::domain::{Ns, ObjId, Value};
use crate::frontend::{self, Node};
use crate::intern::{self, key_str_lit, str_get, KeyId};
use crate::state::{Kind, State};

/// A deferred definition (`Payload::Deferred`).
#[derive(Debug)]
pub struct Deferred {
    /// the function the `def` made
    pub f: Value,
    /// the decorators' functions, innermost first, each with its application's node, call site
    /// and allocation context (as at the `def`)
    pub apps: Vec<(ObjId, Node, String, String)>,
    /// the names their code reads (globals, cells), bound as at the `def`
    pub names: Vec<(Loc, Value)>,
    /// the call chain at the `def`, and its modules
    pub chain: (Vec<u32>, Vec<u32>),
}

/// The library functions a plain inert decorator may call, and how many arguments; all its
/// own values but `setattr`'s value (`functools.wraps(f)` copies what `f` has).
const PLAIN_CALLS: &[(&str, usize, bool)] = &[("builtins.setattr", 3, false), ("builtins.getattr", 3, false), ("builtins.hasattr", 2, true), ("functools.wraps", 1, true), ("functools.update_wrapper", 2, true)];

/// The slot a placeholder's value is kept in once it ran on a path.
fn forced_key() -> KeyId {
    key_str_lit("\u{1}forced")
}

/// The code a placeholder stands for: its decorators' functions and the decorated function.
pub(in crate::interp) fn deferred_code(heap: &Heap, o: ObjId) -> Option<Vec<ObjId>> {
    match &heap.obj(o).payload {
        Payload::Deferred(d) => Some(d.apps.iter().map(|a| a.0).chain(d.f.pts.iter()).collect()),
        _ => None,
    }
}

impl Analyzer {
    /// The value `@d1 … @dn def f` binds when every decorator is the program's function, in a
    /// module's code: the expressions evaluated here (outermost first, as Python does), then a
    /// placeholder when they can run later (see the module's doc), else applied here. None:
    /// `apply_decorators` applies them.
    pub(in crate::interp) fn decorate_def(&mut self, f: &'static ast::StmtFunctionDef, fv: &Value, state: &mut State) -> Option<Value> {
        let fr = self.frame();
        if fr.kind != FrameKind::Module || fr.module == self.program.main_module || fr.loop_depth > 0 || f.decorator_list.is_empty() || self.effects.whole.is_some() {
            return None;
        }
        if !f.decorator_list.iter().all(|d| self.decorator_qualname(&d.expression, state).is_some_and(|q| q.starts_with("user:"))) {
            return None;
        }
        let mut dvs: Vec<Value> = Vec::new();
        for d in &f.decorator_list {
            let (target, call): (&'static ast::Expr, _) = match &d.expression {
                ast::Expr::Call(c) => (&c.func, Some(c)),
                other => (other, None),
            };
            let mut dv = self.eval(target, state);
            if let Some(c) = call {
                dv = self.call_value(&dv, &frontend::node(&d.expression), Some(c), state, None);
            }
            dvs.push(dv);
        }
        let mut names: Vec<(Loc, Value)> = Vec::new();
        let plain = state.reachable && dvs.iter().all(|dv| self.plain_decorator(dv, state, &mut names));
        if !plain {
            let mut v = fv.clone();
            for (d, dv) in f.decorator_list.iter().zip(dvs).rev() {
                v = self.call_value(&dv, &frontend::node(d), None, state, Some(vec![v]));
            }
            return Some(v);
        }
        let mut apps = Vec::new();
        for (d, dv) in f.decorator_list.iter().zip(dvs).rev() {
            let at = frontend::node(d);
            let site = self.callee_site(&at);
            let ctx = self.receiver_ctx(&None, &site);
            apps.push((dv.single()?, at, site, ctx));
        }
        let n = frontend::node(f);
        let d = Deferred { f: fv.clone(), apps, names, chain: (self.cx.call_chain.clone(), self.cx.call_files.clone()) };
        let ctx = self.frame().ctx.clone();
        let p = self.heap.alloc_raw(Kind::Unknown, &format!("deferred:{}", super::site_string(&n)), &ctx, true, &format!("@{}", f.name.id), Payload::Deferred(Rc::new(d)));
        self.heap.deferred += 1;
        Some(Value::obj(p))
    }

    /// Is `dv` one function of the program whose code is plain inert (`inert::inert`), calling
    /// only `PLAIN_CALLS` as they allow, every name it reads bound here? Those names' bindings
    /// go to `names`.
    fn plain_decorator(&mut self, dv: &Value, state: &State, names: &mut Vec<(Loc, Value)>) -> bool {
        let Some(o) = dv.single().filter(|o| self.heap.kind(*o) == Kind::Function && dv.kv.is_bot()) else { return false };
        let Some(inert) = self.inert(o).filter(|i| i.plain) else { return false };
        let fi = self.heap.func_info(o);
        let mut bound: Vec<(intern::StrId, Value)> = Vec::new();
        for &n in &inert.free {
            let loc: Loc = match fi.closure.get(&n) {
                Some(&cell) => (cell, Ns::Cell, super::funcs::cell_key()),
                None => (fi.module, Ns::Attr, intern::key_of_str(n)),
            };
            match state.slot_opt(loc.0, loc.1, loc.2) {
                Some(v) => {
                    bound.push((n, v.clone()));
                    names.push((loc, v));
                }
                None if loc.1 == Ns::Attr && crate::models::tables::is_builtin_name(&str_get(n)) => {}
                None => return false,
            }
        }
        // (each call's callee: a library function by its qualified name, as allowed)
        inert.calls.iter().all(|c| {
            let Some((&head, rest)) = c.callee.split_first() else { return false };
            let q = match bound.iter().find(|(n, _)| *n == head) {
                Some((_, v)) => match v.single().filter(|x| self.heap.kind(*x) == Kind::External && v.kv.is_bot()) {
                    Some(x) => self.heap.external_qual(x),
                    None => return false,
                },
                None => format!("builtins.{}", str_get(head)),
            };
            let q = rest.iter().fold(q, |q, a| format!("{}.{}", q, str_get(*a)));
            PLAIN_CALLS.iter().any(|(name, n, all)| *name == q && c.own_args.len() == *n && c.own_args[0] && (!all || c.own_args.iter().all(|x| *x)))
        })
    }

    /// `v` with each placeholder in it run (`force_one`): what it stands for on this path.
    pub(in crate::interp) fn force(&mut self, v: Value, state: &mut State) -> Value {
        if self.heap.deferred == 0 || !v.pts.iter().any(|o| matches!(self.heap.obj(o).payload, Payload::Deferred(_))) {
            return v;
        }
        let mut out = Value { pts: v.pts.iter().filter(|o| !matches!(self.heap.obj(*o).payload, Payload::Deferred(_))).collect(), ..v.clone() };
        let ps: Vec<ObjId> = v.pts.iter().filter(|o| matches!(self.heap.obj(*o).payload, Payload::Deferred(_))).collect();
        for p in ps {
            out = out.join(&self.force_one(p, state));
        }
        out
    }

    /// The value in slot `(o, Attr, k)` with its placeholders run (`force`), put back there.
    pub(in crate::interp) fn force_slot(&mut self, o: ObjId, k: KeyId, v: Value, state: &mut State) -> Value {
        let r = self.force(v.clone(), state);
        if r != v && state.slot_opt(o, Ns::Attr, k).as_ref() == Some(&v) {
            state.slots.set(&o, (Ns::Attr, k), r.clone());
        }
        r
    }

    /// Every placeholder module `m` holds run (its namespace read as a whole).
    pub fn force_module(&mut self, m: ObjId, state: &mut State) {
        if self.heap.deferred == 0 || self.heap.kind(m) != Kind::Module {
            return;
        }
        for (k, v) in state.obj_slots(m, Ns::Attr) {
            self.force_slot(m, k, v, state);
        }
    }

    /// Run a placeholder's decorators on its function, here, from the bindings of their names
    /// as at the `def` (put back after), in the `def`'s call context; once per path.
    fn force_one(&mut self, p: ObjId, state: &mut State) -> Value {
        if let Some(r) = state.slot_opt(p, Ns::Attr, forced_key()) {
            return r;
        }
        let Payload::Deferred(d) = &self.heap.obj(p).payload else { return Value::obj(p) };
        let d = d.clone();
        if !state.reachable {
            return Value::new();
        }
        self.heap.forced += 1;
        let now: Vec<Option<Value>> = d.names.iter().map(|(l, _)| state.slot_opt(l.0, l.1, l.2)).collect();
        for (l, v) in &d.names {
            state.slots.set(&l.0, (l.1, l.2), v.clone());
        }
        let chain = std::mem::replace(&mut self.cx.call_chain, d.chain.0.clone());
        let files = std::mem::replace(&mut self.cx.call_files, d.chain.1.clone());
        let mut r = d.f.clone();
        for (dv, node, site, ctx) in &d.apps {
            self.gens.force_at = Some((site.clone(), ctx.clone()));
            r = self.call_user(*dv, None, Args::pos(std::slice::from_ref(&r)), state, node, false);
            self.gens.force_at = None;
        }
        self.cx.call_chain = chain;
        self.cx.call_files = files;
        for ((l, _), v) in d.names.iter().zip(now) {
            match v {
                Some(v) => state.slots.set(&l.0, (l.1, l.2), v),
                None => drop(state.slots.remove(&l.0, &(l.1, l.2))),
            }
        }
        state.set_slot(p, Ns::Attr, forced_key(), r.clone(), true);
        r
    }
}
