//! What user code may change beyond what it is given, for a call that does not run its body
//! (past its function's budget, or cut at the deadline): the module globals and closure cells
//! its code (and the code it can reach) names in a position where they may change or escape.

use ruff_python_ast::{Expr, Stmt};
use rustc_hash::{FxHashMap, FxHashSet};

use indexmap::IndexMap;

use crate::domain::{Ns, ObjId, Value};
use crate::frontend::Node;
use crate::intern::{self, key_str_lit, s, str_get, KeyId, StrId};
use crate::state::{Kind, State};

use super::flows::{CodeUses, Flags, Like};
use super::{Analyzer, FuncInfo, Payload};

/// What the cache of code closures (`code_closure`) may hold; past it the cache starts over.
pub const CLOSURE_CACHE: usize = 1 << 18;

/// Caches of what code may change when it does not run. `code_uses` is a function of the
/// program text; a closure is computed once per set of roots from the state of its first
/// computation (the code a name holds is taken as fixed), so it is flow-insensitive state, not
/// only a cache.
#[derive(Default)]
pub struct EffectsCache {
    /// per function code (`FuncNode::addr`): the names it reads beyond its locals
    code_uses: FxHashMap<usize, std::rc::Rc<CodeUses>>,
    /// per code object: what it may change when a call of it does not run (`code_closure`)
    closures: FxHashMap<Vec<ObjId>, std::rc::Rc<Closure>>,
    /// what `closures` holds (roots, code, slots, values), bounded by `CLOSURE_CACHE`
    closures_size: usize,
}

/// What a skipped method may do to its receiver (`receiver_effects`): the attribute names it
/// may store and delete, those it reads (what it reaches through the receiver), and the methods
/// that run with the receiver.
pub struct RecvEffects {
    pub stores: Vec<KeyId>,
    pub deletes: Vec<KeyId>,
    pub used: Vec<KeyId>,
    pub code: Vec<ObjId>,
    /// the templates of names it may store (new attributes too)
    pub stores_like: Vec<Like>,
}

/// A slot: (object, namespace, key).
type Loc = (ObjId, Ns, KeyId);

/// What a piece of code may change beyond what it is given (`code_closure`): the slots whose
/// values may change, those whose values' parts may (`held_locs`: a container iterated, an
/// attribute's value given away), those it may rebind, and other values likewise.
#[derive(Default)]
pub struct Closure {
    /// the code it reaches
    pub code: FxHashSet<ObjId>,
    pub locs: Vec<Loc>,
    pub held_locs: Vec<Loc>,
    pub rebound: Vec<Loc>,
    pub values: Vec<Value>,
    pub held_values: Vec<Value>,
}

impl Analyzer {
    /// The names a function's code reads beyond its locals (computed once per `def`).
    fn code_uses(&mut self, fi: &FuncInfo) -> std::rc::Rc<CodeUses> {
        let addr = fi.node.addr();
        if let Some(u) = self.effects.code_uses.get(&addr) {
            return u.clone();
        }
        let mi = self.heap.module_info(fi.module);
        let scope = mi.scopes.get(fi.scope);
        let locals = |n: &str| scope.is_local(n);
        let first = fi.node.params().and_then(|p| p.posonlyargs.iter().chain(p.args.iter()).next()).map(|p| p.parameter.name.id.as_str());
        // (a method run with its instance: stores on it are not on its class)
        let me = first.filter(|_| fi.cls.is_some() && !fi.is_static.get() && !fi.is_classmethod.get());
        let params: Vec<StrId> = fi.node.params().map(|p| p.iter().map(|x| s(x.name().id.as_str())).collect()).unwrap_or_default();
        let flow = super::flows::Flow::new(&locals, me, mi.pkg.clone());
        let out = match fi.node {
            super::FuncNode::Def(f) => flow.scan(&params, me.map(s), &f.body, None),
            super::FuncNode::Lambda(l) => flow.scan(&params, None, &[], Some(&l.body)),
        };
        let u = std::rc::Rc::new(out);
        self.effects.code_uses.insert(addr, u.clone());
        u
    }

