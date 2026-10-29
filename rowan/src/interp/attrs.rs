//! Attribute access: reads through the class (methods bound, properties and descriptors run,
//! `__getattr__`), instance attributes, unknown objects' attributes, and stores (`__setattr__`,
//! frozen dataclasses, `__slots__`).

use indexmap::IndexMap;
use ruff_python_ast as ast;

use crate::domain::{classify, join_values, kv_exclude, kv_literals, typed_label, BOT, Classification, KeyVal, Ns, ObjId, Row, Tier, Value};
use crate::frontend::{self, Node};
use crate::intern::{self, key_none, key_str_lit, KeyId};
use crate::state::{mark_merged, Kind, State};

use super::{Analyzer, Args, DICT_METHOD_NAMES, Payload, STR_METHOD_NAMES, ViewKind, is_scalar_method};
use super::access::OBJECT_ATTRS;

/// The names every module has (`__name__`, `__file__`, …), bound or not in its namespace.
pub const MODULE_NAMES: &[&str] = &["__name__", "__file__", "__doc__", "__package__", "__spec__", "__loader__", "__path__", "__cached__", "__builtins__"];

impl Analyzer {
    /// Does attribute `name` of `oid` resolve without the object's own row: through its class
    /// (an instance), for a class object through its MRO and `type`, for an object a model
    /// simulates through the model?
    pub(crate) fn class_resolves(&self, oid: u32, name: &str) -> bool {
        if self.heap.kind(oid) == Kind::Class {
            return (name.starts_with("__") && name.ends_with("__")) || name == "mro" || self.class_lookup(oid, name).is_some();
        }
        if self.heap.kind(oid) == Kind::Namespace {
            if let Payload::Method(q, _) = &self.heap.obj(oid).payload {
                if ["stdobj.", "pydverr.", "typeadapter.", "fastapi."].iter().any(|p| q.starts_with(p)) {
                    return true;
                }
            }
        }
        if self.heap.kind(oid) != Kind::Instance {
            return false;
        }
        if OBJECT_ATTRS.contains(&name) {
            return true;
        }
        match self.heap.instance_class(oid) {
            // (`__getattr__` / `__getattribute__` answer any name)
            Some(c) if self.class_lookup(c, "__getattr__").is_some() || self.class_lookup(c, "__getattribute__").is_some() => true,
            // (a field's class-level default is shadowed by the instance's attribute)
            Some(c) => self.class_lookup(c, name).is_some() && !crate::models::dataclasses::dataclass_fields_mro(self, c).iter().any(|(f, _)| f == name),
            None => false,
        }
    }

    /// `class_resolves` on this path: a class attribute stored on some paths only (`if c: Cls.x =
    /// 1`) may be missing on this one.
    pub(crate) fn class_resolves_on_path(&self, oid: u32, name: &str, state: &State) -> bool {
        if !self.class_resolves(oid, name) {
            return false;
        }
        if self.heap.kind(oid) != Kind::Instance || OBJECT_ATTRS.contains(&name) {
            return true;
        }
        match self.heap.instance_class(oid) {
            Some(c) if self.class_lookup(c, "__getattr__").is_none() && self.class_lookup(c, "__getattribute__").is_none() => self.class_lookup_on_path(c, name, state).1,
            _ => true,
        }
    }

    /// The attribute names `__slots__` allows on instances of `cls`, when every class of its MRO
    /// declares them (literally) and none gives a `__dict__`.
    pub(in crate::interp) fn slots_of(&self, cls: u32) -> Option<Vec<String>> {
        let mut out: Vec<String> = Vec::new();
        for c in self.heap.mro(cls) {
            let Some(ci) = self.heap.class_info_opt(c) else {
                // (`object` at the end of the MRO has no instance dict of its own)
                if self.heap.kind(c) == Kind::External && self.heap.external_qual(c) == "builtins.object" {
                    continue;
                }
                return None;
            };
            let names = declared_slots(ci.node).ok()??;
            if names.iter().any(|n| n == "__dict__") {
                return None;
            }
            out.extend(names);
        }
        Some(out)
    }

    /// Is `name` (may it be; None: any name) a `__slots__` member of a class of `cls`'s MRO, not
    /// shadowed by a class attribute before it: a data descriptor that takes reads and stores
    /// of the name over the instance's `__dict__`?
    pub(in crate::interp) fn slot_member(&self, cls: u32, name: Option<&str>) -> bool {
        for c in self.heap.mro(cls) {
            let Some(ci) = self.heap.class_info_opt(c) else { continue };
            if declared_slots(ci.node).map_or(true, |ns| ns.is_some_and(|ns| name.is_none_or(|name| ns.iter().any(|n| n == name)))) {
                return true;
            }
            if name.is_some_and(|n| self.heap.cls_slots.contains_key(&(c, Ns::Attr, key_str_lit(n)))) {
                return false;
            }
        }
        false
    }

    /// `obj.__dict__`: a view of its attributes (None: an AttributeError, `__slots__` on every
    /// class of an instance's MRO).
    fn own_dict(&mut self, oid: ObjId, single: bool, state: &mut State) -> Option<Value> {
        match self.slots_refuse(oid, None) {
            Some(true) => {
                if single {
                    self.raise_here("AttributeError", state);
                } else {
                    self.may_raise_here("AttributeError", state);
                }
                return None;
            }
            Some(false) => self.may_raise_here("AttributeError", state),
            None => {}
        }
        let vid = self.heap.alloc_raw(Kind::View, &format!("view:attrs:{}", oid), "", true, "__dict__", Payload::View(ViewKind::Attrs, Value::obj(oid)));
        self.heap.obj_mut(vid).payload = Payload::View(ViewKind::Attrs, Value::obj(oid));
        Some(Value::obj(vid))
    }

    /// The attributes `obj.__dict__` holds, as the records and namespace that hold them: a
    /// dict's items (`Ns::Item`), or for views of objects' `__dict__` (`vars(obj)` too) those
    /// objects' attributes (`Ns::Attr`; None: such views beside other objects, or of objects
    /// whose rows do not hold every name of their `__dict__`).
    pub(crate) fn mapping_parts(&self, v: &Value) -> Option<(Value, Ns)> {
        let attrs_of = |o: ObjId| self.heap.attrs_view(o).cloned();
        if !v.pts.iter().any(|o| attrs_of(o).is_some()) {
            return Some((v.clone(), Ns::Item));
        }
        let insts: Vec<Value> = v.pts.iter().map(attrs_of).collect::<Option<_>>()?;
        let insts = join_values(insts);
        (v.kv.is_bot() && insts.pts.iter().all(|i| self.attrs_are_dict(i))).then_some((insts, Ns::Attr))
    }

    /// Does object `oid`'s row of attributes hold its `__dict__`, every name and no other (an
    /// instance without `__slots__` members, a function, a namespace; not a module, whose
    /// dict has names its row does not hold)?
    pub(crate) fn attrs_are_dict(&self, oid: ObjId) -> bool {
        match self.heap.kind(oid) {
            Kind::Instance => self.heap.instance_class(oid).is_some_and(|c| !self.slot_member(c, None)),
            Kind::Function | Kind::Namespace => true,
            _ => false,
        }
    }

    /// Does `__slots__` refuse instance `oid` attribute `name` (None: a `__dict__`) — on every
    /// class it may be of (`classes_of_instance`: a subclass without `__slots__` has a
    /// `__dict__`): Some(true) surely, Some(false) on some, None on none?
    pub(crate) fn slots_refuse(&self, oid: ObjId, name: Option<&str>) -> Option<bool> {
        if self.heap.kind(oid) != Kind::Instance {
            return None;
        }
        let own = self.heap.instance_class(oid)?;
        let cs = self.classes_of_instance(oid).unwrap_or_else(|| vec![own, own]);
        let refuse = |c: &u32| self.slots_of(*c).is_some_and(|allowed| name.is_none_or(|n| !allowed.iter().any(|a| a == n)));
        let n = cs.iter().filter(|c| refuse(c)).count();
        // (past the subclasses `classes_of_instance` lists: the own class's, and others may not)
        let all = n == cs.len() && !(cs.len() == 2 && cs[0] == cs[1]);
        (n > 0).then_some(all)
    }

    /// The class of an instance, when it is a user class.
    pub(super) fn user_class_of(&self, o: u32) -> Option<u32> {
        let c = self.heap.instance_class(o)?;
        self.heap.class_info_opt(c).map(|_| c)
    }

    /// Does `o`'s class (one of the program's) define or inherit method `m`?
    pub(super) fn has_user_method(&self, o: u32, m: &str) -> bool {
        self.user_class_of(o).is_some_and(|c| self.class_lookup(c, m).is_some())
    }

    /// Reading / writing attribute `name` of `o` runs user code: a property, a descriptor,
    /// `__getattr__` / `__getattribute__` (reading), `__setattr__` (writing).
    pub(super) fn attr_runs_user_code(&self, o: u32, name: &str, store: bool) -> bool {
        // (a class of the program's: a descriptor's `__get__` runs for a read through it)
        if !store && self.heap.kind(o) == Kind::Class && self.heap.class_info_opt(o).is_some() {
            return self.class_lookup(o, name).is_some_and(|dc| self.heap.class_attr(dc, name).pts.iter().any(|d| self.is_descriptor(d, "__get__")));
        }
        let Some(c) = self.user_class_of(o) else { return false };
        let hooks: &[&str] = if store { &["__setattr__"] } else { &["__getattr__", "__getattribute__"] };
        if hooks.iter().any(|h| self.class_lookup(c, h).is_some()) {
            return true;
        }
        let Some(dc) = self.class_lookup(c, name) else { return false };
        let cv = self.heap.class_attr(dc, name);
        let runs = cv.pts.iter().any(|f| (self.heap.kind(f) == Kind::Function && self.heap.func_info(f).is_property.get()) || (self.heap.kind(f) == Kind::Instance && self.heap.instance_class(f).is_some_and(|fc| self.class_lookup(fc, if store { "__set__" } else { "__get__" }).is_some())));
        runs
    }

    /// Is `d` an instance of a class with method `m` (`__get__`, `__set__`, `__delete__`): a
    /// descriptor (with `__set__` or `__delete__`, a data descriptor: it takes reads over the
    /// instance's own value)?
    pub(in crate::interp) fn is_descriptor(&self, d: ObjId, m: &str) -> bool {
        self.heap.kind(d) == Kind::Instance && self.heap.instance_class(d).and_then(|dc| self.class_lookup(dc, m)).is_some()
    }

