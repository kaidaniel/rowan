//! What user code may change beyond what it is given, for a call that does not run its body
//! (past its function's budget, or cut at the deadline): the module globals and closure cells
//! its code (and the code it can reach) names in a position where they may change or escape.

use ruff_python_ast::{Expr, Stmt};
use rustc_hash::{FxHashMap, FxHashSet};

use indexmap::IndexMap;

use crate::domain::{join_values, Ns, ObjId, Value};
use crate::frontend::Node;
use ruff_python_ast as ast;
use crate::intern::{self, key_str_lit, s, str_get, KeyId, KeySet, StrId};
use crate::state::{Kind, State};

use super::flows::{Callee, CodeUses, Flags, KeptKey, KeptPlace, Level, Like, NameUses, Origin, Pass, Removes, Slot};
use super::{Analyzer, Args, FuncInfo, FuncNode, Payload};

/// What the cache of code closures (`code_closure`) may hold; past it the cache starts over.
pub const CLOSURE_CACHE: usize = 1 << 21;

/// The dunders library code may run on the objects it is given without changing them itself
/// (`library_dunders`): formatting, comparing, hashing, converting, copying, arithmetic.
const LIBRARY_DUNDERS: &[&str] = &[
    "__str__", "__repr__", "__format__", "__bytes__", "__hash__", "__eq__", "__ne__", "__lt__", "__le__", "__gt__", "__ge__", "__bool__", "__len__",
    "__iter__", "__next__", "__contains__", "__getitem__", "__int__", "__float__", "__complex__", "__index__", "__round__", "__trunc__", "__floor__",
    "__ceil__", "__fspath__", "__getattr__", "__getattribute__", "__copy__", "__deepcopy__", "__reduce__", "__reduce_ex__", "__getstate__", "__add__",
    "__radd__", "__sub__", "__rsub__", "__mul__", "__rmul__", "__truediv__", "__rtruediv__", "__neg__", "__abs__",
];

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
    /// per class: its dunders library code may run whose code may change something
    /// (`changing_dunders`; the code a class attribute holds is taken as fixed, as for closures)
    dunders: FxHashMap<ObjId, std::rc::Rc<[ObjId]>>,
    /// per function code (`FuncNode::addr`): what its body does when it is inert
    /// (`inert::inert`), None when it is not
    inert: FxHashMap<usize, Option<std::rc::Rc<super::inert::Inert>>>,
    /// the followed libraries' code that calls not run share (`libclosure.rs`)
    pub lib: super::libclosure::LibClosure,
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
pub type Loc = (ObjId, Ns, KeyId);

/// A skipped call's arguments as its code may change them (`args_as_seen`), the code reading
/// them may run, and those it keeps: each with the dicts whose items may hold it and whether
/// the call returns it.
struct Seen {
    args: Vec<Value>,
    kwargs: IndexMap<StrId, Value>,
    code: Vec<ObjId>,
    kept: Vec<(Value, Vec<(ObjId, KeptKey)>, bool)>,
}

/// A closure's walk (`walk_closure`): the closure so far, the code still to walk, the names the
/// code walked uses, the classes met (those up to `classes_seen` looked at for the names as
/// `names_seen` says they were used then, found by `by_name`), the class attributes followed,
/// and those changed with the flags applied to them.
#[derive(Default)]
pub struct ClosureWalk {
    out: Closure,
    work: Vec<ObjId>,
    attrs: FxHashMap<StrId, Flags>,
    reads: FxHashSet<StrId>,
    class_stores: FxHashSet<StrId>,
    classes: Vec<ObjId>,
    done: FxHashSet<(ObjId, KeyId)>,
    done_attrs: FxHashMap<(ObjId, KeyId), Flags>,
    classes_seen: usize,
    names_seen: FxHashMap<StrId, (bool, Option<Flags>, bool)>,
    /// per attribute name, the classes met that have it
    by_name: FxHashMap<KeyId, Vec<ObjId>>,
    /// the names whose use the code walked since the classes were last looked at may have grown
    dirty: Vec<StrId>,
    /// past this many visits (code objects, class attributes) the walk stops (0: never), `cut`
    /// then
    cap: usize,
    visits: usize,
    cut: bool,
}

/// How much of each of a walk's lists (`Closure`'s, as pushed) has been taken
/// (`ClosureWalk::gained`).
#[derive(Default, Clone, Copy)]
pub struct Taken([usize; 14]);

impl ClosureWalk {
    /// Code still to walk.
    pub fn push(&mut self, code: impl IntoIterator<Item = ObjId>) {
        self.work.extend(code);
    }

    /// Has the walk met code object `o`?
    pub fn walked(&self, o: ObjId) -> bool {
        self.out.code.contains(&o)
    }

    /// What the walk's closure gained since `at` (moved on to now): each list's new items, in
    /// the order the walk found them (some again).
    pub fn gained(&self, at: &mut Taken) -> Closure {
        fn tail<T: Clone>(v: &[T], at: &mut usize) -> Vec<T> {
            let t = v[(*at).min(v.len())..].to_vec();
            *at = v.len();
            t
        }
        let o = &self.out;
        let a = &mut at.0;
        Closure {
            code: FxHashSet::default(),
            locs: tail(&o.locs, &mut a[0]),
            held_locs: tail(&o.held_locs, &mut a[1]),
            held2_locs: tail(&o.held2_locs, &mut a[2]),
            rebound: tail(&o.rebound, &mut a[3]),
            values: tail(&o.values, &mut a[4]),
            held_values: tail(&o.held_values, &mut a[5]),
            edited_locs: tail(&o.edited_locs, &mut a[6]),
            edited_values: tail(&o.edited_values, &mut a[7]),
            removed_locs: tail(&o.removed_locs, &mut a[8]),
            removed_values: tail(&o.removed_values, &mut a[9]),
            iterated_locs: tail(&o.iterated_locs, &mut a[10]),
            iterated_values: tail(&o.iterated_values, &mut a[11]),
            unbound: tail(&o.unbound, &mut a[12]),
            modules: tail(&o.modules, &mut a[13]),
        }
    }

    /// The closure walked so far, its slots sorted (a slot whose flags grew as the walk went
    /// on is where each of its flags put it: a value that may change keeps what it holds that
    /// may, a walk from it does not reach all of that).
    fn finished(&self) -> Closure {
        let mut c = self.out.clone();
        for l in [&mut c.locs, &mut c.held_locs, &mut c.held2_locs, &mut c.rebound, &mut c.edited_locs, &mut c.iterated_locs, &mut c.unbound] {
            l.sort_unstable();
            l.dedup();
        }
        let mut removed: Vec<(Loc, Removes)> = Vec::new();
        for (l, r) in std::mem::take(&mut c.removed_locs) {
            match removed.iter_mut().find(|(m, _)| *m == l) {
                Some(e) => e.1 = e.1.join(r),
                None => removed.push((l, r)),
            }
        }
        removed.sort_unstable_by_key(|(l, _)| *l);
        c.removed_locs = removed;
        c
    }
}

/// What a piece of code may change beyond what it is given (`code_closure`): the slots whose
/// values may change, those whose values' parts may (`held_locs`: a container iterated, an
/// attribute's value given away), those it may rebind, and other values likewise.
#[derive(Default, Clone)]
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
    /// among those, the ones whose items it may remove (`Flags::removes`)
    pub removed_locs: Vec<(Loc, Removes)>,
    pub removed_values: Vec<(Value, Removes)>,
    pub iterated_locs: Vec<Loc>,
    pub iterated_values: Vec<Value>,
    /// the slots the code reads names from that were not bound when it was walked (a name
    /// defined later in its module or enclosing function): once one is, the closure is stale
    pub unbound: Vec<Loc>,
    /// the modules walked as values (all their globals) and the names they had then
    /// (`module_names`): once one has more, the closure is stale
    pub modules: Vec<(ObjId, u64)>,
}