    /// The attribute names the methods of `cls` (and its bases) other than `__init__` store on
    /// some object, and those they delete.
    pub(in crate::interp) fn attrs_set_by_methods(&mut self, cls: ObjId, state: &State) -> (Vec<KeyId>, Vec<KeyId>) {
        let (mut out, mut del): (FxHashSet<StrId>, FxHashSet<StrId>) = Default::default();
        for c in self.heap.mro(cls) {
            for (k, v) in state.obj_slots(c, Ns::Attr) {
                if intern::key_as_str(k).as_deref() == Some("__init__") {
                    continue;
                }
                for f in v.pts.iter() {
                    if let Payload::Func(fi) = &self.heap.obj(f).payload {
                        let fi = fi.clone();
                        let u = self.code_uses(&fi);
                        out.extend(u.stored.iter().copied());
                        del.extend(u.deleted.iter().copied());
                    }
                }
            }
        }
        let keys = |set: FxHashSet<StrId>| {
            let mut ks: Vec<KeyId> = set.into_iter().map(|a| key_str_lit(&str_get(a))).collect();
            ks.sort_unstable();
            ks
        };
        (keys(out), keys(del))
    }

    /// What a call of method `foid` that does not run may do to its receiver `recv` itself, by
    /// its own code and the methods and properties of the receiver's class that run with it:
    /// the attribute names it may store and delete, and the names it reads on it (what it can
    /// reach through the receiver is under those). None: anything (the receiver escapes,
    /// the class hooks attribute access or has a base rowan does not see, a descriptor).
    pub(in crate::interp) fn receiver_effects(&mut self, foid: ObjId, recv: ObjId, state: &State, bind: &dyn Fn(&str) -> Option<String>) -> Option<RecvEffects> {
        let cls = self.heap.instance_class(recv)?;
        let mro = self.heap.mro(cls);
        if mro.iter().any(|c| self.heap.class_info_opt(*c).is_none_or(|ci| ci.open_base)) || ["__getattr__", "__getattribute__", "__setattr__", "__delattr__"].iter().any(|h| self.class_lookup(cls, h).is_some()) {
            return None;
        }
        let (mut stores, mut deletes, mut used): (FxHashSet<StrId>, FxHashSet<StrId>, FxHashSet<StrId>) = Default::default();
        let (mut loads_like, mut stores_like, mut deletes_like): (Vec<Like>, Vec<Like>, Vec<Like>) = Default::default();
        let mut work = vec![foid];
        let mut done: FxHashSet<ObjId> = FxHashSet::default();
        let runs_with_recv = |an: &Analyzer, g: ObjId| an.heap.kind(g) == Kind::Function && !an.heap.func_info(g).is_static.get() && !an.heap.func_info(g).is_classmethod.get();
        while let Some(f) = work.pop() {
            if !done.insert(f) {
                continue;
            }
            let Payload::Func(fi) = &self.heap.obj(f).payload else { return None };
            let fi = fi.clone();
            let uses = self.code_uses(&fi);
            let u = uses.recv.as_ref().filter(|u| !u.escapes)?;
            stores.extend(u.stores.iter().copied());
            deletes.extend(u.deletes.iter().copied());
            // (the called method's own templates: the names its arguments make)
            let named = |an: &Analyzer, likes: &[Like], to: &mut FxHashSet<StrId>, rest: &mut Vec<Like>| -> Option<()> {
                for l in likes {
                    match l.resolve(bind).filter(|_| f == foid) {
                        Some(n) if an.class_lookup(cls, &n).is_some() => return None,
                        Some(n) => drop(to.insert(s(&n))),
                        None => rest.push(l.clone()),
                    }
                }
                Some(())
            };
            named(self, &u.loads_like, &mut used, &mut loads_like)?;
            named(self, &u.stores_like, &mut stores, &mut stores_like)?;
            named(self, &u.deletes_like, &mut deletes, &mut deletes_like)?;
            for &n in u.loads.iter().chain(u.stores.iter()).chain(u.deletes.iter()).chain(u.calls.iter()) {
                // (what a name only stored into or deleted held before is not the code's to reach)
                if u.loads.contains(&n) || u.calls.contains(&n) {
                    used.insert(n);
                }
                // (a name the class does not define is the instance's own attribute)
                let k = key_str_lit(&str_get(n));
                let Some(owner) = self.class_lookup(cls, &str_get(n)) else { continue };
                for g in self.heap.cls_slots.get(&(owner, Ns::Attr, k)).cloned().unwrap_or_default().pts.iter() {
                    match self.heap.kind(g) {
                        // (a method read without a call: a bound method holding the receiver)
                        Kind::Function if runs_with_recv(self, g) && u.loads.contains(&n) && !self.heap.func_info(g).is_property.get() => return None,
                        Kind::Function if runs_with_recv(self, g) => work.push(g),
                        // (a descriptor's code runs with the receiver)
                        Kind::Instance if self.heap.instance_class(g).is_some_and(|c| ["__get__", "__set__", "__delete__"].iter().any(|h| self.class_lookup(c, h).is_some())) => return None,
                        _ => {}
                    }
                }
                if u.stores.contains(&n) {
                    work.extend(self.heap.prop_setters.get(&(owner, k)).copied());
                }
            }
            for &n in u.supers.iter() {
                let k = key_str_lit(&str_get(n));
                for c in &mro {
                    let fv = self.heap.cls_slots.get(&(*c, Ns::Attr, k)).cloned().unwrap_or_default();
                    work.extend(fv.pts.iter().filter(|g| runs_with_recv(self, *g)));
                }
            }
        }
        // (names by a template: the receiver's attributes they name now — not one of the
        // class's, which may be a method bound to the receiver or a descriptor —, and new ones)
        let any_like = |likes: &[Like], k: KeyId| likes.iter().any(|l| l.matches(k));
        let all_like: Vec<Like> = loads_like.iter().chain(&stores_like).chain(&deletes_like).cloned().collect();
        if !all_like.is_empty() && mro.iter().any(|c| state.obj_slots(*c, Ns::Attr).iter().any(|(k, _)| any_like(&all_like, *k))) {
            return None;
        }
        let as_str = |k: KeyId| s(&intern::key_as_str(k).unwrap_or_default());
        for k in state.row(recv, Ns::Attr).present.iter() {
            if any_like(&loads_like, k) {
                used.insert(as_str(k));
            }
            if any_like(&stores_like, k) {
                stores.insert(as_str(k));
            }
            if any_like(&deletes_like, k) {
                deletes.insert(as_str(k));
            }
        }
        let keys = |set: FxHashSet<StrId>| {
            let mut ks: Vec<KeyId> = set.into_iter().map(|a| key_str_lit(&str_get(a))).collect();
            ks.sort_unstable();
            ks
        };
        let mut code: Vec<ObjId> = done.into_iter().collect();
        code.sort_unstable();
        // (a deleted name stored again: as a store, the row opened for both)
        stores_like.extend(deletes_like);
        Some(RecvEffects { stores: keys(stores), deletes: keys(deletes), used: keys(used), code, stores_like })
    }