    // ---------------------------------------------------------- attributes
    /// A read `obj.name`: per receiver when one runs code for it (a property, a descriptor,
    /// `__getattr__`), else on each object by its kind — an instance (`load_instance_attr`), a
    /// class, a module, an unknown (`load_unknown_attr`), a library object (its method or
    /// attribute model) — with a verdict where a record may lack the name.
    pub fn load_attr(&mut self, obj: &Value, name: &str, node: &Node, state: &mut State, report: bool) -> Value {
        let forced = self.force(obj.clone(), state);
        let obj = &forced;
        if let Some(v) = self.per_receiver(obj, &|an, o| an.attr_runs_user_code(o, name, false), state, &mut |an, o, st| an.load_attr(o, name, node, st, report)) {
            return v;
        }
        let unk = self.unkey(obj);
        let obj = &unk;
        if let Some(v) = crate::models::torch::t_getattr(self, obj, name, node, state) {
            return v;
        }
        self.none_beside_lacks(obj, name, report, state);
        let mut acc = Value::new();
        // (one k-way union of all targets' values: a receiver of many objects, joined pairwise,
        // costs the product of their sizes)
        let mut parts: Vec<Value> = Vec::new();
        let guarded = self.is_guarded(Ns::Attr) || !report;
        let caught = report && self.catches(Ns::Attr);
        // (one that may fail raises to a handler, or to a finally that runs code)
        let raises = caught || (report && self.finally_sees());
        // (a value that may also be a scalar — a str, a number — may have the attribute: no
        // object's lack of it makes the read fail for sure)
        let scalar = self.may_be_scalar(obj);
        let single = obj.pts.len() == 1 && !scalar;
        let saved_scalar = std::mem::replace(&mut self.mode.recv_scalar, scalar);
        let saved_wide = self.wide_read(obj, node);
        let nk = key_str_lit(name);
        for oid in obj.pts.iter() {
            let k = self.heap.kind(oid);
            let v: Value = match k {
                // (its namespace, as `vars` gives it: the deferred definitions in it run)
                Kind::Module if name == "__dict__" => {
                    self.force_module(oid, state);
                    self.own_dict(oid, single, state).unwrap_or_default()
                }
                Kind::Module => self.module_attr(oid, name, nk, node, state, caught, guarded, single),
                Kind::External if matches!(name, "__name__" | "__qualname__") && self.heap.external_qual(oid).starts_with("builtins.") => {
                    let q = self.heap.external_qual(oid);
                    Value::lit(key_str_lit(q.rsplit('.').next().unwrap_or("")))
                }
                Kind::External => self.external_attr(oid, name, state),
                Kind::Partial if matches!(name, "func" | "args" | "keywords") => crate::models::partial_attr(self, oid, name, node, state),
                Kind::Parser | Kind::Groupby | Kind::Partial => self.method_value(&format!("{}.{}", k.name(), name), &Value::obj(oid)),
                Kind::Instance if name == "__class__" => match self.classes_of_instance(oid) {
                    Some(cs) => Value::objs(crate::domain::ObjSet::from_vec(cs)),
                    None => Value::data(&format!("L{}", node.line)),
                },
                Kind::Namespace | Kind::Instance | Kind::Function if name == "__dict__" => match self.own_dict(oid, single, state) {
                    Some(v) => v,
                    None => continue,
                },
                Kind::Namespace | Kind::List | Kind::View if crate::models::model_object_attr(self, oid, name, node, state).map(|v| parts.push(v)).is_some() => continue,
                Kind::Namespace | Kind::Instance => self.load_instance_attr(oid, name, node, state, guarded, raises, single),
                Kind::Class if name == "__annotations__" && self.class_lookup(oid, name).is_none() && self.heap.class_info_opt(oid).is_some() => {
                    let names = crate::models::stdlib::annotated_names(self, oid, false).unwrap_or_default();
                    crate::models::stdlib::names_dict(self, state, node, &names, None, "annotation")
                }
                Kind::Class if crate::models::record_class_attr(self, oid, name, node, state).map(|v| parts.push(v)).is_some() => continue,
                Kind::Class if matches!(name, "__name__" | "__qualname__") && self.class_lookup(oid, name).is_none() => {
                    let n = self.heap.class_info_opt(oid).map(|ci| ci.name.clone()).unwrap_or_default();
                    Value::lit(key_str_lit(&n))
                }
                Kind::Class => self.class_attr(oid, name, node, state, guarded, report, single),
                Kind::Dict => self.dict_attr(oid, name, node, state),
                Kind::List if self.m.deques.contains_key(&oid) && crate::models::counter::DEQUE_METHODS.contains(&name) => self.deque_attr(oid, name),
                Kind::List | Kind::Tuple | Kind::Set => self.method_value(&format!("{}.{}", k.name(), name), &Value::obj(oid)),
                Kind::Frame => crate::models::pandas::frame_getattr(self, oid, name, node, state, guarded),
                Kind::Series => crate::models::pandas::series_getattr(self, oid, name, node, state),
                Kind::Locidx if matches!(&self.heap.obj(oid).payload, Payload::Locidx(a, _) if matches!(a.as_str(), "str" | "dt" | "cat" | "sparse" | "list" | "struct")) => self.accessor_attr(oid, name, node, state),
                Kind::Unknown => self.load_unknown_attr(oid, name, node, state, guarded, raises),
                Kind::Function => self.function_attr(oid, name, nk, single, node, state),
                Kind::Bound => Value::data(&format!("L{}", node.line)),
                Kind::View => match self.view_attr(oid, name, node, state, report) {
                    Ok(v) => v,
                    Err(v) => {
                        acc = acc.join(&v);
                        continue;
                    }
                },
                Kind::Zip | Kind::Enumerate => self.method_value(&format!("{}.{}", k.name(), name), &Value::obj(oid)),
                Kind::Response => self.method_value(&format!("response.{}", name), &Value::obj(oid)),
                Kind::Locidx => Value::data("loc"),
                Kind::Super => match self.super_attr(oid, name, nk) {
                    Some(v) => v,
                    None => continue,
                },
                _ => Value::data(&format!("L{}", node.line)),
            };
            parts.push(v);
        }
        self.mode.recv_scalar = saved_scalar;
        self.end_wide_read(saved_wide, state);
        acc = acc.join(&join_values(parts));
        if report && self.literals_lack_attr(obj, name, state) {
            return Value::new();
        }
        if obj.pts.is_empty() {
            return crate::models::builtins::scalar_attr(self, obj, name);
        }
        acc.join(&self.mixed_literals_attr(obj, name))
    }

    /// None beside `obj`'s objects has no attribute `name` but what every object has and
    /// `__bool__`: an AttributeError.
    fn none_beside_lacks(&mut self, obj: &Value, name: &str, report: bool, state: &mut State) {
        let lacks = crate::models::tables::builtin_has_attr("NoneType", name) == Some(false);
        if report && !obj.pts.is_empty() && obj.kv.is_literal() && lacks && kv_literals(&obj.kv).contains(key_none()) {
            self.may_raise_here("AttributeError", state);
        }
    }

    /// An AttributeError where the builtin type of one of `obj`'s literals lacks attribute
    /// `name` (true: surely — each of them lacks it, no object beside them); a typed unknown's
    /// type may be a subclass's (it may). None beside objects is `none_beside_lacks`'.
    fn literals_lack_attr(&mut self, obj: &Value, name: &str, state: &mut State) -> bool {
        use crate::models::tables::builtin_has_attr;
        // (per part: Some(true) lacks it, Some(false) may, None has it or is not known)
        let parts: Vec<Option<bool>> = match &obj.kv {
            kv if kv.is_literal() => kv_literals(kv).iter().filter(|k| obj.pts.is_empty() || *k != key_none()).map(|k| builtin_has_attr(lit_type(k)?, name).and_then(|h| (!h).then_some(true))).collect(),
            // (an untyped unknown, or a key of some objects, may be any object: it may lack it)
            KeyVal::Data(l, _) => vec![match crate::domain::label_type(&intern::str_get(*l)) {
                Some(t) => builtin_has_attr(t, name).and_then(|h| (!h).then_some(false)),
                None => Some(false),
            }],
            kv if !kv.is_bot() => vec![Some(false)],
            _ => vec![],
        };
        if obj.pts.is_empty() && !parts.is_empty() && parts.iter().all(|p| *p == Some(true)) {
            self.raise_here("AttributeError", state);
            return true;
        }
        if parts.iter().any(|p| p.is_some()) {
            self.may_raise_here("AttributeError", state);
        }
        false
    }

    /// The attribute of the literals beside a value's objects (`ops::mixed`), else nothing.
    fn mixed_literals_attr(&mut self, obj: &Value, name: &str) -> Value {
        if !self.mixed(obj) {
            return Value::new();
        }
        // (not those whose type surely lacks it: they raise, `literals_lack_attr`)
        let lacks: crate::intern::KeySet = kv_literals(&obj.kv).iter().filter(|k| *k == key_none() || lit_type(*k).and_then(|t| crate::models::tables::builtin_has_attr(t, name)) == Some(false)).collect();
        let lits = Value::kv_only(kv_exclude(&obj.kv, &lacks).unwrap_or(BOT));
        if lits.kv.is_bot() {
            return Value::new();
        }
        crate::models::builtins::scalar_attr(self, &lits, name)
    }

    /// `del obj.name`: on an instance whose class has a property of the name, its deleter runs
    /// (none: AttributeError); the others lose the attribute (`delete_item`).
    pub(super) fn delete_attr(&mut self, obj: &Value, name: &str, node: &Node, state: &mut State) {
        // (an instance whose class defines `__delattr__`: it runs instead)
        let hook = if self.mode.bypass_setattr { None } else { self.run_dunder(obj, &["__delattr__"], vec![Value::lit_str(name)], node, state) };
        let obj = &match hook {
            Some((_, rest)) if rest.empty() => return,
            Some((_, rest)) => rest,
            None => obj.clone(),
        };
        let nk = key_str_lit(name);
        // (a class's own attribute: gone on this path, `cls_late`; one it does not hold itself,
        // a base's or none, raises)
        let classes: Vec<ObjId> = obj.pts.iter().filter(|o| self.heap.kind(*o) == Kind::Class).collect();
        if !classes.is_empty() {
            let strong = obj.pts.len() == 1;
            for c in classes {
                let (defs, sure) = self.class_lookup_on_path(c, name, state);
                if defs.first() != Some(&c) {
                    self.raise_here("AttributeError", state);
                    return;
                }
                if !sure || defs.len() > 1 {
                    self.may_raise_here("AttributeError", state);
                }
                self.heap.cls_late.insert((c, nk));
                let r = state.row(c, Ns::Attr).delete(nk, strong);
                state.set_row(c, Ns::Attr, r);
                if strong {
                    state.pop_slot(c, Ns::Attr, nk);
                }
            }
            let rest = Value::with_kv(obj.pts.filter(|o| self.heap.kind(o) != Kind::Class), obj.kv.clone());
            if rest.empty() {
                return;
            }
            return self.delete_attr(&rest, name, node, state);
        }
        let prop = |an: &Analyzer, o: ObjId| -> Option<Option<ObjId>> {
            let cls = an.heap.instance_class(o).filter(|_| an.heap.kind(o) == Kind::Instance)?;
            let defc = an.class_lookup(cls, name)?;
            let cv = an.heap.cls_slots.get(&(defc, Ns::Attr, nk))?;
            let is_prop = !cv.pts.is_empty() && cv.pts.iter().all(|f| an.heap.kind(f) == Kind::Function && an.heap.func_info(f).is_property.get());
            is_prop.then(|| an.heap.prop_deleters.get(&(defc, nk)).copied())
        };
        let mut one = |an: &mut Analyzer, o: &Value, st: &mut State| {
            match o.single().and_then(|x| prop(an, x)) {
                Some(Some(deleter)) => {
                    an.call_user(deleter, Some(o.clone()), Args::pos(&[]), st, node, false);
                }
                Some(None) => an.raise_here("AttributeError", st),
                None => an.delete_item(o, Ns::Attr, &Value::lit(nk), node, st, true),
            }
            Value::new()
        };
        if self.per_receiver(obj, &|an, o| prop(an, o).is_some(), state, &mut one).is_none() {
            one(self, obj, state);
        }
    }