/// The names a module has on a path, as a fingerprint (their number and their keys).
pub(in crate::interp) fn module_names(state: &State, m: ObjId) -> u64 {
    let mut h: u64 = 0;
    state.slots.each(&m, &mut |k, _| h = (h.rotate_left(5) ^ u64::from(k.1.0)).wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(1));
    h
}

impl Closure {
    /// What this closure has that `prev` (a subset: an earlier round's) has not; `resumed`:
    /// this one's walk went on from `prev`'s, so its values lists start with `prev`'s.
    pub fn since(&self, prev: &Closure, resumed: bool) -> Closure {
        fn locs(a: &[Loc], b: &[Loc]) -> Vec<Loc> {
            a.iter().filter(|l| b.binary_search(l).is_err()).copied().collect()
        }
        fn vals<T: Clone>(a: &[T], b: &[T], resumed: bool) -> Vec<T> {
            a[if resumed { b.len().min(a.len()) } else { 0 }..].to_vec()
        }
        Closure {
            code: FxHashSet::default(),
            locs: locs(&self.locs, &prev.locs),
            held_locs: locs(&self.held_locs, &prev.held_locs),
            held2_locs: locs(&self.held2_locs, &prev.held2_locs),
            rebound: locs(&self.rebound, &prev.rebound),
            values: vals(&self.values, &prev.values, resumed),
            held_values: vals(&self.held_values, &prev.held_values, resumed),
            edited_locs: locs(&self.edited_locs, &prev.edited_locs),
            edited_values: vals(&self.edited_values, &prev.edited_values, resumed),
            removed_locs: self.removed_locs.iter().filter(|r| !prev.removed_locs.contains(r)).cloned().collect(),
            removed_values: vals(&self.removed_values, &prev.removed_values, resumed),
            iterated_locs: locs(&self.iterated_locs, &prev.iterated_locs),
            iterated_values: vals(&self.iterated_values, &prev.iterated_values, resumed),
            unbound: locs(&self.unbound, &prev.unbound),
            modules: self.modules.iter().filter(|m| !prev.modules.contains(m)).copied().collect(),
        }
    }

    /// The slots it names.
    pub fn slot_locs(&self) -> impl Iterator<Item = Loc> + '_ {
        let locs = [&self.locs, &self.held_locs, &self.held2_locs, &self.rebound, &self.edited_locs, &self.iterated_locs, &self.unbound];
        locs.into_iter().flat_map(|l| l.iter().copied()).chain(self.removed_locs.iter().map(|(l, _)| *l))
    }

    /// The objects its values are.
    pub fn value_objects(&self) -> impl Iterator<Item = ObjId> + '_ {
        let values = [&self.values, &self.held_values, &self.edited_values, &self.iterated_values];
        values.into_iter().flat_map(|v| v.iter().flat_map(|v| v.pts.iter())).chain(self.removed_values.iter().flat_map(|(v, _)| v.pts.iter()))
    }
}

impl Analyzer {
    /// What the function's body does when it is inert (`inert::inert`: a
    /// bounded handful of steps that make functions and store attributes): its own steps cost
    /// next to nothing, so a call of it runs even past an entry's allowance or the work of a
    /// run on demand (within its budget); what it calls follows the rules of every call.
    pub(in crate::interp) fn inert(&mut self, foid: ObjId) -> Option<std::rc::Rc<super::inert::Inert>> {
        let fi = self.heap.func_info_opt(foid)?;
        let FuncNode::Def(f) = fi.node else { return None };
        self.effects.inert.entry(fi.node.addr()).or_insert_with(|| super::inert::inert(&f.parameters, &f.body).map(std::rc::Rc::new)).clone()
    }