    /// The attribute names a method's own code stores on its receiver on every way it
    /// completes: the top-level `self.x = …` statements before any statement that may return.
    pub(in crate::interp) fn must_stores(&self, foid: ObjId) -> Vec<KeyId> {
        let Payload::Func(fi) = &self.heap.obj(foid).payload else { return Vec::new() };
        let (super::FuncNode::Def(f), Some(me)) = (fi.node, fi.node.params().and_then(|p| p.posonlyargs.iter().chain(p.args.iter()).next())) else { return Vec::new() };
        let me = me.parameter.name.id.as_str();
        let mine = |t: &Expr| match t {
            Expr::Attribute(a) if matches!(&*a.value, Expr::Name(n) if n.id.as_str() == me) => Some(key_str_lit(a.attr.as_str())),
            _ => None,
        };
        let mut out = Vec::new();
        for st in f.body.iter().take_while(|st| !super::stmts::contains_return(std::slice::from_ref(*st)) && !super::is_generator(std::slice::from_ref(*st))) {
            match st {
                Stmt::Assign(a) => out.extend(a.targets.iter().filter_map(mine)),
                Stmt::AnnAssign(a) if a.value.is_some() => out.extend(mine(&a.target)),
                Stmt::AugAssign(a) => out.extend(mine(&a.target)),
                _ => {}
            }
        }
        out
    }