    /// A deque's `maxlen` and methods.
    fn deque_attr(&mut self, oid: ObjId, name: &str) -> Value {
        if name == "maxlen" {
            match self.m.deques[&oid] {
                Some(m) => Value::lit(intern::key_int(m)),
                None => Value::none(),
            }
        } else {
            self.method_value(&format!("deque.{}", name), &Value::obj(oid))
        }
    }

    /// An attribute of a Series accessor (`.str`, `.dt`, …): its methods; `.dt.components` a frame.
    fn accessor_attr(&mut self, oid: ObjId, name: &str, node: &Node, state: &mut State) -> Value {
        let Payload::Locidx(a, recv) = self.heap.obj(oid).payload.clone() else { unreachable!() };
        if a == "dt" && name == "components" {
            // a property: the timedelta parts as columns
            let ks: Vec<KeyId> = ["days", "hours", "minutes", "seconds", "milliseconds", "microseconds", "nanoseconds"].iter().map(|x| key_str_lit(x)).collect();
            let row = Row::closed_with(crate::intern::KeySet::from_iter(ks.iter().copied())).with_order(ks);
            let nid = self.alloc_at(Kind::Frame, node, state, &format!("frame@L{}", node.line), Payload::None, Some(row), Ns::Item, None, None);
            Value::obj(nid)
        } else {
            self.method_value(&format!("Series.{}.{}", a, name), &Value::obj(recv))
        }
    }

    /// An attribute of a view: a mapping proxy's read methods are its dict's (Err: joined
    /// beside the other objects' values, not one of them), a keys view's `.str`, else the view's
    /// methods.
    fn view_attr(&mut self, oid: ObjId, name: &str, node: &Node, state: &mut State, report: bool) -> Result<Value, Value> {
        // a proxy's read methods are the dict's (it has no mutators)
        if let Payload::View(ViewKind::Proxy, inner) = self.heap.obj(oid).payload.clone() {
            if matches!(name, "get" | "keys" | "values" | "items" | "copy") {
                return Err(self.method_value(&format!("dict.{}", name), &inner));
            } else {
                return Err(self.load_attr(&Value::data("proxy"), name, node, state, report));
            }
        }
        if let Payload::View(ViewKind::Keys, recv) = &self.heap.obj(oid).payload {
            if name == "str" {
                return Err(Value::kv_only(KeyVal::KeysOf(state.keys_of(recv.pts.clone(), Ns::Item))));
            }
        }
        Ok(self.method_value(&format!("view.{}", name), &Value::obj(oid)))
    }

    /// An attribute of a module: its global, what every module has, else missing (open when
    /// unknown code may have added names).
    #[allow(clippy::too_many_arguments)]
    fn module_attr(&mut self, oid: ObjId, name: &str, nk: KeyId, node: &Node, state: &mut State, caught: bool, guarded: bool, single: bool) -> Value {
        match state.slot_opt(oid, Ns::Attr, nk) {
            // (`from m import f` in a module's code binds what m holds, a deferred definition too)
            Some(v) if self.mode.keep_deferred => v,
            Some(v) => self.force_slot(oid, nk, v, state),
            // (what every module has)
            None if MODULE_NAMES.contains(&name) => {
                if name == "__name__" { Value::lit_str(&self.heap.module_info(oid).name) } else { Value::data(&format!("module {}", name)) }
            }
            // (inside a try that catches the AttributeError: the handler's way)
            None if caught => self.load_record(oid, Ns::Attr, &Value::lit(nk), node, state, guarded, single, true),
            None => {
                // (a namespace changed by code rowan cannot see — `globals().update(…)`
                // with unknown names — may have it)
                let row = state.row(oid, Ns::Attr);
                let cls = if row.closed() { Classification::with_note(Tier::Missing, "is not defined in the module") } else { classify(&row, nk) };
                if !guarded {
                    self.verdict(node, nk, &cls, oid, Ns::Attr, None, None);
                }
                // (an AttributeError; the path goes on, as after any read that fails)
                self.may_raise_here("AttributeError", state);
                Value::data(&format!("L{}", node.line))
            }
        }
    }

    /// An attribute of a library object rowan sees only by name: a class a model generates, a
    /// constant, a record, else the external object `qual.name`.
    fn external_attr(&mut self, oid: ObjId, name: &str, state: &mut State) -> Value {
        // (in a read on many objects, an unknown value of the read and name: named by their
        // access paths, attributes of attributes made names without end)
        if let Some((_, site)) = self.mode.wide_read.clone().filter(|_| self.in_wide_read()) {
            let ctx = self.frame().ctx.clone();
            let o = self.heap.alloc_raw(Kind::Unknown, &format!("ext@{}:{}", site, name), &ctx, false, &format!("?.{}", name), Payload::None);
            mark_merged(o);
            return Value::obj(o);
        }
        let q = format!("{}.{}", self.heap.external_qual(oid), name);
        if let Some(v) = crate::models::stdlib::generated_class(self, state, &q) {
            return v;
        }
        if let Some(v) = crate::models::external_constant(self, &q) {
            return v;
        }
        match crate::models::external_record(self, &q, state) {
            Some(rec) => Value::obj(rec),
            None => Value::obj(self.heap.alloc_raw(Kind::External, &format!("ext:{}", q), "", true, &q, Payload::Str(q.clone()))),
        }
    }

    /// An attribute of a dict: a Series', a config parser's or section's, a Counter's, a
    /// ChainMap's, a cookie jar's methods, else `dict`'s.
    fn dict_attr(&mut self, oid: ObjId, name: &str, node: &Node, state: &mut State) -> Value {
        self.sync_chainmaps(&Value::obj(oid), state);
        // (a ChainMap's changes go to its first map: read through it again later)
        if let Some(first) = self.m.chainmaps.get(&oid).and_then(|ms| ms.first()).cloned().filter(|_| matches!(name, "update" | "setdefault" | "pop" | "popitem" | "clear" | "__setitem__" | "__delitem__")) {
            return self.load_attr(&first, name, node, state, false);
        }
        if crate::models::pandas::is_series_obj(self, oid) {
            crate::models::pandas::series_getattr(self, oid, name, node, state)
        } else if self.m.cfg_parsers.contains_key(&oid) && crate::models::configparser::PARSER_METHODS.contains(&name) {
            self.method_value(&format!("cfgparser.{}", name), &Value::obj(oid))
        } else if self.m.cfg_sections.contains_key(&oid) && crate::models::configparser::SECTION_METHODS.contains(&name) {
            self.method_value(&format!("cfgsection.{}", name), &Value::obj(oid))
        } else if self.m.cfg_sections.contains_key(&oid) && name == "name" {
            Value::lit_str(&self.m.cfg_sections[&oid].1)
        } else if self.m.counters.contains(&oid) && crate::models::counter::COUNTER_METHODS.contains(&name) {
            self.method_value(&format!("counter.{}", name), &Value::obj(oid))
        } else if self.m.chainmaps.contains_key(&oid) && matches!(name, "maps" | "parents") {
            let c = super::Call { qual: format!("chainmap.{}", name), recv: Some(Value::obj(oid)), args: vec![], kwargs: IndexMap::new(), star_args: vec![], star_kwargs: vec![], node: *node, call_ast: None, site: format!("L{}", node.line), line: node.line };
            match self.program.models.lookup(&c.qual) {
                Some(m) => m(self, state, &c),
                None => Value::data("chainmap"),
            }
        } else if self.m.chainmaps.contains_key(&oid) && name == "new_child" {
            self.method_value("chainmap.new_child", &Value::obj(oid))
        } else if matches!(name, "set" | "delete") && self.m.cookie_jars.contains(&oid) {
            // a TestClient cookie jar's set / delete
            self.method_value(&format!("fastapi.cookies.{}", name), &Value::obj(oid))
        } else {
            self.method_value(&format!("dict.{}", name), &Value::obj(oid))
        }
    }

    /// `super().name` on a `super()` object: the method after the defining class in the
    /// receiver's MRO, bound to the receiver; `object`'s own (or a library base's) as modelled /
    /// unknown code given the receiver. None: not a `super()` object.
    fn super_attr(&mut self, oid: ObjId, name: &str, nk: KeyId) -> Option<Value> {
        let Payload::Super(cls, self_v) = self.heap.obj(oid).payload.clone() else { return None };
        let ci = self.heap.class_info(cls);
        let mut found: Option<u32> = None;
        // the receiver's own class decides the order: the classes after `cls` in
        // its MRO (one receiver class; otherwise the defining class's bases)
        let recv_cls: Vec<u32> = self_v
            .pts
            .iter()
            .filter_map(|o| match self.heap.kind(o) {
                Kind::Instance => self.heap.instance_class(o),
                Kind::Class => Some(o),
                _ => None,
            })
            .collect();
        if let [rc] = recv_cls[..] {
            let order = self.heap.mro(rc);
            if let Some(pos) = order.iter().position(|c| *c == cls) {
                found = order[pos + 1..].iter().copied().find(|c| self.heap.cls_slots.contains_key(&(*c, Ns::Attr, nk)));
            }
        }
        if found.is_none() {
            for b in &ci.bases {
                found = self.class_lookup(*b, name);
                if found.is_some() {
                    break;
                }
            }
        }
        // (a builtin base answers nothing here: its own methods are not user code)
        let found = found.filter(|fc| self.heap.kind(*fc) != Kind::External);
        let ids = self_v.pts.iter().map(|x| x.to_string()).collect::<Vec<String>>().join(",");
        Some(match found {
            // (`__new__` is a static method: `super().__new__(cls)` passes the class)
            Some(fc) if name == "__new__" => self.heap.cls_slots.get(&(fc, Ns::Attr, nk)).cloned().unwrap_or_default(),
            None if name == "__new__" => {
                let q = "builtins.object.__new__";
                Value::obj(self.heap.alloc_raw(Kind::External, &format!("ext:{}", q), "", true, q, Payload::Str(q.to_string())))
            }
            Some(fc) => {
                let fv = self.heap.cls_slots.get(&(fc, Ns::Attr, nk)).cloned().unwrap_or_default();
                let mut out = Value::new();
                for f in fv.pts.iter() {
                    if self.heap.kind(f) == Kind::Function {
                        let label = format!("{} via super", self.heap.obj(f).label);
                        let b = self.heap.alloc_raw(Kind::Bound, &format!("bound:{}:{}", f, ids), "", true, &label, Payload::Bound(f, self_v.clone()));
                        out = out.join(&Value::obj(b));
                    }
                }
                out
            }
            None if name == "__init__" && ci.bases.iter().any(|b| crate::models::dataclasses::dataclass_init_owner(self, *b).is_some()) => {
                let dc = ci.bases.iter().find_map(|b| crate::models::dataclasses::dataclass_init_owner(self, *b)).unwrap();
                Value::obj(self.heap.alloc_raw(Kind::Method, &format!("dcinit:{}:{}", dc, ids), "", true, "dataclass __init__ via super", Payload::Method(format!("dcinit.{}", dc), self_v.clone())))
            }
            // `super().__init__(msg)` in an Exception subclass: the instance's args
            None if name == "__init__" && self.builtin_exc_base(cls) => {
                Value::obj(self.heap.alloc_raw(Kind::Method, &format!("excinit:{}", ids), "", true, "Exception.__init__ via super", Payload::Method("excinit".to_string(), self_v.clone())))
            }
            // `object`'s own method (or a library base's), run on the receiver:
            // `object.__setattr__` as modelled, a library base's as unknown code
            // given the receiver (it was a call given nothing)
            None => {
                let closed = match recv_cls[..] {
                    [rc] => self.hierarchy_closed(rc),
                    _ => self.hierarchy_closed(cls),
                };
                let q = if closed { format!("builtins.object.{}", name) } else { format!("super.{}", name) };
                let f = self.heap.alloc_raw(Kind::External, &format!("ext:{}", q), "", true, &q, Payload::Str(q.clone()));
                let payload = Payload::Partial(Value::obj(f), vec![self_v.clone()], IndexMap::new());
                let b = self.heap.alloc_raw(Kind::Partial, &format!("super:{}:{}", q, ids), "", true, &format!("super().{}", name), payload.clone());
                self.heap.obj_mut(b).payload = payload;
                Value::obj(b)
            }
        })
    }