    /// Is `o` code a closure walk follows (`walk_closure`): a function, class, bound method,
    /// instance (its class), partial, module, deferred definition?
    fn holds_code(&self, o: ObjId) -> bool {
        matches!(self.heap.kind(o), Kind::Function | Kind::Class | Kind::Bound | Kind::Instance | Kind::Partial | Kind::Module) || matches!(self.heap.obj(o).payload, Payload::Deferred(_))
    }

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
        let (mut walk, mut edited, mut removed): (Vec<Value>, Vec<ObjId>, Vec<(ObjId, Removes)>) = Default::default();
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
                    if f.removes.any() {
                        removed.extend(v.pts.iter().map(|o| (o, f.removes)));
                    }
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
        super::unknown::may_have_removed(&self.heap, &removed, state);
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
        if mro.iter().any(|c| self.heap.class_info_opt(*c).is_none_or(|ci| ci.open_base.get())) || ["__getattr__", "__getattribute__", "__setattr__", "__delattr__"].iter().any(|h| self.class_lookup(cls, h).is_some()) {
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

    /// The attribute names method `foid`'s code stores on its receiver `recv` on every way it
    /// completes (`MustWalk`), those of the method `super().m(…)` calls in it too.
    pub(in crate::interp) fn must_stores(&self, foid: ObjId, recv: ObjId, depth: usize) -> KeySet {
        let Some(fi) = self.heap.func_info_opt(foid) else { return KeySet::new() };
        let (super::FuncNode::Def(f), Some(me)) = (fi.node, fi.node.params().and_then(|p| p.posonlyargs.iter().chain(p.args.iter()).next())) else { return KeySet::new() };
        if depth > 4 || crate::syntax::is_generator(&f.body) {
            return KeySet::new();
        }
        // (`super().m(…)`: the next class after `foid`'s along the receiver's MRO defining m)
        let sup = |name: &str| -> KeySet {
            let (Some(cls), Some(rc)) = (fi.cls, self.heap.instance_class(recv)) else { return KeySet::new() };
            let k = key_str_lit(name);
            let mro = self.heap.mro(rc);
            let Some(at) = mro.iter().position(|c| *c == cls) else { return KeySet::new() };
            // (a base rowan does not see may be the one called)
            if mro.iter().any(|c| self.heap.class_info_opt(*c).is_none_or(|ci| ci.open_base.get())) {
                return KeySet::new();
            }
            for c in &mro[at + 1..] {
                if let Some(v) = self.heap.cls_slots.get(&(*c, Ns::Attr, k)) {
                    return match v.single().filter(|g| fi_is_method(self, *g) && v.kv.is_bot()) {
                        Some(g) => self.must_stores(g, recv, depth + 1),
                        None => KeySet::new(),
                    };
                }
            }
            KeySet::new()
        };
        let cls = fi.qualname.rsplit('.').nth(1).unwrap_or("");
        let mut w = MustWalk { me: me.parameter.name.id.as_str(), cls, ret: None, supers: Vec::new(), sup: &sup, rebound: false };
        let fall = w.block(&f.body, KeySet::new());
        if w.rebound {
            return KeySet::new();
        }
        match (fall, w.ret) {
            (Some(a), Some(b)) => a.intersection(&b),
            (Some(a), None) | (None, Some(a)) => a,
            (None, None) => KeySet::new(),
        }
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
    /// name holds is taken as fixed (rebinding a function or class is monkey-patching; a
    /// builtin's name holds the builtin), until a name it reads that was not bound then is
    /// (`Closure::unbound`: a `def` later in the module); methods called by a computed name are not followed; the code in the data those values
    /// hold is followed where the call is (`skipped_call`). `from`: the walk of a subset of the
    /// roots, resumed (and left there), unless the closure is cached.
    pub(in crate::interp) fn code_closure(&mut self, roots: &[ObjId], from: &mut Option<ClosureWalk>, state: &State) -> std::rc::Rc<Closure> {
        // (the program followed whole is not the cache's: its returns go to known callers)
        let whole = self.effects.whole.is_some();
        if let Some(c) = self.cached_closure(roots, state).filter(|_| !whole) {
            return c;
        }
        // (the walk of a subset of the roots resumed: `follow_code`'s rounds, each with the
        // code the last one's data held)
        let mut w = from.take().unwrap_or_default();
        w.work.extend_from_slice(roots);
        self.walk_closure(&mut w, state);
        let c = std::rc::Rc::new(w.finished());
        *from = Some(w);
        if !whole {
            self.cache_closure(roots, c.clone());
        }
        c
    }

    /// The closure of `roots` cached, unless a name its code reads was bound since (it is
    /// computed again).
    fn cached_closure(&self, roots: &[ObjId], state: &State) -> Option<std::rc::Rc<Closure>> {
        let c = self.effects.closures.get(roots)?;
        (!c.unbound.iter().any(|l| state.has_slot(l.0, l.1, l.2)) && c.modules.iter().all(|&(m, h)| module_names(state, m) == h)).then(|| c.clone())
    }

    /// A cache of what the program's code reaches, by root set: on large programs the root sets
    /// rarely repeat, so it is emptied past a bound on what it holds.
    fn cache_closure(&mut self, roots: &[ObjId], c: std::rc::Rc<Closure>) {
        let size = roots.len() + c.code.len() + c.locs.len() + c.held_locs.len() + c.rebound.len() + c.values.len() + c.held_values.len();
        self.effects.closures_size += size;
        if self.effects.closures_size > CLOSURE_CACHE {
            self.effects.closures.clear();
            self.effects.closures_size = size;
        }
        self.effects.closures.insert(roots.to_vec(), c);
    }

    /// Is the closure of the code `roots` hold at most `cap` walk visits? (It is then cached,
    /// with `cache`; past that, the walk stops there.)
    pub(in crate::interp) fn closure_within(&mut self, roots: &[ObjId], cap: usize, cache: bool, state: &State) -> bool {
        if let Some(c) = self.cached_closure(roots, state) {
            return c.code.len() + c.slot_locs().count() <= cap;
        }
        let mut w = ClosureWalk { cap, ..Default::default() };
        w.work.extend_from_slice(roots);
        self.walk_closure(&mut w, state);
        if w.cut {
            return false;
        }
        if cache {
            self.cache_closure(roots, std::rc::Rc::new(w.finished()));
        }
        true
    }

    /// Is the closure of `roots` cached (and not stale)?
    pub(in crate::interp) fn closure_cached(&self, roots: &[ObjId], state: &State) -> bool {
        self.effects.whole.is_none() && self.cached_closure(roots, state).is_some()
    }

    /// Resume walk `w` with the code it was given since (`ClosureWalk::push`).
    pub(in crate::interp) fn walk_more(&mut self, w: &mut ClosureWalk, state: &State) {
        self.walk_closure(w, state);
    }

    /// Walk the code `w.work` holds into `w`'s closure (resumable: code already in it is not
    /// walked again; a class already met is looked at again only for the names whose use grew
    /// since: the closure is the same whatever order the code is met in).
    fn walk_closure(&mut self, w: &mut ClosureWalk, state: &State) {
        let code = |an: &Analyzer, v: &Value, work: &mut Vec<ObjId>| work.extend(v.pts.iter().filter(|x| an.holds_code(*x)));
        let ClosureWalk { out, work, attrs, reads, class_stores, classes, done, done_attrs, classes_seen, names_seen, by_name, dirty, cap, visits, cut } = w;
        loop {
            while let Some(o) = work.pop() {
                if !out.code.insert(o) {
                    continue;
                }
                if self.closure_visit(visits, *cap) {
                    *cut = true;
                    return;
                }
                match (self.heap.kind(o), &self.heap.obj(o).payload) {
                    (Kind::Function, Payload::Func(fi)) => {
                        let fi = fi.clone();
                        self.function_closure(&fi, state, out, attrs, reads, class_stores, work);
                        // (the names whose use it may have grown)
                        let u = self.code_uses(&fi);
                        dirty.extend(u.attrs.keys().chain(u.reads.iter()).chain(u.class_stores.iter()).copied());
                        dirty.extend(u.class_stores_result.iter().map(|(_, a)| *a).chain(u.uses.iter().filter_map(|x| x.attr)));
                    }
                    (Kind::Module, _) => {
                        // (a module used as a value: any of its globals, those it has now)
                        out.modules.push((o, module_names(state, o)));
                        for (k, v) in state.obj_slots(o, Ns::Attr) {
                            out.locs.push((o, Ns::Attr, k));
                            code(self, &v, work);
                        }
                    }
                    (Kind::Class, _) => {
                        classes.push(o);
                        if let Some(ci) = self.heap.class_info_opt(o) {
                            work.extend(ci.bases.iter().copied());
                        }
                    }
                    (Kind::Instance, Payload::Cls(c)) => work.push(*c),
                    // (a deferred definition: its decorators' code, the function's)
                    (Kind::Unknown, Payload::Deferred(_)) => work.extend(super::deferred::deferred_code(&self.heap, o).unwrap_or_default()),
                    (Kind::Bound, Payload::Bound(f, recv)) => {
                        work.push(*f);
                        out.values.push(recv.clone());
                    }
                    (Kind::Partial, Payload::Partial(f, args, kw)) => {
                        code(self, f, work);
                        out.values.extend(args.iter().chain(kw.values()).cloned());
                    }
                    _ => {}
                }
            }
            // the methods of the classes reached that the code reads (dunders run implicitly),
            // and their attributes it may change or rebind: of a class met before, by the names
            // whose use grew since (newly read, changed in more ways, or stored)
            let mut fresh_keys: Vec<KeyId> = Vec::new();
            dirty.sort_unstable();
            dirty.dedup();
            for n in dirty.drain(..) {
                let now = (reads.contains(&n), attrs.get(&n).copied(), class_stores.contains(&n));
                if names_seen.insert(n, now) != Some(now) {
                    fresh_keys.push(key_str_lit(&str_get(n)));
                }
            }
            fresh_keys.sort_unstable();
            fresh_keys.dedup();
            // (a class met before: its attributes under those names, by the index of the names
            // the classes have; a class met now: all of them, indexed)
            let mut slots: Vec<(ObjId, KeyId, Value)> = Vec::new();
            for &k in &fresh_keys {
                for &c in by_name.get(&k).map(|v| v.as_slice()).unwrap_or_default() {
                    slots.extend(state.slot_opt(c, Ns::Attr, k).map(|v| (c, k, v)));
                }
            }
            for &c in &classes[*classes_seen..] {
                for (k, v) in state.obj_slots(c, Ns::Attr) {
                    by_name.entry(k).or_default().push(c);
                    slots.push((c, k, v));
                }
            }
            for (c, k, v) in slots {
                self.closure_visit(visits, 0);
                let name = intern::key_str_id(k);
                if (name.is_some_and(|n| reads.contains(&n) || str_get(n).starts_with("__"))) && done.insert((c, k)) {
                    code(self, &v, work);
                }
                let (flags, rebinds) = name.map(|n| (attrs.get(&n).copied(), class_stores.contains(&n))).unwrap_or_default();
                if flags.is_some() || rebinds {
                    let mut f = flags.unwrap_or_default();
                    f.rebinds = rebinds;
                    // (again when its flags grew: the walk met more code changing it)
                    if done_attrs.insert((c, k), f) != Some(f) {
                        self.apply_flags(&f, (c, Ns::Attr, k), state, out);
                    }
                }
            }
            *classes_seen = classes.len();
            if work.is_empty() {
                return;
            }
        }
    }

    /// One more visit of a closure walk: past `cap` (0: none) of them, the walk stops.
    fn closure_visit(&self, visits: &mut usize, cap: usize) -> bool {
        self.budget.walk_visit();
        *visits += 1;
        cap != 0 && *visits > cap
    }

    /// A call of user code whose body does not run (past its budget, cut at the deadline): an
    /// unknown call that may also change what the code reaches beyond its arguments (the globals
    /// and cells it, and the code it can call, reads where they may change).
    pub(super) fn skipped_call(&mut self, foid: ObjId, q: &str, self_v: Option<&Value>, a: Args, state: &mut State, node: &Node) -> Value {
        let Args { pos: args, kw: kwargs, star: star_args, star_kw: star_kwargs } = a;
        self.budget.skipped += 1;
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
        let returned = self.returned_args(foid, self_v, &Args::new(args, kwargs, star_args, star_kwargs), state, true);
        // (a getter: the attribute it returns, read here; it changes nothing, it may raise)
        if let Some(v) = self.getter_result(foid, self_v, &positional, &Args::new(args, kwargs, star_args, star_kwargs), node, state) {
            self.flag_may_raise(state);
            return v;
        }
        let effs: Vec<(ObjId, RecvEffects)> = recvs.iter().map(|&o| Some((o, self.receiver_effects(foid, o, state, &bind)?))).collect::<Option<_>>().unwrap_or_default();
        let must = self.must_after(foid, &recvs, &effs);
        self.havoc.guard = effs.iter().map(|(o, e)| (*o, 0, std::rc::Rc::from(e.used.as_slice()))).collect();
        let (walked, walks) = (self.havoc.visited, self.budget.walked.get());
        let by_names = !effs.is_empty();
        let removed = self.removed_args(foid, self_v.is_some(), args, kwargs);
        let Seen { args, kwargs, code: arg_code, kept } = self.args_as_seen(foid, self_v, args, kwargs, state);
        // (the classes it meets are followed below, their dunders among their code)
        self.havoc.classes_followed = true;
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
        super::unknown::may_have_removed(&self.heap, &removed, state);
        let r = self.put_kept(kept, r, node, state);
        let r = self.summarized(foid, r, node, state);
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
            eprintln!("skip {} at M{}:L{} havoc={} walk={}{}", q, node.module, node.line, self.havoc.visited - walked, self.budget.walked.get() - walks, if by_names { " (receiver by names)" } else { "" });
        }
        // (a raise after the code's effects on what it reaches beyond its arguments)
        self.flag_may_raise(state);
        // (what its own code stores on every way it completes is there after the call: the
        // raises above may come before)
        if let ([o], false) = (recvs.as_slice(), must.is_empty()) {
            let o = *o;
            let any = Value::obj(crate::interp::any_object(&mut self.heap));
            let mut row = state.row(o, Ns::Attr);
            for &k in &must {
                state.set_slot(o, Ns::Attr, k, any.clone(), true);
                row = row.store(k, true);
            }
            if !by_names {
                row = row.mutate_like(site, &|k| must.contains(&k));
            }
            state.set_row(o, Ns::Attr, row);
        }
        let r = r.join(&returned);
        // (a method returning its receiver gives it back: a call chained on the result runs on it)
        match self_v.filter(|_| self.code_uses(&self.heap.func_info(foid)).recv.as_ref().is_some_and(|u| u.returned)) {
            Some(v) => r.join(v),
            None => r,
        }
    }

    /// What a skipped call's result may be of what its code is given and reads: the arguments
    /// given for the parameters its code returns themselves (`return xs`, `return a or b`,
    /// `kept_returned`, `returns_params`; a parameter not given by name or position: anything
    /// given), and what else its `return`s give (`returned_aliases`).
    fn returned_args(&mut self, foid: ObjId, self_v: Option<&Value>, a: &Args, state: &State, param_held: bool) -> Value {
        let fi = self.heap.func_info(foid);
        let uses = self.code_uses(&fi);
        // (most code gives back nothing it is given or reads: nothing to look up)
        let aliasing = |(o, l): &(Origin, Level)| *l != Level::Copy && matches!(o, Origin::Param(_) | Origin::Name(_) | Origin::NameAttr(..) | Origin::Result(..));
        if uses.kept_returned.is_empty() && uses.returns_params.is_empty() && !uses.returns.iter().any(aliasing) {
            return Value::new();
        }
        let params = || fi.node.params().into_iter().flat_map(|p| p.posonlyargs.iter().chain(p.args.iter()));
        let recv_name = self_v.and_then(|_| params().next()).map(|a| s(a.parameter.name.id.as_str()));
        let given = |p: StrId| -> Value {
            if let (Some(r), Some(v)) = (recv_name, self_v) {
                if r == p {
                    return v.clone();
                }
            }
            let at = params().skip(usize::from(self_v.is_some())).position(|x| s(x.parameter.name.id.as_str()) == p);
            match (at.and_then(|i| a.pos.get(i)), a.kw.get(&p)) {
                (Some(v), _) | (None, Some(v)) => v.clone(),
                _ => a.pos.iter().chain(a.kw.values()).chain(a.star).chain(a.star_kw).fold(Value::new(), |acc, v| acc.join(v)),
            }
        };
        let own = uses.kept_returned.iter().chain(uses.returns_params.iter()).fold(Value::new(), |acc, p| acc.join(&given(*p)));
        let mut seen: Vec<ObjId> = Vec::new();
        let mut out = own;
        self.returned_aliases(foid, Some(&given), param_held, state, 1, &mut seen, &mut out);
        out
    }

    /// What the `return`s of `foid`'s code give back (`CodeUses::returns`) of what it is given
    /// (`given`: None, not known; what an argument holds where `param_held`) and reads: the globals and module attributes themselves, their
    /// parts under a literal key, of what they hold the program's functions and classes (a
    /// registry's: `return REG.get(name, fn)`), and the same of the program's functions it calls
    /// by name (`depth` deep: one). What else they hold is taken as for unknown code (no instance of
    /// the program's: known to be hard).
    fn returned_aliases(&mut self, foid: ObjId, given: Option<&dyn Fn(StrId) -> Value>, param_held: bool, state: &State, depth: usize, seen: &mut Vec<ObjId>, out: &mut Value) {
        if seen.contains(&foid) || !matches!(self.heap.obj(foid).payload, Payload::Func(_)) {
            return;
        }
        seen.push(foid);
        let fi = self.heap.func_info(foid);
        let uses = self.code_uses(&fi);
        for &(o, l) in uses.returns.iter() {
            let v = match o {
                // (what an argument holds: not where unknown code is given the arguments whole —
                // it may change the functions they hold, `skipped_calls`)
                Origin::Param(_) if l == Level::Held && !param_held => continue,
                Origin::Param(p) => match given {
                    Some(g) => g(p),
                    None => continue,
                },
                Origin::Name(n) => self.callee_value(&fi, &uses, (n, None), state).unwrap_or_default(),
                // (of a value that is no module: its attribute)
                Origin::NameAttr(n, a) => match self.callee_value(&fi, &uses, (n, Some(a)), state) {
                    Some(v) => v,
                    None => {
                        let base = self.callee_value(&fi, &uses, (n, None), state).unwrap_or_default();
                        join_values(base.pts.iter().map(|b| state.slot(b, Ns::Attr, key_str_lit(&str_get(a)))).collect::<Vec<_>>())
                    }
                },
                Origin::Result(_, _) if depth == 0 => continue,
                Origin::Result(n, m) => {
                    for f in self.program_callees(&fi, &uses, (n, m), state, false).unwrap_or_default() {
                        self.returned_aliases(f, None, param_held, state, depth - 1, seen, out);
                    }
                    continue;
                }
                _ => continue,
            };
            let part = match l {
                Level::Own => Value::objs(v.pts.clone()),
                Level::Part(k) => join_values(v.pts.iter().map(|x| state.slot(x, Ns::Item, k)).collect::<Vec<_>>()),
                Level::Held => self.code_held(&v, state),
                Level::Copy => continue,
            };
            *out = out.join(&Value::objs(part.pts));
        }
    }

    /// The program's functions and classes `v` holds: its items' and attributes' values, and
    /// theirs (a registry's, an instance's dict of handlers), at most 64 objects looked at.
    fn code_held(&self, v: &Value, state: &State) -> Value {
        let mut found: Vec<ObjId> = Vec::new();
        let mut level: Vec<ObjId> = v.pts.iter().collect();
        let mut looked = 0;
        for _ in 0..2 {
            let mut next: Vec<ObjId> = Vec::new();
            for o in level {
                if matches!(self.heap.kind(o), Kind::Function | Kind::Class | Kind::Module | Kind::External | Kind::Unknown) {
                    continue;
                }
                looked += 1;
                if looked > 64 {
                    break;
                }
                state.slots.each(&o, &mut |_, x| {
                    for p in x.pts.iter() {
                        match self.heap.kind(p) {
                            Kind::Function | Kind::Class => found.push(p),
                            _ => next.push(p),
                        }
                    }
                });
            }
            level = next;
        }
        found.sort_unstable();
        found.dedup();
        Value::objs(crate::domain::ObjSet::from_vec(found))
    }

    /// A skipped call's arguments its code keeps (`Seen::kept`) put where the program may read
    /// them back: the dicts' items (under the literal key, the function's `__name__` when that
    /// is known, else any), the call's result `r`.
    fn put_kept(&mut self, kept: Vec<(Value, Vec<(ObjId, KeptKey)>, bool)>, mut r: Value, node: &Node, state: &mut State) -> Value {
        for (v, places) in kept.iter().map(|(v, p, _)| (v, p)) {
            for &(d, key) in places {
                let key = match key {
                    KeptKey::Lit(k) => crate::domain::KeyVal::Lit(k),
                    KeptKey::Name => self.load_attr(v, "__name__", node, state, false).kv,
                    KeptKey::Any => crate::domain::DATA,
                    // (among a list's items, appended some number of times: its length not
                    // known past what it was)
                    KeptKey::Elem => {
                        let n = self.heap.llen(d, state).or_else(|| self.heap.prefix(d, state).map(|p| p as usize)).unwrap_or(0);
                        self.add_elem(d, v, state);
                        self.heap.set_prefix(d, state, n);
                        state.clear_last(d);
                        continue;
                    }
                };
                let key = if key.as_lit().is_some() { key } else { crate::domain::DATA };
                self.store_key(d, Ns::Item, &Value::kv_only(key), v, false, node, state);
            }
        }
        for (v, _, returned) in kept {
            if returned {
                r = r.join(&v);
            }
        }
        r
    }

    /// What a skipped call of `foid` on its one receiver among `recvs` stores on it on every way
    /// it completes (`must_stores`), but what its code deletes or a setter takes; where the
    /// receiver escapes (no `effs`), unknown code may remove them after.
    fn must_after(&mut self, foid: ObjId, recvs: &[ObjId], effs: &[(ObjId, RecvEffects)]) -> Vec<KeyId> {
        let hooked = |an: &Analyzer, o: ObjId| an.heap.instance_class(o).is_none_or(|c| ["__setattr__", "__delattr__", "__getattribute__"].iter().any(|h| an.class_lookup(c, h).is_some()));
        let [o] = recvs else { return Vec::new() };
        if !fi_is_method(self, foid) || hooked(self, *o) {
            return Vec::new();
        }
        let deleted: Vec<KeyId> = match effs {
            [(_, e)] => e.deletes.clone(),
            _ => self.code_uses(&self.heap.func_info(foid)).deleted.iter().map(|n| key_str_lit(&str_get(*n))).collect(),
        };
        self.must_stores(foid, *o, 0).iter().filter(|k| !deleted.contains(k) && !self.setter_of(*o, *k)).collect()
    }

    /// The objects among a skipped call's arguments whose items its code may remove, with what
    /// it removes (not `likely` after: `may_have_removed`).
    fn removed_args(&mut self, foid: ObjId, bound: bool, args: &[Value], kwargs: &IndexMap<StrId, Value>) -> Vec<(ObjId, Removes)> {
        let bound = usize::from(bound);
        let slots = args.iter().enumerate().map(|(i, v)| (Slot::Pos(i + bound), v)).chain(kwargs.iter().map(|(k, v)| (Slot::Kw(*k), v)));
        let mut out = Vec::new();
        for (slot, v) in slots.collect::<Vec<_>>() {
            let rm = self.param_flags(foid, slot).removes;
            out.extend(v.pts.iter().filter(|_| rm.any()).map(|o| (o, rm)));
        }
        out
    }

    /// Library code given `vals` (taken to change nothing it is given, or code rowan cannot
    /// see) may run the dunders of the program's instances they reach (`LIBRARY_DUNDERS`:
    /// formatting, comparing, hashing, converting, copying): those whose code may change
    /// something (`changes_nothing`) are one call none of which runs (`skipped_calls`).
    pub fn library_dunders(&mut self, vals: &[Value], state: &mut State, node: &Node) {
        let fns = self.given_dunders(vals, state);
        if !fns.is_empty() {
            let fns: Vec<(ObjId, Option<Value>)> = fns.into_iter().map(|(f, r)| (f, Some(r))).collect();
            self.skipped_calls(&fns, Args::pos(&[]), state, node, None);
        }
    }

    /// The dunders of the instances `vals` reach (through at most 256 objects) whose code may
    /// change something, by function, with their receivers.
    pub(in crate::interp) fn given_dunders(&mut self, vals: &[Value], state: &State) -> Vec<(ObjId, Value)> {
        let mut work: Vec<ObjId> = vals.iter().flat_map(|v| v.pts.iter()).collect();
        let mut seen: FxHashSet<ObjId> = FxHashSet::default();
        let mut by_class: FxHashMap<ObjId, Vec<ObjId>> = FxHashMap::default();
        while let Some(o) = work.pop() {
            if seen.len() >= 256 || !seen.insert(o) || matches!(self.heap.kind(o), Kind::Class | Kind::Module | Kind::Function | Kind::External | Kind::Unknown) {
                continue;
            }
            if let (Kind::Instance, Some(c)) = (self.heap.kind(o), self.heap.instance_class(o)) {
                by_class.entry(c).or_default().push(o);
            }
            self.obj_edges(o, state, &mut work);
        }
        let mut out: Vec<(ObjId, Value)> = Vec::new();
        let mut classes: Vec<(ObjId, Vec<ObjId>)> = by_class.into_iter().collect();
        classes.sort_unstable();
        for (c, recvs) in classes {
            for f in self.changing_dunders(c, state).iter().copied() {
                let r = Value::objs(recvs.iter().copied().collect());
                match out.iter_mut().find(|(g, _)| *g == f) {
                    Some((_, v)) => *v = v.join(&r),
                    None => out.push((f, r)),
                }
            }
        }
        out
    }

    /// The dunders of class `c` library code may run (`LIBRARY_DUNDERS`) whose code may change
    /// something.
    pub(in crate::interp) fn changing_dunders(&mut self, c: ObjId, state: &State) -> std::rc::Rc<[ObjId]> {
        if let Some(d) = self.effects.dunders.get(&c) {
            return d.clone();
        }
        let mut out = Vec::new();
        for name in LIBRARY_DUNDERS {
            let Some(owner) = self.class_lookup(c, name) else { continue };
            let k = key_str_lit(name);
            let v = state.slot_opt(owner, Ns::Attr, k).or_else(|| self.heap.cls_slots.get(&(owner, Ns::Attr, k)).cloned()).unwrap_or_default();
            for f in v.pts.iter().filter(|f| matches!(self.heap.obj(*f).payload, Payload::Func(_))).collect::<Vec<_>>() {
                if !out.contains(&f) && !self.changes_nothing(f, c, state, 0, &mut Vec::new()) {
                    out.push(f);
                }
            }
        }
        let out: std::rc::Rc<[ObjId]> = out.into();
        self.effects.dunders.insert(c, out.clone());
        out
    }

    /// The program's `__call__` of class `c`, and its `__init__` and `__new__` where the class
    /// itself is met (`called`): what code calling an instance of it, or it, runs.
    pub(in crate::interp) fn construct_dunders(&mut self, c: ObjId, called: bool, state: &State) -> Vec<ObjId> {
        let mut out = Vec::new();
        let names: &[&str] = if called { &["__init__", "__new__", "__call__"] } else { &["__call__"] };
        for &name in names {
            let Some(owner) = self.class_lookup(c, name) else { continue };
            let k = key_str_lit(name);
            let v = state.slot_opt(owner, Ns::Attr, k).or_else(|| self.heap.cls_slots.get(&(owner, Ns::Attr, k)).cloned()).unwrap_or_default();
            out.extend(v.pts.iter().filter(|f| matches!(self.heap.obj(*f).payload, Payload::Func(_))));
        }
        out
    }

    /// Does a call of `f` on an instance of class `c` change nothing (its receiver, its
    /// arguments, what its code reaches)? The methods it calls on its receiver neither
    /// (`depth`: at most 3 calls deep; `seen`: those met).
    fn changes_nothing(&mut self, f: ObjId, c: ObjId, state: &State, depth: u32, seen: &mut Vec<ObjId>) -> bool {
        if seen.contains(&f) {
            return true;
        }
        seen.push(f);
        let fi = self.heap.func_info(f);
        if let super::FuncNode::Def(d) = fi.node {
            if d.is_async || crate::syntax::is_generator(&d.body) {
                return false;
            }
        }
        let u = self.code_uses(&fi);
        let quiet = |f: &Flags| !(f.changes || f.held || f.held2 || f.called || f.kept || f.methods || f.rebinds || f.edits);
        // (the receiver: by what its code does with it; a part of it returned is not changed)
        let me = fi.node.params().and_then(|p| p.posonlyargs.iter().chain(p.args.iter()).next()).map(|p| s(p.parameter.name.id.as_str()));
        let Some(r) = u.recv.as_ref() else { return false };
        if r.escapes || !r.stores.is_empty() || !r.deletes.is_empty() || !r.stores_like.is_empty() || !r.deletes_like.is_empty() || !u.params.iter().all(|(p, f)| Some(*p) == me || quiet(f)) {
            return false;
        }
        let called: Vec<StrId> = r.calls.iter().chain(r.supers.iter()).copied().collect();
        for m in called {
            if depth >= 3 {
                return false;
            }
            for k in self.heap.mro(c) {
                let v = state.slot_opt(k, Ns::Attr, key_str_lit(&str_get(m))).or_else(|| self.heap.cls_slots.get(&(k, Ns::Attr, key_str_lit(&str_get(m)))).cloned()).unwrap_or_default();
                if !v.kv.is_bot() || v.pts.iter().any(|g| !matches!(self.heap.obj(g).payload, Payload::Func(_))) {
                    return false;
                }
                for g in v.pts.iter().collect::<Vec<_>>() {
                    if !self.changes_nothing(g, c, state, depth + 1, seen) {
                        return false;
                    }
                }
            }
        }
        let cl = self.code_closure(&[f], &mut None, state);
        [&cl.locs, &cl.held_locs, &cl.held2_locs, &cl.rebound, &cl.edited_locs].iter().all(|l| l.is_empty())
            && [&cl.values, &cl.held_values, &cl.edited_values].iter().all(|v| v.is_empty())
            && cl.removed_locs.is_empty()
            && cl.removed_values.is_empty()
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
        self.budget.skipped += 1;
        self.gens.force_at = None;
        let pre = state.copy();
        let recv = join_values(fns.iter().filter_map(|(_, r)| r.clone()).collect::<Vec<_>>());
        let text = call_ast.and_then(|c| crate::syntax::chain_text(&c.func)).unwrap_or_else(|| self.heap.func_info(fns[0].0).qualname.clone());
        let mut removed: Vec<(ObjId, Removes)> = Vec::new();
        for (f, r) in fns {
            removed.extend(self.removed_args(*f, r.is_some(), a.pos, a.kw));
        }
        self.havoc.classes_followed = true;
        let r = self.unknown_call(&format!("call of {}", text), (!recv.pts.is_empty()).then_some(&recv), a, state, node);
        let site = s(&format!("call of {}@L{}", text, node.line));
        self.names.skipped_sites.insert(site);
        let mut roots: Vec<ObjId> = fns.iter().map(|(f, _)| *f).collect();
        roots.append(&mut self.havoc.code);
        let rid = r.single().unwrap_or(0);
        self.follow_code(roots, rid, site, state);
        super::unknown::may_have_removed(&self.heap, &removed, state);
        self.flag_may_raise(state);
        let back: Vec<Value> = fns.iter().filter(|(f, r)| r.is_some() && self.code_uses(&self.heap.func_info(*f)).recv.as_ref().is_some_and(|u| u.returned)).filter_map(|(_, r)| r.clone()).collect();
        // (and what each one's code gives back of what it is given and reads)
        let given: Vec<Value> = fns.iter().map(|(f, r)| self.returned_args(*f, r.as_ref(), &a, &pre, false)).collect();
        join_values(std::iter::once(r).chain(back).chain(given).collect::<Vec<_>>())
    }

    /// The arguments of a call of `foid` that does not run, as what its code may change of
    /// them: one its parameter only reads is left out, one whose parts only it may change gives
    /// its parts (a generator's or coroutine's code runs later: all of them); and the code
    /// those left out hold (instances' classes, functions it calls: what reading them may run).
    /// One its code keeps (`Flags::kept`) where its places are known is left out too: it goes
    /// there (`kept`), not to code that may call it.
    fn args_as_seen(&mut self, foid: ObjId, recv: Option<&Value>, args: &[Value], kwargs: &IndexMap<StrId, Value>, state: &State) -> Seen {
        let bound = recv.is_some();
        let slots: Vec<Slot> = (0..args.len()).map(|i| Slot::Pos(i + usize::from(bound))).chain(kwargs.keys().map(|k| Slot::Kw(*k))).collect();
        let mut kept = Vec::new();
        let mut flags: Vec<Flags> = Vec::new();
        for (slot, v) in slots.into_iter().zip(args.iter().chain(kwargs.values())) {
            let mut f = self.param_flags(foid, slot);
            // (functions: what calling them may do is followed where they are called from there)
            let code = !v.pts.is_empty() && v.kv.is_bot() && v.pts.iter().all(|o| self.heap.kind(o) == Kind::Function);
            if let Some((dicts, returned)) = (f.kept && code).then(|| self.kept_places(foid, slot, recv, state, 0)).flatten() {
                f.kept = false;
                kept.push((v.clone(), dicts, returned));
            }
            flags.push(f);
        }
        let kw = flags.split_off(args.len());
        let pos = flags;
        let mut code: Vec<ObjId> = Vec::new();
        let mut seen = |an: &Analyzer, f: Flags, v: &Value| -> Value {
            let (own, held) = an.flag_effect(&f, v, state);
            let (own, held) = (own || f.edits, held || f.held2);
            // (a container iterated: what its items run when compared)
            if !own && f.iterated {
                v.pts.iter().for_each(|o| state.slots.each(&o, &mut |_, x| code.extend(x.pts.iter().filter_map(|i| super::unknown::compared_code(&an.heap, i)))));
            }
            if !own {
                code.extend(v.pts.iter().filter_map(|o| match (an.heap.kind(o), &an.heap.obj(o).payload) {
                    // (reading a function's attributes runs none of its code)
                    (Kind::Function, _) if !f.called => None,
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
        Seen { args, kwargs, code, kept }
    }

    /// Where the code of `foid` (on receiver `recv`, if bound) keeps the argument in `slot`
    /// (`Flags::kept`): the containers whose items may hold it (dicts by a key, lists among
    /// their items: what the names and attributes it names hold), those of the program's
    /// functions it gives it to (`depth`: calls deep, at most 3; their code does nothing else
    /// with it), and whether it returns it; None where those places are not known.
    fn kept_places(&mut self, foid: ObjId, slot: Slot, recv: Option<&Value>, state: &State, depth: u32) -> Option<(Vec<(ObjId, KeptKey)>, bool)> {
        let fi = self.heap.func_info(foid);
        let ps = fi.node.params()?;
        let p = match slot {
            Slot::Pos(i) => ps.posonlyargs.iter().chain(ps.args.iter()).nth(i)?,
            Slot::Kw(k) => ps.args.iter().chain(ps.kwonlyargs.iter()).find(|p| s(p.parameter.name.id.as_str()) == k)?,
        };
        let p = s(p.parameter.name.id.as_str());
        let uses = self.code_uses(&fi);
        let mut places = Vec::new();
        for place in uses.kept_in.get(&p).into_iter().flatten() {
            let (base, attrs, key) = match place {
                KeptPlace::Name(n, attrs, key) => {
                    let base = match self.resolve_use(&fi, &uses, *n) {
                        Ok(loc) => state.slot(loc.0, loc.1, loc.2),
                        Err(m) => Value::obj(m),
                    };
                    (base, attrs, *key)
                }
                KeptPlace::Recv(attrs, key) => (recv?.clone(), attrs, *key),
                KeptPlace::Passed(callee, slot) => {
                    if depth >= 3 {
                        return None;
                    }
                    for f in self.program_callees(&fi, &uses, *callee, state, true)? {
                        let fl = self.param_flags(f, *slot);
                        let (more, returned) = self.kept_places(f, *slot, None, state, depth + 1)?;
                        // (its code does nothing else with it; what it returns is dropped)
                        let _ = returned;
                        if fl != (Flags { kept: true, ..Flags::default() }) {
                            return None;
                        }
                        places.extend(more);
                    }
                    continue;
                }
            };
            let v = self.attr_path(base, attrs, state)?;
            let fits = |o: ObjId| match key {
                KeptKey::Elem => self.heap.kind(o) == Kind::List && !self.heap.obj(o).iterator,
                _ => self.heap.kind(o) == Kind::Dict,
            };
            if v.pts.is_empty() || !v.kv.is_bot() || !v.pts.iter().all(fits) {
                return None;
            }
            places.extend(v.pts.iter().map(|d| (d, key)));
        }
        Some((places, uses.kept_returned.contains(&p)))
    }

    /// What `base` holds through attributes `attrs` (a module's globals, a class's attributes,
    /// an instance's on this path); None where one of them is something else, or not there.
    fn attr_path(&self, mut v: Value, attrs: &[StrId], state: &State) -> Option<Value> {
        for a in attrs {
            let k = key_str_lit(&str_get(*a));
            let mut next = Value::new();
            for o in v.pts.iter() {
                let x = match self.heap.kind(o) {
                    Kind::Module | Kind::Instance if state.row(o, Ns::Attr).definite.contains(k) || self.heap.kind(o) == Kind::Module => state.slot(o, Ns::Attr, k),
                    Kind::Class => self.class_attr_value(o, &str_get(*a), state),
                    _ => return None,
                };
                next = next.join(&x);
            }
            if next.empty() || !v.kv.is_bot() {
                return None;
            }
            v = next;
        }
        Some(v)
    }

    /// What a function's code adds to a closure: what may happen to the values of the names it
    /// reads (globals, cells, modules' globals) and of its defaults, the code they hold, and
    /// what may happen to attributes by name (`attrs`, `class_stores`: class attributes).
    #[allow(clippy::too_many_arguments)]
    fn function_closure(&mut self, fi: &FuncInfo, state: &State, out: &mut Closure, attrs: &mut FxHashMap<StrId, Flags>, reads: &mut FxHashSet<StrId>, class_stores: &mut FxHashSet<StrId>, work: &mut Vec<ObjId>) {
        let code = |an: &Analyzer, v: &Value, work: &mut Vec<ObjId>| work.extend(v.pts.iter().filter(|x| an.holds_code(*x)));
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
                    (true, _) => {
                        out.edited_values.push(v.clone());
                        out.removed_values.extend(f.removes.any().then(|| (v.clone(), f.removes)));
                    }
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
                let removes = if f.called && super::flows::CONTAINER_REMOVERS.contains(&&*str_get(a)) { if f.removes.any() { f.removes } else { Removes::Any } } else { Removes::No };
                f = Flags { changes: f.called && !edit || f.rebinds, edits: edit, removes, held: f.changes || f.methods || f.held || f.held2 || f.iterated || f.edits, ..Flags::default() };
            }
            self.apply_flags(&f, loc, state, out);
            match state.slot_opt(loc.0, loc.1, loc.2) {
                Some(v) => code(self, &v, work),
                // (a builtin's name not bound in the module: the builtin, taken as fixed)
                None if !intern::key_as_str(loc.2).is_some_and(|n| crate::models::tables::is_builtin_name(&n)) => out.unbound.push(loc),
                None => {}
            }
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
                (Kind::Class, _) if self.heap.class_info_opt(o).is_some_and(|ci| !ci.open_base.get()) => match self.class_lookup(o, "__new__") {
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
                    if *l != Level::Own && v.pts.iter().any(|o| !matches!(self.heap.kind(o), Kind::Dict | Kind::List | Kind::Tuple | Kind::Set)) {
                        return true;
                    }
                    self.classes_within(&v, state, if *l != Level::Own { 2 } else { 0 }, out);
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
        let rest = |p: Option<&ast::Parameter>| p.map(flags).map(|f| Flags { changes: f.changes || f.kept || f.methods || f.held, ..Flags::default() }).unwrap_or_default();
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
        // (what it holds under a few literal keys only, where those are its dicts' slots: they)
        let parts: Option<Vec<Loc>> = f.parts.keys().filter(|_| held && !(f.iterated && !f.held)).and_then(|ks| {
            v.pts.iter().flat_map(|d| ks.iter().map(move |k| (d, *k))).map(|(d, k)| (self.heap.kind(d) == Kind::Dict && state.has_slot(d, Ns::Item, k)).then_some((d, Ns::Item, k))).collect()
        }).filter(|_| v.kv.is_bot() && !v.pts.is_empty());
        if own {
            out.locs.push(loc);
        } else if let Some(ps) = parts {
            out.locs.extend(ps);
        } else if held {
            out.held_locs.push(loc);
        } else if f.held2 {
            out.held2_locs.push(loc);
        }
        match self.edited(f, &v, state) {
            (true, _) if f.removes.any() => {
                out.edited_locs.push(loc);
                out.removed_locs.push((loc, f.removes));
            }
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
        let own = f.changes || f.kept || f.methods || ((f.iterated || f.edits) && !plain) || (f.called && !v.pts.iter().all(code_only));
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

/// A walk of a method's statements for what it stores on its receiver (`me`) on every way it
/// completes: `block` gives the names stored when the block falls through (None: it never
/// does), from those stored before it; each `return` meets its names into `ret`. A raise does
/// not complete; a loop, `with` or `match` keeps what was stored before it (its returns count).
struct MustWalk<'a> {
    me: &'a str,
    /// the name of the method's class
    cls: &'a str,
    ret: Option<KeySet>,
    /// locals bound to a method of `super()` (`init = super().__init__`)
    supers: Vec<(&'static str, &'static str)>,
    sup: &'a dyn Fn(&str) -> KeySet,
    /// the receiver's name is rebound: its stores may go elsewhere
    rebound: bool,
}

impl MustWalk<'_> {
    fn block(&mut self, body: &'static [Stmt], mut acc: KeySet) -> Option<KeySet> {
        for st in body {
            acc = self.stmt(st, acc)?;
        }
        Some(acc)
    }

    fn meet(a: Option<KeySet>, b: Option<KeySet>) -> Option<KeySet> {
        match (a, b) {
            (Some(a), Some(b)) => Some(a.intersection(&b)),
            (a, b) => a.or(b),
        }
    }

    fn target(&mut self, t: &'static Expr, acc: KeySet) -> KeySet {
        match t {
            Expr::Attribute(a) if matches!(&*a.value, Expr::Name(n) if n.id.as_str() == self.me) => acc.insert(key_str_lit(a.attr.as_str())),
            Expr::Name(n) if n.id.as_str() == self.me => {
                self.rebound = true;
                acc
            }
            Expr::Tuple(t) => t.elts.iter().fold(acc, |acc, e| self.target(e, acc)),
            Expr::List(t) => t.elts.iter().fold(acc, |acc, e| self.target(e, acc)),
            _ => acc,
        }
    }

    /// `super().m` / `super(C, self).m` of the method's own class C: m.
    fn super_method(&self, e: &'static Expr) -> Option<&'static str> {
        let Expr::Attribute(a) = e else { return None };
        let Expr::Call(c) = &*a.value else { return None };
        let name = |x: &Expr, n: &str| matches!(x, Expr::Name(y) if y.id.as_str() == n);
        let own = match &c.arguments.args[..] {
            [] => true,
            [k, me] => name(k, self.cls) && name(me, self.me),
            _ => false,
        };
        (name(&c.func, "super") && own).then(|| a.attr.as_str())
    }

    fn stmt(&mut self, st: &'static Stmt, acc: KeySet) -> Option<KeySet> {
        match st {
            Stmt::Assign(a) => {
                if let ([Expr::Name(n)], Some(m)) = (a.targets.as_slice(), self.super_method(&a.value)) {
                    self.supers.push((n.id.as_str(), m));
                }
                Some(a.targets.iter().fold(acc, |acc, t| self.target(t, acc)))
            }
            Stmt::AnnAssign(a) if a.value.is_some() => Some(self.target(&a.target, acc)),
            Stmt::AugAssign(a) => Some(self.target(&a.target, acc)),
            Stmt::Expr(e) => {
                let Expr::Call(c) = &*e.value else { return Some(acc) };
                let m = self.super_method(&c.func).or_else(|| match &*c.func {
                    Expr::Name(n) => self.supers.iter().rev().find(|(l, _)| *l == n.id.as_str()).map(|(_, m)| *m),
                    _ => None,
                });
                Some(match m {
                    Some(m) => acc.union(&(self.sup)(m)),
                    None => acc,
                })
            }
            Stmt::Return(_) => {
                self.ret = Self::meet(self.ret.take(), Some(acc));
                None
            }
            Stmt::Raise(_) => None,
            Stmt::If(i) => {
                let mut fall = self.block(&i.body, acc.clone());
                let mut has_else = false;
                for c in &i.elif_else_clauses {
                    has_else |= c.test.is_none();
                    let f = self.block(&c.body, acc.clone());
                    fall = if fall.is_none() { f } else if f.is_none() { fall } else { Self::meet(fall, f) };
                }
                if !has_else {
                    fall = Self::meet(fall, Some(acc));
                }
                fall
            }
            Stmt::Try(t) if !crate::syntax::contains_return(&t.finalbody) => {
                let mut fall = self.block(&t.body, acc.clone()).and_then(|b| self.block(&t.orelse, b));
                for h in &t.handlers {
                    let ast::ExceptHandler::ExceptHandler(h) = h;
                    let f = self.block(&h.body, acc.clone());
                    fall = if fall.is_none() { f } else if f.is_none() { fall } else { Self::meet(fall, f) };
                }
                fall.and_then(|f| self.block(&t.finalbody, f))
            }
            Stmt::For(_) | Stmt::While(_) | Stmt::With(_) | Stmt::Match(_) => {
                // (their returns, with what was stored before them along the body)
                let bodies: Vec<&'static [Stmt]> = match st {
                    Stmt::For(f) => vec![&f.body, &f.orelse],
                    Stmt::While(w) => vec![&w.body, &w.orelse],
                    Stmt::With(w) => vec![&w.body],
                    Stmt::Match(m) => m.cases.iter().map(|c| c.body.as_slice()).collect(),
                    _ => Vec::new(),
                };
                for b in bodies {
                    self.block(b, acc.clone());
                }
                Some(acc)
            }
            // (a `return` in a `finally`: every way out has what was stored before)
            Stmt::Try(_) => {
                self.ret = Self::meet(self.ret.take(), Some(acc.clone()));
                Some(acc)
            }
            Stmt::Delete(d) => {
                let mut acc = acc;
                for t in &d.targets {
                    match t {
                        Expr::Attribute(a) if matches!(&*a.value, Expr::Name(n) if n.id.as_str() == self.me) => acc = acc.remove(key_str_lit(a.attr.as_str())),
                        Expr::Name(n) if n.id.as_str() == self.me => self.rebound = true,
                        _ => {}
                    }
                }
                Some(acc)
            }
            _ => Some(acc),
        }
    }
}