    /// Where `name` (read in the code of `fi`) is bound — a cell, a module's global — or the
    /// module it names (an import of a whole module in the function).
    fn resolve_use(&self, fi: &FuncInfo, uses: &CodeUses, name: StrId) -> Result<Loc, ObjId> {
        if let Some(&cell) = fi.closure.get(&name) {
            return Ok((cell, Ns::Cell, super::funcs::cell_key()));
        }
        if let Some(full) = uses.imports.get(&name) {
            if let Some(&m) = self.program.modules.get(full) {
                return Err(m);
            }
            if let Some((m, a)) = full.rsplit_once('.').and_then(|(m, a)| self.program.modules.get(m).map(|mo| (*mo, a))) {
                return Ok((m, Ns::Attr, key_str_lit(a)));
            }
        }
        Ok((fi.module, Ns::Attr, key_str_lit(&str_get(name))))
    }

    /// What the code `roots` hold (functions, bound methods, classes, instances' classes) may
    /// change when it runs, through the code it can reach by the names it reads (functions,
    /// classes, the methods of those classes it reads, dunders): the slots whose values may
    /// change (globals, cells, class attributes by name) or be rebound, and other values it may
    /// change (mutable defaults, bound receivers). Computed once per set of roots: the code a
    /// name holds is taken as fixed (rebinding a function or class is monkey-patching), and
    /// methods called by a computed name are not followed; the code in the data those values
    /// hold is followed where the call is (`skipped_call`).
    pub(in crate::interp) fn code_closure(&mut self, roots: &[ObjId], state: &State) -> std::rc::Rc<Closure> {
        if let Some(c) = self.effects.closures.get(roots) {
            return c.clone();
        }
        let mut out = Closure::default();
        let mut work: Vec<ObjId> = roots.to_vec();
        let (mut attrs, mut reads, mut class_stores): (FxHashMap<StrId, Flags>, FxHashSet<StrId>, FxHashSet<StrId>) = Default::default();
        let mut classes: Vec<ObjId> = Vec::new();
        let (mut done, mut done_attrs): (FxHashSet<(ObjId, KeyId)>, FxHashSet<(ObjId, KeyId)>) = Default::default();
        let code = |an: &Analyzer, v: &Value, work: &mut Vec<ObjId>| work.extend(v.pts.iter().filter(|x| matches!(an.heap.kind(*x), Kind::Function | Kind::Class | Kind::Bound | Kind::Instance | Kind::Partial | Kind::Module)));
        loop {
            while let Some(o) = work.pop() {
                if !out.code.insert(o) {
                    continue;
                }
                match (self.heap.kind(o), &self.heap.obj(o).payload) {
                    (Kind::Function, Payload::Func(fi)) => {
                        let fi = fi.clone();
                        self.function_closure(&fi, state, &mut out, &mut attrs, &mut reads, &mut class_stores, &mut work);
                    }
                    (Kind::Module, _) => {
                        // (a module used as a value: any of its globals)
                        for (k, v) in state.obj_slots(o, Ns::Attr) {
                            out.locs.push((o, Ns::Attr, k));
                            code(self, &v, &mut work);
                        }
                    }
                    (Kind::Class, _) => {
                        classes.push(o);
                        if let Some(ci) = self.heap.class_info_opt(o) {
                            work.extend(ci.bases.iter().copied());
                        }
                    }
                    (Kind::Instance, Payload::Cls(c)) => work.push(*c),
                    (Kind::Bound, Payload::Bound(f, recv)) => {
                        work.push(*f);
                        out.values.push(recv.clone());
                    }
                    (Kind::Partial, Payload::Partial(f, args, kw)) => {
                        code(self, f, &mut work);
                        out.values.extend(args.iter().chain(kw.values()).cloned());
                    }
                    _ => {}
                }
            }
            // the methods of the classes reached that the code reads (dunders run implicitly),
            // and their attributes it may change or rebind
            for &c in &classes {
                for (k, v) in state.obj_slots(c, Ns::Attr) {
                    let name = intern::key_as_str(k).map(|n| s(&n));
                    if (name.is_some_and(|n| reads.contains(&n) || str_get(n).starts_with("__"))) && done.insert((c, k)) {
                        code(self, &v, &mut work);
                    }
                    let (flags, rebinds) = name.map(|n| (attrs.get(&n).copied(), class_stores.contains(&n))).unwrap_or_default();
                    if (flags.is_some() || rebinds) && done_attrs.insert((c, k)) {
                        let mut f = flags.unwrap_or_default();
                        f.rebinds = rebinds;
                        self.apply_flags(&f, (c, Ns::Attr, k), state, &mut out);
                    }
                }
            }
            if work.is_empty() {
                for l in [&mut out.locs, &mut out.held_locs, &mut out.rebound] {
                    l.sort_unstable();
                    l.dedup();
                }
                let c = std::rc::Rc::new(out);
                // (a cache of what the program's code reaches, by root set: on large programs
                // the root sets rarely repeat, so it is emptied past a bound on what it holds)
                let size = roots.len() + c.code.len() + c.locs.len() + c.held_locs.len() + c.rebound.len() + c.values.len() + c.held_values.len();
                self.effects.closures_size += size;
                if self.effects.closures_size > CLOSURE_CACHE {
                    self.effects.closures.clear();
                    self.effects.closures_size = size;
                }
                self.effects.closures.insert(roots.to_vec(), c.clone());
                return c;
            }
        }
    }