    /// Attribute `name` of the program's function `oid` (`single`: the read's only object): an
    /// `lru_cache`'s and singledispatch's methods, its names and defaults unless stored, else what
    /// the program stored on it; on paths it did not, an AttributeError — surely where it surely
    /// lacks it (a call that did not run gives back the functions its code may,
    /// `returned_aliases`).
    fn function_attr(&mut self, oid: ObjId, name: &str, nk: KeyId, single: bool, node: &Node, state: &mut State) -> Value {
        let unset = state.slot_opt(oid, Ns::Attr, nk).is_none();
        match name {
            // an lru_cache'd function's cache_info / cache_clear
            "cache_info" | "cache_clear" if self.heap.func_info(oid).is_memo.get() => return self.method_value(&format!("memo.{}", name), &Value::obj(oid)),
            // a function's names (a `@wraps(fn)` wrapper: fn's)
            "__annotations__" if unset => {
                let names = crate::models::stdlib::annotated_names(self, oid, false).unwrap_or_default();
                return crate::models::stdlib::names_dict(self, state, node, &names, None, "annotation");
            }
            "__name__" | "__qualname__" | "__module__" | "__wrapped__" | "__doc__" if unset => return self.function_name_attr(oid, name, node, state),
            // (its defaults: made at the first read, kept on it: what changes them reaches its calls)
            "__defaults__" | "__kwdefaults__" if unset => return self.function_defaults_attr(oid, name, node, state),
            // a singledispatch function's register
            "register" if self.m.sdispatch.contains_key(&oid) => return self.method_value("sdispatch.register", &Value::obj(oid)),
            _ => {}
        }
        let row = state.row(oid, Ns::Attr);
        match state.slot_opt(oid, Ns::Attr, nk) {
            // (a wrapper of an unknown, stored on some paths only: on the others what the
            // unknown has)
            Some(v) if row.open() && !row.likely.contains(nk) => v.join(&Value::data(&format!("L{}", node.line))),
            Some(v) => {
                if !row.definite.contains(nk) {
                    self.may_raise_here("AttributeError", state);
                }
                v
            }
            None if single && !name.starts_with("__") && self.plain_function(oid) && self.has_facts(oid, state) && row.lacks(nk) => {
                self.raise_here("AttributeError", state);
                Value::new()
            }
            None => {
                self.may_raise_here("AttributeError", state);
                Value::data(&format!("L{}", node.line))
            }
        }
    }

    /// Is `f` a plain function of the program, whose attributes are what the program gave it
    /// (not a property, static or class method, cached property, lru_cache, singledispatch or
    /// contextmanager function, nor one a decorator not modelled gave back: those may be
    /// objects with attributes of their own; nor one unknown code was given, `escaped_fns`)?
    /// What was stored on it (also through `__dict__`, `vars`, `setattr`) is all it has:
    /// `hasattr` decides, and a read of what it surely lacks raises.
    pub fn plain_function(&self, f: ObjId) -> bool {
        let fi = self.heap.func_info(f);
        !(fi.is_property.get() || fi.is_static.get() || fi.is_classmethod.get() || fi.is_cached.get() || fi.is_memo.get() || fi.is_contextmanager.get() || fi.opaque.get() || self.m.sdispatch.contains_key(&f) || self.heap.escaped_fns.contains(&f))
    }

    /// A function's `__name__` / `__qualname__` / `__module__` / `__wrapped__` / `__doc__` (a
    /// `@wraps(fn)` wrapper: fn's).
    fn function_name_attr(&mut self, oid: ObjId, name: &str, node: &Node, state: &mut State) -> Value {
        match self.m.wrapped.get(&oid).cloned() {
            Some(w) if name == "__wrapped__" => w,
            Some(w) => self.load_attr(&w, name, node, state, false),
            None => {
                let fi = self.heap.func_info(oid);
                match name {
                    "__name__" => Value::lit_str(fi.qualname.rsplit('.').next().unwrap_or(&fi.qualname)),
                    "__qualname__" => Value::lit_str(&fi.py_qualname()),
                    "__module__" => Value::lit_str(&self.heap.module_info(fi.module).name),
                    "__doc__" => Value::data("doc"),
                    _ => Value::data(name),
                }
            }
        }
    }

    /// A function's `__defaults__` (a tuple, None without any) or `__kwdefaults__` (a dict of
    /// its keyword-only parameters' defaults, None without any) before anything stored one:
    /// made from its defaults and stored on it, so that its calls take what is changed through
    /// it (`live_defaults`).
    fn function_defaults_attr(&mut self, oid: ObjId, name: &str, node: &Node, state: &mut State) -> Value {
        let fi = self.heap.func_info(oid);
        let Some(params) = fi.node.params() else { return Value::none() };
        let kw = name == "__kwdefaults__";
        let ps: Vec<&ast::ParameterWithDefault> = if kw { params.kwonlyargs.iter().collect() } else { params.posonlyargs.iter().chain(params.args.iter()).collect() };
        let (names, vals): (Vec<String>, Vec<Value>) = ps.iter().filter_map(|p| fi.defaults.get(&intern::s(p.parameter.name.id.as_str())).map(|v| (p.parameter.name.id.to_string(), v.clone()))).unzip();
        let v = if vals.is_empty() {
            Value::none()
        } else if kw {
            crate::models::stdlib::names_dict(self, state, node, &names, Some(vals), "kwdefaults")
        } else {
            let kv = vals.iter().map(|v| v.kv.clone()).reduce(|a, b| crate::domain::kv_join(&a, &b)).unwrap_or(crate::domain::DATA);
            self.alloc_positional_sfx(Kind::Tuple, node, ":defaults", Some(vals), Value::new(), kv, state)
        };
        self.store_attr(&Value::obj(oid), name, &v, node, state);
        v
    }

    /// An attribute of a class object: a class attribute (a classmethod bound to this class), an Enum
    /// member, a pydantic model's attribute or method, a dataclass `__init__`, else a method of the
    /// class object.
    #[allow(clippy::too_many_arguments)]
    fn class_attr(&mut self, oid: ObjId, name: &str, node: &Node, state: &mut State, guarded: bool, report: bool, single: bool) -> Value {
        if self.class_lookup(oid, name).is_some() {
            // (one no class of the MRO holds on this path — deleted, or stored on other paths
            // only — raises; held on some paths: may; not a read with a default, `getattr`;
            // one of several objects read: may)
            let (defs, sure) = self.class_lookup_on_path(oid, name, state);
            if defs.is_empty() {
                if report && single {
                    self.raise_here("AttributeError", state);
                } else if report {
                    self.may_raise_here("AttributeError", state);
                }
                return Value::new();
            }
            if !sure && report {
                self.may_raise_here("AttributeError", state);
            }
            // a classmethod reached through a class binds that class (not the
            // class that defines it): `Sub.make()` constructs `Sub`
            let v = self.class_attr_value(oid, name, state);
            let mut out = Value::new();
            let mut changed = false;
            for f in v.pts.iter().collect::<Vec<_>>() {
                if self.heap.kind(f) == Kind::Function && self.heap.func_info(f).is_classmethod.get() {
                    let label = format!("{} bound", self.heap.obj(f).label);
                    out = out.join(&Value::obj(self.heap.alloc_raw(Kind::Bound, &format!("bound:{}:cls{}", f, oid), "", true, &label, Payload::Bound(f, Value::obj(oid)))));
                    changed = true;
                } else if self.is_descriptor(f, "__get__") {
                    // (a descriptor read through the class: what `__get__(None, cls)` returns)
                    let r = self.call_method_with(&Value::obj(f), "__get__", vec![Value::none(), Value::obj(oid)], node, state);
                    out = out.join(&r.unwrap_or_else(|| Value::data(&format!("L{}", node.line))));
                    changed = true;
                } else {
                    out = out.join(&Value::obj(f));
                }
            }
            if changed { out.join(&Value::kv_only(v.kv.clone())) } else { v }
        } else if self.m.enum_members.contains_key(&oid) && (name == "__members__" || (!name.starts_with("__") && !self.heap.class_info(oid).open_base.get())) {
            crate::models::enums::class_attr(self, oid, name, node, guarded, state)
        } else if crate::models::pydantic::PYD_ATTRS.contains(&name) && crate::models::pydantic::is_model(self, oid) {
            crate::models::pydantic::attr_value(self, state, oid, name, node)
        } else if crate::models::pydantic::PYD_METHODS.contains(&name) && crate::models::pydantic::is_model(self, oid) {
            self.method_value(&format!("pydantic.cls.{}", name), &Value::obj(oid))
        } else if name == "__init__" && crate::models::dataclasses::dataclass_init_owner(self, oid).is_some() {
            let dc = crate::models::dataclasses::dataclass_init_owner(self, oid).unwrap();
            Value::obj(self.heap.alloc_raw(Kind::Method, &format!("dcinit:{}", oid), "", true, &format!("{}.__init__", self.heap.obj(oid).label), Payload::Method(format!("dcinit.class.{}", dc), Value::obj(oid))))
        } else {
            let label = format!("{}.{}", self.heap.obj(oid).label, name);
            Value::obj(self.heap.alloc_raw(Kind::Method, &format!("classattr:{}.{}", oid, name), "", true, &label, Payload::Method(format!("class.{}", name), Value::obj(oid))))
        }
    }

