//! Attribute access: reads through the class (methods bound, properties and descriptors run,
//! `__getattr__`), instance attributes, unknown objects' attributes, and stores (`__setattr__`,
//! frozen dataclasses, `__slots__`).

use indexmap::IndexMap;
use ruff_python_ast as ast;

use crate::domain::{ObjId, classify, join_values, Classification, KeyVal, Ns, Row, Tier, Value};
use crate::frontend::{self, Node};
use crate::intern::{self, key_str_lit, KeyId};
use crate::state::{Kind, State};

use super::{is_scalar_method, Analyzer, Payload, ViewKind, DICT_METHOD_NAMES, STR_METHOD_NAMES};
use super::access::OBJECT_ATTRS;

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

    /// The class of an instance, when it is a user class.
    pub(super) fn user_class_of(&self, o: u32) -> Option<u32> {
        let c = self.heap.instance_class(o)?;
        self.heap.class_info_opt(c).map(|_| c)
    }

    pub(super) fn has_user_method(&self, o: u32, m: &str) -> bool {
        self.user_class_of(o).is_some_and(|c| self.class_lookup(c, m).is_some())
    }

    /// Reading / writing attribute `name` of `o` runs user code: a property, a descriptor,
    /// `__getattr__` / `__getattribute__` (reading), `__setattr__` (writing).
    pub(super) fn attr_runs_user_code(&self, o: u32, name: &str, store: bool) -> bool {
        let Some(c) = self.user_class_of(o) else { return false };
        let hooks: &[&str] = if store { &["__setattr__"] } else { &["__getattr__", "__getattribute__"] };
        if hooks.iter().any(|h| self.class_lookup(c, h).is_some()) {
            return true;
        }
        let Some(dc) = self.class_lookup(c, name) else { return false };
        let cv = self.heap.cls_slots.get(&(dc, Ns::Attr, key_str_lit(name))).cloned().unwrap_or_default();
        let runs = cv.pts.iter().any(|f| (self.heap.kind(f) == Kind::Function && self.heap.func_info(f).is_property.get()) || (self.heap.kind(f) == Kind::Instance && self.heap.instance_class(f).is_some_and(|fc| self.class_lookup(fc, if store { "__set__" } else { "__get__" }).is_some())));
        runs
    }

    // ---------------------------------------------------------- attributes
    pub fn load_attr(&mut self, obj: &Value, name: &str, node: &Node, state: &mut State, report: bool) -> Value {
        if let Some(v) = self.per_receiver(obj, &|an, o| an.attr_runs_user_code(o, name, false), state, &mut |an, o, st| an.load_attr(o, name, node, st, report)) {
            return v;
        }
        let unk = self.unkey(obj);
        let obj = &unk;
        if let Some(v) = crate::models::torch::t_getattr(self, obj, name, node, state) {
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
        let saved_scalar = std::mem::replace(&mut self.mode.recv_scalar, scalar);
        let saved_wide = self.wide_read(obj, node);
        let nk = key_str_lit(name);
        for oid in obj.pts.iter() {
            let k = self.heap.kind(oid);
            let v: Value = match k {
                Kind::Module => self.module_attr(oid, name, nk, node, state, report, guarded, single),
                Kind::External if matches!(name, "__name__" | "__qualname__") && self.heap.external_qual(oid).starts_with("builtins.") => {
                    let q = self.heap.external_qual(oid);
                    Value::lit(key_str_lit(q.rsplit('.').next().unwrap_or("")))
                }
                Kind::External => self.external_attr(oid, name, state),
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
                Kind::Class => self.class_attr(oid, name, node, state, guarded),
                Kind::Dict => self.dict_attr(oid, name, node, state),
                Kind::List if self.m.deques.contains_key(&oid) && crate::models::counter::DEQUE_METHODS.contains(&name) => self.deque_attr(oid, name),
                Kind::List | Kind::Tuple | Kind::Set => self.method_value(&format!("{}.{}", k.name(), name), &Value::obj(oid)),
                Kind::Frame => crate::models::pandas::frame_getattr(self, oid, name, node, state, guarded),
                Kind::Series => crate::models::pandas::series_getattr(self, oid, name, node, state),
                Kind::Locidx if matches!(&self.heap.obj(oid).payload, Payload::Locidx(a, _) if matches!(a.as_str(), "str" | "dt" | "cat" | "sparse" | "list" | "struct")) => self.accessor_attr(oid, name, node, state),
                Kind::Unknown => self.load_unknown_attr(oid, name, node, state, guarded),
                // an lru_cache'd function's cache_info / cache_clear
                Kind::Function if matches!(name, "cache_info" | "cache_clear") && self.heap.func_info(oid).is_memo.get() => self.method_value(&format!("memo.{}", name), &Value::obj(oid)),
                // a function's names (a `@wraps(fn)` wrapper: fn's)
                Kind::Function if name == "__annotations__" && state.slot_opt(oid, Ns::Attr, nk).is_none() => {
                    let names = crate::models::annotated_names(self, oid, false).unwrap_or_default();
                    crate::models::names_dict(self, state, node, &names, None, "annotation")
                }
                Kind::Function if matches!(name, "__name__" | "__qualname__" | "__module__" | "__wrapped__" | "__doc__") && state.slot_opt(oid, Ns::Attr, nk).is_none() => self.function_name_attr(oid, name, node, state, report),
                // a singledispatch function's register
                Kind::Function if name == "register" && self.m.sdispatch.contains_key(&oid) => self.method_value("sdispatch.register", &Value::obj(oid)),
                Kind::Function => state.slot_opt(oid, Ns::Attr, nk).unwrap_or_else(|| Value::data(&format!("L{}", node.line))),
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
        acc = acc.join(&crate::domain::join_values(parts));
        if obj.pts.is_empty() {
            return crate::models::scalar_attr(self, obj, name, report, state);
        }
        acc.join(&self.mixed_literals_attr(obj, name, state))
    }

    /// The attribute of the literals beside a value's objects (`ops::mixed`), else nothing.
    fn mixed_literals_attr(&mut self, obj: &Value, name: &str, state: &mut State) -> Value {
        if !self.mixed(obj) {
            return Value::new();
        }
        let lits = Value::kv_only(crate::domain::kv_exclude(&obj.kv, &crate::intern::KeySet::one(crate::intern::key_none())).unwrap_or(crate::domain::BOT));
        crate::models::scalar_attr(self, &lits, name, false, state)
    }

    /// A deque's `maxlen` and methods.
    fn deque_attr(&mut self, oid: ObjId, name: &str) -> Value {
        if name == "maxlen" {
            match self.m.deques[&oid] {
                Some(m) => Value::lit(intern::key_int(m)),
                None => self.none_v.clone(),
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
                return Err(Value::kv_only(KeyVal::KeysOf(crate::domain::KeysOf::new(recv.pts.clone(), Ns::Item))));
            }
        }
        Ok(self.method_value(&format!("view.{}", name), &Value::obj(oid)))
    }

    /// An attribute of a module: its global, what every module has, else missing (open when
    /// unknown code may have added names).
    #[allow(clippy::too_many_arguments)]
    fn module_attr(&mut self, oid: ObjId, name: &str, nk: KeyId, node: &Node, state: &mut State, report: bool, guarded: bool, single: bool) -> Value {
        match state.slot_opt(oid, Ns::Attr, nk) {
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
            crate::state::mark_merged(o);
            return Value::obj(o);
        }
        let q = format!("{}.{}", self.heap.external_qual(oid), name);
        if let Some(v) = crate::models::generated_class(self, state, &q) {
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

    /// A function's `__name__` / `__qualname__` / `__module__` / `__wrapped__` / `__doc__` (a
    /// `@wraps(fn)` wrapper: fn's).
    fn function_name_attr(&mut self, oid: ObjId, name: &str, node: &Node, state: &mut State, report: bool) -> Value {
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
                    _ => {
                        let _ = report;
                        Value::data(name)
                    }
                }
            }
        }
    }

    /// An attribute of a class object: a class attribute (a classmethod bound to this class), an Enum
    /// member, a pydantic model's attribute or method, a dataclass `__init__`, else a method of the
    /// class object.
    fn class_attr(&mut self, oid: ObjId, name: &str, node: &Node, state: &mut State, guarded: bool) -> Value {
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
        } else if self.m.enum_members.contains_key(&oid) && (name == "__members__" || (!name.starts_with("__") && !self.heap.class_info(oid).open_base)) {
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

    /// Return the class oid (in MRO) that defines `name`, or None.
    pub fn class_lookup(&self, cls: u32, name: &str) -> Option<u32> {
        let nk = key_str_lit(name);
        let order = self.heap.mro(cls);
        if !order.is_empty() {
            return order.into_iter().find(|c| self.heap.cls_slots.contains_key(&(*c, Ns::Attr, nk)));
        }
        let mut seen: Vec<u32> = Vec::new();
        let mut order = vec![cls];
        while !order.is_empty() {
            let c = order.remove(0);
            if seen.contains(&c) {
                continue;
            }
            seen.push(c);
            if self.heap.cls_slots.contains_key(&(c, Ns::Attr, nk)) {
                return Some(c);
            }
            if let Some(ci) = self.heap.class_info_opt(c) {
                order.extend(ci.bases.iter().copied());
            }
        }
        None
    }

    /// Is the value class `c` holds as `k` plain data (no function, descriptor or property)?
    pub(in crate::interp) fn plain_class_attr(&self, c: u32, k: KeyId, state: &State) -> bool {
        let cv = state.slot_opt(c, Ns::Attr, k).or_else(|| self.heap.cls_slots.get(&(c, Ns::Attr, k)).cloned()).unwrap_or_default();
        let plain = !cv.pts.iter().any(|f| matches!(self.heap.kind(f), Kind::Function | Kind::Bound | Kind::Instance | Kind::External | Kind::Unknown));
        plain
    }

    /// The classes of `cls`'s MRO that may hold `name` on this path, up to the first that surely
    /// does (its body bound it, or a store on every path), and whether one surely does: a class
    /// attribute stored on some paths only (`if c: Cls.x = 1`) may be missing.
    pub fn class_lookup_on_path(&self, cls: u32, name: &str, state: &State) -> (Vec<u32>, bool) {
        let nk = key_str_lit(name);
        let mut out = Vec::new();
        let mut order = self.heap.mro(cls);
        if order.is_empty() {
            order = self.class_lookup(cls, name).into_iter().collect();
        }
        for c in order {
            if !self.heap.cls_slots.contains_key(&(c, Ns::Attr, nk)) {
                continue;
            }
            let row = state.row(c, Ns::Attr);
            // (or there unless unknown code removed it: as optimistic as a `likely` read)
            if !state.has_row(c, Ns::Attr) || row.definite.contains(nk) || row.likely.contains(nk) {
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
        join_values(defs.iter().map(|c| state.slot_opt(*c, Ns::Attr, nk).or_else(|| self.heap.cls_slots.get(&(*c, Ns::Attr, nk)).cloned()).unwrap_or_default()))
    }

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
                    let r = self.call_user(f, Some(Value::obj(oid)), vec![], IndexMap::new(), vec![], vec![], state, node, false);
                    if fi.is_cached.get() && matches!(self.heap.kind(oid), Kind::Instance) {
                        // stored on the instance: later accesses find this value
                        let strong = self.heap.obj(oid).single;
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
        // (and what is no object: literals beside the objects too)
        if cv.pts.is_empty() || !cv.kv.is_bot() {
            outs.push(Value::kv_only(cv.kv.clone()));
        }
        join_values(outs)
    }

    pub(in crate::interp) fn load_instance_attr(&mut self, oid: u32, name: &str, node: &Node, state: &mut State, guarded: bool, try_guard: bool, single_target: bool) -> Value {
        let nk = key_str_lit(name);
        if let Some(v) = self.instance_library_attr(oid, name, nk, node, state, guarded) {
            return v;
        }
        if let Some(v) = self.instance_getattr(oid, name, nk, node, state) {
            // (found through `__getattr__`: on this object the read succeeds where it returned;
            // beside other objects that lack the name, it may fail)
            if state.reachable && (!single_target || self.mode.one_of_several) {
                let cls = Classification::new(Tier::Proven);
                if guarded {
                    self.verdict_guarded(node, nk, &cls, oid, Ns::Attr);
                } else {
                    self.verdict(node, nk, &cls, oid, Ns::Attr, None, None);
                }
            }
            return v;
        }
        let v = state.slot_opt(oid, Ns::Attr, nk);
        let row = self.read_row(oid, Ns::Attr, state);
        let cls_of = self.heap.instance_class(oid);
        if let Some(mut v) = v {
            // (set on some paths only, over a class attribute of the name: the others read the
            // class's — the read cannot fail where the class surely has it)
            let class_attr = cls_of.filter(|c| !row.definite.contains(nk) && !crate::models::dataclasses::dataclass_fields_mro(self, *c).iter().any(|(f, _)| f == name)).map(|c| self.class_lookup_on_path(c, name, state));
            let mut class_v: Option<Value> = None;
            if let (true, Some((defs, sure))) = (row.present.contains(nk), class_attr) {
                let cv = join_values(defs.iter().map(|d| state.slot_opt(*d, Ns::Attr, nk).or_else(|| self.heap.cls_slots.get(&(*d, Ns::Attr, nk)).cloned()).unwrap_or_default()));
                let plain = !cv.pts.iter().any(|f| matches!(self.heap.kind(f), Kind::Function | Kind::Bound));
                if plain && sure {
                    if !guarded {
                        self.verdict(node, nk, &Classification::new(Tier::Proven), oid, Ns::Attr, None, None);
                    }
                    return v.join(&cv);
                }
                class_v = (plain && !defs.is_empty()).then_some(cv);
            }
            if let Some(cv) = class_v {
                v = v.join(&cv);
            }
            // (stored on some paths only, on an object unknown code changed: on the others it
            // holds what that code put there — of a class of the program's only: a library
            // base's own code is taken not to set the program's names, `nn.Module`'s)
            if !row.mutators.is_empty() && !(row.definite.contains(nk) && row.stored.contains(nk)) && cls_of.is_some_and(|c| self.hierarchy_closed(c)) {
                v = v.join(&self.path_placeholder_value(oid, Ns::Attr, nk, &row, state));
            }
            if row.present.contains(nk) {
                let cls = classify(&row, nk);
                if !guarded {
                    self.verdict(node, nk, &cls, oid, Ns::Attr, None, None);
                } else {
                    self.verdict_guarded(node, nk, &cls, oid, Ns::Attr);
                }
                // (there on some paths only: an AttributeError the enclosing try handles)
                if try_guard && !matches!(cls.tier, Tier::Proven | Tier::Likely | Tier::Map) && !self.class_resolves(oid, name) {
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
        if let Some(c) = cls_of {
            let is_field = crate::models::dataclasses::dataclass_fields_mro(self, c).iter().any(|(f, _)| f == name);
            let (defs, sure) = self.class_lookup_on_path(c, name, state);
            if !defs.is_empty() && !is_field {
                let cv = join_values(defs.iter().map(|d| state.slot_opt(*d, Ns::Attr, nk).or_else(|| self.heap.cls_slots.get(&(*d, Ns::Attr, nk)).cloned()).unwrap_or_default()));
                // a class attribute: the access succeeds (a method read counts only where the
                // receiver may be other objects too, which may lack it); one stored on some
                // paths only may fail
                let plain = !cv.pts.iter().any(|f| matches!(self.heap.kind(f), Kind::Function | Kind::Bound) && !(self.heap.kind(f) == Kind::Function && self.heap.func_info(f).is_property.get()));
                let cls = Classification::new(if sure { Tier::Proven } else { Tier::Possibly });
                if !guarded && (plain || !single_target) {
                    self.verdict(node, nk, &cls, oid, Ns::Attr, None, None);
                } else if guarded && (plain || !single_target) {
                    self.verdict_guarded(node, nk, &cls, oid, Ns::Attr);
                }
                if !sure && try_guard {
                    self.may_raise_here("AttributeError", state);
                }
                let out = self.bound_outs(&cv, oid, c, name, node, state);
                // (an instance of a class of the program unknown code changed: it may hold the
                // data name itself, over the class's)
                if plain && !row.mutators.is_empty() && !row.absent.contains(nk) && self.hierarchy_closed(c) {
                    return out.join(&self.path_placeholder_value(oid, Ns::Attr, nk, &row, state));
                }
                return out;
            }
        }
        let cls = classify(&row, nk);
        if !guarded {
            self.verdict(node, nk, &cls, oid, Ns::Attr, None, None);
            if cls.tier == Tier::Input {
                self.require_providers_k(&cls, &row, false, state, Some(nk), Ns::Attr);
            }
            if cls.tier != Tier::Missing && single_target && self.heap.obj(oid).single {
                state.set_row_narrowed(oid, Ns::Attr, row.narrow_in(nk, true));
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
        if matches!(name, "_asdict" | "_replace" | "_fields") {
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
                    if !row.present.contains(nk) {
                        return Some(self.call_method_with(&Value::obj(oid), "__getattr__", vec![Value::lit(nk)], node, state).unwrap_or_default());
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
                    return Some(own.join(&fallback));
                }
            }
        }
        None
    }

    /// A value declared dynamically typed (`Any`): accesses on it are not checkable (§6.6).
    pub(in crate::interp) fn dynamic_value(&mut self, label: &str, state: &mut State) -> Value {
        let oid = self.heap.alloc_raw(Kind::Unknown, &format!("any:{}", label), "", true, &format!("Any {}", label), Payload::None);
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

    /// An unknown object's row; none on this path (collected, or never made here): nothing is
    /// known of it (an empty, closed row would claim every key missing).
    /// The row a read classifies against: of an object this path has no facts about at all
    /// (made on other paths: a recursion's deeper activations make what its widened
    /// arguments hold), nothing is known.
    pub(in crate::interp) fn read_row(&self, oid: u32, ns: Ns, state: &State) -> Row {
        if self.has_facts(oid, state) { state.row(oid, ns) } else { Row::top() }
    }

    pub(in crate::interp) fn unknown_row(&self, oid: u32, ns: Ns, state: &State) -> Row {
        if state.has_row(oid, ns) {
            state.row(oid, ns)
        } else {
            Row::top()
        }
    }

    pub(in crate::interp) fn load_unknown_attr(&mut self, oid: u32, name: &str, node: &Node, state: &mut State, guarded: bool) -> Value {
        // (its class, as `type(x)`: calling it does not give it the object, so not a method of it)
        if name == "__class__" {
            return Value::data(&format!("type@L{}", node.line));
        }
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
            // (a parameter annotated `str`: a str, its methods' results known by type)
            let recv = match self.heap.obj(oid).ty {
                Some(t @ ("str" | "bytes")) => Value::data(&crate::domain::typed_label(t, "")),
                _ => Value::data(&format!("L{}", node.line)),
            };
            return self.method_value(&q, &recv);
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
            if cls.tier == Tier::Input && !self.mode.callee_nodes.contains(&node.addr) {
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
        let split = self.per_receiver(obj, &|an, o| an.attr_runs_user_code(o, name, true), state, &mut |an, o, st| {
            an.store_attr(o, name, v, node, st);
            Value::new()
        });
        if split.is_some() {
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
            state.set_slot(oid, Ns::Attr, nk, v.clone(), strong);
            if kind == Kind::Class {
                self.heap.cls_slots.insert((oid, Ns::Attr, nk), v.clone());
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
                if let Some(&setter) = self.heap.prop_setters.get(&(defc, nk)) {
                    self.call_user(setter, Some(Value::obj(oid)), vec![v.clone()], IndexMap::new(), vec![], vec![], state, node, false);
                    return true;
                }
                // a property without a setter: the assignment raises
                if kind == Kind::Instance {
                    let cv = state.slot_opt(defc, Ns::Attr, nk).or_else(|| self.heap.cls_slots.get(&(defc, Ns::Attr, nk)).cloned()).unwrap_or_default();
                    if !cv.pts.is_empty() && cv.pts.iter().all(|f| self.heap.kind(f) == Kind::Function && self.heap.func_info(f).is_property.get()) {
                        self.raise_here("AttributeError", state);
                        return true;
                    }
                }
                // a data descriptor (an object whose class has `__set__`) takes the store
                let cv = state.slot_opt(defc, Ns::Attr, nk).or_else(|| self.heap.cls_slots.get(&(defc, Ns::Attr, nk)).cloned()).unwrap_or_default();
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
                self.call_method_with(&Value::obj(oid), "__setattr__", vec![Value::lit(nk), v.clone()], node, state);
                return true;
            }
        }
        // `__slots__` on every class of the MRO (no `__dict__`): another name raises
        if kind == Kind::Instance {
            if let Some(allowed) = self.heap.instance_class(oid).and_then(|c| self.slots_of(c)) {
                if !allowed.iter().any(|a| a == name) {
                    self.raise_here("AttributeError", state);
                    return true;
                }
            }
        }
        false
    }
}
