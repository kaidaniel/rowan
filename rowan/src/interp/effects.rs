//! What user code may change beyond what it is given, for a call that does not run its body
//! (past its function's budget, or cut at the deadline): the module globals and closure cells
//! its code (and the code it can reach) names in a position where they may change or escape.

use ruff_python_ast::{Expr, Stmt};
use rustc_hash::{FxHashMap, FxHashSet};

use indexmap::IndexMap;

use crate::domain::{Ns, ObjId, Value};
use crate::frontend::Node;
use ruff_python_ast as ast;
use crate::intern::{self, key_str_lit, s, str_get, KeyId, StrId};
use crate::state::{Kind, State};

use super::flows::{Callee, CodeUses, Flags, Level, Like, NameUses, Origin, Pass, Slot};
use super::{Analyzer, Args, FuncInfo, Payload};

/// What the cache of code closures (`code_closure`) may hold; past it the cache starts over.
pub const CLOSURE_CACHE: usize = 1 << 21;

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
    /// on demand: what the program's functions may do to attributes by name, and the names
    /// they store (`program_uses`)
    program: Option<std::rc::Rc<(FxHashMap<StrId, Flags>, FxHashSet<StrId>)>>,
    /// while the program's code is followed whole (`after_any_code`): per function code
    /// (`FuncNode::addr`) whose callers are all the program's code by name, what they may do
    /// with its result (`caller_returns`); a function not here gives its result away
    pub whole: Option<std::rc::Rc<FxHashMap<usize, Flags>>>,
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
    /// those whose values' parts' parts may (`Flags::held2`)
    pub held2_locs: Vec<Loc>,
    pub rebound: Vec<Loc>,
    pub values: Vec<Value>,
    pub held_values: Vec<Value>,
    /// the slots and values changed in place as containers (their items, not what they are),
    /// and those only iterated (what their items run when compared is met)
    pub edited_locs: Vec<Loc>,
    pub edited_values: Vec<Value>,
    pub iterated_locs: Vec<Loc>,
    pub iterated_values: Vec<Value>,
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

    /// On demand, an instance any call may get: any of the program's functions may have run with
    /// it since it was made, given anything. An attribute some function stores may hold
    /// anything; what their code may do to the values of attributes, by name (`CodeUses::attrs`),
    /// is done: instances and torch layers held there (layers in a list, a called member)
    /// likewise, other values walked by unknown code. An instance a method of whose class gives
    /// its receiver away is walked whole.
    pub(in crate::interp) fn methods_may_have_run(&mut self, inst: ObjId, state: &mut State) {
        let site = s("program@");
        let any = Value::obj(crate::interp::any_object(&mut self.heap));
        let uses = self.program_uses();
        let (by_name, stored) = (&uses.0, &uses.1);
        let mut work: Vec<ObjId> = vec![inst];
        let mut seen: FxHashSet<ObjId> = FxHashSet::default();
        let (mut walk, mut edited): (Vec<Value>, Vec<ObjId>) = Default::default();
        while let Some(inst) = work.pop() {
            if !seen.insert(inst) {
                continue;
            }
            // (a method that gives its receiver away: unknown code may do anything to it — what
            // the program's code may change otherwise is changed already, `on_demand`)
            if self.heap.instance_class(inst).is_some_and(|cls| self.escapes_in_methods(cls, state)) {
                walk.push(Value::obj(inst));
                continue;
            }
            for (k, v) in state.obj_slots(inst, Ns::Attr) {
                let Some(name) = intern::key_as_str(k).map(|n| s(&n)) else { continue };
                if stored.contains(&name) {
                    state.set_slot(inst, Ns::Attr, k, v.join(&any), false);
                }
                let Some(f) = by_name.get(&name).copied() else { continue };
                let (own, held) = self.flag_effect(&f, &v, state);
                let held = held || f.held2;
                // (a value given to code rowan does not follow is walked; one only called or
                // iterated: its instances likewise by name, the rest walked; a container changed
                // in place: its items)
                if self.edited(&f, &v, state).0 {
                    edited.extend(v.pts.iter());
                }
                let mut objs: Vec<ObjId> = Vec::new();
                if own && f.changes {
                    walk.push(v.clone());
                } else {
                    if own {
                        objs.extend(v.pts.iter());
                    }
                    if held {
                        for o in v.pts.iter() {
                            self.obj_edges(o, state, &mut objs);
                        }
                    }
                }
                for o in objs {
                    // (a torch layer, called, runs its model: it changes by name as an instance does)
                    if self.heap.kind(o) == Kind::Instance || crate::models::torch::t_layer_type(self, o).is_some() {
                        work.push(o);
                    } else {
                        walk.push(Value::obj(o));
                    }
                }
            }
        }
        let rid = crate::interp::any_object(&mut self.heap);
        let w = self.havoc.new_walk();
        self.havoc.walk_from(&mut self.heap, &walk, rid, site, state, w);
        self.havoc.edit(&mut self.heap, &edited, true, site, state);
    }

    /// What the program's own functions may do to the values of attributes by name, and the
    /// attribute names they store (computed once; an `__init__`'s on its own receiver aside).
    fn program_uses(&mut self) -> std::rc::Rc<(FxHashMap<StrId, Flags>, FxHashSet<StrId>)> {
        if let Some(u) = &self.effects.program {
            return u.clone();
        }
        let (mut by_name, mut stored): (FxHashMap<StrId, Flags>, FxHashSet<StrId>) = Default::default();
        let mut seen: FxHashSet<usize> = FxHashSet::default();
        for o in 1..self.heap.len() as ObjId {
            let Payload::Func(fi) = &self.heap.obj(o).payload else { continue };
            if !self.is_local_module(fi.module) || !seen.insert(fi.node.addr()) {
                continue;
            }
            let fi = fi.clone();
            let uses = self.code_uses(&fi);
            // (an `__init__` runs on an instance being made: what it does to its receiver's
            // attributes is not done again; what it does through other objects is)
            let init = fi.name == "__init__";
            let own = |a: &StrId| init && uses.recv.as_ref().is_some_and(|r| !r.escapes && (r.loads.contains(a) || r.stores.contains(a) || r.calls.contains(a)));
            for (a, f) in uses.attrs.iter().filter(|(a, _)| !own(a)) {
                by_name.entry(*a).or_default().join(f);
            }
            stored.extend(if init { uses.class_stores.iter() } else { uses.stored.iter() }.copied());
        }
        let u = std::rc::Rc::new((by_name, stored));
        self.effects.program = Some(u.clone());
        u
    }

    /// Does a method of `cls` or its bases (but `__init__`, static and class methods) give its
    /// receiver away?
    fn escapes_in_methods(&mut self, cls: ObjId, state: &State) -> bool {
        for c in self.heap.mro(cls) {
            for (k, v) in state.obj_slots(c, Ns::Attr) {
                if intern::key_as_str(k).as_deref() == Some("__init__") {
                    continue;
                }
                for f in v.pts.iter() {
                    let Payload::Func(fi) = &self.heap.obj(f).payload else { continue };
                    let fi = fi.clone();
                    if !fi.is_static.get() && !fi.is_classmethod.get() && self.code_uses(&fi).recv.as_ref().is_none_or(|r| r.escapes) {
                        return true;
                    }
                }
            }
        }
        false
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
        let mut by_setter: FxHashSet<StrId> = FxHashSet::default();
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
                // (a property with a setter: the setter runs, nothing is stored on the instance)
                if let (true, Some(&setter)) = (u.stores.contains(&n), self.heap.prop_setters.get(&(owner, k))) {
                    work.push(setter);
                    by_setter.insert(n);
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
        stores.retain(|n| !by_setter.contains(n));
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
        for st in f.body.iter().take_while(|st| !crate::syntax::contains_return(std::slice::from_ref(*st)) && !crate::syntax::is_generator(std::slice::from_ref(*st))) {
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
        // (the program followed whole is not the cache's: its returns go to known callers)
        let whole = self.effects.whole.is_some();
        if let Some(c) = self.effects.closures.get(roots).filter(|_| !whole) {
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
                    let name = intern::key_str_id(k);
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
                for l in [&mut out.locs, &mut out.held_locs, &mut out.held2_locs, &mut out.rebound] {
                    l.sort_unstable();
                    l.dedup();
                }
                let c = std::rc::Rc::new(out);
                // (a cache of what the program's code reaches, by root set: on large programs
                // the root sets rarely repeat, so it is emptied past a bound on what it holds)
                if whole {
                    return c;
                }
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
    pub(super) fn skipped_call(&mut self, foid: ObjId, q: &str, self_v: Option<&Value>, a: Args, state: &mut State, node: &Node) -> Value {
        let Args { pos: args, kw: kwargs, star: star_args, star_kw: star_kwargs } = a;
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
            [(_, e)] if fi_is_method(self, foid) => self.must_stores(foid).into_iter().filter(|k| !e.deletes.contains(k) && !self.setter_of(recvs[0], *k)).collect(),
            _ => Vec::new(),
        };
        self.havoc.guard = effs.iter().map(|(o, e)| (*o, 0, std::rc::Rc::from(e.used.as_slice()))).collect();
        let walked = self.havoc.visited;
        let by_names = !effs.is_empty();
        let (args, kwargs, arg_code) = self.args_as_seen(foid, self_v.is_some(), args, kwargs, state);
        let r = self.unknown_call(q, self_v, Args::new(&args, &kwargs, star_args, star_kwargs), state, node);
        let site = s(&format!("{}@L{}", q, node.line));
        self.names.skipped_sites.insert(site);
        let mut roots = vec![foid];
        roots.append(&mut self.havoc.code);
        roots.extend(arg_code);
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
        // (a method returning its receiver gives it back: a call chained on the result runs on it)
        match self_v.filter(|_| self.code_uses(&self.heap.func_info(foid)).recv.as_ref().is_some_and(|u| u.returned)) {
            Some(v) => r.join(v),
            None => r,
        }
    }

    /// One call of several of the program's functions (with their receivers, if bound) none of
    /// which runs (on demand, past the work its run may do): an unknown call given the arguments
    /// and every receiver whole, that may also change what all their code reaches. It covers
    /// each one's `skipped_call` (which leaves out what a function only reads, and walks a
    /// receiver by the names its code uses); a method returning its receiver gives it back.
    pub(super) fn skipped_calls(&mut self, fns: &[(ObjId, Option<Value>)], a: Args, state: &mut State, node: &Node, call_ast: Option<&'static ast::ExprCall>) -> Value {
        for (f, r) in fns {
            self.note_call(*f, r.as_ref(), state);
        }
        self.gens.force_at = None;
        let recv = crate::domain::join_values(fns.iter().filter_map(|(_, r)| r.clone()).collect::<Vec<_>>());
        let text = call_ast.and_then(|c| crate::syntax::chain_text(&c.func)).unwrap_or_else(|| self.heap.func_info(fns[0].0).qualname.clone());
        let r = self.unknown_call(&format!("call of {}", text), (!recv.pts.is_empty()).then_some(&recv), a, state, node);
        let site = s(&format!("call of {}@L{}", text, node.line));
        self.names.skipped_sites.insert(site);
        let mut roots: Vec<ObjId> = fns.iter().map(|(f, _)| *f).collect();
        roots.append(&mut self.havoc.code);
        let rid = r.single().unwrap_or(0);
        self.follow_code(roots, rid, site, state);
        self.flag_may_raise(state);
        let back: Vec<Value> = fns.iter().filter(|(f, r)| r.is_some() && self.code_uses(&self.heap.func_info(*f)).recv.as_ref().is_some_and(|u| u.returned)).filter_map(|(_, r)| r.clone()).collect();
        crate::domain::join_values(std::iter::once(r).chain(back).collect::<Vec<_>>())
    }

    /// The arguments of a call of `foid` that does not run, as what its code may change of
    /// them: one its parameter only reads is left out, one whose parts only it may change gives
    /// its parts (a generator's or coroutine's code runs later: all of them); and the code
    /// those left out hold (functions, instances' classes: what reading them may run).
    #[allow(clippy::type_complexity)]
    fn args_as_seen(&mut self, foid: ObjId, bound: bool, args: &[Value], kwargs: &IndexMap<StrId, Value>, state: &State) -> (Vec<Value>, IndexMap<StrId, Value>, Vec<ObjId>) {
        let pos: Vec<Flags> = (0..args.len()).map(|i| self.param_flags(foid, Slot::Pos(i + usize::from(bound)))).collect();
        let kw: Vec<Flags> = kwargs.keys().map(|k| self.param_flags(foid, Slot::Kw(*k))).collect();
        let mut code: Vec<ObjId> = Vec::new();
        let mut seen = |an: &Analyzer, f: Flags, v: &Value| -> Value {
            let (own, held) = an.flag_effect(&f, v, state);
            let (own, held) = (own || f.edits, held || f.held2);
            // (a container iterated: what its items run when compared)
            if !own && f.iterated {
                v.pts.iter().for_each(|o| state.slots.each(&o, &mut |_, x| code.extend(x.pts.iter().filter_map(|i| super::unknown::code_of(&an.heap, i)))));
            }
            if !own {
                code.extend(v.pts.iter().filter_map(|o| match (an.heap.kind(o), &an.heap.obj(o).payload) {
                    (Kind::Function | Kind::Class | Kind::Bound | Kind::Partial, _) => Some(o),
                    (Kind::Instance, Payload::Cls(c)) => Some(*c),
                    _ => None,
                }));
            }
            match (own, held) {
                (true, _) => v.clone(),
                (false, true) => {
                    let mut parts = Vec::new();
                    v.pts.iter().for_each(|o| an.obj_edges(o, state, &mut parts));
                    Value::objs(parts.into_iter().collect())
                }
                _ => Value::new(),
            }
        };
        let args = args.iter().zip(pos).map(|(v, f)| seen(self, f, v)).collect();
        let kwargs = kwargs.iter().zip(kw).map(|((k, v), f)| (*k, seen(self, f, v))).collect();
        (args, kwargs, code)
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
        for (n, a) in &uses.class_stores_in {
            if let Ok(loc) = self.resolve_use(fi, &uses, *n) {
                let k = key_str_lit(&str_get(*a));
                let mut cs = Vec::new();
                self.classes_within(&state.slot(loc.0, loc.1, loc.2), state, 2, &mut cs);
                out.rebound.extend(cs.into_iter().map(|c| (c, Ns::Attr, k)));
            }
        }
        for (c, a) in &uses.class_stores_result {
            let mut cs = Vec::new();
            if self.result_classes(fi, &uses, *c, state, 2, &mut cs) {
                class_stores.insert(*a);
            } else {
                let k = key_str_lit(&str_get(*a));
                out.rebound.extend(cs.into_iter().map(|c| (c, Ns::Attr, k)));
            }
        }
        // (a default is the function's own state: it changes where its parameter does)
        for (p, v) in fi.defaults.iter() {
            code(self, v, work);
            if let Some(f) = uses.params.get(p) {
                let (own, held) = self.flag_effect(f, v, state);
                if own {
                    out.values.push(v.clone());
                } else if held || f.held2 {
                    out.held_values.push(v.clone());
                }
                match self.edited(f, v, state) {
                    (true, _) => out.edited_values.push(v.clone()),
                    (false, true) => out.iterated_values.push(v.clone()),
                    _ => {}
                }
            }
        }
        // (what the program's functions it calls by name do with the names' values it gives them,
        // and its callers with what it returns)
        let mut more: FxHashMap<Callee, Flags> = FxHashMap::default();
        let mut add = |o: &Origin, f: Flags| match *o {
            Origin::Name(n) => more.entry((n, None)).or_default().join(&f),
            Origin::NameAttr(n, a) => more.entry((n, Some(a))).or_default().join(&f),
            _ => {}
        };
        for p in &uses.passes {
            let f = self.passed_flags(fi, &uses, p, state);
            p.srcs.iter().for_each(|(o, l)| add(o, f.lift(*l)));
        }
        let ret = self.return_flags(fi);
        uses.returns.iter().for_each(|(o, l)| add(o, ret.lift(*l)));
        for u in &uses.uses {
            // (`m.x` of a module m: its global x; of anything else: what m holds, and a class
            // attribute x — a method call on it may change it, a store changes it)
            let mut f = u.flags;
            if let Some(m) = more.get(&(u.name, u.attr)) {
                f.join(m);
            }
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
                // (a method of library layers that changes nothing they hold)
                if f.called && crate::models::torch::t_keeping_method(&str_get(a)) && self.inert_layers(&state.slot(loc.0, loc.1, loc.2), state) {
                    f.called = false;
                }
                let by_name = Flags { rebinds: false, called: false, ..f };
                attrs.entry(a).or_default().join(&by_name);
                // (`K.x = …` of a class K: K's attribute rebound)
                if f.rebinds {
                    let k = key_str_lit(&str_get(a));
                    out.rebound.extend(state.slot(loc.0, loc.1, loc.2).pts.iter().filter(|c| self.heap.kind(*c) == Kind::Class).map(|c| (c, Ns::Attr, k)));
                }
                // (`n.append(…)`: n changes in place as a container, if it is one)
                let edit = f.called && super::flows::CONTAINER_MUTATORS.contains(&&*str_get(a));
                f = Flags { changes: f.called && !edit || f.rebinds, edits: edit, held: f.changes || f.methods || f.held || f.held2 || f.iterated || f.edits, ..Flags::default() };
            }
            self.apply_flags(&f, loc, state, out);
            code(self, &state.slot(loc.0, loc.1, loc.2), work);
        }
    }

    /// The classes a value is or its containers hold (`depth` levels of items).
    fn classes_within(&self, v: &Value, state: &State, depth: usize, out: &mut Vec<ObjId>) {
        for o in v.pts.iter() {
            match self.heap.kind(o) {
                Kind::Class => out.push(o),
                Kind::Dict | Kind::List | Kind::Tuple | Kind::Set if depth > 0 => {
                    for ns in [Ns::Item, Ns::Pos, Ns::Elem] {
                        self.classes_within(&state.all_slots(o, ns), state, depth - 1, out);
                    }
                }
                _ => {}
            }
        }
    }

    /// The program's functions a call by name in the code of `fi` calls (`callee`): those the name
    /// holds; with `all`, None unless it holds nothing else.
    fn program_callees(&self, fi: &FuncInfo, uses: &CodeUses, callee: Callee, state: &State, all: bool) -> Option<Vec<ObjId>> {
        let v = self.callee_value(fi, uses, callee, state)?;
        let fs: Vec<ObjId> = v.pts.iter().filter(|o| matches!(self.heap.obj(*o).payload, Payload::Func(_)) && self.heap.kind(*o) == Kind::Function).collect();
        let whole = fs.len() == v.pts.len() && matches!(v.kv, crate::domain::KeyVal::Bot);
        (!fs.is_empty() && (whole || !all)).then_some(fs)
    }

    /// What a call by name in the code of `fi` calls (`callee`): the value of the name (of a
    /// module's attribute); None for a module or a name not resolved.
    fn callee_value(&self, fi: &FuncInfo, uses: &CodeUses, callee: Callee, state: &State) -> Option<Value> {
        let loc = match (self.resolve_use(fi, uses, callee.0), callee.1) {
            (Ok(l), None) => l,
            (Ok(l), Some(a)) => {
                let m = state.slot(l.0, l.1, l.2).single().filter(|m| self.heap.kind(*m) == Kind::Module)?;
                (m, Ns::Attr, key_str_lit(&str_get(a)))
            }
            (Err(m), Some(a)) => (m, Ns::Attr, key_str_lit(&str_get(a))),
            (Err(_), None) => return None,
        };
        Some(state.slot(loc.0, loc.1, loc.2))
    }

    /// The classes the program holds that a call by name in the code of `fi` may give back
    /// (`out`, following calls `depth` deep), or true: any class (a library's or unknown
    /// code's result, a function giving back a parameter or an attribute). A class of the
    /// program makes an instance (unless a base or `__new__` of its own may make it otherwise);
    /// a class a function makes is a new one, none the program holds.
    fn result_classes(&mut self, fi: &FuncInfo, uses: &CodeUses, callee: Callee, state: &State, depth: usize, out: &mut Vec<ObjId>) -> bool {
        let v = self.callee_value(fi, uses, callee, state).unwrap_or_default();
        // (`object.__new__(cls)`: an instance)
        if v.empty() && &*str_get(callee.0) == "object" && callee.1.is_some_and(|a| &*str_get(a) == "__new__") {
            return false;
        }
        if v.pts.is_empty() || !v.kv.is_bot() {
            return true;
        }
        for o in v.pts.iter() {
            let code = match (self.heap.kind(o), &self.heap.obj(o).payload) {
                // (an instance, or what a `__new__` of its own gives back)
                (Kind::Class, _) if self.heap.class_info_opt(o).is_some_and(|ci| !ci.open_base) => match self.class_lookup(o, "__new__") {
                    None => continue,
                    Some(c) => match self.heap.cls_slots.get(&(c, Ns::Attr, key_str_lit("__new__"))).and_then(|f| f.single()) {
                        Some(f) if self.heap.kind(f) == Kind::Function => f,
                        _ => return true,
                    },
                },
                (Kind::Function, Payload::Func(_)) => o,
                _ => return true,
            };
            if self.returned_classes(code, state, depth, out) {
                return true;
            }
        }
        false
    }

    /// The classes the program holds that function `f` may give back (`result_classes`).
    fn returned_classes(&mut self, f: ObjId, state: &State, depth: usize, out: &mut Vec<ObjId>) -> bool {
        let gi = self.heap.func_info(f);
        let gu = self.code_uses(&gi);
        if gu.returns_other {
            return true;
        }
        for (src, l) in gu.returns.iter() {
            match src {
                Origin::Name(n) => {
                    let v = match self.resolve_use(&gi, &gu, *n) {
                        Ok(loc) => state.slot(loc.0, loc.1, loc.2),
                        Err(m) => Value::obj(m),
                    };
                    // (what an object other than a container holds: a module's, an instance's
                    // attribute by a computed name)
                    if *l == Level::Held && v.pts.iter().any(|o| !matches!(self.heap.kind(o), Kind::Dict | Kind::List | Kind::Tuple | Kind::Set)) {
                        return true;
                    }
                    self.classes_within(&v, state, if *l == Level::Held { 2 } else { 0 }, out);
                }
                Origin::Result(n, a) if depth > 0 => {
                    if self.result_classes(&gi, &gu, (*n, *a), state, depth - 1, out) {
                        return true;
                    }
                }
                _ => return true,
            }
        }
        false
    }

    /// What the code of function `f` may do with the argument in `slot` (given away when it is
    /// a generator's or a coroutine's, whose code runs later; a part of `*args` / `**kw` changes
    /// where they or their parts may).
    fn param_flags(&mut self, f: ObjId, slot: Slot) -> Flags {
        let fi = self.heap.func_info(f);
        let Some(ps) = fi.node.params() else { return Flags::CHANGE };
        if let super::FuncNode::Def(d) = fi.node {
            if d.is_async || crate::syntax::is_generator(&d.body) {
                return Flags::CHANGE;
            }
        }
        let uses = self.code_uses(&fi);
        let flags = |p: &ast::Parameter| uses.params.get(&s(p.name.id.as_str())).copied().unwrap_or_default();
        let rest = |p: Option<&ast::Parameter>| p.map(flags).map(|f| Flags { changes: f.changes || f.methods || f.held, ..Flags::default() }).unwrap_or_default();
        match slot {
            Slot::Pos(i) => match ps.posonlyargs.iter().chain(ps.args.iter()).nth(i) {
                Some(p) => flags(&p.parameter),
                None => rest(ps.vararg.as_deref()),
            },
            Slot::Kw(k) => match ps.args.iter().chain(ps.kwonlyargs.iter()).find(|p| s(p.parameter.name.id.as_str()) == k) {
                Some(p) => flags(&p.parameter),
                None => rest(ps.kwarg.as_deref()),
            },
        }
    }

    /// What may happen to a value the code of `fi` gives to a call by name: what the callee does
    /// with that parameter, when the name holds only the program's functions; else anything.
    fn passed_flags(&mut self, fi: &FuncInfo, uses: &CodeUses, p: &Pass, state: &State) -> Flags {
        let Some(fs) = self.program_callees(fi, uses, p.callee, state, true) else {
            // (a library function that only reads it: used, as an iteration uses it)
            return if self.library_reader(fi, uses, p, state) { Flags { iterated: true, ..Flags::default() } } else { Flags::CHANGE };
        };
        let mut out = Flags::default();
        fs.into_iter().for_each(|f| out.join(&self.param_flags(f, p.slot)));
        out
    }

    /// Whether the call by name that `p` is given to is of a library function that only reads
    /// its arguments (`tables::read_only_library`), given none of the program's code: no
    /// argument may be code but the names' values, and those are none.
    fn library_reader(&self, fi: &FuncInfo, uses: &CodeUses, p: &Pass, state: &State) -> bool {
        let Some(args) = &p.args else { return false };
        let external = |v: &Value| v.single().filter(|o| self.heap.kind(*o) == Kind::External && v.kv.is_bot()).map(|o| self.heap.external_qual(o));
        let qual = match p.callee.1 {
            None => self.callee_value(fi, uses, p.callee, state).and_then(|v| external(&v)),
            Some(a) => match self.resolve_use(fi, uses, p.callee.0) {
                Ok(l) => external(&state.slot(l.0, l.1, l.2)).map(|m| format!("{}.{}", m, str_get(a))),
                Err(_) => None,
            },
        };
        if !qual.is_some_and(|q| crate::models::tables::read_only_library(&q)) {
            return false;
        }
        args.iter().all(|(o, _)| {
            let callee = match *o {
                Origin::Name(n) => (n, None),
                Origin::NameAttr(n, a) => (n, Some(a)),
                _ => return false,
            };
            self.callee_value(fi, uses, callee, state).is_some_and(|v| !v.pts.iter().any(|x| match self.heap.kind(x) {
                Kind::Function | Kind::Bound | Kind::Partial | Kind::Class => true,
                Kind::Instance => self.heap.instance_class(x).is_none_or(|c| self.class_lookup(c, "__call__").is_some()),
                _ => false,
            }))
        })
    }

    /// What may happen to what the code of `fi` returns: what its callers do with it, when the
    /// program is followed whole and they are all its code (`caller_returns`); else anything.
    fn return_flags(&self, fi: &FuncInfo) -> Flags {
        self.effects.whole.as_ref().and_then(|w| w.get(&fi.node.addr()).copied()).unwrap_or(Flags::CHANGE)
    }

    /// For the program followed whole: per module-level function the program calls by name and
    /// never uses as a value (its callers are the program's code; a route handler's, the
    /// framework too, which only serializes the result), what they may do with its
    /// result: where they use it, give it to the program's functions (their parameters'
    /// flags) or return it in turn (their own callers', to a fixpoint). Generators and coroutines
    /// give theirs away (their code runs later).
    pub(in crate::interp) fn caller_returns(&mut self, state: &State) -> FxHashMap<usize, Flags> {
        let mut names = NameUses::default();
        for m in 1..self.heap.len() as ObjId {
            if let Payload::Module(mi) = &self.heap.obj(m).payload {
                names.scan(&mi.tree.body);
            }
        }
        let mut funcs: Vec<std::rc::Rc<FuncInfo>> = Vec::new();
        let (mut seen, mut handlers): (FxHashSet<usize>, FxHashSet<usize>) = Default::default();
        for o in 1..self.heap.len() as ObjId {
            if let Payload::Func(fi) = &self.heap.obj(o).payload {
                if seen.insert(fi.node.addr()) {
                    funcs.push(fi.clone());
                }
                // (a route handler's result goes to the framework, which serializes it)
                if self.m.fa_handlers.contains(&o) {
                    handlers.insert(fi.node.addr());
                }
            }
        }
        for fi in &funcs {
            names.values.extend(self.code_uses(fi).untracked.iter().copied());
        }
        names.close();
        let by_name = |fi: &FuncInfo| match fi.node {
            super::FuncNode::Def(d) => {
                let (n, handler) = (s(&fi.name), handlers.contains(&fi.node.addr()));
                let callers = handler || names.called.contains(&n) && !names.decorated.contains(&n);
                fi.cls.is_none() && fi.qualname == fi.name && !d.is_async && !crate::syntax::is_generator(&d.body) && callers && !names.values.contains(&n)
            }
            super::FuncNode::Lambda(_) => false,
        };
        let mut ret: FxHashMap<usize, Flags> = funcs.iter().filter(|fi| by_name(fi)).map(|fi| (fi.node.addr(), Flags::default())).collect();
        let mut chained: Vec<(usize, Level, usize)> = Vec::new();
        let mut fixed: Vec<(usize, Flags)> = Vec::new();
        for fi in &funcs {
            let uses = self.code_uses(fi);
            let to = |an: &Analyzer, callee: Callee, f: Flags, out: &mut Vec<(usize, Flags)>| {
                for c in an.program_callees(fi, &uses, callee, state, false).unwrap_or_default() {
                    out.push((an.heap.func_info(c).node.addr(), f));
                }
            };
            for (c, f) in &uses.results {
                to(self, *c, *f, &mut fixed);
            }
            for p in &uses.passes {
                let f = self.passed_flags(fi, &uses, p, state);
                for (o, l) in &p.srcs {
                    if let Origin::Result(n, a) = o {
                        to(self, (*n, *a), f.lift(*l), &mut fixed);
                    }
                }
            }
            for (o, l) in &uses.returns {
                if let Origin::Result(n, a) = o {
                    for c in self.program_callees(fi, &uses, (*n, *a), state, false).unwrap_or_default() {
                        chained.push((fi.node.addr(), *l, self.heap.func_info(c).node.addr()));
                    }
                }
            }
        }
        for (f, fl) in fixed {
            if let Some(r) = ret.get_mut(&f) {
                r.join(&fl);
            }
        }
        loop {
            let mut grew = false;
            for &(g, l, f) in &chained {
                let fl = ret.get(&g).copied().unwrap_or(Flags::CHANGE).lift(l);
                if let Some(r) = ret.get_mut(&f) {
                    let before = *r;
                    r.join(&fl);
                    grew |= *r != before;
                }
            }
            if !grew {
                return ret;
            }
        }
    }

    /// What `f` makes of a slot's value: it may change, what it holds may, it may be rebound.
    fn apply_flags(&self, f: &Flags, loc: Loc, state: &State, out: &mut Closure) {
        let v = state.slot(loc.0, loc.1, loc.2);
        let (own, held) = self.flag_effect(f, &v, state);
        if own {
            out.locs.push(loc);
        } else if held {
            out.held_locs.push(loc);
        } else if f.held2 {
            out.held2_locs.push(loc);
        }
        match self.edited(f, &v, state) {
            (true, _) => out.edited_locs.push(loc),
            (false, true) => out.iterated_locs.push(loc),
            _ => {}
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
        // (a library layer called runs its model)
        let code_only = |o: ObjId| code_only(o) || crate::models::torch::t_layer_type(self, o).is_some() && crate::models::torch::t_inert_layer(self, o, _state);
        let plain = v.pts.iter().all(plain);
        let own = f.changes || f.methods || ((f.iterated || f.edits) && !plain) || (f.called && !v.pts.iter().all(code_only));
        (own, f.held || f.iterated && !plain)
    }

    /// Whether `f` changes `v` in place as plain containers only (`Flags::edits`), and whether it
    /// only iterates them.
    fn edited(&self, f: &Flags, v: &Value, state: &State) -> (bool, bool) {
        let own = self.flag_effect(f, v, state).0;
        (f.edits && !own, f.iterated && !own)
    }
}

impl Analyzer {
    /// Is every object of `v` a library layer that calling changes nothing of (and is there one)?
    fn inert_layers(&self, v: &Value, state: &State) -> bool {
        !v.pts.is_empty() && v.pts.iter().all(|o| crate::models::torch::t_layer_type(self, o).is_some() && crate::models::torch::t_inert_layer(self, o, state))
    }

    /// Whether attribute `k` of instance `o` is a property with a setter (a store runs it).
    fn setter_of(&self, o: ObjId, k: KeyId) -> bool {
        let Some(c) = self.heap.instance_class(o) else { return false };
        intern::key_as_str(k).and_then(|n| self.class_lookup(c, &n)).is_some_and(|owner| self.heap.prop_setters.contains_key(&(owner, k)))
    }
}

/// A function run with a receiver (not a static / class method).
fn fi_is_method(an: &Analyzer, f: ObjId) -> bool {
    an.heap.func_info_opt(f).is_some_and(|fi| fi.cls.is_some() && !fi.is_static.get() && !fi.is_classmethod.get())
}