    /// The library method `qual` (`dict.get`) bound to `recv`: one object per method and
    /// receiver.
    pub fn method_value(&mut self, qual: &str, recv: &Value) -> Value {
        // (a receiver of many objects is keyed by a hash of their ids, not by the list)
        let ids: String = if recv.pts.len() > 32 {
            use std::hash::BuildHasher;
            let v = recv.pts.to_vec();
            format!("h{:x}.{:x}.{}.{}.{}", rustc_hash::FxBuildHasher.hash_one(&v), rustc_hash::FxBuildHasher.hash_one((&v, 0x9e37u16)), v.len(), v[0], v[v.len() - 1])
        } else {
            recv.pts.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",")
        };
        let key = format!("method:{}:{}:{}", qual, ids, recv.kv);
        Value::obj(self.heap.alloc_raw(Kind::Method, &key, "", true, qual, Payload::Method(qual.to_string(), recv.clone())))
    }

    /// The class of `cls`'s MRO that defines `name`, if any.
    pub fn class_lookup(&self, cls: u32, name: &str) -> Option<u32> {
        let nk = key_str_lit(name);
        self.heap.mro(cls).into_iter().find(|c| self.heap.cls_slots.contains_key(&(*c, Ns::Attr, nk)))
    }

    /// Is the value class `c` holds as `k` plain data (no function, descriptor or property)?
    pub(in crate::interp) fn plain_class_attr(&self, c: u32, k: KeyId, state: &State) -> bool {
        !self.class_slot(c, k, state).pts.iter().any(|f| matches!(self.heap.kind(f), Kind::Function | Kind::Bound | Kind::Instance | Kind::External | Kind::Unknown))
    }

    /// The classes of `cls`'s MRO that may hold `name` on this path, up to the first that surely
    /// does (its body bound it, or a store on every path), and whether one surely does: a class
    /// attribute stored on some paths only (`if c: Cls.x = 1`) may be missing.
    pub fn class_lookup_on_path(&self, cls: u32, name: &str, state: &State) -> (Vec<u32>, bool) {
        let nk = key_str_lit(name);
        let mut out = Vec::new();
        for c in self.heap.mro(cls) {
            if !self.heap.cls_slots.contains_key(&(c, Ns::Attr, nk)) {
                continue;
            }
            let row = state.row(c, Ns::Attr);
            // (made with the class — its body, a model — it is there; or there unless unknown
            // code removed it: as optimistic as a `likely` read)
            if !self.heap.cls_late.contains(&(c, nk)) || !state.has_row(c, Ns::Attr) || row.definite.contains(nk) || row.likely.contains(nk) {
                out.push(c);
                return (out, true);
            }
            if row.present.contains(nk) || row.open() {
                out.push(c);
            }
        }
        (out, false)
    }

    /// A class attribute's value: on this path when the path has it (a class attribute
    /// rebound after the class was made: `Cls.mode = "x"`), else what any path stored.
    /// (One stored on some paths only over a base class's: either.)
    pub(in crate::interp) fn class_attr_value(&self, cls: u32, name: &str, state: &State) -> Value {
        let nk = key_str_lit(name);
        let (mut defs, _) = self.class_lookup_on_path(cls, name, state);
        if defs.is_empty() {
            defs.extend(self.class_lookup(cls, name));
        }
        self.class_values(&defs, nk, state)
    }

    /// What reading a class attribute `cv` through instance `oid` gives: a descriptor's `__get__`
    /// result, a method bound to the instance (a property run, a classmethod bound to the class,
    /// a staticmethod as it is), else the value.
    pub(super) fn bound_outs(&mut self, cv: &Value, oid: u32, cls: u32, name: &str, node: &Node, state: &mut State) -> Value {
        let mut outs: Vec<Value> = Vec::new();
        // a descriptor object in the class (its class has `__get__`): what `__get__` returns
        let descs: Vec<u32> = cv.pts.iter().filter(|d| self.heap.kind(*d) == Kind::Instance && self.heap.instance_class(*d).and_then(|dc| self.class_lookup(dc, "__get__")).is_some()).collect();
        if !descs.is_empty() && descs.len() == cv.pts.len() {
            for d in descs {
                if let Some(r) = self.call_method_with(&Value::obj(d), "__get__", vec![Value::obj(oid), Value::obj(cls)], node, state) {
                    outs.push(r);
                }
            }
            return join_values(outs);
        }
        for f in cv.pts.iter() {
            if self.heap.kind(f) == Kind::Function {
                let fi = self.heap.func_info(f);
                if fi.is_property.get() {
                    let r = self.call_user(f, Some(Value::obj(oid)), Args::pos(&[]), state, node, false);
                    if fi.is_cached.get() && matches!(self.heap.kind(oid), Kind::Instance) {
                        // stored on the instance: later accesses find this value (maybe, where the
                        // class may hold something else under the name)
                        let only = cv.kv.is_bot() && cv.pts.len() == 1;
                        let strong = self.heap.obj(oid).single && only;
                        self.store_key(oid, Ns::Attr, &Value::lit(key_str_lit(name)), &r, strong, node, state);
                    }
                    outs.push(r);
                } else if fi.is_static.get() {
                    outs.push(Value::obj(f));
                } else if fi.is_classmethod.get() {
                    let label = format!("{} bound", self.heap.obj(f).label);
                    outs.push(Value::obj(self.heap.alloc_raw(Kind::Bound, &format!("bound:{}:cls{}", f, cls), "", true, &label, Payload::Bound(f, Value::obj(cls)))));
                } else {
                    let label = format!("{} bound to {}", self.heap.obj(f).label, self.heap.obj(oid).label);
                    outs.push(Value::obj(self.heap.alloc_raw(Kind::Bound, &format!("bound:{}:{}", f, oid), "", true, &label, Payload::Bound(f, Value::obj(oid)))));
                }
            } else if self.is_descriptor(f, "__get__") {
                let r = self.call_method_with(&Value::obj(f), "__get__", vec![Value::obj(oid), Value::obj(cls)], node, state);
                outs.push(r.unwrap_or_else(|| Value::data(&format!("L{}", node.line))));
            } else {
                outs.push(Value::obj(f));
            }
        }
        // (and what is no object: literals beside the objects too)
        if cv.pts.is_empty() || !cv.kv.is_bot() {
            outs.push(Value::kv_only(cv.kv.clone()));
        }
        join_values(outs)
    }

    /// What classes `defs` hold as `nk` (on this path, else on any).
    pub(in crate::interp) fn class_values(&self, defs: &[u32], nk: KeyId, state: &State) -> Value {
        join_values(defs.iter().map(|d| self.class_slot(*d, nk, state)))
    }

    /// What class `c` holds as `k`: on this path when the path has it, else what any path stored.
    pub(in crate::interp) fn class_slot(&self, c: ObjId, k: KeyId, state: &State) -> Value {
        state.slot_opt(c, Ns::Attr, k).or_else(|| self.heap.cls_slots.get(&(c, Ns::Attr, k)).cloned()).unwrap_or_default()
    }

    /// An attribute read's verdict, guarded or not.
    fn attr_verdict(&mut self, node: &Node, nk: KeyId, cls: &Classification, oid: u32, guarded: bool) {
        if guarded {
            self.verdict_guarded(node, nk, cls, oid, Ns::Attr);
        } else {
            self.verdict(node, nk, cls, oid, Ns::Attr, None, None);
        }
    }

    /// An instance's attribute its class `c` holds (it does not itself on this path): a class
    /// attribute the read finds (it succeeds; a method read counts only where the receiver may
    /// be other objects too, which may lack it; one stored on some paths only may fail). None:
    /// no class of its MRO has it.
    fn instance_class_attr(&mut self, oid: u32, c: u32, name: &str, row: &Row, node: &Node, state: &mut State, (guarded, try_guard, single_target): (bool, bool, bool)) -> Option<Value> {
        let nk = key_str_lit(name);
        let is_field = crate::models::dataclasses::dataclass_fields_mro(self, c).iter().any(|(f, _)| f == name);
        let (defs, sure) = self.class_lookup_on_path(c, name, state);
        if defs.is_empty() || is_field {
            return None;
        }
        let cv = self.class_values(&defs, nk, state);
        let plain = !cv.pts.iter().any(|f| matches!(self.heap.kind(f), Kind::Function | Kind::Bound) && !(self.heap.kind(f) == Kind::Function && self.heap.func_info(f).is_property.get()));
        if plain || !single_target {
            self.attr_verdict(node, nk, &Classification::new(if sure { Tier::Proven } else { Tier::Possibly }), oid, guarded);
        }
        if !sure && try_guard {
            self.may_raise_here("AttributeError", state);
        }
        let out = self.bound_outs(&cv, oid, c, name, node, state);
        // (an instance of a class of the program unknown code changed: it may hold the data
        // name itself, over the class's — a scalar or an object, not a method or a class:
        // rebinding those is not modelled)
        let code = cv.pts.iter().any(|f| matches!(self.heap.kind(f), Kind::Function | Kind::Bound | Kind::Method | Kind::Partial | Kind::Class));
        if !code && !row.mutators.is_empty() && !row.absent.contains(nk) && self.hierarchy_closed(c) {
            return Some(out.join(&Value::data(&format!("L{}", node.line))));
        }
        // (stored under a name not known, `setattr(o, name, v)`: the instance may hold it, over the
        // class's)
        if let Some(star) = self.data_stored(oid, nk, row, state).filter(|_| !code) {
            return Some(out.join(&star));
        }
        Some(out)
    }

    /// Instance `oid`'s `name` where classes `defs` of its class `c` hold it as a
    /// `functools.cached_property` and it holds `cached` on some paths only: there that, on the
    /// others the getter runs and caches it. The read cannot fail. (None: no such property.)
    #[allow(clippy::too_many_arguments)]
    fn cached_on_some_paths(&mut self, oid: u32, c: u32, defs: &[u32], name: &str, cached: &Value, node: &Node, state: &mut State, guarded: bool) -> Option<Value> {
        let nk = key_str_lit(name);
        let cv = self.class_values(defs, nk, state);
        if cv.pts.is_empty() || !cv.pts.iter().all(|f| self.heap.kind(f) == Kind::Function && self.heap.func_info(f).is_cached.get()) {
            return None;
        }
        let mut fresh = state.copy();
        self.narrow_record(&Value::obj(oid), Ns::Attr, nk, false, &mut fresh);
        let got = if fresh.reachable { self.bound_outs(&cv, oid, c, name, node, &mut fresh) } else { Value::new() };
        let row = state.row(oid, Ns::Attr).narrow_in(nk, self.heap.obj(oid).single);
        state.set_row_narrowed(oid, Ns::Attr, row);
        let joined = state.join(&mut fresh);
        state.replace_with(joined);
        if !guarded {
            self.verdict(node, nk, &Classification::new(Tier::Proven), oid, Ns::Attr, None, None);
        }
        Some(cached.join(&got))
    }

    /// A read of attribute `name` of an instance of the program's classes: a library base's
    /// attribute, `__getattr__`, the instance's own slot, else the class's (`bound_outs`), with
    /// the verdict of its row (a class attribute is there for every instance).
    pub(in crate::interp) fn load_instance_attr(&mut self, oid: u32, name: &str, node: &Node, state: &mut State, guarded: bool, try_guard: bool, single_target: bool) -> Value {
        let nk = key_str_lit(name);
        if let Some(v) = self.instance_library_attr(oid, name, nk, node, state, guarded) {
            return v;
        }
        if let Some(v) = self.instance_getattr(oid, name, nk, node, state) {
            // (found through `__getattr__`: on this object the read succeeds where it returned;
            // beside other objects that lack the name, it may fail)
            if state.reachable && (!single_target || self.mode.one_of_several) {
                self.attr_verdict(node, nk, &Classification::new(Tier::Proven), oid, guarded);
            }
            return v;
        }
        let v = state.slot_opt(oid, Ns::Attr, nk);
        let row = self.read_row(oid, Ns::Attr, state);
        let cls_of = self.heap.instance_class(oid);
        // (a property is a data descriptor: it takes the read over the instance's own value;
        // where the class holds it on some paths only, both)
        let is_prop = |an: &Analyzer, f: u32| (an.heap.kind(f) == Kind::Function && an.heap.func_info(f).is_property.get() && !an.heap.func_info(f).is_cached.get()) || an.is_descriptor(f, "__set__") || an.is_descriptor(f, "__delete__");
        if let (Some(c), Some(own)) = (cls_of.filter(|_| self.heap.prop_names.contains(&nk)), &v) {
            let (defs, sure) = self.class_lookup_on_path(c, name, state);
            let cv = self.class_values(&defs, nk, state);
            if cv.pts.iter().any(|f| is_prop(self, f)) {
                let surely = sure && cv.kv.is_bot() && cv.pts.iter().all(|f| is_prop(self, f));
                let got = self.instance_class_attr(oid, c, name, &row, node, state, (guarded, try_guard, single_target)).unwrap_or_default();
                return if surely { got } else { got.join(own) };
            }
        }
        if let Some(mut v) = v {
            // (stored on some paths only, on an object unknown code changed: on the others it
            // holds what that code put there — of a class of the program's only: a library
            // base's own code is taken not to set the program's names, `nn.Module`'s)
            if !row.mutators.is_empty() && !row.definite.contains(nk) && cls_of.is_some_and(|c| self.hierarchy_closed(c)) {
                v = v.join(&self.path_placeholder_value(oid, Ns::Attr, nk, &row, state));
            }
            // (on the paths that did not store it, a store under a name not known may have)
            if let Some(star) = self.data_stored(oid, nk, &row, state) {
                v = v.join(&star);
            }
            // (set on some paths only, over a class attribute of the name: the others read the
            // class's — the read cannot fail where the class surely has it)
            let class_attr = cls_of.filter(|c| !row.definite.contains(nk) && !crate::models::dataclasses::dataclass_fields_mro(self, *c).iter().any(|(f, _)| f == name)).map(|c| self.class_lookup_on_path(c, name, state));
            if let (Some(c), Some((defs, _)), true) = (cls_of, &class_attr, row.present.contains(nk)) {
                if let Some(r) = self.cached_on_some_paths(oid, c, defs, name, &v, node, state, guarded) {
                    return r;
                }
            }
            let mut class_v: Option<Value> = None;
            if let (true, Some((defs, sure))) = (row.present.contains(nk), class_attr) {
                let cv = self.class_values(&defs, nk, state);
                let plain = !cv.pts.iter().any(|f| matches!(self.heap.kind(f), Kind::Function | Kind::Bound) || self.is_descriptor(f, "__get__"));
                if plain && sure {
                    if !guarded {
                        self.verdict(node, nk, &Classification::new(Tier::Proven), oid, Ns::Attr, None, None);
                    }
                    return v.join(&cv);
                }
                class_v = (plain && !defs.is_empty()).then_some(cv.clone());
                // (a method, a descriptor's or a cached property's: what reading it through the
                // instance gives — bound, `__get__`'s, the getter's —, on the paths without the
                // own value)
                if let (Some(c), true) = (cls_of, !defs.is_empty() && !plain) {
                    let mut through = state.copy();
                    let got = self.bound_outs(&cv, oid, c, name, node, &mut through);
                    *state = state.join(&mut through);
                    class_v = Some(got);
                }
            }
            if let Some(cv) = class_v {
                v = v.join(&cv);
            }
            if row.present.contains(nk) {
                let cls = classify(&row, nk);
                self.attr_verdict(node, nk, &cls, oid, guarded);
                // (there on some paths only: an AttributeError the enclosing try handles)
                if try_guard && !matches!(cls.tier, Tier::Proven | Tier::Likely) && !self.class_resolves_on_path(oid, name, state) {
                    self.may_raise_here("AttributeError", state);
                }
                if let Some(c) = cls_of {
                    if self.field_is_any(c, name) {
                        let label = format!("{}.{}", self.heap.obj(oid).label, name);
                        v = v.join(&self.dynamic_value(&label, state));
                    }
                }
                return v;
            }
        }
        if let Some(v) = cls_of.and_then(|c| self.instance_class_attr(oid, c, name, &row, node, state, (guarded, try_guard, single_target))) {
            return v;
        }
        let cls = classify(&row, nk);
        if !guarded {
            self.verdict(node, nk, &cls, oid, Ns::Attr, None, None);
            if cls.tier == Tier::Input {
                self.require_providers_k(&cls, &row, false, state, Some(nk), Ns::Attr);
            }
            // (a catch-all of a caller's handles it)
            if try_guard && !matches!(cls.tier, Tier::Proven | Tier::Likely) {
                self.may_raise_here("AttributeError", state);
            }
            if cls.tier != Tier::Missing && single_target && self.heap.obj(oid).single {
                state.set_row_narrowed(oid, Ns::Attr, row.narrow_in(nk, true));
                self.propagate_narrowing(oid, Ns::Attr, nk, state);
            }
        } else {
            self.verdict_guarded(node, nk, &cls, oid, Ns::Attr);
            // (a read that cannot fail, `getattr(o, k, None)`, raises nothing; a try-guarded one may)
            if try_guard && !matches!(cls.tier, Tier::Proven | Tier::Likely) {
                // an AttributeError the enclosing try handles
                if cls.tier == Tier::Missing && single_target && !self.class_resolves(oid, name) {
                    self.raise_here("AttributeError", state);
                } else {
                    self.may_raise_here("AttributeError", state);
                }
            }
        }
        if row.open() {
            return self.path_placeholder_value(oid, Ns::Attr, nk, &row, state);
        }
        Value::data(&format!("L{}", node.line))
    }

    /// What a library makes of an instance's attribute before its own attributes: pydantic's
    /// private attributes and API, `object`'s attributes, a Thread subclass's `start` /
    /// `join`, a NamedTuple's methods.
    fn instance_library_attr(&mut self, oid: ObjId, name: &str, nk: KeyId, node: &Node, state: &mut State, guarded: bool) -> Option<Value> {
        // a pydantic private attribute: its hidden slot
        if let Some(cls) = self.heap.instance_class(oid) {
            if name.starts_with('_') && crate::models::pydantic::pyd_is_private(self, cls, name, state) {
                if !guarded {
                    self.verdict(node, nk, &Classification::new(Tier::Proven), oid, Ns::Attr, None, None);
                }
                return Some(state.slot(oid, Ns::Attr, crate::models::pydantic::priv_key(name)));
            }
            if name == "__pydantic_private__" && crate::models::pydantic::is_model(self, cls) {
                let pc = crate::models::pydantic::pyd_class(self, cls, state);
                let did = self.alloc_at(Kind::Dict, node, state, &format!("private@L{}", node.line), Payload::None, Some(Row::new()), Ns::Item, None, Some(&format!("priv{}", oid)));
                let mut row = Row::new();
                for (pn, _, _) in pc.privates.iter() {
                    let k = key_str_lit(pn);
                    row = row.store(k, true);
                    let v = state.slot(oid, Ns::Attr, crate::models::pydantic::priv_key(pn));
                    state.set_slot(did, Ns::Item, k, v, true);
                }
                state.set_row(did, Ns::Item, row);
                return Some(Value::obj(did));
            }
        }
        // what every instance has from `object` and its class (`__module__`, `__repr__`, …),
        // when neither the class nor the instance defines it
        if OBJECT_ATTRS.contains(&name) && name != "__dict__" && name != "__class__" {
            if let Some(cls) = self.heap.instance_class(oid) {
                if self.class_lookup(cls, name).is_none() && state.slot_opt(oid, Ns::Attr, nk).is_none() {
                    if !guarded {
                        self.verdict(node, nk, &Classification::new(Tier::Proven), oid, Ns::Attr, None, None);
                    }
                    return Some(match name {
                        "__module__" | "__doc__" | "__weakref__" => Value::data(&format!("{}@L{}", name, node.line)),
                        _ => self.method_value(&format!("object.{}", name), &Value::obj(oid)),
                    });
                }
            }
        }
        // pydantic's API on a model instance (`model_dump`, `model_copy`, …)
        if crate::models::pydantic::PYD_METHODS.contains(&name) {
            if let Some(cls) = self.heap.instance_class(oid) {
                if self.class_lookup(cls, name).is_none() && crate::models::pydantic::is_model(self, cls) && !state.row(oid, Ns::Attr).present.contains(nk) {
                    if crate::models::pydantic::PYD_ATTRS.contains(&name) {
                        return Some(crate::models::pydantic::attr_value(self, state, oid, name, node));
                    }
                    return Some(self.method_value(&format!("pydantic.{}", name), &Value::obj(oid)));
                }
            }
        }
        // a `threading.Thread` subclass's `start()` runs its `run()`
        if matches!(name, "start" | "join") {
            if let Some(cls) = self.heap.instance_class(oid).filter(|c| self.class_lookup(*c, name).is_none()) {
                if self.heap.mro(cls).iter().any(|c| self.heap.class_info_opt(*c).is_some_and(|ci| ci.lib.quals.iter().any(|q| q == "threading.Thread"))) {
                    return Some(self.method_value(&format!("threadsub.{}", name), &Value::obj(oid)));
                }
            }
        }
        // the methods every NamedTuple has
        if matches!(name, "_asdict" | "_replace" | "_fields" | "index" | "count") {
            if let Some(cls) = self.heap.instance_class(oid) {
                if self.class_lookup(cls, name).is_none() && crate::models::dataclasses::is_named_tuple_class(self, cls) {
                    if name == "_fields" {
                        let fs: Vec<Value> = crate::models::dataclasses::dataclass_fields_mro(self, cls).iter().map(|(f, _)| Value::lit(key_str_lit(f))).collect();
                        let kv = KeyVal::lits(fs.iter().filter_map(|v| v.kv.as_lit()).collect());
                        return Some(self.alloc_positional(Kind::Tuple, node, Some(fs), Value::new(), kv, state));
                    }
                    return Some(self.method_value(&format!("namedtuple.{}", name), &Value::obj(oid)));
                }
            }
        }
        None
    }

    /// The class's `__getattr__`, for a name normal lookup does not find (on the paths the
    /// instance lacks it).
    fn instance_getattr(&mut self, oid: ObjId, name: &str, nk: KeyId, node: &Node, state: &mut State) -> Option<Value> {
        // `__getattr__` answers what normal lookup does not find
        if let Some(cls) = self.heap.instance_class(oid) {
            if !name.starts_with("__") && self.class_lookup(cls, name).is_none() && self.class_lookup(cls, "__getattr__").is_some() {
                let row = state.row(oid, Ns::Attr);
                if !row.definite.contains(nk) {
                    let by_data = self.data_stored(oid, nk, &row, state);
                    // (a row unknown code may have added the name to: there or not)
                    let opened = !row.present.contains(nk) && !row.closed() && !row.absent.contains(nk);
                    if !row.present.contains(nk) && by_data.is_none() && !opened {
                        return Some(self.call_method_with(&Value::obj(oid), "__getattr__", vec![Value::lit(nk)], node, state).unwrap_or_default());
                    }
                    let by_data = if opened && (by_data.is_none() || !row.mutators.is_empty()) { Some(by_data.unwrap_or_default().join(&self.path_placeholder_value(oid, Ns::Attr, nk, &row, state))) } else { by_data };
                    // (there unless unknown code removed it: on the path where it did, what
                    // `__getattr__` does follows from that code's doing — a call that does not
                    // run, its result unknown, as any attribute that code may have replaced; it
                    // ran there for every read, nested in its own read of what it delegates to)
                    if row.likely.contains(nk) && by_data.is_none() {
                        let own = state.slot(oid, Ns::Attr, nk);
                        let mut fallback = Value::new();
                        for (o, f) in self.method_calls(&Value::obj(oid), &["__getattr__"]).0 {
                            let q = self.heap.func_info(f).qualname.clone();
                            fallback = fallback.join(&self.skipped_call(f, &q, Some(&Value::obj(o)), Args::pos(&[Value::lit(nk)]), state, node));
                        }
                        return Some(own.join(&fallback));
                    }
                    // (set on some paths: `__getattr__` runs on the others only — when it raises
                    // there, the paths that have the attribute go on)
                    let mut missing = state.copy();
                    let fallback = self.call_method_with(&Value::obj(oid), "__getattr__", vec![Value::lit(nk)], node, &mut missing).unwrap_or_default();
                    let own = state.slot(oid, Ns::Attr, nk).join(&by_data.unwrap_or_default());
                    if missing.reachable {
                        let j = state.join(&mut missing);
                        state.replace_with(j);
                    }
                    return Some(own.join(&fallback));
                }
            }
        }
        None
    }

    /// What stores under names not known (`setattr(o, name, v)`) may have put under `nk` of an
    /// instance that does not surely have it: the `*` slot.
    fn data_stored(&self, oid: ObjId, nk: KeyId, row: &Row, state: &State) -> Option<Value> {
        if row.definite.contains(nk) || row.absent.contains(nk) || !row.sources.iter().any(|(s, _)| crate::source::source_is_data(s)) {
            return None;
        }
        state.slot_opt(oid, Ns::Attr, crate::intern::STAR)
    }

    /// A value declared dynamically typed (`Any`): accesses on it are not checkable.
    pub(in crate::interp) fn dynamic_value(&mut self, label: &str, state: &mut State) -> Value {
        let oid = self.heap.alloc_raw(Kind::Unknown, &format!("any:{}", label), "", true, &format!("Any {}", label), Payload::None);
        crate::state::mark_found(oid, Ns::Item, &Row::top());
        crate::state::mark_found(oid, Ns::Attr, &Row::top());
        if !state.has_row(oid, Ns::Item) {
            state.set_row(oid, Ns::Item, Row::top());
            state.set_row(oid, Ns::Attr, Row::top());
        }
        Value::obj(oid)
    }

    /// Is the attribute declared `Any` (or `object`) in the class body or its bases?
    pub(in crate::interp) fn field_is_any(&self, cls: u32, name: &str) -> bool {
        for c in self.heap.mro(cls) {
            let Some(ci) = self.heap.class_info_opt(c) else { continue };
            let mut cache = ci.any_fields.borrow_mut();
            if cache.is_none() {
                let mut anns: rustc_hash::FxHashSet<String> = rustc_hash::FxHashSet::default();
                for st in &ci.node.body {
                    if let ast::Stmt::AnnAssign(a) = st {
                        if let ast::Expr::Name(t) = &*a.target {
                            let txt = match &*a.annotation {
                                ast::Expr::StringLiteral(sl) => sl.value.to_str().to_string(),
                                other => frontend::text(other),
                            };
                            let txt = txt.trim();
                            if matches!(txt, "Any" | "typing.Any" | "object" | "t.Any") {
                                anns.insert(t.id.to_string());
                            }
                        }
                    }
                }
                *cache = Some(anns);
            }
            if cache.as_ref().unwrap().contains(name) {
                return true;
            }
        }
        false
    }

    /// Does the value have a scalar part besides its objects (other than None, which has no
    /// keys or attributes of its own)?
    pub(in crate::interp) fn may_be_scalar(&self, v: &Value) -> bool {
        // (an unknown object stands for any value, scalars too: its scalar part adds nothing)
        !v.pts.is_empty() && !v.kv.is_bot() && v.kv.as_lit() != Some(intern::key_none()) && v.pts.iter().any(|o| self.heap.kind(o) != Kind::Unknown)
    }

    /// The row a read classifies against. Of an object this path has no facts about at all
    /// (collected, or made on other paths only: a recursion's deeper activations make what its
    /// widened arguments hold), nothing is known: an empty, closed row would claim every key
    /// missing.
    pub(in crate::interp) fn read_row(&self, oid: u32, ns: Ns, state: &State) -> Row {
        if self.has_facts(oid, state) { state.row(oid, ns) } else { Row::top() }
    }

    /// An unknown object's row on this path; none: anything (`top`).
    pub(in crate::interp) fn unknown_row(&self, oid: u32, ns: Ns, state: &State) -> Row {
        if state.has_row(oid, ns) {
            state.row(oid, ns)
        } else {
            Row::top()
        }
    }

    /// A read of attribute `name` of an object rowan cannot see: a dict or str method name
    /// gives that method (bound to it), another method name an unknown method; else what its
    /// row holds (a placeholder for an input's attribute).
    pub(in crate::interp) fn load_unknown_attr(&mut self, oid: u32, name: &str, node: &Node, state: &mut State, guarded: bool, raises: bool) -> Value {
        // (its class, as `type(x)`: calling it does not give it the object, so not a method of it)
        if name == "__class__" {
            return Value::data(&format!("type@L{}", node.line));
        }
        let nk = key_str_lit(name);
        // (a dict's or a scalar's method: the object may be of another type, an AttributeError
        // a running try sees; a str's methods of one annotated `str` are there)
        let opaque = self.heap.obj(oid).opaque;
        let method = !opaque && (DICT_METHOD_NAMES.contains(&name) || is_scalar_method(name));
        if raises && method && !(STR_METHOD_NAMES.contains(&name) && self.heap.obj(oid).ty == Some("str")) {
            self.may_raise_here("AttributeError", state);
        }
        // (a list or set annotated for any call: what its methods put into it, `put_items`)
        if matches!(self.heap.obj(oid).ty, Some("list" | "set")) && self.heap.ann.contains_key(&oid) && matches!(name, "append" | "add" | "insert" | "extend" | "update" | "pop") {
            self.kill_facts_on(&[oid], state);
            return self.method_value(&format!("annotated.{}", name), &Value::obj(oid));
        }
        // (one holding attributes of its own is no dict: its method is its own, given it)
        if DICT_METHOD_NAMES.contains(&name) && !opaque && !state.obj_slots(oid, Ns::Attr).is_empty() {
            return self.method_value(&format!("unknown.{}", name), &Value::obj(oid));
        }
        if DICT_METHOD_NAMES.contains(&name) && !opaque {
            return self.method_value(&format!("dict.{}", name), &Value::obj(oid));
        }
        if is_scalar_method(name) && !opaque {
            // a mutating method of a list-like unknown: facts about tests on it are void
            if matches!(name, "append" | "extend" | "insert" | "remove" | "clear" | "sort" | "reverse" | "pop") {
                self.kill_facts_on(&[oid], state);
            }
            // (one that moves the items it has, or any object's own method of the name: given it)
            if matches!(name, "sort" | "reverse" | "remove" | "insert") {
                return self.method_value(&format!("unknown.{}", name), &Value::obj(oid));
            }
            // (a str's method on one known to be a str or bytes; of another unknown — a date, an
            // object of the program's — its result's type is not known)
            let q = if STR_METHOD_NAMES.contains(&name) && matches!(self.heap.obj(oid).ty, Some("str" | "bytes")) { format!("str.{}", name) } else { format!("unknown.{}", name) };
            // (a parameter annotated `str`: a str, its methods' results known by type)
            let recv = match self.heap.obj(oid).ty {
                Some(t @ ("str" | "bytes")) => Value::data(&typed_label(t, "")),
                _ => Value::data(&format!("L{}", node.line)),
            };
            return self.method_value(&q, &recv);
        }
        let row = self.unknown_row(oid, Ns::Attr, state);
        if let Some(v) = state.slot_opt(oid, Ns::Attr, nk) {
            // (stored on some paths only: on the others it is what the object has under the
            // name, as for an open record's key, `literal_key_value`)
            // (one with an unknown part already — a weak store's, `store_attr` — covers it)
            let known = matches!(v.kv, KeyVal::Bot | KeyVal::Lit(_) | KeyVal::Lits(_)) && v.pts.iter().all(|o| self.heap.kind(o) != Kind::Unknown);
            if known && row.open() && !(row.likely.contains(nk) && row.stored.contains(nk)) {
                let v = v.join(&self.path_placeholder_value(oid, Ns::Attr, nk, &row, state));
                state.set_slot_found(oid, Ns::Attr, nk, v.clone(), false);
                return v;
            }
            return v;
        }
        let cls = classify(&row, nk);
        if guarded && cls.tier != Tier::Map {
            self.verdict_guarded(node, nk, &cls, oid, Ns::Attr);
        }
        if !guarded && cls.tier != Tier::Map {
            self.verdict(node, nk, &cls, oid, Ns::Attr, None, None);
            // (a method called on an input — `df.groupby(…)` — is not a key the input must have)
            if cls.tier == Tier::Input && !self.mode.callee_nodes.contains(&node.addr) {
                self.require_providers_k(&cls, &row, false, state, Some(nk), Ns::Attr);
            }
        }
        // (an object that may lack it: an AttributeError a running try sees)
        if raises && !matches!(cls.tier, Tier::Proven | Tier::Likely) {
            self.may_raise_here("AttributeError", state);
        }
        let ph = self.path_placeholder_value(oid, Ns::Attr, nk, &row, state);
        if !ph.pts.is_empty() {
            return ph;
        }
        self.method_value(&format!("unknown.{}", name), &Value::obj(oid))
    }

    /// A store `obj.name = v`: per receiver when one runs code for it (`__setattr__`, a
    /// property's setter, a descriptor's `__set__`), else into each object's attributes (a frozen
    /// dataclass raises; `__slots__` limit the names).
    pub fn store_attr(&mut self, obj: &Value, name: &str, v: &Value, node: &Node, state: &mut State) {
        let split = self.per_receiver(obj, &|an, o| an.attr_runs_user_code(o, name, true), state, &mut |an, o, st| {
            an.store_attr(o, name, v, node, st);
            Value::new()
        });
        if split.is_some() {
            return;
        }
        // (an unknown may be what takes no attribute: None, a number, a builtin container)
        if !self.unwind.tries.is_empty() && self.untyped_operand(obj) {
            self.may_raise_here("AttributeError", state);
        }
        // (a list, tuple, set, plain dict or one-shot iterator has no attribute dict:
        // AttributeError)
        let no_dict = |an: &Analyzer, o: ObjId| match an.heap.kind(o) {
            Kind::List | Kind::Tuple | Kind::Set => true,
            // (an unknown of a builtin type, as annotated: `x: int`, `x: list`)
            Kind::Unknown => an.heap.obj(o).ty.is_some(),
            Kind::Dict => an.heap.obj(o).label.starts_with("dict@") || an.heap.obj(o).label.starts_with("**"),
            _ => false,
        };
        let typed_scalar = matches!(&obj.kv, KeyVal::Data(l, _) if crate::domain::label_base(&intern::str_get(*l)).is_some());
        if typed_scalar || obj.pts.iter().any(|o| no_dict(self, o)) {
            let rest = Value::with_kv(obj.pts.filter(|o| !no_dict(self, o)), if typed_scalar { crate::domain::BOT } else { obj.kv.clone() });
            if rest.empty() {
                self.raise_here("AttributeError", state);
                return;
            }
            self.may_raise_here("AttributeError", state);
            return self.store_attr(&rest, name, v, node, state);
        }
        // (a literal — None, a number, a str — takes no attribute)
        if obj.kv.is_literal() && !obj.kv.is_bot() {
            if obj.pts.is_empty() {
                self.raise_here("AttributeError", state);
                return;
            }
            self.may_raise_here("AttributeError", state);
        }
        let nk = key_str_lit(name);
        state.forget_narrowed(nk);
        for o in obj.pts.iter() {
            if self.heap.kind(o) == Kind::View {
                crate::models::pandas::view_setattr(self, o, name, v, state);
            }
            if crate::models::pandas::is_series_obj(self, o) {
                let strong = obj.pts.len() == 1 && self.heap.obj(o).single;
                crate::models::pandas::series_setattr(self, o, name, v, state, strong);
            }
        }
        let targets: Vec<u32> = obj.pts.iter().filter(|o| matches!(self.heap.kind(*o), Kind::Instance | Kind::Module | Kind::Class | Kind::Unknown | Kind::Frame | Kind::Namespace | Kind::Function)).collect();
        for &oid in &targets {
            let kind = self.heap.kind(oid);
            if self.is_any(oid) {
                self.escape(v, &format!("store into unknown@L{}", node.line), state);
                continue;
            }
            let strong = targets.len() == 1 && self.heap.obj(oid).single;
            // a store that may raise on some classes (a frozen model / dataclass, a
            // `__setattr__`): decided per call site, where the class is known
            if kind == Kind::Frame {
                crate::models::pandas::frame_setattr(self, oid, name, v, node, state, strong);
                continue;
            }
            if self.store_by_class(oid, kind, name, nk, v, strong, node, state) {
                continue;
            }
            let strong_key = strong || (targets.len() == 1 && self.cx.fresh_init.contains_key(&oid));
            if kind == Kind::Unknown && !strong && state.slot_opt(oid, Ns::Attr, nk).is_none() {
                // a weak store on open content (an element under an unknown key, say)
                // joins with what was there, which is unknown
                let row = state.row(oid, Ns::Attr);
                let prior = self.path_placeholder_value(oid, Ns::Attr, nk, &row, state);
                state.set_slot(oid, Ns::Attr, nk, prior, true);
            }
            let r = state.row(oid, Ns::Attr).store(nk, strong_key);
            state.set_row(oid, Ns::Attr, r);
            // (an attribute the class declares, `count: int`: an unknown stored is of its type)
            let v = match self.heap.instance_class(oid).filter(|_| kind == Kind::Instance) {
                Some(c) => self.attr_as_annotated(c, name, v.clone(), state),
                None => v.clone(),
            };
            state.set_slot(oid, Ns::Attr, nk, v.clone(), strong);
            if kind == Kind::Class {
                self.heap.set_cls_slot((oid, Ns::Attr, nk), v.clone());
                self.heap.cls_late.insert((oid, nk));
            }
        }
    }

    /// What the class does with a store of `name` on instance `oid`: a frozen dataclass or a
    /// NamedTuple raises, a pydantic model validates, a property's setter, a data descriptor's
    /// `__set__` or the class's `__setattr__` runs instead, `__slots__` refuses other names.
    /// True: the store is done (or raised).
    #[allow(clippy::too_many_arguments)]
    fn store_by_class(&mut self, oid: ObjId, kind: Kind, name: &str, nk: KeyId, v: &Value, strong: bool, node: &Node, state: &mut State) -> bool {
        // a frozen dataclass: assignment raises FrozenInstanceError (an AttributeError)
        if kind == Kind::Instance {
            if let Some(cls) = self.heap.instance_class(oid) {
                if self.heap.mro(cls).iter().any(|c| self.heap.class_info_opt(*c).map(|ci| ci.dc_frozen.get()).unwrap_or(false)) && !self.cx.fresh_init.contains_key(&oid) {
                    self.raise_here("FrozenInstanceError", state);
                    return true;
                }
                // a NamedTuple's fields are read-only, and it has no instance dict
                if crate::models::dataclasses::is_named_tuple_class(self, cls) && self.class_lookup(cls, "__setattr__").is_none() {
                    self.raise_here("AttributeError", state);
                    return true;
                }
            }
        }
        // a pydantic model's field / private attribute (`models::pydantic::model_setattr`)
        if kind == Kind::Instance && crate::models::pydantic::model_setattr(self, oid, name, v, strong, node, state) {
            return true;
        }
        // a property with a setter: the setter runs instead of the store
        let cls_of = match kind {
            Kind::Instance => self.heap.instance_class(oid),
            _ => None,
        };
        if let Some(cls) = cls_of {
            if let Some(defc) = self.class_lookup(cls, name) {
                // (what the class holds under the name on this path: a store or `del` of the class
                // attribute may have replaced the property)
                let (defs, sure) = self.class_lookup_on_path(cls, name, state);
                let cv = self.class_values(&defs, nk, state);
                let is_prop = |an: &Analyzer, f: u32| an.heap.kind(f) == Kind::Function && an.heap.func_info(f).is_property.get() && !an.heap.func_info(f).is_cached.get();
                let surely_prop = sure && cv.kv.is_bot() && !cv.pts.is_empty() && cv.pts.iter().all(|f| is_prop(self, f));
                if let (Some(&setter), true) = (self.heap.prop_setters.get(&(defc, nk)), cv.pts.iter().any(|f| is_prop(self, f))) {
                    // (the property on some paths only: the others store on the instance)
                    let mut plain = (!surely_prop).then(|| state.copy());
                    self.call_user(setter, Some(Value::obj(oid)), Args::pos(&[v.clone()]), state, node, false);
                    if let Some(mut plain) = plain.take() {
                        let r = plain.row(oid, Ns::Attr).store(nk, strong);
                        plain.set_row(oid, Ns::Attr, r);
                        plain.set_slot(oid, Ns::Attr, nk, v.clone(), strong);
                        let j = state.join(&mut plain);
                        state.replace_with(j);
                    }
                    return true;
                }
                // a property without a setter: the assignment raises (one the class attribute
                // may be, beside what else it may hold: may raise)
                if kind == Kind::Instance {
                    if surely_prop {
                        self.raise_here("AttributeError", state);
                        return true;
                    }
                    if cv.pts.iter().any(|f| is_prop(self, f)) {
                        self.may_raise_here("AttributeError", state);
                    }
                }
                // a data descriptor (an object whose class has `__set__`) takes the store
                let cv = self.class_slot(defc, nk, state);
                let descs: Vec<u32> = cv.pts.iter().filter(|d| self.heap.kind(*d) == Kind::Instance && self.heap.instance_class(*d).and_then(|dc| self.class_lookup(dc, "__set__")).is_some()).collect();
                if !descs.is_empty() {
                    for d in descs {
                        self.call_method_with(&Value::obj(d), "__set__", vec![Value::obj(oid), v.clone()], node, state);
                    }
                    return true;
                }
            }
            // a user `__setattr__` takes every assignment (`object.__setattr__` bypasses it)
            if !self.mode.bypass_setattr && self.class_lookup(cls, "__setattr__").is_some() {
                let before = state.row(oid, Ns::Attr).mutators.clone();
                let skipped = self.budget.skipped;
                self.call_method_with(&Value::obj(oid), "__setattr__", vec![Value::lit(nk), v.clone()], node, state);
                // (a call of it that did not run, unknown code to its receiver: it takes the
                // name to store, likely there after, as a skipped method's own stores are)
                let row = state.row(oid, Ns::Attr);
                if state.reachable && !row.present.contains(nk) && (row.mutators != before || self.budget.skipped != skipped) {
                    let mut d = row.store(nk, true).data();
                    d.definite = d.definite.remove(nk);
                    state.set_row(oid, Ns::Attr, Row::from_data(d));
                    state.set_slot(oid, Ns::Attr, nk, Value::obj(crate::interp::any_object(&mut self.heap)), true);
                }
                return true;
            }
        }
        // `__slots__` on every class of the MRO (no `__dict__`): another name raises
        match self.slots_refuse(oid, Some(name)) {
            Some(true) => {
                self.raise_here("AttributeError", state);
                return true;
            }
            Some(false) => self.may_raise_here("AttributeError", state),
            None => {}
        }
        false
    }
}