    /// A call of user code whose body does not run (past its budget, cut at the deadline): an
    /// unknown call that may also change what the code reaches beyond its arguments (the globals
    /// and cells it, and the code it can call, reads where they may change).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn skipped_call(&mut self, foid: ObjId, q: &str, self_v: Option<&Value>, args: &[Value], kwargs: &IndexMap<StrId, Value>, star_args: &[Value], star_kwargs: &[Value], state: &mut State, node: &Node) -> Value {
        // a method whose code touches its receiver only by attribute names (`self.x`, methods
        // run on it): the receiver's own attributes change only by those names, unless what
        // the call reaches otherwise reaches the receiver too (`Havoc::guard`); one of several
        // receivers (a method of each, past its budget) likewise, each by its names
        let recvs: Vec<ObjId> = self_v.map(|v| v.pts.to_vec()).filter(|r| !r.is_empty() && r.iter().all(|o| self.heap.kind(*o) == Kind::Instance)).unwrap_or_default();
        // (a literal argument's text, for the names its templates make)
        let fi = self.heap.func_info(foid);
        let positional: Vec<String> = fi.node.params().map(|p| p.posonlyargs.iter().chain(p.args.iter()).skip(usize::from(self_v.is_some())).map(|a| a.parameter.name.id.to_string()).collect()).unwrap_or_default();
        let bind = |n: &str| -> Option<String> {
            let given = positional.iter().position(|p| p == n).filter(|_| star_args.is_empty()).and_then(|i| args.get(i)).or_else(|| kwargs.get(&s(n)));
            let v = match given {
                Some(v) => v,
                None if star_kwargs.is_empty() && !positional.iter().position(|p| p == n).is_some_and(|i| i < args.len() || !star_args.is_empty()) => fi.defaults.get(&s(n))?,
                None => return None,
            };
            v.kv.as_lit().filter(|_| v.pts.is_empty()).and_then(intern::key_as_str).map(|x| x.to_string())
        };
        let effs: Vec<(ObjId, RecvEffects)> = recvs.iter().map(|&o| Some((o, self.receiver_effects(foid, o, state, &bind)?))).collect::<Option<_>>().unwrap_or_default();
        // (on one receiver: the stores its code makes on every way it completes)
        let must: Vec<KeyId> = match effs.as_slice() {
            [(_, e)] if fi_is_method(self, foid) => self.must_stores(foid).into_iter().filter(|k| !e.deletes.contains(k)).collect(),
            _ => Vec::new(),
        };
        self.havoc.guard = effs.iter().map(|(o, e)| (*o, 0, std::rc::Rc::from(e.used.as_slice()))).collect();
        let walked = self.havoc.visited;
        let by_names = !effs.is_empty();
        let r = self.unknown_call(q, self_v, args, kwargs, star_args, star_kwargs, state, node);
        let site = s(&format!("{}@L{}", q, node.line));
        self.names.skipped_sites.insert(site);
        let mut roots = vec![foid];
        roots.append(&mut self.havoc.code);
        // (the methods that run with the receiver: what its code reaches through it)
        roots.extend(effs.iter().flat_map(|(_, e)| e.code.iter().copied()));
        let rid = r.single().unwrap_or(0);
        self.follow_code(roots, rid, site, state);
        let met: Vec<u32> = std::mem::take(&mut self.havoc.guard).into_iter().map(|(_, n, _)| n).collect();
        for ((o, RecvEffects { stores, deletes, stores_like, .. }), n) in effs.into_iter().zip(met) {
            if n > 1 {
                let walk = self.havoc.new_walk();
                self.havoc.walk_from(&mut self.heap, &[Value::obj(o)], rid, site, state, walk);
            } else {
                let any = Value::obj(crate::interp::any_object(&mut self.heap));
                let mut row = state.row(o, Ns::Attr);
                for k in stores {
                    let v = state.slot(o, Ns::Attr, k).join(&any);
                    state.set_slot(o, Ns::Attr, k, v, false);
                    row = row.store(k, false);
                }
                for k in deletes {
                    row = row.delete(k, false);
                }
                if !stores_like.is_empty() {
                    row = row.mutate_like(site, &|k| stores_like.iter().any(|l| l.matches(k)));
                }
                state.set_row(o, Ns::Attr, row);
            }
        }
        if super::debug_enabled() {
            eprintln!("skip {} at M{}:L{} havoc={}{}", q, node.module, node.line, self.havoc.visited - walked, if by_names { " (receiver by names)" } else { "" });
        }
        // (a raise after the code's effects on what it reaches beyond its arguments)
        self.flag_may_raise(state);
        // (what its own code stores on every way it completes is there after the call: the
        // raises above may come before)
        if let ([o], false) = (recvs.as_slice(), must.is_empty()) {
            let o = *o;
            let any = Value::obj(crate::interp::any_object(&mut self.heap));
            let mut row = state.row(o, Ns::Attr);
            for k in must {
                state.set_slot(o, Ns::Attr, k, any.clone(), true);
                row = row.store(k, true);
            }
            state.set_row(o, Ns::Attr, row);
        }
        r
    }

    /// What a function's code adds to a closure: what may happen to the values of the names it
    /// reads (globals, cells, modules' globals) and of its defaults, the code they hold, and
    /// what may happen to attributes by name (`attrs`, `class_stores`: class attributes).
    #[allow(clippy::too_many_arguments)]
    fn function_closure(&mut self, fi: &FuncInfo, state: &State, out: &mut Closure, attrs: &mut FxHashMap<StrId, Flags>, reads: &mut FxHashSet<StrId>, class_stores: &mut FxHashSet<StrId>, work: &mut Vec<ObjId>) {
        let code = |an: &Analyzer, v: &Value, work: &mut Vec<ObjId>| work.extend(v.pts.iter().filter(|x| matches!(an.heap.kind(*x), Kind::Function | Kind::Class | Kind::Bound | Kind::Instance | Kind::Partial | Kind::Module)));
        let uses = self.code_uses(fi);
        for (a, f) in &uses.attrs {
            attrs.entry(*a).or_default().join(f);
        }
        reads.extend(uses.reads.iter().copied());
        class_stores.extend(uses.class_stores.iter().copied());
        // (a default is the function's own state: it changes where its parameter does)
        for (p, v) in fi.defaults.iter() {
            code(self, v, work);
            if let Some(f) = uses.params.get(p) {
                let (own, held) = self.flag_effect(f, v, state);
                if own {
                    out.values.push(v.clone());
                } else if held {
                    out.held_values.push(v.clone());
                }
            }
        }
        for u in &uses.uses {
            // (`m.x` of a module m: its global x; of anything else: what m holds, and a class
            // attribute x — a method call on it may change it, a store changes it)
            let mut f = u.flags;
            let (loc, in_module) = match (self.resolve_use(fi, &uses, u.name), u.attr) {
                (Ok(l), None) => (l, false),
                (Ok(l), Some(a)) => match state.slot(l.0, l.1, l.2).single().filter(|m| self.heap.kind(*m) == Kind::Module) {
                    Some(m) => ((m, Ns::Attr, key_str_lit(&str_get(a))), true),
                    None => (l, false),
                },
                (Err(m), Some(a)) => ((m, Ns::Attr, key_str_lit(&str_get(a))), true),
                (Err(m), None) => {
                    work.push(m);
                    continue;
                }
            };
            if let (Some(a), false) = (u.attr, in_module) {
                let by_name = Flags { rebinds: false, called: false, ..f };
                attrs.entry(a).or_default().join(&by_name);
                // (`K.x = …` of a class K: K's attribute rebound)
                if f.rebinds {
                    let k = key_str_lit(&str_get(a));
                    out.rebound.extend(state.slot(loc.0, loc.1, loc.2).pts.iter().filter(|c| self.heap.kind(*c) == Kind::Class).map(|c| (c, Ns::Attr, k)));
                }
                f = Flags { changes: f.called || f.rebinds, held: f.changes || f.held || f.iterated, ..Flags::default() };
            }
            self.apply_flags(&f, loc, state, out);
            code(self, &state.slot(loc.0, loc.1, loc.2), work);
        }
    }

    /// What `f` makes of a slot's value: it may change, what it holds may, it may be rebound.
    fn apply_flags(&self, f: &Flags, loc: Loc, state: &State, out: &mut Closure) {
        let (own, held) = self.flag_effect(f, &state.slot(loc.0, loc.1, loc.2), state);
        if own {
            out.locs.push(loc);
        } else if held {
            out.held_locs.push(loc);
        }
        if f.rebinds {
            out.rebound.push(loc);
        }
    }

    /// Whether value `v` (own) or what it holds (held) may change by `f`: iterating a plain
    /// container uses what it holds (an iterator advances); calling code runs it (the closure
    /// follows it), calling a library method that keeps nothing changes nothing.
    fn flag_effect(&self, f: &Flags, v: &Value, _state: &State) -> (bool, bool) {
        let plain = |o: ObjId| match self.heap.kind(o) {
            Kind::Dict | Kind::Set | Kind::Tuple => true,
            Kind::List => self.heap.obj(o).label.starts_with("list@"),
            _ => false,
        };
        let code_only = |o: ObjId| match (self.heap.kind(o), &self.heap.obj(o).payload) {
            (Kind::Function | Kind::Class | Kind::Partial | Kind::Module | Kind::External, _) => true,
            (Kind::Method, Payload::Method(q, _)) => q.rsplit('.').next().is_some_and(|m| super::PURE_METHODS.contains(&m)),
            _ => false,
        };
        let own = f.changes || (f.iterated && !v.pts.iter().all(plain)) || (f.called && !v.pts.iter().all(code_only));
        (own, f.held || f.iterated)
    }
}

/// A function run with a receiver (not a static / class method).
fn fi_is_method(an: &Analyzer, f: ObjId) -> bool {
    an.heap.func_info_opt(f).is_some_and(|fi| fi.cls.is_some() && !fi.is_static.get() && !fi.is_classmethod.get())
}
