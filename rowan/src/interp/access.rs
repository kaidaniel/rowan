//! Record access: items, attributes, stores, deletes, class lookup (port of the
//! "record access" / "attributes" sections of `rowan/interp.py`).

use indexmap::IndexMap;
use ruff_python_ast as ast;

use crate::domain::{classify, join_rows, join_values, lens_apply, Classification, KeyVal, Ns, ObjSet, Row, Source, SourceKind, Tier, Value, DATA};
use crate::frontend::{self, Node};
use crate::intern::{self, key_str_lit, s, KeyId, KeySet, STAR};
use crate::state::{Kind, State};

use super::{is_scalar_method, Analyzer, Payload, ViewKind, ATTR_GUARDS, DICT_METHOD_NAMES, ITEM_GUARDS, PATH_DEPTH, STR_METHOD_NAMES};

/// The attributes every instance has (`object`'s, and its class's `__module__` / `__doc__`;
/// `__dict__` / `__weakref__` unless `__slots__` drop them: taken as there).
pub const OBJECT_ATTRS: &[&str] = &[
    "__class__", "__delattr__", "__dir__", "__doc__", "__eq__", "__format__", "__ge__", "__getattribute__", "__getstate__", "__gt__", "__hash__", "__init__", "__init_subclass__", "__le__", "__lt__", "__ne__", "__new__", "__reduce__", "__reduce_ex__", "__repr__", "__setattr__", "__sizeof__", "__str__", "__subclasshook__", "__module__", "__dict__", "__weakref__",
];

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
            Some(c) => self.class_lookup(c, name).is_some() && !self.dataclass_fields_mro(c).iter().any(|(f, _)| f == name),
            None => false,
        }
    }

    /// The innermost handler that catches this namespace's error re-raises it.
    pub fn guard_reraises(&self, ns: Ns) -> bool {
        let want: &[&str] = if ns == Ns::Item { match self.item_exc { Some("ValueError") => super::VALUE_GUARDS, Some("ValidationError") => super::VALIDATION_GUARDS, _ => ITEM_GUARDS } } else { ATTR_GUARDS };
        for g in self.guards().rev() {
            if g.iter().any(|x| want.contains(&x.as_str())) {
                return g.contains("!reraise");
            }
        }
        false
    }

    /// Some class of `cls`'s MRO derives from a builtin exception class directly.
    pub fn builtin_exc_base(&self, cls: u32) -> bool {
        // (external bases are not in `bases`: read the class statement's base expressions)
        self.mro(cls).iter().any(|c| {
            self.heap.class_info_opt(*c)
                .map(|ci| {
                    let user: Vec<String> = ci.bases.iter().filter_map(|b| self.heap.class_info_opt(*b).map(|x| x.name.clone())).collect();
                    ci.node.arguments.as_ref().map(|a| a.args.iter().any(|e| {
                        let n = super::dotted_last(e);
                        super::exprs::is_exception_name(&n) && !user.contains(&n)
                    })).unwrap_or(false)
                })
                .unwrap_or(false)
        })
    }

    /// The attribute names `__slots__` allows on instances of `cls`, when every class of its MRO
    /// declares them (literally) and none gives a `__dict__`.
    pub fn slots_of(&self, cls: u32) -> Option<Vec<String>> {
        let mut out: Vec<String> = Vec::new();
        for c in self.mro(cls) {
            let Some(ci) = self.heap.class_info_opt(c) else {
                // (`object` at the end of the MRO has no instance dict of its own)
                if self.heap.kind(c) == Kind::External && self.heap.external_qual(c) == "builtins.object" {
                    continue;
                }
                return None;
            };
            let mut found = false;
            for st in ci.node.body.iter() {
                let ast::Stmt::Assign(a) = st else { continue };
                if !matches!(&a.targets[..], [ast::Expr::Name(n)] if n.id.as_str() == "__slots__") {
                    continue;
                }
                let elts: Vec<&ast::Expr> = match &*a.value {
                    ast::Expr::Tuple(t) => t.elts.iter().collect(),
                    ast::Expr::List(l) => l.elts.iter().collect(),
                    e @ ast::Expr::StringLiteral(_) => vec![e],
                    _ => return None,
                };
                for e in elts {
                    let ast::Expr::StringLiteral(s0) = e else { return None };
                    let nm = s0.value.to_str().to_string();
                    if nm == "__dict__" {
                        return None;
                    }
                    out.push(nm);
                }
                found = true;
            }
            if !found {
                return None;
            }
        }
        Some(out)
    }

    pub fn is_guarded(&self, ns: Ns) -> bool {
        let want: &[&str] = if ns == Ns::Item { match self.item_exc { Some("ValueError") => super::VALUE_GUARDS, Some("ValidationError") => super::VALIDATION_GUARDS, _ => ITEM_GUARDS } } else { ATTR_GUARDS };
        self.guards().any(|g| g.iter().any(|x| want.contains(&x.as_str())))
    }

    pub fn record_objects(&self, v: &Value) -> Vec<u32> {
        v.pts.iter().filter(|o| self.heap.is_record_like(*o)).collect()
    }

    pub fn require_providers(&mut self, cls: &Classification, row: &Row, with_alts: bool, state: &mut State) {
        self.require_providers_k(cls, row, with_alts, state, None, Ns::Item)
    }

    /// The access of `key` needs it of the inputs that may provide it; a key the program
    /// stored on some path is needed only on the others (sometimes).
    pub fn require_providers_k(&mut self, cls: &Classification, row: &Row, with_alts: bool, state: &mut State, key: Option<KeyId>, ns: Ns) {
        let srcs: Vec<crate::domain::SourceId> = cls.providers.iter().map(|(s, _)| *s).collect();
        let sometimes = key.map(|k| row.stored.contains(k)).unwrap_or(false);
        for (src, terms) in &cls.providers {
            for t in terms.iter() {
                // (an access needed on some paths only adds nothing to whether it is always)
                let was_always = state.reqs.get(src, &t).map(|r| r.always).unwrap_or(false);
                if with_alts {
                    let alts = intern::IdSet::from_iter(srcs.iter().copied().filter(|x| x != src));
                    state.require(*src, t, alts, row.mutators.clone(), terms.clone());
                } else {
                    state.require(*src, t, intern::IdSet::new(), row.mutators.clone(), intern::IdSet::new());
                }
                if ns == Ns::Attr {
                    if let Some(cur) = state.reqs.get(src, &t).filter(|r| !r.attr) {
                        state.reqs.set(src, t, crate::domain::ReqInfo { attr: true, ..cur });
                    }
                }
                if sometimes && !was_always {
                    if let Some(cur) = state.reqs.get(src, &t) {
                        state.reqs.set(src, t, crate::domain::ReqInfo { always: false, ..cur });
                    }
                }
            }
        }
    }

    /// `True`, `1` and `1.0` are one dict key: a numeric literal key equal to a key the record
    /// has (or may have) is that key.
    /// A mixed-in Enum member (`StrEnum`, `IntEnum`) and its value are one key too.
    pub fn canon_key(&self, oid: u32, ns: Ns, k: KeyId, state: &State) -> KeyId {
        if !self.m.ci_dicts.is_empty() && self.m.ci_dicts.contains(&oid) {
            if let Some(s) = intern::key_as_str(k) {
                return key_str_lit(&s.to_lowercase());
            }
        }
        let members = !self.member_key.is_empty();
        let base = |k: KeyId| if members { intern::key_member_value(k).unwrap_or(k) } else { k };
        let kb = base(k);
        let x = numeric_of(kb);
        if x.is_none() && !members {
            return k;
        }
        let row = state.row(oid, ns);
        if row.present.contains(k) {
            return k;
        }
        for p in row.present.iter() {
            let pb = base(p);
            if (members && pb == kb) || (x.is_some() && numeric_of(pb) == x) {
                return p;
            }
        }
        k
    }

    /// Enum members (and Flag combinations) used as keys are their `Key::Member` literals.
    pub fn keyify(&self, v: &Value) -> Value {
        if v.pts.is_empty() {
            return v.clone();
        }
        let mut ks: Vec<KeyId> = Vec::new();
        let mut rest: Vec<u32> = Vec::new();
        for o in v.pts.iter() {
            match self.member_key.get(&o).copied().or_else(|| self.identity_key(o)) {
                Some(k) => ks.push(k),
                None => rest.push(o),
            }
        }
        if ks.is_empty() {
            return v.clone();
        }
        let out = Value::with_kv(crate::domain::ObjSet::from_vec(rest), v.kv.clone());
        out.join(&Value::kv_only(KeyVal::lits(KeySet::from_iter(ks))))
    }

    /// The key a tuple / frozenset with known items hashes as (`(1, "a")`, `frozenset({"r"})`).
    pub fn value_key(&self, o: u32, state: &State, depth: usize) -> Option<KeyId> {
        let kind = self.heap.kind(o);
        // a frozen dataclass / pydantic instance hashes by its fields: `Key(ns='a', name='b')`
        if kind == Kind::Instance && depth <= 3 && self.heap.obj(o).single {
            return self.frozen_instance_key(o, state, depth);
        }
        if !matches!(kind, Kind::Tuple | Kind::Set) || depth > 3 || !self.heap.obj(o).single {
            return None;
        }
        let n = self.heap.llen(o, state)?;
        if n > 16 {
            return None;
        }
        let mut ks = Vec::with_capacity(n);
        for i in 0..n {
            let v = state.slot(o, Ns::Pos, intern::key_int(i as i64));
            let k = match (v.pts.is_empty(), v.kv.as_lit(), v.single()) {
                (true, Some(k), _) => k,
                (false, _, Some(x)) if v.kv.is_bot() => self.value_key(x, state, depth + 1)?,
                _ => return None,
            };
            ks.push(k);
        }
        Some(if kind == Kind::Tuple { intern::key_tuple(ks) } else { intern::key_frozen(ks) })
    }

    /// `keyify`, with tuples / frozensets of known items as their value keys.
    pub fn keyify_in(&self, v: &Value, state: &State) -> Value {
        if v.pts.is_empty() {
            return v.clone();
        }
        let mut ks: Vec<KeyId> = Vec::new();
        let mut rest: Vec<u32> = Vec::new();
        for o in v.pts.iter() {
            match self.value_key(o, state, 0) {
                Some(k) => ks.push(k),
                None => rest.push(o),
            }
        }
        if ks.is_empty() {
            return self.keyify(v);
        }
        let out = self.keyify(&Value::with_kv(crate::domain::ObjSet::from_vec(rest), v.kv.clone()));
        out.join(&Value::kv_only(KeyVal::lits(KeySet::from_iter(ks))))
    }


    /// A key standing for one object hashed by its identity (a future, a function, an instance
    /// of a class without `__eq__` / `__hash__`), while that object is one runtime object.
    pub fn identity_key(&self, o: u32) -> Option<KeyId> {
        if let Some(k) = self.identity_keys.borrow().get(&o) {
            return Some(*k);
        }
        let ob = self.heap.obj(o);
        if !ob.single || ob.site.ends_with("#recent") {
            return None;
        }
        let ok = match ob.kind {
            Kind::Namespace => matches!(&ob.payload, Payload::Method(q, _) if q.starts_with("stdobj.")),
            Kind::Function | Kind::Class => true,
            Kind::Instance => match self.heap.instance_class(o) {
                Some(c) => {
                    let ci = self.heap.class_info(c);
                    !ci.open_base && ci.dataclass_fields.borrow().is_none() && !ci.lib.pydantic && !crate::models::pydantic::is_model(self, c) && !ci.lib.named_tuple && !ci.lib.dict_base && !ci.lib.typed_dict && !ci.pyd_dataclass.get() && self.class_lookup(c, "__eq__").is_none() && self.class_lookup(c, "__hash__").is_none() && !self.enum_members.contains_key(&c)
                }
                None => false,
            },
            _ => false,
        };
        if !ok {
            return None;
        }
        let k = intern::key_member(&format!("<{} #{}>", ob.label, o), None);
        self.identity_keys.borrow_mut().insert(o, k);
        self.identity_objs.borrow_mut().insert(k, o);
        Some(k)
    }

    /// Member keys (read back from a record's keys) are the member objects.
    pub fn unkey(&self, v: &Value) -> Value {
        if self.key_member.is_empty() && self.identity_objs.borrow().is_empty() {
            return v.clone();
        }
        let lits = crate::domain::kv_literals(&v.kv);
        if !lits.iter().any(intern::key_is_member) || !v.kv.is_literal() {
            return v.clone();
        }
        let mut pts = v.pts.clone();
        let mut rest: Vec<KeyId> = Vec::new();
        for k in lits.iter() {
            match self.key_member.get(&k).copied().or_else(|| self.identity_objs.borrow().get(&k).copied()) {
                Some(m) => pts = pts.union(&crate::domain::ObjSet::one(m)),
                None => rest.push(k),
            }
        }
        let kv = if rest.is_empty() { KeyVal::Bot } else { KeyVal::lits(KeySet::from_iter(rest)) };
        Value::with_kv(pts, kv)
    }

    pub fn canon_kv(&self, oid: u32, ns: Ns, kv: &KeyVal, state: &State) -> KeyVal {
        match kv {
            KeyVal::Lit(k) => KeyVal::Lit(self.canon_key(oid, ns, *k, state)),
            KeyVal::Lits(ks) => KeyVal::Lits(ks.map(|k| self.canon_key(oid, ns, k, state))),
            other => other.clone(),
        }
    }

    /// A receiver of several objects (or objects and a scalar) one of which runs user code for
    /// the operation: they are alternatives, one state each (the second must not see the
    /// first's effects, nor the first's raise end the others' paths).
    pub(crate) fn split_dispatch(&self, obj: &Value, dispatch: &dyn Fn(&Analyzer, u32) -> bool) -> bool {
        obj.pts.len() + usize::from(!obj.kv.is_bot()) > 1 && obj.pts.iter().any(|o| dispatch(self, o))
    }

    pub(crate) fn per_receiver(&mut self, obj: &Value, state: &mut State, f: &mut dyn FnMut(&mut Analyzer, &Value, &mut State) -> Value) -> Value {
        let mut parts: Vec<Value> = obj.pts.iter().map(Value::obj).collect();
        if !obj.kv.is_bot() {
            parts.push(Value::kv_only(obj.kv.clone()));
        }
        let mut outs: Vec<Value> = Vec::new();
        let mut acc: Option<State> = None;
        for p in parts {
            let mut st = state.copy();
            outs.push(f(self, &p, &mut st));
            acc = Some(match acc {
                None => st,
                Some(mut a) => a.join(&mut st),
            });
        }
        if let Some(a) = acc {
            state.replace_with(a);
        }
        crate::domain::join_values(outs)
    }

    /// The class of an instance, when it is a user class.
    fn user_class_of(&self, o: u32) -> Option<u32> {
        let c = self.heap.instance_class(o)?;
        self.heap.class_info_opt(c).map(|_| c)
    }

    fn has_user_method(&self, o: u32, m: &str) -> bool {
        self.user_class_of(o).is_some_and(|c| self.class_lookup(c, m).is_some())
    }

    /// Reading / writing attribute `name` of `o` runs user code: a property, a descriptor,
    /// `__getattr__` / `__getattribute__` (reading), `__setattr__` (writing).
    fn attr_runs_user_code(&self, o: u32, name: &str, store: bool) -> bool {
        let Some(c) = self.user_class_of(o) else { return false };
        let hooks: &[&str] = if store { &["__setattr__"] } else { &["__getattr__", "__getattribute__"] };
        if hooks.iter().any(|h| self.class_lookup(c, h).is_some()) {
            return true;
        }
        let Some(dc) = self.class_lookup(c, name) else { return false };
        let cv = self.cls_slots.get(&(dc, Ns::Attr, key_str_lit(name))).cloned().unwrap_or_default();
        let runs = cv.pts.iter().any(|f| (self.heap.kind(f) == Kind::Function && self.heap.func_info(f).is_property.get()) || (self.heap.kind(f) == Kind::Instance && self.heap.instance_class(f).is_some_and(|fc| self.class_lookup(fc, if store { "__set__" } else { "__get__" }).is_some())));
        runs
    }

    pub fn load_item(&mut self, obj: &Value, ns: Ns, key: &Value, node: &Node, state: &mut State, report: bool) -> Value {
        if ns == Ns::Item && self.split_dispatch(obj, &|an, o| an.has_user_method(o, "__getitem__")) {
            return self.per_receiver(obj, state, &mut |an, o, st| an.load_item(o, ns, key, node, st, report));
        }
        // a numpy structured array's missing field raises ValueError
        let exc = if ns == Ns::Item && !obj.pts.is_empty() && !self.m.np_struct.is_empty() && obj.pts.iter().all(|o| self.m.np_struct.contains(&o)) { Some("ValueError") } else { None };
        let exc = if ns == Ns::Item { self.item_exc_force.or(exc) } else { exc };
        let saved = std::mem::replace(&mut self.item_exc, exc);
        let scalar = self.may_be_scalar(obj);
        let saved_scalar = std::mem::replace(&mut self.recv_scalar, scalar);
        let v = self.load_item_inner(obj, ns, key, node, state, report);
        self.recv_scalar = saved_scalar;
        self.item_exc = saved;
        v
    }

    fn load_item_inner(&mut self, obj: &Value, ns: Ns, key: &Value, node: &Node, state: &mut State, report: bool) -> Value {
        // (objects in the key: an unknown key; lists / tuples are indexed by their own rules)
        let okey = if ns == Ns::Item && !key.pts.is_empty() && obj.pts.iter().all(|o| matches!(self.heap.kind(o), Kind::Dict | Kind::Unknown)) && !obj.pts.is_empty() {
            self.opaque_key(key, state)
        } else {
            key.clone()
        };
        let key = &okey;
        let mut acc = Value::new();
        let guarded = self.is_guarded(ns) || !report;
        // a real try/except guard (as opposed to a model's unreported load): no exception
        // means the key was present from here on
        let try_guard = report && self.is_guarded(ns);
        let scalar = self.may_be_scalar(obj);
        let single_target = obj.pts.len() == 1 && !scalar;
        let line_data = Value::data(&format!("L{}", node.line));
        // a frame column read raises KeyError when the column is missing
        let frame_tiers: Option<Vec<Tier>> = if try_guard && ns == Ns::Item && !obj.pts.is_empty() && obj.pts.iter().all(|o| self.heap.kind(o) == Kind::Frame) {
            key.kv.as_lit().filter(|_| key.pts.is_empty()).map(|k| obj.pts.iter().map(|o| crate::models::pandas::column_tier(&state.row(o, ns), k)).collect())
        } else {
            None
        };
        // (a handler starts from the state before the read: the read narrows the frame to
        // having the column)
        if let Some(ts) = &frame_tiers {
            if !ts.iter().all(|t| *t == Tier::Missing) && ts.iter().any(|t| !matches!(t, Tier::Proven | Tier::Likely)) {
                self.may_raise_here("KeyError", state);
            }
        }
        // `lst[-1]` right after `lst.append(x)`: x
        if ns == Ns::Item && !state.lasts.is_empty() && key.pts.is_empty() && key.kv.as_lit() == Some(intern::key_int(-1)) && !obj.pts.is_empty() && obj.kv.is_bot() {
            let vs: Option<Vec<Value>> = obj.pts.iter().map(|o| state.last_of(o)).collect();
            if let Some(vs) = vs {
                return join_values(vs);
            }
        }
        // `Color["RED"]`: an Enum's member by name
        if ns == Ns::Item && !obj.pts.is_empty() && obj.pts.iter().all(|o| self.enum_members.contains_key(&o)) {
            return crate::models::enums::member_by_name(self, obj, key, node, guarded, state);
        }
        // (all targets' values joined at once: a pairwise join over many targets is quadratic)
        let mut parts: Vec<Value> = vec![std::mem::take(&mut acc)];
        for oid in obj.pts.iter() {
            let kind = self.heap.kind(oid);
            // (a defaultdict / Counter key read: `models::counter::default_on_read`)
            let counter_restore = if kind == Kind::Dict && ns == Ns::Item && (self.heap.obj(oid).label.starts_with("defaultdict@") || self.heap.obj(oid).label.starts_with("Counter@")) {
                crate::models::counter::default_on_read(self, oid, key, node, single_target, state)
            } else {
                None
            };
            match kind {
                Kind::Locidx => {
                    let v = crate::models::pandas::loc_get(self, oid, key, node, state);
                    parts.push(v);
                    continue;
                }
                Kind::Groupby => {
                    // (several groupby objects: each selection is an object of its own)
                    let several = obj.pts.iter().filter(|o| self.heap.kind(*o) == Kind::Groupby).count() > 1;
                    let saved = if several { self.alloc_extra.replace(format!("gb{}", oid)) } else { self.alloc_extra.clone() };
                    let v = crate::models::pandas::groupby_select(self, oid, key, node, state);
                    self.alloc_extra = saved;
                    parts.push(v);
                    continue;
                }
                Kind::Instance if ns == Ns::Item => {
                    let cls = self.heap.instance_class(oid);
                    if let Some(cls) = cls {
                        if let Some(m) = self.class_lookup(cls, "__getitem__") {
                            let fv = self.cls_slots.get(&(m, Ns::Attr, key_str_lit("__getitem__"))).cloned().unwrap_or_default();
                            for f in fv.pts.iter() {
                                if self.heap.kind(f) == Kind::Function {
                                    let r = self.call_user(f, Some(Value::obj(oid)), vec![key.clone()], IndexMap::new(), vec![], vec![], state, node, None, false);
                                    parts.push(r);
                                }
                            }
                            continue;
                        }
                        // `__missing__` answers the keys the dict does not have
                        if self.class_lookup(cls, "__missing__").is_some() {
                            if let Some(k) = key.kv.as_lit().filter(|_| key.pts.is_empty()) {
                                let row = state.row(oid, Ns::Item);
                                if !row.definite.contains(k) {
                                    let r = self.call_method_with(&Value::obj(oid), "__missing__", vec![key.clone()], node, state).unwrap_or_default();
                                    parts.push(r);
                                    if row.present.contains(k) {
                                        parts.push(state.slot(oid, Ns::Item, k));
                                    }
                                    // an open / symbolic dict may hold it (never a missing key)
                                    if !row.present.contains(k) && (row.open()) {
                                        parts.push(line_data.clone());
                                    }
                                    continue;
                                }
                            }
                        }
                        // a NamedTuple by position: its fields in order, from either end
                        if self.is_named_tuple_class(cls) {
                            let v = self.named_tuple_item(oid, cls, key, single_target, node, state);
                            parts.push(v);
                            continue;
                        }
                        if let Some(ci) = self.heap.class_info_opt(cls) {
                            if !ci.lib.typed_dict && !ci.lib.named_tuple && !ci.open_base {
                                parts.push(line_data.clone());
                                continue;
                            }
                        }
                    }
                }
                _ => {}
            }
            match kind {
                Kind::Pair => {
                    // a (key, value) pair indexed by 0 / 1 (or -2 / -1)
                    let idx = key.kv.as_lit().filter(|_| key.pts.is_empty()).and_then(intern::key_as_int);
                    match (idx, self.heap.obj(oid).payload.clone()) {
                        (Some(0 | -2), Payload::Pair(k, _)) => parts.push(k),
                        (Some(1 | -1), Payload::Pair(_, v)) => parts.push(v),
                        _ => parts.push(line_data.clone()),
                    }
                }
                Kind::View => {
                    if let Payload::View(ViewKind::Attrs, inst) = self.heap.obj(oid).payload.clone() {
                        // `obj.__dict__[k]`: an attribute load
                        let v = self.load_item(&inst, Ns::Attr, key, node, state, report);
                        parts.push(v);
                    } else if let Payload::View(ViewKind::Proxy, inner) = self.heap.obj(oid).payload.clone() {
                        let v = self.load_item(&inner, Ns::Item, key, node, state, report);
                        parts.push(v);
                    } else {
                        parts.push(line_data.clone());
                    }
                }
                // an sqlite3.Row indexed by position
                Kind::Dict if self.m.sql_rows.contains_key(&oid) && key.kv.as_lit().and_then(intern::key_as_int).is_some() => match crate::models::sqlite::row_index(self, state, oid, key) {
                    Some(v) => parts.push(v),
                    None => {
                        self.raise_here("IndexError", state);
                    }
                },
                // a regex match: `m[g]` is its group
                Kind::Namespace if self.m.re_matches.contains_key(&oid) => match crate::models::remodel::match_item(self, oid, key) {
                    Some(v) => parts.push(v),
                    None => parts.push(line_data.clone()),
                },
                Kind::Zip | Kind::Enumerate => {
                    parts.push(line_data.clone());
                }
                // `C[T]`: a generic class's alias, called like the class (a TypeError for a class
                // that is not generic)
                Kind::Class if ns == Ns::Item => {
                    self.may_raise_here("TypeError", state);
                    parts.push(Value::obj(oid));
                }
                Kind::List | Kind::Tuple | Kind::Set => {
                    let is_slice = matches!(&key.kv, KeyVal::Data(s) if intern::str_get(*s).starts_with("slice"));
                    // a slice object with known bounds (`xs[slice(0, n)]`) on known positions
                    if is_slice && kind != Kind::Set && single_target {
                        if let (Some((lo, hi, step)), Some(n)) = (Self::slice_label_bounds(&key.kv), self.heap.llen(oid, state)) {
                            let step = step.unwrap_or(1);
                            if step != 0 {
                                let idx = crate::interp::exprs::slice_positions(n as i64, lo, hi, step);
                                let vals: Vec<Value> = idx.iter().map(|i| state.slot(oid, Ns::Pos, intern::key_int(*i))).collect();
                                let kv = state.elems.get(&oid).unwrap_or(DATA);
                                parts.push(self.alloc_positional(kind, node, Some(vals), Value::new(), kv, state));
                                continue;
                            }
                        }
                    }
                    if is_slice && kind != Kind::Set {
                        // a slice is a container of the same elements (positions unknown)
                        let elem = state.slot(oid, Ns::Elem, STAR);
                        let kv = state.elems.get(&oid).unwrap_or(DATA);
                        let nid = self.alloc_at(kind, node, state, &format!("{}@L{}", kind.name(), node.line), Payload::None, None, Ns::Item, None, None);
                        self.heap.set_llen(nid, state, None);
                        state.set_slot(nid, Ns::Elem, STAR, elem, true);
                        state.elems.insert(nid, kv);
                        parts.push(Value::obj(nid));
                        continue;
                    }
                    let ll = self.heap.llen(oid, state);
                    let idx = key.kv.as_lit().and_then(intern::key_as_int);
                    // the element appended last
                    if let (Some(-1), None, Some(v)) = (idx, ll, state.last_of(oid)) {
                        parts.push(v);
                        continue;
                    }
                    // positions below a known prefix stay valid after appends of unknown count
                    if let (Some(i), None, Some(p)) = (idx, ll, self.heap.prefix(oid, state)) {
                        if 0 <= i && (i as usize) < p {
                            parts.push(state.slot(oid, Ns::Pos, intern::key_int(i)));
                            continue;
                        }
                    }
                    match (idx, ll) {
                        (Some(i), Some(n)) => {
                            let n = n as i64;
                            if 0 <= i && i < n {
                                parts.push(state.slot(oid, Ns::Pos, intern::key_int(i)));
                            } else if -n <= i && i < 0 {
                                parts.push(state.slot(oid, Ns::Pos, intern::key_int(n + i)));
                            } else if kind != Kind::Set && single_target && self.heap.obj(oid).single {
                                // out of range on a list of known length
                                self.raise_here("IndexError", state);
                            } else {
                                self.may_raise_here("IndexError", state);
                                parts.push(state.slot(oid, Ns::Pos, intern::key_int(i)).join(&state.slot(oid, Ns::Elem, STAR)));
                            }
                        }
                        _ => {
                            // (an index or a length not known: it may be out of range)
                            if kind != Kind::Set {
                                self.may_raise_here("IndexError", state);
                            }
                            // (the elements' objects and scalars: `elems`, nothing known when unset)
                            let e = state.slot(oid, Ns::Elem, STAR);
                            parts.push(Value::with_kv(e.pts, state.elems.get(&oid).unwrap_or(DATA)));
                        }
                    }
                }
                Kind::Series => parts.push(Value::with_kv(ObjSet::one(self.series), DATA)),
                Kind::Frame if ns == Ns::Item => {
                    let v = crate::models::pandas::frame_getitem_n(self, oid, key, node, state, guarded, report);
                    parts.push(v);
                }
                k if k.is_record() => {
                    let ckv = self.canon_kv(oid, ns, &key.kv, state);
                    let v = if ckv != key.kv {
                        let saved = self.key_alias.take();
                        if let (KeyVal::Lit(c), KeyVal::Lit(o)) = (&ckv, &key.kv) {
                            self.key_alias = Some((*c, *o));
                        }
                        let ck = Value::with_kv(key.pts.clone(), ckv);
                        let v = self.load_record(oid, ns, &ck, node, state, guarded, single_target, try_guard);
                        self.key_alias = saved;
                        v
                    } else {
                        self.load_record(oid, ns, key, node, state, guarded, single_target, try_guard)
                    };
                    parts.push(v);
                }
                // a library object rowan does not model (`sys.argv`): its subscript runs code
                // rowan cannot see, which may raise anything
                Kind::External if ns == Ns::Item => {
                    self.flag_may_raise(state);
                    parts.push(line_data.clone());
                }
                _ => parts.push(line_data.clone()),
            }
            if let Some(r) = counter_restore {
                state.set_row(oid, ns, r);
            }
        }
        acc = crate::domain::join_values(parts);
        if obj.pts.is_empty() {
            acc = line_data;
        }
        if let Some(ts) = frame_tiers {
            if ts.iter().all(|t| *t == Tier::Missing) {
                self.raise_here("KeyError", state);
            }
        }
        acc
    }

    #[allow(clippy::too_many_arguments)]
    fn load_record(&mut self, oid: u32, ns: Ns, key: &Value, node: &Node, state: &mut State, guarded: bool, single_target: bool, try_guard: bool) -> Value {
        // (a load a `try` handler guards: its exception still fires when the key is missing, so
        // the key is required of the input all the same — the runtime oracle's rule)
        let caught = try_guard;
        let saved = std::mem::replace(&mut self.caught_load, caught);
        let v = self.load_record_inner(oid, ns, key, node, state, guarded, single_target, try_guard);
        self.caught_load = saved;
        v
    }

    #[allow(clippy::too_many_arguments)]
    fn load_record_inner(&mut self, oid: u32, ns: Ns, key: &Value, node: &Node, state: &mut State, guarded: bool, single_target: bool, try_guard: bool) -> Value {
        let kind = self.heap.kind(oid);
        let mut row = if kind == Kind::Unknown { self.unknown_row(oid, ns, state) } else { self.read_row(oid, ns, state) };
        // (the row before this read narrows it)
        let row0 = row.clone();
        let kv = &key.kv;
        if kv.is_literal() {
            let mut acc = Value::new();
            let ks = crate::domain::kv_literals(kv);
            let is_lit = matches!(kv, KeyVal::Lit(_));
            for k in intern::sorted_by_str(ks.iter()) {
                let cls = classify(&row, k);
                if guarded && cls.tier != Tier::Proven && (try_guard) {
                    if crate::interp::debug_level() == 2 {
                        eprintln!("  guarded access may raise: {:?} tier {:?} at L{}", intern::key_repr(k), cls.tier, node.line);
                    }
                    // (a model's own read — pydantic collecting its field errors — raises nothing here:
                    // the model raises its ValidationError, with every error, afterwards)
                    let model_read = ns == Ns::Item && self.item_exc == Some("ValidationError");
                    if model_read {
                        if self.caught_load && cls.tier == Tier::Input {
                            self.require_providers_k(&cls, &row, true, state, Some(k), ns);
                        }
                    }
                    // the handler sees the state as it is here (and a KeyError / AttributeError
                    // carrying the key)
                    if model_read {
                    } else if !self.tries.is_empty() {
                        let cname = if ns == Ns::Item { self.item_exc.unwrap_or("KeyError") } else { "AttributeError" };
                        let e = self.exception_object(cname, vec![Value::lit(k)], node, state);
                        self.cur_exc = Some(e);
                    }
                    // (required on the handler's path too: the exception fired for the key)
                    if !model_read && self.caught_load && cls.tier == Tier::Input {
                        self.require_providers_k(&cls, &row, true, state, Some(k), ns);
                    }
                    if !model_read {
                        self.cur_exc_cls = Some(super::builtin_exc_names(if ns == Ns::Item { self.item_exc.unwrap_or("KeyError") } else { "AttributeError" }));
                        self.record_raise_state(state);
                        self.cur_exc_cls = None;
                        // (an object of unknown kind indexed by a number may be a sequence: an
                        // IndexError then)
                        if ns == Ns::Item && intern::key_as_int(k).is_some() && matches!(kind, Kind::Unknown | Kind::External) {
                            self.cur_exc = None;
                            self.may_raise_here("IndexError", state);
                        }
                    }
                    if model_read {
                    } else if try_guard && cls.tier == Tier::Missing && is_lit && single_target {
                        // the access raises here: the rest of the try body is unreachable
                        state.reachable = false;
                    }
                    if try_guard && !model_read && cls.tier != Tier::Missing && is_lit && single_target && self.strongable(oid) {
                        // no exception raised: the key is present from here on
                        row = row.narrow_in(k, true);
                        state.set_row(oid, ns, row.clone());
                    }
                }
                if guarded {
                    self.verdict_guarded(node, k, &cls, oid, ns);
                }
                // (a handler that re-raises, a thread / future: the input still needs the key)
                if guarded && cls.tier == Tier::Input && (self.catching_depth > 0 || self.guard_reraises(ns) || self.caught_load) {
                    self.require_providers_k(&cls, &row, true, state, Some(k), ns);
                }
                if !guarded {
                    self.verdict(node, k, &cls, oid, ns, None, None);
                    if cls.tier == Tier::Input {
                        self.require_providers_k(&cls, &row, true, state, Some(k), ns);
                    }
                    if cls.tier == Tier::Proven && ns == Ns::Item {
                        if let Some(src) = self.td_param_src.get(&oid).copied() {
                            let fresh = !state.reqs.contains(&src, &k);
                            state.require(src, k, intern::IdSet::new(), intern::IdSet::new(), intern::IdSet::new());
                            if fresh && (self.branch_depth > 0 || self.cond_depth > 0) {
                                if let Some(cur) = state.reqs.get(&src, &k) {
                                    state.reqs.set(&src, k, crate::domain::ReqInfo { always: false, ..cur });
                                }
                            }
                        }
                    }
                    if cls.tier != Tier::Missing && is_lit && single_target && self.strongable(oid) {
                        row = row.narrow_in(k, true);
                        state.set_row(oid, ns, row.clone());
                        self.propagate_narrowing(oid, ns, k, state);
                    }
                }
                let mut v = state.slot(oid, ns, k);
                let star = state.slot(oid, ns, STAR);
                // (a map only the program filled, under keys it computed: a key it has holds one
                // of the values stored, `idx["a"]` of `{c: i for i, c in enumerate(chars)}`)
                let own_map = !row.top && row.mutators.is_empty() && !row.sources.is_empty() && row.sources.iter().all(|(src, _)| crate::domain::source_is_data(src));
                let from_map = own_map && v.empty() && !star.empty();
                if from_map {
                    v = star;
                } else if !star.pts.is_empty() {
                    v = v.join(&Value::with_kv(star.pts, DATA));
                }
                // (a key of an open record stored on some paths only: on the others its value is
                // the input's, which a slot one path made does not cover)
                let partly_stored = !v.empty() && row0.has_input() && !(row0.definite.contains(k) && row0.stored.contains(k));
                if !from_map && (partly_stored || (v.pts.is_empty() && matches!(v.kv, KeyVal::Data(_) | KeyVal::Bot) && (row.open() || cls.tier == Tier::Map))) {
                    let p = self.path_placeholder_value(oid, ns, k, &row, state);
                    v = v.join(&p);
                    if is_lit {
                        state.set_slot(oid, ns, k, v.clone(), false);
                    }
                }
                acc = acc.join(&v);
            }
            return acc;
        }
        if let KeyVal::KeysOf(ko) = kv {
            let srcrow = join_rows(ko.objs.iter().map(|x| state.row(x, ko.ns)));
            if !guarded && srcrow.closed() && row.closed() {
                let mut missing: Vec<KeyId> = Vec::new();
                for k in srcrow.present.difference(&ko.exclude).iter() {
                    if let Some(k2) = lens_apply(ko.chain, k) {
                        if !row.present.contains(k2) && !missing.contains(&k2) {
                            missing.push(k2);
                        }
                    }
                }
                for k in intern::sorted_by_str(missing) {
                    self.verdict(node, k, &Classification::with_note(Tier::Possibly, "may be missing: the key comes from another object's keys"), oid, ns, None, None);
                }
            }
            return self.all_values(oid, ns, state).join(&Value::kv_only(DATA));
        }
        // Data key: it may be one the record lacks (KeyError), unless the read cannot fail
        if !guarded || try_guard {
            self.cur_exc = None;
            self.may_raise_here(if ns == Ns::Item { self.item_exc.unwrap_or("KeyError") } else { "AttributeError" }, state);
        }
        if !guarded && row.closed() && !row.is_map() && !row.present.is_empty() && matches!(kind, Kind::Dict | Kind::Frame) {
            let k = key_str_lit(&format!("<data {}>", kv));
            self.verdict(node, k, &Classification::with_note(Tier::DataKey, "is computed from data and may not be one of this object's keys"), oid, ns, None, None);
        }
        let mut v = self.all_values(oid, ns, state);
        // (a closed record: the unknown key is one of its keys — one of their values; a map the
        // program filled under data keys: its values are all in its slots)
        let own_map = !row.top && row.mutators.is_empty() && row.sources.iter().all(|(src, _)| crate::domain::source_is_data(src));
        if (row.closed() || own_map) && !row.top {
            return v;
        }
        if v.pts.is_empty() && row.open() {
            let p = self.path_placeholder_value(oid, ns, STAR, &row, state);
            v = v.join(&p);
        }
        Value::with_kv(v.pts, DATA)
    }

    /// `_path_placeholder`: the (unknown) content of an open object's slot.
    pub fn path_placeholder_value(&mut self, oid: u32, ns: Ns, k: KeyId, row: &Row, state: &mut State) -> Value {
        if self.heap.obj(oid).depth >= PATH_DEPTH {
            return Value::data("deep");
        }
        let label = format!("{}[{}]", self.heap.obj(oid).label, intern::key_repr(k));
        // the content under an unknown key stands for many values: stores on it are weak
        let single = k != STAR;
        let pid = self.heap.alloc_raw(Kind::Unknown, &format!("path:{}:{}:{}", oid, ns, intern::key_repr(k)), "", single, &label, Payload::None);
        if let Some(sym) = self.names.access(&self.heap, oid, ns, k) {
            if !self.names.has(pid) {
                let scope = self.names.scope(oid);
                self.names.set(pid, sym, scope);
            }
        }
        let d = self.heap.obj(oid).depth + 1;
        self.heap.obj_mut(pid).depth = d;
        if !state.has_row(pid, Ns::Item) {
            let srcs = row.sources.filter(|(src, _)| !crate::domain::source_is_data(src)).map(|(src, _)| (crate::domain::source_extend(src, k), intern::EMPTY_LENS));
            // (the parent's value was read on this path: its requirements are known here, none
            // so far — a path without the entry never read it)
            for (cs, _) in srcs.iter() {
                state.reqs.mark(&cs, crate::domain::ReqInfo::default());
            }
            let r = if srcs.is_empty() && row.mutators.is_empty() {
                Row::top()
            } else {
                Row::from_data(crate::domain::RowData { sources: srcs, mutators: row.mutators.clone(), ..Default::default() })
            };
            state.set_row(pid, Ns::Item, r.clone());
            state.set_row(pid, Ns::Attr, r);
        }
        Value::obj(pid)
    }


    /// A key whose value includes objects (an unknown value, a non-literal tuple, an instance):
    /// for the record model an unknown key (its literal part kept in the join).
    pub fn opaque_key(&self, key: &Value, state: &State) -> Value {
        if key.pts.is_empty() {
            return key.clone();
        }
        let key = self.keyify_in(key, state);
        if key.pts.is_empty() {
            return key;
        }
        Value::with_kv(key.pts.clone(), crate::domain::kv_join(&key.kv, &DATA))
    }

    pub fn store_item(&mut self, obj: &Value, ns: Ns, key: &Value, v: &Value, node: &Node, state: &mut State) {
        if ns == Ns::Item && self.split_dispatch(obj, &|an, o| an.has_user_method(o, "__setitem__")) {
            self.per_receiver(obj, state, &mut |an, o, st| {
                an.store_item(o, ns, key, v, node, st);
                Value::new()
            });
            return;
        }
        let okey = if ns == Ns::Item { self.opaque_key(key, state) } else { key.clone() };
        let key = &okey;
        let targets: Vec<u32> = obj.pts.iter().filter(|o| !matches!(self.heap.kind(*o), Kind::Series | Kind::External | Kind::Function | Kind::Bound | Kind::Class | Kind::Method)).collect();
        for &oid in &targets {
            let kind = self.heap.kind(oid);
            let strong = targets.len() == 1 && self.strongable(oid);
            match kind {
                Kind::Locidx => {
                    crate::models::pandas::loc_set(self, oid, key, v, node, state);
                    continue;
                }
                Kind::List | Kind::Tuple | Kind::Set => {
                    state.set_slot(oid, Ns::Elem, STAR, Value::with_kv(v.pts.clone(), crate::models::torch::elem_kv(&v.kv)), false);
                    if let (Some(k), Some(n)) = (key.kv.as_lit(), self.heap.llen(oid, state)) {
                        if let Some(i) = intern::key_as_int(k) {
                            if 0 <= i && (i as usize) < n {
                                state.set_slot(oid, Ns::Pos, intern::key_int(i), v.clone(), strong);
                            }
                        }
                    }
                    continue;
                }
                Kind::Frame => {
                    crate::models::pandas::frame_setitem(self, oid, key, v, node, state, strong);
                    continue;
                }
                Kind::View => {
                    // a read-only proxy: the store raises
                    if let Payload::View(ViewKind::Proxy, _) = self.heap.obj(oid).payload {
                        if targets.len() == 1 {
                            self.raise_here("TypeError", state);
                        } else {
                            self.may_raise_here("TypeError", state);
                        }
                        continue;
                    }
                    // `obj.__dict__[k] = v`: an attribute store
                    if let Payload::View(ViewKind::Attrs, inst) = self.heap.obj(oid).payload.clone() {
                        let insts: Vec<u32> = inst.pts.iter().collect();
                        for i in &insts {
                            let s1 = strong && insts.len() == 1 && self.strongable(*i);
                            self.store_key(*i, Ns::Attr, &key.kv, v, s1, node, state);
                        }
                    }
                    continue;
                }
                Kind::Dict if self.m.chainmaps.contains_key(&oid) => {
                    // a ChainMap: the write lands in its first map too
                    let first = self.m.chainmaps[&oid].first().cloned().unwrap_or_default();
                    for m in self.record_objects(&first) {
                        let s1 = strong && first.pts.len() == 1 && self.strongable(m);
                        self.store_key(m, Ns::Item, &key.kv, v, s1, node, state);
                    }
                    if self.m.chainmaps[&oid].len() > 1 {
                        self.refresh_chainmap(oid, state);
                    } else {
                        let s1 = strong && self.strongable(oid);
                        self.store_key(oid, Ns::Item, &key.kv, v, s1, node, state);
                    }
                }
                Kind::Dict if self.attr_mirrors.contains_key(&oid) => {
                    // a `vars(obj)` dict: obj's attributes change with it
                    let inst = self.attr_mirrors[&oid];
                    let s1 = strong && self.strongable(inst);
                    self.store_key(inst, Ns::Attr, &key.kv, v, s1, node, state);
                }
                Kind::Instance if ns == Ns::Item => {
                    if let Some(cls) = self.heap.instance_class(oid) {
                        if let Some(m) = self.class_lookup(cls, "__setitem__") {
                            let fv = self.cls_slots.get(&(m, Ns::Attr, key_str_lit("__setitem__"))).cloned().unwrap_or_default();
                            for f in fv.pts.iter() {
                                if self.heap.kind(f) == Kind::Function {
                                    self.call_user(f, Some(Value::obj(oid)), vec![key.clone(), v.clone()], IndexMap::new(), vec![], vec![], state, node, None, false);
                                }
                            }
                            continue;
                        }
                    }
                }
                _ => {}
            }
            self.store_key(oid, ns, &key.kv, v, strong, node, state);
        }
    }

    pub fn store_key(&mut self, oid: u32, ns: Ns, kv: &KeyVal, v: &Value, strong: bool, node: &Node, state: &mut State) {
        // a store into what unknown code holds (the shared unknown object): the value goes to
        // unknown code, as if given to it; the shared object stays "anything"
        if self.is_any(oid) {
            self.escape(v, &format!("store into unknown@L{}", node.line), state);
            return;
        }
        let kv = &self.canon_kv(oid, ns, kv, state);
        let row = state.row(oid, ns);
        match kv {
            KeyVal::Lit(k) => {
                state.set_row(oid, ns, row.store(*k, strong));
                state.set_slot(oid, ns, *k, v.clone(), strong);
            }
            KeyVal::Lits(ks) => {
                state.set_row(oid, ns, row.store_many(ks));
                for k in ks.iter() {
                    state.set_slot(oid, ns, k, v.clone(), false);
                }
            }
            KeyVal::KeysOf(ko) => {
                let srcrow = join_rows(ko.objs.iter().map(|x| state.row(x, ko.ns)));
                let exhaustive = strong && ko.exhaustive_depth == self.cond_depth;
                state.set_row(oid, ns, row.copy_from(&srcrow, exhaustive, &ko.exclude, ko.chain));
                for x in ko.objs.iter() {
                    for k in state.slot_keys(x, ko.ns) {
                        if ko.exclude.contains(k) {
                            continue;
                        }
                        if let Some(k2) = lens_apply(ko.chain, k) {
                            state.set_slot(oid, ns, k2, v.clone(), false);
                        }
                    }
                }
                state.set_slot(oid, ns, STAR, v.clone(), false);
            }
            _ => {
                let site = format!("store@L{}", node.line);
                state.set_row(oid, ns, row.data_store(Source::new(&site, SourceKind::Data)));
                state.set_slot(oid, ns, STAR, v.clone(), false);
                // the unknown key may be any of the keys there: each may hold the value now
                let keys: Vec<KeyId> = state.slot_keys(oid, ns).into_iter().filter(|k| *k != STAR).collect();
                for k in keys {
                    state.set_slot(oid, ns, k, v.clone(), false);
                }
            }
        }
    }

    /// oid[ns] ∪= keys of every record in src (dict spread / update / dict(x)).
    pub fn copy_into(&mut self, oid: u32, ns: Ns, src: &Value, state: &mut State, strong: bool, kill: &KeySet) {
        let recs = self.record_objects(src);
        // an unknown value copied in (`{**entry}` with entry unknown): any keys
        if recs.is_empty() && src.pts.is_empty() && !src.kv.is_bot() && !src.kv.is_literal() {
            let n = state.rows.len();
            let r = state.row(oid, ns).data_store(Source::new(&format!("copy@{}", n), SourceKind::Data));
            state.set_row(oid, ns, r);
        }
        if recs.is_empty() {
            if !src.pts.is_empty() {
                let n = state.rows.len();
                let r = state.row(oid, ns).data_store(Source::new(&format!("copy@{}", n), SourceKind::Data));
                state.set_row(oid, ns, r);
            }
            return;
        }
        let ns_of = |an: &Analyzer, x: u32| if an.heap.kind(x) != Kind::Instance { Ns::Item } else { Ns::Attr };
        let srcrow = join_rows(recs.iter().map(|x| state.row(*x, ns_of(self, *x))));
        let r = state.row(oid, ns).copy_from(&srcrow, strong, kill, intern::EMPTY_LENS);
        state.set_row(oid, ns, r);
        // a key every source surely has replaces the value (`{**a, **b}`: b's wins)
        let mut vals: indexmap::IndexMap<KeyId, Value> = indexmap::IndexMap::new();
        for &x in &recs {
            let xns = ns_of(self, x);
            for (k, val) in state.obj_slots(x, xns) {
                if !kill.contains(k) {
                    let cur = vals.entry(k).or_default();
                    *cur = cur.join(&val);
                }
            }
        }
        // (a source with keys not known — an input — may replace the value under any key the
        // record has: its value there joins in)
        for k in state.slot_keys(oid, ns) {
            if kill.contains(k) || vals.contains_key(&k) {
                continue;
            }
            for &x in &recs {
                let xns = ns_of(self, x);
                let xr = state.row(x, xns);
                if xr.open() && !xr.lacks(k) {
                    let part = self.path_placeholder_value(x, xns, k, &xr, state);
                    let cur = vals.entry(k).or_default();
                    *cur = cur.join(&part);
                }
            }
        }
        for (k, val) in vals {
            let sure = strong && self.heap.obj(oid).single && k != intern::STAR && srcrow.definite.contains(k) && recs.iter().all(|x| state.row(*x, ns_of(self, *x)).definite.contains(k));
            state.set_slot(oid, ns, k, val, sure);
        }
    }

    /// A ChainMap's merged keys and values, rebuilt from its maps (the first one wins).
    pub fn refresh_chainmap(&mut self, oid: u32, state: &mut State) {
        let maps = self.m.chainmaps.get(&oid).cloned().unwrap_or_default();
        state.set_row(oid, Ns::Item, Row::new());
        state.drop_obj_ns(oid, Ns::Item);
        for sa in maps.iter().rev() {
            if !self.record_objects(sa).is_empty() {
                self.copy_into(oid, Ns::Item, sa, state, true, &KeySet::new());
            } else {
                let r = state.row(oid, Ns::Item).data_store(Source::new(&format!("ChainMap@{}", oid), SourceKind::Data));
                state.set_row(oid, Ns::Item, r);
            }
        }
    }

    pub fn delete_item(&mut self, obj: &Value, ns: Ns, key: &Value, node: &Node, state: &mut State, report: bool) {
        if ns == Ns::Item && self.split_dispatch(obj, &|an, o| an.has_user_method(o, "__delitem__")) {
            self.per_receiver(obj, state, &mut |an, o, st| {
                an.delete_item(o, ns, key, node, st, report);
                Value::new()
            });
            return;
        }
        let objs: Vec<u32> = obj.pts.iter().collect();
        self.kill_facts_on(&objs, state);
        state.drop_name_keys(None);
        let targets = self.record_objects(obj);
        for &oid in &targets {
            let kind = self.heap.kind(oid);
            let strong = targets.len() == 1 && self.heap.obj(oid).single;
            if kind == Kind::Frame && ns == Ns::Item {
                crate::models::pandas::frame_delitem(self, oid, key, node, state, strong, report);
                continue;
            }
            // a ChainMap: the first map loses the key (KeyError when only a later map has it);
            // the view is the maps' again
            if kind == Kind::Dict && ns == Ns::Item && self.m.chainmaps.contains_key(&oid) {
                let first = self.m.chainmaps[&oid].first().cloned().unwrap_or_default();
                self.delete_item(&first, ns, key, node, state, report);
                self.refresh_chainmap(oid, state);
                continue;
            }
            if kind == Kind::Instance && ns == Ns::Item {
                if let Some(cls) = self.heap.instance_class(oid) {
                    if let Some(m) = self.class_lookup(cls, "__delitem__") {
                        let fv = self.cls_slots.get(&(m, Ns::Attr, key_str_lit("__delitem__"))).cloned().unwrap_or_default();
                        for f in fv.pts.iter() {
                            if self.heap.kind(f) == Kind::Function {
                                self.call_user(f, Some(Value::obj(oid)), vec![key.clone()], IndexMap::new(), vec![], vec![], state, node, None, false);
                            }
                        }
                        continue;
                    }
                }
            }
            let ckey = Value::with_kv(key.pts.clone(), self.canon_kv(oid, ns, &key.kv, state));
            let key = &ckey;
            let mut row = state.row(oid, ns);
            let is_lit = matches!(key.kv, KeyVal::Lit(_));
            let mut raise: Option<(bool, KeyId)> = None;
            for k in intern::sorted_by_str(crate::domain::kv_literals(&key.kv).iter()) {
                let cls = classify(&row, k);
                if report && !self.is_guarded(ns) {
                    self.verdict(node, k, &cls, oid, ns, None, None);
                    if cls.tier == Tier::Input {
                        self.require_providers_k(&cls, &row, false, state, Some(k), ns);
                    }
                }
                // (a `del` of a key that is not there raises KeyError)
                if !matches!(cls.tier, Tier::Proven | Tier::Map) {
                    let surely = cls.tier == Tier::Missing && is_lit && strong;
                    raise = Some((surely || raise.map(|r| r.0).unwrap_or(false), k));
                }
                row = row.delete(k, strong && is_lit);
                state.pop_slot(oid, ns, k);
            }
            if let Some((surely, k)) = raise.filter(|_| ns == Ns::Item && matches!(kind, Kind::Dict | Kind::Unknown)) {
                state.set_row(oid, ns, row.clone());
                let exc = if ns == Ns::Item { "KeyError" } else { "AttributeError" };
                if !self.tries.is_empty() {
                    let e = self.exception_object(exc, vec![Value::lit(k)], node, state);
                    self.cur_exc = Some(e);
                }
                if surely {
                    self.raise_here(exc, state);
                } else {
                    self.may_raise_here(exc, state);
                }
                self.cur_exc = None;
                if surely {
                    continue;
                }
            }
            // an unknown key: any key may be the one deleted
            if !key.kv.is_literal() {
                row = row.mutate(s(&format!("del@L{}", node.line)));
            }
            state.set_row(oid, ns, row);
        }
    }

    // ---------------------------------------------------------- attributes
    pub fn load_attr(&mut self, obj: &Value, name: &str, node: &Node, state: &mut State, report: bool) -> Value {
        if self.split_dispatch(obj, &|an, o| an.attr_runs_user_code(o, name, false)) {
            return self.per_receiver(obj, state, &mut |an, o, st| an.load_attr(o, name, node, st, report));
        }
        let unk = self.unkey(obj);
        let obj = &unk;
        if let Some(v) = self.t_getattr(obj, name, node, state) {
            return v;
        }
        let mut acc = Value::new();
        // (one k-way union of all targets' values: a receiver of many objects, joined pairwise,
        // costs the product of their sizes)
        let mut parts: Vec<Value> = Vec::new();
        let guarded = self.is_guarded(Ns::Attr) || !report;
        // (a value that may also be a scalar — a str, a number — may have the attribute: no
        // object's lack of it makes the read fail for sure)
        let scalar = self.may_be_scalar(obj);
        let single = obj.pts.len() == 1 && !scalar;
        let saved_scalar = std::mem::replace(&mut self.recv_scalar, scalar);
        let nk = key_str_lit(name);
        for oid in obj.pts.iter() {
            let k = self.heap.kind(oid);
            let v: Value = match k {
                Kind::Module => match state.slot_opt(oid, Ns::Attr, nk) {
                    Some(v) => v,
                    // (what every module has)
                    None if matches!(name, "__name__" | "__file__" | "__doc__" | "__package__" | "__spec__" | "__loader__" | "__path__" | "__cached__" | "__builtins__") => {
                        if name == "__name__" { Value::lit_str(&self.heap.module_info(oid).name) } else { Value::data(&format!("module {}", name)) }
                    }
                    // (inside a try that catches the AttributeError: the handler's way)
                    None if report && guarded => self.load_record(oid, Ns::Attr, &Value::lit(nk), node, state, guarded, single, true),
                    None => {
                        // (a namespace changed by code rowan cannot see — `globals().update(…)`
                        // with unknown names — may have it)
                        let row = state.row(oid, Ns::Attr);
                        let cls = if row.closed() { Classification::with_note(Tier::Missing, "is not defined in the module") } else { classify(&row, nk) };
                        if !guarded {
                            self.verdict(node, nk, &cls, oid, Ns::Attr, None, None);
                        }
                        Value::data(&format!("L{}", node.line))
                    }
                },
                Kind::External if matches!(name, "__name__" | "__qualname__") && self.heap.external_qual(oid).starts_with("builtins.") => {
                    let q = self.heap.external_qual(oid);
                    Value::lit(key_str_lit(q.rsplit('.').next().unwrap_or("")))
                }
                Kind::External => {
                    let q = format!("{}.{}", self.heap.external_qual(oid), name);
                    if let Some(v) = crate::models::generated_class(self, state, &q) {
                        parts.push(v);
                        continue;
                    }
                    if let Some(v) = crate::models::external_constant(self, &q) {
                        parts.push(v);
                        continue;
                    }
                    match crate::models::external_record(self, &q, state) {
                        Some(rec) => Value::obj(rec),
                        None => Value::obj(self.heap.alloc_raw(Kind::External, &format!("ext:{}", q), "", true, &q, Payload::Str(q.clone()))),
                    }
                }
                Kind::Partial if matches!(name, "func" | "args" | "keywords") => crate::models::partial_attr(self, oid, name, node, state),
                Kind::Parser | Kind::Groupby | Kind::Partial => self.method_value(&format!("{}.{}", k.name(), name), &Value::obj(oid)),
                Kind::Instance if name == "__class__" => match self.heap.instance_class(oid) {
                    Some(c) => Value::obj(c),
                    None => Value::data(&format!("L{}", node.line)),
                },
                Kind::Namespace | Kind::Instance if name == "__dict__" => {
                    let vid = self.heap.alloc_raw(Kind::View, &format!("view:attrs:{}", oid), "", true, "__dict__", Payload::View(ViewKind::Attrs, Value::obj(oid)));
                    self.heap.obj_mut(vid).payload = Payload::View(ViewKind::Attrs, Value::obj(oid));
                    Value::obj(vid)
                }
                Kind::Namespace | Kind::List | Kind::View if crate::models::model_object_attr(self, oid, name, node, state).map(|v| parts.push(v)).is_some() => continue,
                Kind::Namespace | Kind::Instance => self.load_instance_attr(oid, name, node, state, guarded, report && guarded, single),
                Kind::Class if name == "__annotations__" && self.class_lookup(oid, name).is_none() && self.heap.class_info_opt(oid).is_some() => {
                    let names = crate::models::annotated_names(self, oid, false).unwrap_or_default();
                    crate::models::names_dict(self, state, node, &names, None, "annotation")
                }
                Kind::Class if crate::models::record_class_attr(self, oid, name, node, state).map(|v| parts.push(v)).is_some() => continue,
                Kind::Class if matches!(name, "__name__" | "__qualname__") && self.class_lookup(oid, name).is_none() => {
                    let n = self.heap.class_info_opt(oid).map(|ci| ci.name.clone()).unwrap_or_default();
                    Value::lit(key_str_lit(&n))
                }
                Kind::Class => {
                    if self.class_lookup(oid, name).is_some() {
                        // a classmethod reached through a class binds that class (not the
                        // class that defines it): `Sub.make()` constructs `Sub`
                        let v = self.class_attr_value(oid, name, state);
                        let mut out = Value::new();
                        let mut changed = false;
                        for f in v.pts.iter() {
                            if self.heap.kind(f) == Kind::Function && self.heap.func_info(f).is_classmethod.get() {
                                let label = format!("{} bound", self.heap.obj(f).label);
                                out = out.join(&Value::obj(self.heap.alloc_raw(Kind::Bound, &format!("bound:{}:cls{}", f, oid), "", true, &label, Payload::Bound(f, Value::obj(oid)))));
                                changed = true;
                            } else {
                                out = out.join(&Value::obj(f));
                            }
                        }
                        if changed { Value::with_kv(out.pts, v.kv.clone()) } else { v }
                    } else if self.enum_members.contains_key(&oid) && (name == "__members__" || (!name.starts_with("__") && !self.heap.class_info(oid).open_base)) {
                        crate::models::enums::class_attr(self, oid, name, node, guarded, state)
                    } else if crate::models::pydantic::PYD_ATTRS.contains(&name) && crate::models::pydantic::is_model(self, oid) {
                        crate::models::pydantic::attr_value(self, state, oid, name, node)
                    } else if crate::models::pydantic::PYD_METHODS.contains(&name) && crate::models::pydantic::is_model(self, oid) {
                        self.method_value(&format!("pydantic.cls.{}", name), &Value::obj(oid))
                    } else if name == "__init__" && self.dataclass_init_owner(oid).is_some() {
                        let dc = self.dataclass_init_owner(oid).unwrap();
                        Value::obj(self.heap.alloc_raw(Kind::Method, &format!("dcinit:{}", oid), "", true, &format!("{}.__init__", self.heap.obj(oid).label), Payload::Method(format!("dcinit.class.{}", dc), Value::obj(oid))))
                    } else {
                        let label = format!("{}.{}", self.heap.obj(oid).label, name);
                        Value::obj(self.heap.alloc_raw(Kind::Method, &format!("classattr:{}.{}", oid, name), "", true, &label, Payload::Method(format!("class.{}", name), Value::obj(oid))))
                    }
                }
                Kind::Dict if crate::models::pandas::is_series_obj(self, oid) => crate::models::pandas::series_getattr(self, oid, name, node, state),
                Kind::Dict if self.m.cfg_parsers.contains_key(&oid) && crate::models::configparser::PARSER_METHODS.contains(&name) => self.method_value(&format!("cfgparser.{}", name), &Value::obj(oid)),
                Kind::Dict if self.m.cfg_sections.contains_key(&oid) && crate::models::configparser::SECTION_METHODS.contains(&name) => self.method_value(&format!("cfgsection.{}", name), &Value::obj(oid)),
                Kind::Dict if self.m.cfg_sections.contains_key(&oid) && name == "name" => Value::lit_str(&self.m.cfg_sections[&oid].1),
                Kind::Dict if self.m.counters.contains(&oid) && crate::models::counter::COUNTER_METHODS.contains(&name) => self.method_value(&format!("counter.{}", name), &Value::obj(oid)),
                Kind::Dict if self.m.chainmaps.contains_key(&oid) && matches!(name, "maps" | "parents") => {
                    let c = super::Call { qual: format!("chainmap.{}", name), recv: Some(Value::obj(oid)), args: vec![], kwargs: IndexMap::new(), star_args: vec![], star_kwargs: vec![], node: *node, call_ast: None, site: format!("L{}", node.line), line: node.line };
                    match self.models.lookup(&c.qual) {
                        Some(m) => m(self, state, &c),
                        None => Value::data("chainmap"),
                    }
                }
                Kind::Dict if self.m.chainmaps.contains_key(&oid) && name == "new_child" => self.method_value("chainmap.new_child", &Value::obj(oid)),
                // a TestClient cookie jar's set / delete
                Kind::Dict if matches!(name, "set" | "delete") && self.m.cookie_jars.contains(&oid) => self.method_value(&format!("fastapi.cookies.{}", name), &Value::obj(oid)),
                Kind::Dict => self.method_value(&format!("dict.{}", name), &Value::obj(oid)),
                Kind::List if self.m.deques.contains_key(&oid) && crate::models::counter::DEQUE_METHODS.contains(&name) => {
                    if name == "maxlen" {
                        match self.m.deques[&oid] {
                            Some(m) => Value::lit(intern::key_int(m)),
                            None => self.none_v.clone(),
                        }
                    } else {
                        self.method_value(&format!("deque.{}", name), &Value::obj(oid))
                    }
                }
                Kind::List | Kind::Tuple | Kind::Set => self.method_value(&format!("{}.{}", k.name(), name), &Value::obj(oid)),
                Kind::Frame => crate::models::pandas::frame_getattr(self, oid, name, node, state, guarded),
                Kind::Series => crate::models::pandas::series_getattr(self, oid, name, node, state),
                Kind::Locidx if matches!(&self.heap.obj(oid).payload, Payload::Locidx(a, _) if matches!(a.as_str(), "str" | "dt" | "cat" | "sparse" | "list" | "struct")) => {
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
                Kind::Unknown => self.load_unknown_attr(oid, name, node, state, guarded),
                // an lru_cache'd function's cache_info / cache_clear
                Kind::Function if matches!(name, "cache_info" | "cache_clear") && self.heap.func_info(oid).is_memo.get() => self.method_value(&format!("memo.{}", name), &Value::obj(oid)),
                // a function's names (a `@wraps(fn)` wrapper: fn's)
                Kind::Function if name == "__annotations__" && state.slot_opt(oid, Ns::Attr, nk).is_none() => {
                    let names = crate::models::annotated_names(self, oid, false).unwrap_or_default();
                    crate::models::names_dict(self, state, node, &names, None, "annotation")
                }
                Kind::Function if matches!(name, "__name__" | "__qualname__" | "__module__" | "__wrapped__" | "__doc__") && state.slot_opt(oid, Ns::Attr, nk).is_none() => {
                    match self.wrapped.get(&oid).cloned() {
                        Some(w) if name == "__wrapped__" => w,
                        Some(w) => self.load_attr(&w, name, node, state, false),
                        None => {
                            let fi = self.heap.func_info(oid);
                            match name {
                                "__name__" => Value::lit_str(fi.qualname.rsplit('.').next().unwrap_or(&fi.qualname)),
                                "__qualname__" => Value::lit_str(&fi.py_qualname()),
                                "__module__" => Value::lit_str(&self.heap.module_info(fi.module).name),
                                "__doc__" => Value::data("doc"),
                                _ => {
                                    let _ = report;
                                    Value::data(name)
                                }
                            }
                        }
                    }
                }
                // a singledispatch function's register
                Kind::Function if name == "register" && self.sdispatch.contains_key(&oid) => self.method_value("sdispatch.register", &Value::obj(oid)),
                Kind::Function => state.slot_opt(oid, Ns::Attr, nk).unwrap_or_else(|| Value::data(&format!("L{}", node.line))),
                Kind::Bound => Value::data(&format!("L{}", node.line)),
                Kind::View => {
                    // a proxy's read methods are the dict's (it has no mutators)
                    if let Payload::View(ViewKind::Proxy, inner) = self.heap.obj(oid).payload.clone() {
                        if matches!(name, "get" | "keys" | "values" | "items" | "copy") {
                            acc = acc.join(&self.method_value(&format!("dict.{}", name), &inner));
                        } else {
                            acc = acc.join(&self.load_attr(&Value::data("proxy"), name, node, state, report));
                        }
                        continue;
                    }
                    if let Payload::View(ViewKind::Keys, recv) = &self.heap.obj(oid).payload {
                        if name == "str" {
                            acc = acc.join(&Value::kv_only(KeyVal::KeysOf(crate::domain::KeysOf::new(recv.pts.clone(), Ns::Item))));
                            continue;
                        }
                    }
                    self.method_value(&format!("view.{}", name), &Value::obj(oid))
                }
                Kind::Zip | Kind::Enumerate => self.method_value(&format!("{}.{}", k.name(), name), &Value::obj(oid)),
                Kind::Response => self.method_value(&format!("response.{}", name), &Value::obj(oid)),
                Kind::Locidx => Value::data("loc"),
                Kind::Super => {
                    let Payload::Super(cls, self_v) = self.heap.obj(oid).payload.clone() else { continue };
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
                        let order = self.mro(rc);
                        if let Some(pos) = order.iter().position(|c| *c == cls) {
                            found = order[pos + 1..].iter().copied().find(|c| self.cls_slots.contains_key(&(*c, Ns::Attr, nk)));
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
                    match found {
                        // (`__new__` is a static method: `super().__new__(cls)` passes the class)
                        Some(fc) if name == "__new__" => self.cls_slots.get(&(fc, Ns::Attr, nk)).cloned().unwrap_or_default(),
                        None if name == "__new__" => {
                            let q = "builtins.object.__new__";
                            Value::obj(self.heap.alloc_raw(Kind::External, &format!("ext:{}", q), "", true, q, Payload::Str(q.to_string())))
                        }
                        Some(fc) => {
                            let fv = self.cls_slots.get(&(fc, Ns::Attr, nk)).cloned().unwrap_or_default();
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
                        None if name == "__init__" && ci.bases.iter().any(|b| self.dataclass_init_owner(*b).is_some()) => {
                            let dc = ci.bases.iter().find_map(|b| self.dataclass_init_owner(*b)).unwrap();
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
                    }
                }
                _ => Value::data(&format!("L{}", node.line)),
            };
            parts.push(v);
        }
        self.recv_scalar = saved_scalar;
        acc = acc.join(&crate::domain::join_values(parts));
        if obj.pts.is_empty() {
            return crate::models::scalar_attr(self, obj, name, report, state);
        }
        acc
    }

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

    /// Breadth-first linearisation of the user classes above `cls` (when C3 fails).
    pub fn mro_bfs(&self, cls: u32) -> Vec<u32> {
        let mut seen: Vec<u32> = Vec::new();
        let mut order = vec![cls];
        let mut out: Vec<u32> = Vec::new();
        while !order.is_empty() {
            let c = order.remove(0);
            if seen.contains(&c) || !self.heap.has_obj(c) {
                continue;
            }
            seen.push(c);
            out.push(c);
            if let Some(ci) = self.heap.class_info_opt(c) {
                order.extend(ci.bases.iter().copied());
            }
        }
        out
    }

    /// Return the class oid (in MRO) that defines `name`, or None.
    /// The C3 method resolution order of a user class (user classes only).
    pub fn mro(&self, cls: u32) -> Vec<u32> {
        fn c3(an: &Analyzer, cls: u32, depth: usize) -> Option<Vec<u32>> {
            if depth > 64 {
                return None;
            }
            let bases: Vec<u32> = an.heap.class_info_opt(cls).map(|ci| ci.bases.clone()).unwrap_or_default();
            let mut seqs: Vec<Vec<u32>> = Vec::new();
            for b in &bases {
                seqs.push(c3(an, *b, depth + 1)?);
            }
            seqs.push(bases.clone());
            let mut out = vec![cls];
            loop {
                seqs.retain(|s| !s.is_empty());
                if seqs.is_empty() {
                    return Some(out);
                }
                // the first head that is in no other sequence's tail
                let head = seqs.iter().map(|s| s[0]).find(|h| seqs.iter().all(|s| !s[1..].contains(h)))?;
                out.push(head);
                for s in seqs.iter_mut() {
                    if s[0] == head {
                        s.remove(0);
                    }
                }
            }
        }
        c3(self, cls, 0).unwrap_or_else(|| self.mro_bfs(cls))
    }

    pub fn class_lookup(&self, cls: u32, name: &str) -> Option<u32> {
        let nk = key_str_lit(name);
        let order = self.mro(cls);
        if !order.is_empty() {
            return order.into_iter().find(|c| self.cls_slots.contains_key(&(*c, Ns::Attr, nk)));
        }
        let mut seen: Vec<u32> = Vec::new();
        let mut order = vec![cls];
        while !order.is_empty() {
            let c = order.remove(0);
            if seen.contains(&c) {
                continue;
            }
            seen.push(c);
            if self.cls_slots.contains_key(&(c, Ns::Attr, nk)) {
                return Some(c);
            }
            if let Some(ci) = self.heap.class_info_opt(c) {
                order.extend(ci.bases.iter().copied());
            }
        }
        None
    }


    /// A class attribute's value: on this path when the path has it (a class attribute
    /// rebound after the class was made: `Cls.mode = "x"`), else what any path stored.
    pub fn class_attr_value(&self, cls: u32, name: &str, state: &State) -> Value {
        match self.class_lookup(cls, name) {
            None => Value::new(),
            Some(c) => state.slot_opt(c, Ns::Attr, key_str_lit(name)).or_else(|| self.cls_slots.get(&(c, Ns::Attr, key_str_lit(name))).cloned()).unwrap_or_default(),
        }
    }

    fn bound_outs(&mut self, cv: &Value, oid: u32, cls: u32, name: &str, node: &Node, state: &mut State) -> Value {
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
                    let r = self.call_user(f, Some(Value::obj(oid)), vec![], IndexMap::new(), vec![], vec![], state, node, None, false);
                    if fi.is_cached.get() && matches!(self.heap.kind(oid), Kind::Instance) {
                        // stored on the instance: later accesses find this value
                        let strong = self.strongable(oid);
                        self.store_key(oid, Ns::Attr, &KeyVal::Lit(key_str_lit(name)), &r, strong, node, state);
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
            } else {
                outs.push(Value::obj(f));
            }
        }
        if cv.pts.is_empty() {
            outs.push(cv.clone());
        }
        join_values(outs)
    }

    pub fn load_instance_attr(&mut self, oid: u32, name: &str, node: &Node, state: &mut State, guarded: bool, try_guard: bool, single_target: bool) -> Value {
        let nk = key_str_lit(name);
        // a pydantic private attribute: its hidden slot
        if let Some(cls) = self.heap.instance_class(oid) {
            if name.starts_with('_') && self.pyd_is_private(cls, name, state) {
                if !guarded {
                    self.verdict(node, nk, &Classification::new(Tier::Proven), oid, Ns::Attr, None, None);
                }
                return state.slot(oid, Ns::Attr, crate::models::pydantic::priv_key(name));
            }
            if name == "__pydantic_private__" && crate::models::pydantic::is_model(self, cls) {
                let pc = self.pyd_class(cls, state);
                let did = self.alloc_at(Kind::Dict, node, state, &format!("private@L{}", node.line), Payload::None, Some(Row::new()), Ns::Item, None, Some(&format!("priv{}", oid)));
                let mut row = Row::new();
                for (pn, _, _) in pc.privates.iter() {
                    let k = key_str_lit(pn);
                    row = row.store(k, true);
                    let v = state.slot(oid, Ns::Attr, crate::models::pydantic::priv_key(pn));
                    state.set_slot(did, Ns::Item, k, v, true);
                }
                state.set_row(did, Ns::Item, row);
                return Value::obj(did);
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
                    return match name {
                        "__module__" | "__doc__" | "__weakref__" => Value::data(&format!("{}@L{}", name, node.line)),
                        _ => self.method_value(&format!("object.{}", name), &Value::obj(oid)),
                    };
                }
            }
        }
        // pydantic's API on a model instance (`model_dump`, `model_copy`, …)
        if crate::models::pydantic::PYD_METHODS.contains(&name) {
            if let Some(cls) = self.heap.instance_class(oid) {
                if self.class_lookup(cls, name).is_none() && crate::models::pydantic::is_model(self, cls) && !state.row(oid, Ns::Attr).present.contains(nk) {
                    if crate::models::pydantic::PYD_ATTRS.contains(&name) {
                        return crate::models::pydantic::attr_value(self, state, oid, name, node);
                    }
                    return self.method_value(&format!("pydantic.{}", name), &Value::obj(oid));
                }
            }
        }
        // a `threading.Thread` subclass's `start()` runs its `run()`
        if matches!(name, "start" | "join") {
            if let Some(cls) = self.heap.instance_class(oid).filter(|c| self.class_lookup(*c, name).is_none()) {
                if self.mro(cls).iter().any(|c| self.heap.class_info_opt(*c).is_some_and(|ci| ci.lib.quals.iter().any(|q| q == "threading.Thread"))) {
                    return self.method_value(&format!("threadsub.{}", name), &Value::obj(oid));
                }
            }
        }
        // the methods every NamedTuple has
        if matches!(name, "_asdict" | "_replace" | "_fields") {
            if let Some(cls) = self.heap.instance_class(oid) {
                if self.class_lookup(cls, name).is_none() && self.is_named_tuple_class(cls) {
                    if name == "_fields" {
                        let fs: Vec<Value> = self.dataclass_fields_mro(cls).iter().map(|(f, _)| Value::lit(key_str_lit(f))).collect();
                        let kv = KeyVal::lits(fs.iter().filter_map(|v| v.kv.as_lit()).collect());
                        return self.alloc_positional(Kind::Tuple, node, Some(fs), Value::new(), kv, state);
                    }
                    return self.method_value(&format!("namedtuple.{}", name), &Value::obj(oid));
                }
            }
        }
        // `__getattr__` answers what normal lookup does not find
        if let Some(cls) = self.heap.instance_class(oid) {
            if !name.starts_with("__") && self.class_lookup(cls, name).is_none() && self.class_lookup(cls, "__getattr__").is_some() {
                let row = state.row(oid, Ns::Attr);
                if !row.definite.contains(nk) {
                    if !row.present.contains(nk) {
                        return self.call_method_with(&Value::obj(oid), "__getattr__", vec![Value::lit(nk)], node, state).unwrap_or_default();
                    }
                    // (set on some paths: `__getattr__` runs on the others only — when it raises
                    // there, the paths that have the attribute go on)
                    let mut missing = state.copy();
                    let fallback = self.call_method_with(&Value::obj(oid), "__getattr__", vec![Value::lit(nk)], node, &mut missing).unwrap_or_default();
                    let own = state.slot(oid, Ns::Attr, nk);
                    if missing.reachable {
                        let j = state.join(&mut missing);
                        state.replace_with(j);
                    }
                    return own.join(&fallback);
                }
            }
        }
        let v = state.slot_opt(oid, Ns::Attr, nk);
        let row = self.read_row(oid, Ns::Attr, state);
        let cls_of = self.heap.instance_class(oid);
        if let Some(mut v) = v {
            // (set on some paths only, over a class attribute of the name: the others read the
            // class's — the read cannot fail)
            let class_attr = cls_of.filter(|c| !row.definite.contains(nk) && !self.dataclass_fields_mro(*c).iter().any(|(f, _)| f == name)).and_then(|c| self.class_lookup(c, name));
            if let (true, Some(defc)) = (row.present.contains(nk), class_attr) {
                let cv = state.slot_opt(defc, Ns::Attr, nk).or_else(|| self.cls_slots.get(&(defc, Ns::Attr, nk)).cloned()).unwrap_or_default();
                let plain = !cv.pts.iter().any(|f| matches!(self.heap.kind(f), Kind::Function | Kind::Bound));
                if plain {
                    if !guarded {
                        self.verdict(node, nk, &Classification::new(Tier::Proven), oid, Ns::Attr, None, None);
                    }
                    return v.join(&cv);
                }
            }
            if row.present.contains(nk) {
                let cls = classify(&row, nk);
                if !guarded {
                    self.verdict(node, nk, &cls, oid, Ns::Attr, None, None);
                } else {
                    self.verdict_guarded(node, nk, &cls, oid, Ns::Attr);
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
        if let Some(c) = cls_of {
            let is_field = self.dataclass_fields_mro(c).iter().any(|(f, _)| f == name);
            if let (Some(defc), false) = (self.class_lookup(c, name), is_field) {
                let cv = state.slot_opt(defc, Ns::Attr, nk).or_else(|| self.cls_slots.get(&(defc, Ns::Attr, nk)).cloned()).unwrap_or_default();
                // a class attribute: the access succeeds (a method read counts only where the
                // receiver may be other objects too, which may lack it)
                let plain = !cv.pts.iter().any(|f| matches!(self.heap.kind(f), Kind::Function | Kind::Bound) && !(self.heap.kind(f) == Kind::Function && self.heap.func_info(f).is_property.get()));
                if !guarded && (plain || !single_target) {
                    self.verdict(node, nk, &Classification::new(Tier::Proven), oid, Ns::Attr, None, None);
                }
                return self.bound_outs(&cv, oid, c, name, node, state);
            }
        }
        let cls = classify(&row, nk);
        if !guarded {
            self.verdict(node, nk, &cls, oid, Ns::Attr, None, None);
            if cls.tier == Tier::Input {
                self.require_providers_k(&cls, &row, false, state, Some(nk), Ns::Attr);
            }
            if cls.tier != Tier::Missing && single_target && self.strongable(oid) {
                state.set_row(oid, Ns::Attr, row.narrow_in(nk, true));
                self.propagate_narrowing(oid, Ns::Attr, nk, state);
            }
        } else {
            self.verdict_guarded(node, nk, &cls, oid, Ns::Attr);
            // (a read that cannot fail, `getattr(o, k, None)`, raises nothing; a try-guarded one may)
            if try_guard && !matches!(cls.tier, Tier::Proven | Tier::Likely | Tier::Map) {
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

    /// A value declared dynamically typed (`Any`): accesses on it are not checkable (§6.6).
    pub fn dynamic_value(&mut self, label: &str, state: &mut State) -> Value {
        let oid = self.heap.alloc_raw(Kind::Unknown, &format!("any:{}", label), "", true, &format!("Any {}", label), Payload::None);
        if !state.has_row(oid, Ns::Item) {
            state.set_row(oid, Ns::Item, Row::top());
            state.set_row(oid, Ns::Attr, Row::top());
        }
        Value::obj(oid)
    }

    /// Is the attribute declared `Any` (or `object`) in the class body or its bases?
    pub fn field_is_any(&self, cls: u32, name: &str) -> bool {
        for c in self.mro(cls) {
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
    pub fn may_be_scalar(&self, v: &Value) -> bool {
        // (an unknown object stands for any value, scalars too: its scalar part adds nothing)
        !v.pts.is_empty() && !v.kv.is_bot() && v.kv.as_lit() != Some(intern::key_none()) && v.pts.iter().any(|o| self.heap.kind(o) != Kind::Unknown)
    }

    /// An unknown object's row; none on this path (collected, or never made here): nothing is
    /// known of it (an empty, closed row would claim every key missing).
    /// The row a read classifies against: of an object this path has no facts about at all
    /// (made on other paths: a recursion's deeper activations make what its widened
    /// arguments hold), nothing is known.
    pub fn read_row(&self, oid: u32, ns: Ns, state: &State) -> Row {
        if self.has_facts(oid, state) { state.row(oid, ns) } else { Row::top() }
    }

    pub fn unknown_row(&self, oid: u32, ns: Ns, state: &State) -> Row {
        if state.has_row(oid, ns) {
            state.row(oid, ns)
        } else {
            Row::top()
        }
    }

    pub fn load_unknown_attr(&mut self, oid: u32, name: &str, node: &Node, state: &mut State, guarded: bool) -> Value {
        let nk = key_str_lit(name);
        if DICT_METHOD_NAMES.contains(&name) {
            return self.method_value(&format!("dict.{}", name), &Value::obj(oid));
        }
        if is_scalar_method(name) {
            // a mutating method of a list-like unknown: facts about tests on it are void
            if matches!(name, "append" | "extend" | "insert" | "remove" | "clear" | "sort" | "reverse" | "pop") {
                self.kill_facts_on(&[oid], state);
            }
            let q = if STR_METHOD_NAMES.contains(&name) { format!("str.{}", name) } else { format!("unknown.{}", name) };
            return self.method_value(&q, &Value::data(&format!("L{}", node.line)));
        }
        let row = self.unknown_row(oid, Ns::Attr, state);
        if let Some(v) = state.slot_opt(oid, Ns::Attr, nk) {
            return v;
        }
        let cls = classify(&row, nk);
        if guarded && cls.tier != Tier::Map {
            self.verdict_guarded(node, nk, &cls, oid, Ns::Attr);
        }
        if !guarded && cls.tier != Tier::Map {
            self.verdict(node, nk, &cls, oid, Ns::Attr, None, None);
            // (a method called on an input — `df.groupby(…)` — is not a key the input must have)
            if cls.tier == Tier::Input && !self.callee_nodes.contains(&node.addr) {
                self.require_providers_k(&cls, &row, false, state, Some(nk), Ns::Attr);
            }
        }
        let ph = self.path_placeholder_value(oid, Ns::Attr, nk, &row, state);
        if !ph.pts.is_empty() {
            return ph;
        }
        self.method_value(&format!("unknown.{}", name), &Value::obj(oid))
    }

    pub fn store_attr(&mut self, obj: &Value, name: &str, v: &Value, node: &Node, state: &mut State) {
        if self.split_dispatch(obj, &|an, o| an.attr_runs_user_code(o, name, true)) {
            self.per_receiver(obj, state, &mut |an, o, st| {
                an.store_attr(o, name, v, node, st);
                Value::new()
            });
            return;
        }
        if !state.narrowed.is_empty() {
            let needle = format!(".{}", name);
            state.narrowed.retain(|k, _| !intern::str_get(*k).contains(&needle));
        }
        let nk = key_str_lit(name);
        for o in obj.pts.iter() {
            if self.heap.kind(o) == Kind::View {
                crate::models::pandas::view_setattr(self, o, name, v, state);
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
            // a frozen dataclass: assignment raises FrozenInstanceError (an AttributeError)
            if kind == Kind::Instance {
                if let Some(cls) = self.heap.instance_class(oid) {
                    if self.mro(cls).iter().any(|c| self.heap.class_info_opt(*c).map(|ci| ci.dc_frozen.get()).unwrap_or(false)) && !self.fresh_init.contains(&oid) {
                        self.raise_here("FrozenInstanceError", state);
                        continue;
                    }
                    // a NamedTuple's fields are read-only, and it has no instance dict
                    if self.is_named_tuple_class(cls) && self.class_lookup(cls, "__setattr__").is_none() {
                        self.raise_here("AttributeError", state);
                        continue;
                    }
                }
            }
            // a pydantic model's field / private attribute (`models::pydantic::model_setattr`)
            if kind == Kind::Instance && crate::models::pydantic::model_setattr(self, oid, name, v, strong, node, state) {
                continue;
            }
            // a property with a setter: the setter runs instead of the store
            let cls_of = match kind {
                Kind::Instance => self.heap.instance_class(oid),
                _ => None,
            };
            if let Some(cls) = cls_of {
                if let Some(defc) = self.class_lookup(cls, name) {
                    if let Some(&setter) = self.prop_setters.get(&(defc, nk)) {
                        self.call_user(setter, Some(Value::obj(oid)), vec![v.clone()], IndexMap::new(), vec![], vec![], state, node, None, false);
                        continue;
                    }
                    // a property without a setter: the assignment raises
                    if kind == Kind::Instance {
                        let cv = state.slot_opt(defc, Ns::Attr, nk).or_else(|| self.cls_slots.get(&(defc, Ns::Attr, nk)).cloned()).unwrap_or_default();
                        if !cv.pts.is_empty() && cv.pts.iter().all(|f| self.heap.kind(f) == Kind::Function && self.heap.func_info(f).is_property.get()) {
                            self.raise_here("AttributeError", state);
                            continue;
                        }
                    }
                    // a data descriptor (an object whose class has `__set__`) takes the store
                    let cv = state.slot_opt(defc, Ns::Attr, nk).or_else(|| self.cls_slots.get(&(defc, Ns::Attr, nk)).cloned()).unwrap_or_default();
                    let descs: Vec<u32> = cv.pts.iter().filter(|d| self.heap.kind(*d) == Kind::Instance && self.heap.instance_class(*d).and_then(|dc| self.class_lookup(dc, "__set__")).is_some()).collect();
                    if !descs.is_empty() {
                        for d in descs {
                            self.call_method_with(&Value::obj(d), "__set__", vec![Value::obj(oid), v.clone()], node, state);
                        }
                        continue;
                    }
                }
                // a user `__setattr__` takes every assignment (`object.__setattr__` bypasses it)
                if !self.bypass_setattr && self.class_lookup(cls, "__setattr__").is_some() {
                    self.call_method_with(&Value::obj(oid), "__setattr__", vec![Value::lit(nk), v.clone()], node, state);
                    continue;
                }
            }
            // `__slots__` on every class of the MRO (no `__dict__`): another name raises
            if kind == Kind::Instance {
                if let Some(allowed) = self.heap.instance_class(oid).and_then(|c| self.slots_of(c)) {
                    if !allowed.iter().any(|a| a == name) {
                        self.raise_here("AttributeError", state);
                        continue;
                    }
                }
            }
            let strong_key = strong || (targets.len() == 1 && self.fresh_init.contains(&oid));
            if kind == Kind::Unknown && !strong && state.slot_opt(oid, Ns::Attr, nk).is_none() {
                // a weak store on open content (an element under an unknown key, say)
                // joins with what was there, which is unknown
                let row = state.row(oid, Ns::Attr);
                let prior = self.path_placeholder_value(oid, Ns::Attr, nk, &row, state);
                state.set_slot(oid, Ns::Attr, nk, prior, true);
            }
            let r = state.row(oid, Ns::Attr).store(nk, strong_key);
            state.set_row(oid, Ns::Attr, r);
            state.set_slot(oid, Ns::Attr, nk, v.clone(), strong);
            if kind == Kind::Class {
                self.cls_slots.insert((oid, Ns::Attr, nk), v.clone());
            }
        }
    }
}

/// All statements nested in a body (ast.walk restricted to statements).
/// builtin container classes a field may be declared with (they select a model family)

pub fn walk_stmts(body: &'static [ast::Stmt]) -> Vec<&'static ast::Stmt> {
    let mut out: Vec<&'static ast::Stmt> = Vec::new();
    fn go(s: &'static ast::Stmt, out: &mut Vec<&'static ast::Stmt>) {
        out.push(s);
        match s {
            ast::Stmt::FunctionDef(f) => f.body.iter().for_each(|x| go(x, out)),
            ast::Stmt::ClassDef(c) => c.body.iter().for_each(|x| go(x, out)),
            ast::Stmt::For(f) => {
                f.body.iter().for_each(|x| go(x, out));
                f.orelse.iter().for_each(|x| go(x, out));
            }
            ast::Stmt::While(w) => {
                w.body.iter().for_each(|x| go(x, out));
                w.orelse.iter().for_each(|x| go(x, out));
            }
            ast::Stmt::If(i) => {
                i.body.iter().for_each(|x| go(x, out));
                i.elif_else_clauses.iter().for_each(|c| c.body.iter().for_each(|x| go(x, out)));
            }
            ast::Stmt::With(w) => w.body.iter().for_each(|x| go(x, out)),
            ast::Stmt::Try(t) => {
                t.body.iter().for_each(|x| go(x, out));
                for h in &t.handlers {
                    let ast::ExceptHandler::ExceptHandler(h) = h;
                    h.body.iter().for_each(|x| go(x, out));
                }
                t.orelse.iter().for_each(|x| go(x, out));
                t.finalbody.iter().for_each(|x| go(x, out));
            }
            ast::Stmt::Match(m) => m.cases.iter().for_each(|c| c.body.iter().for_each(|x| go(x, out))),
            _ => {}
        }
    }
    body.iter().for_each(|x| go(x, &mut out));
    out
}

/// The number a literal key stands for as a dict key (`True` is 1, `2.0` is 2).
fn numeric_of(k: KeyId) -> Option<(i64, u64)> {
    intern::with_key(k, |kk| match kk {
        intern::Key::Int(i) => Some((*i, 0)),
        intern::Key::Bool(b) => Some((*b as i64, 0)),
        intern::Key::Float(bits) => {
            let f = f64::from_bits(*bits);
            if f.fract() == 0.0 && f.abs() < 9.0e15 {
                Some((f as i64, 0))
            } else {
                Some((i64::MIN, *bits))
            }
        }
        _ => None,
    })
}