/// The builtin type of a literal key (None: not one of these).
fn lit_type(k: KeyId) -> Option<&'static str> {
    intern::with_key(k, |k| match k {
        intern::Key::Str(_) => Some("str"),
        intern::Key::Int(_) => Some("int"),
        intern::Key::Bool(_) => Some("bool"),
        intern::Key::Float(_) => Some("float"),
        intern::Key::Bytes(_) => Some("bytes"),
        intern::Key::None => Some("NoneType"),
        _ => None,
    })
}

/// The names a class statement's `__slots__` declares: None without one, Err where they are not
/// literal.
fn declared_slots(c: &ast::StmtClassDef) -> Result<Option<Vec<String>>, ()> {
    let mut out: Option<Vec<String>> = None;
    for st in c.body.iter() {
        let ast::Stmt::Assign(a) = st else { continue };
        if !matches!(&a.targets[..], [ast::Expr::Name(n)] if n.id.as_str() == "__slots__") {
            continue;
        }
        let elts: Vec<&ast::Expr> = match &*a.value {
            ast::Expr::Tuple(t) => t.elts.iter().collect(),
            ast::Expr::List(l) => l.elts.iter().collect(),
            e @ ast::Expr::StringLiteral(_) => vec![e],
            _ => return Err(()),
        };
        let names = out.get_or_insert_with(Vec::new);
        for e in elts {
            let ast::Expr::StringLiteral(s0) = e else { return Err(()) };
            names.push(s0.value.to_str().to_string());
        }
    }
    Ok(out)
}
