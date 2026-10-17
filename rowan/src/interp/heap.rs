//! The object heap: every abstract object, identified by its allocation site and context, and
//! the allocation counter the collectors' birth marks compare with (`Obj::born`).
//!
//! The heap is flow-insensitive: an object, once made, exists on every path; what a path knows
//! of it (rows, slots, elements, lengths) is in its `State`. Only an object's identity and its
//! payload (the function, class or module it is, a bound method's parts, …) live here; a
//! path without a length for an object does not know it. Allocation *policy* (contexts, strong / recent /
//! variant / merged objects) is `Analyzer::alloc` (`interp/alloc.rs`); this is the table.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use indexmap::IndexMap;
use ruff_python_ast as ast;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::domain::{KeyVal, Ns, ObjId, Value};
use crate::frontend::{self, Node};
use crate::intern::{self, KeyId, KeySet, StrId};
use crate::scopes::Scopes;
use crate::state::{Kind, State};

/// Objects one allocation site may have across contexts; past it the site's objects collapse
/// onto one context-free summary object.
pub const MAX_CLONES: usize = 256;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ViewKind {
    Keys,
    Values,
    Items,
    Rows,
    Elems,
    /// `obj.__dict__`: the instance's attributes as a dict
    Attrs,
    /// `types.MappingProxyType(d)`: a read-only live view of the dict
    Proxy,
}

impl ViewKind {
    pub fn name(self) -> &'static str {
        match self {
            ViewKind::Keys => "keys",
            ViewKind::Values => "values",
            ViewKind::Items => "items",
            ViewKind::Rows => "rows",
            ViewKind::Elems => "elems",
            ViewKind::Attrs => "attrs",
            ViewKind::Proxy => "proxy",
        }
    }
    pub fn from_name(n: &str) -> ViewKind {
        match n {
            "keys" => ViewKind::Keys,
            "values" => ViewKind::Values,
            "items" => ViewKind::Items,
            "rows" => ViewKind::Rows,
            _ => ViewKind::Elems,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Payload {
    None,
    Module(Rc<ModuleInfo>),
    Func(Rc<FuncInfo>),
    Class(Rc<ClassInfo>),
    /// instance: its class
    Cls(ObjId),
    /// external qualname / response tag
    Str(String),
    Method(String, Value),
    Bound(ObjId, Value),
    Super(ObjId, Value),
    View(ViewKind, Value),
    Zip(Value, Value),
    Enumerate(Value, Option<i64>),
    Pair(Value, Value),
    Partial(Value, Vec<Value>, IndexMap<StrId, Value>),
    Groupby(Vec<ObjId>, Option<KeySet>, bool, Option<(KeySet, bool)>),
    Locidx(String, ObjId),
    /// `range(start, stop, step)` with known bounds
    Range(i64, i64, i64),
    /// a str known to start and end with these texts (an f-string with one unknown hole)
    Affix(String, String),
}

impl Payload {
    pub fn is_none(&self) -> bool {
        matches!(self, Payload::None)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum FuncNode {
    Def(&'static ast::StmtFunctionDef),
    Lambda(&'static ast::ExprLambda),
}

impl FuncNode {
    pub(in crate::interp) fn params(&self) -> Option<&'static ast::Parameters> {
        match self {
            FuncNode::Def(f) => Some(&f.parameters),
            FuncNode::Lambda(l) => l.parameters.as_deref(),
        }
    }
    pub(in crate::interp) fn node(&self) -> Node {
        match self {
            FuncNode::Def(f) => frontend::node(*f),
            FuncNode::Lambda(l) => frontend::node(*l),
        }
    }
    pub(in crate::interp) fn addr(&self) -> usize {
        match self {
            FuncNode::Def(f) => *f as *const _ as usize,
            FuncNode::Lambda(l) => *l as *const _ as usize,
        }
    }
    pub(in crate::interp) fn body(&self) -> &'static [ast::Stmt] {
        match self {
            FuncNode::Def(f) => &f.body,
            FuncNode::Lambda(_) => &[],
        }
    }
}

#[derive(Debug)]
pub struct FuncInfo {
    pub node: FuncNode,
    pub name: String,
    pub qualname: String,
    pub module: ObjId,
    pub scope: usize,
    /// free name -> cell oid
    pub closure: IndexMap<StrId, ObjId>,
    pub defaults: IndexMap<StrId, Value>,
    /// defining class oid
    pub cls: Option<ObjId>,
    pub is_property: Cell<bool>,
    pub is_static: Cell<bool>,
    pub is_classmethod: Cell<bool>,
    /// decorated with `contextlib.contextmanager`
    pub is_contextmanager: Cell<bool>,
    /// `functools.cached_property`: the first access stores the value on the instance
    pub is_cached: Cell<bool>,
    /// `functools.lru_cache` / `cache`: repeated arguments give the cached result
    pub is_memo: Cell<bool>,
}

impl FuncInfo {
    /// The qualified name as Python spells it (no `class ` markers): `Conn.__init__`.
    pub fn py_qualname(&self) -> String {
        self.qualname.replace("class ", "")
    }
}

#[derive(Debug)]
pub struct ClassInfo {
    pub name: String,
    pub node: &'static ast::StmtClassDef,
    pub bases: Vec<ObjId>,
    pub module: ObjId,
    /// inherits from something we do not see
    pub open_base: bool,
    pub dataclass_fields: RefCell<Option<Vec<(String, Option<&'static ast::Expr>)>>>,
    /// what its direct library bases make of it (`dict`, `TypedDict`, `NamedTuple`, pydantic's
    /// `BaseModel`, `nn.Module`, …)
    pub lib: crate::models::ClassBase,
    /// decorated with `pydantic.dataclasses.dataclass`: validated like a model, no model API
    pub pyd_dataclass: std::cell::Cell<bool>,
    /// `@dataclass(frozen=True)` / `@dataclass(kw_only=True)`
    pub dc_frozen: std::cell::Cell<bool>,
    pub dc_kw_only: std::cell::Cell<bool>,
    /// `@dataclass(order=True)`: instances of the class compare their fields as tuples
    pub dc_order: std::cell::Cell<bool>,
    pub any_fields: RefCell<Option<FxHashSet<String>>>,
}

impl ClassInfo {
    pub fn has_fields(&self) -> bool {
        self.dataclass_fields.borrow().is_some()
    }
    pub fn fields(&self) -> Vec<(String, Option<&'static ast::Expr>)> {
        self.dataclass_fields.borrow().clone().unwrap_or_default()
    }
}

#[derive(Debug)]
pub struct ModuleInfo {
    pub name: String,
    pub path: Option<String>,
    /// the dotted package the module belongs to (its own name for an `__init__.py`), the
    /// base of its relative imports
    pub pkg: String,
    pub idx: u32,
    pub tree: &'static ast::ModModule,
    pub scopes: Scopes,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct EnumKind {
    pub mixed: bool,
    pub repr_value: bool,
    pub flag: bool,
    /// str-based (`auto()` is the lower-cased name in a StrEnum)
    pub is_str: bool,
}

impl EnumKind {
    pub fn join(&mut self, o: EnumKind) {
        (self.mixed, self.repr_value, self.flag, self.is_str) = (self.mixed | o.mixed, self.repr_value | o.repr_value, self.flag | o.flag, self.is_str | o.is_str);
    }
}

#[derive(Debug)]
pub struct Obj {
    pub id: ObjId,
    pub kind: Kind,
    /// site, context and label are shared strings (one allocation per distinct string: many
    /// objects have the same context or label)
    pub site: std::sync::Arc<str>,
    pub ctx: std::sync::Arc<str>,
    pub single: bool,
    pub label: std::sync::Arc<str>,
    pub payload: Payload,
    /// an element record a model or an iteration made (a frame's row, a csv row, an input's
    /// element): a loop takes one of its own per iteration (`bind_views`)
    pub iter_elem: bool,
    /// record this object is a view of (virtual row -> frame)
    pub derived_from: Option<ObjId>,
    /// access-path depth under an open source (placeholders, elements)
    pub depth: u32,
    /// a one-shot iterator (generator, generator expression, `iter(...)`): a full iteration
    /// exhausts it
    pub iterator: bool,
    /// when its state was last made new (`Heap::serial` then): a strong re-allocation of
    /// its site starts it over, so it is younger than any mark taken before
    pub born: u64,
    /// an unknown known to be of a builtin type (`domain::TYPE_NAMES`): a parameter annotated
    /// so, in a run for any call
    pub ty: Option<&'static str>,
}


/// Keys standing for objects: Enum members and Flag combinations (`Key::Member`), objects
/// hashed by identity used as dict keys (`Analyzer::identity_key`), read back by `unkey`.
#[derive(Default)]
pub struct ObjKeys {
    /// members and Flag combinations ↔ their keys
    pub member_key: FxHashMap<ObjId, KeyId>,
    pub key_member: FxHashMap<KeyId, ObjId>,
    /// identity-hashed objects used as dict keys ↔ their keys
    pub identity_keys: std::cell::RefCell<FxHashMap<ObjId, KeyId>>,
    pub identity_objs: std::cell::RefCell<FxHashMap<KeyId, ObjId>>,
}

/// The objects made so far (object 0 is a placeholder: no value points to it).
pub struct Heap {
    objects: Vec<Obj>,
    /// (kind, site, context) -> object; site and context interned
    index: FxHashMap<(Kind, StrId, StrId), ObjId>,
    /// objects made per allocation site (`MAX_CLONES`)
    site_clones: FxHashMap<String, usize>,
    /// allocations so far (`Obj::born`): the collectors' marks
    serial: u64,
    /// keys standing for objects
    pub keys: ObjKeys,
    /// class attributes no path's state holds (flow-insensitive)
    pub cls_slots: FxHashMap<(ObjId, crate::domain::Ns, KeyId), Value>,
    /// class attributes the program stores after the class is made (`Cls.x = 1`): those a
    /// path may lack (`class_lookup_on_path`)
    pub cls_late: rustc_hash::FxHashSet<(ObjId, KeyId)>,
    /// `@name.setter` functions by (defining class, attribute)
    pub prop_setters: FxHashMap<(ObjId, KeyId), ObjId>,
    /// a copy's original (through copies made at the same node: `out = deepcopy(out)` in a loop
    /// makes one copy site, not a new one per round)
    pub copy_root: FxHashMap<ObjId, ObjId>,
    /// the builtin container annotation an unknown follows (`list[dict[str, int]]`): its
    /// parts' types (`entry::part_annotation`)
    pub ann: FxHashMap<ObjId, &'static ruff_python_ast::Expr>,
}

impl Default for Heap {
    fn default() -> Heap {
        let none = Obj { id: 0, kind: Kind::Unknown, site: "".into(), ctx: "".into(), single: true, label: "<none>".into(), payload: Payload::None, iter_elem: false, derived_from: None, depth: 0, iterator: false, born: 0, ty: None };
        Heap { objects: vec![none], index: FxHashMap::default(), site_clones: FxHashMap::default(), serial: 0, keys: ObjKeys::default(), cls_slots: FxHashMap::default(), cls_late: Default::default(), prop_setters: FxHashMap::default(), copy_root: FxHashMap::default(), ann: FxHashMap::default() }
    }
}

impl Heap {
    /// The number of objects made (ids are below it).
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.objects.len() <= 1
    }

    pub fn obj(&self, oid: ObjId) -> &Obj {
        &self.objects[oid as usize]
    }
    pub fn obj_mut(&mut self, oid: ObjId) -> &mut Obj {
        &mut self.objects[oid as usize]
    }
    pub fn kind(&self, oid: ObjId) -> Kind {
        self.objects[oid as usize].kind
    }
    pub fn label(&self, oid: ObjId) -> String {
        self.objects[oid as usize].label.to_string()
    }
    pub fn has_obj(&self, oid: ObjId) -> bool {
        (oid as usize) < self.objects.len() && oid > 0
    }

    /// The object made at (kind, site, context), if any.
    pub fn find(&self, kind: Kind, site: &str, ctx: &str) -> Option<ObjId> {
        self.index.get(&(kind, intern::s(site), intern::s(ctx))).copied()
    }

    /// How many objects the site has made (across contexts).
    pub fn clones(&self, site: &str) -> usize {
        self.site_clones.get(site).copied().unwrap_or(0)
    }

    /// The allocation counter: a mark taken now is older than every object made (or made new)
    /// from here on.
    pub fn serial(&self) -> u64 {
        self.serial
    }

    /// Was `o` made (or made new) before `mark`?
    pub fn born_before(&self, o: ObjId, mark: u64) -> bool {
        self.objects[o as usize].born < mark
    }

    /// A strong re-allocation: the object's state starts over, younger than every mark taken.
    pub fn renew(&mut self, oid: ObjId) {
        self.objects[oid as usize].born = self.serial;
        self.serial += 1;
    }

    /// The object at (kind, site, context), made if there is none yet; an existing one is weak
    /// if either allocation is, and takes a new payload.
    pub fn alloc_raw(&mut self, kind: Kind, site: &str, ctx: &str, single: bool, label: &str, payload: Payload) -> ObjId {
        let key = (kind, intern::s(site), intern::s(ctx));
        if let Some(&oid) = self.index.get(&key) {
            let o = &mut self.objects[oid as usize];
            o.single = o.single && single;
            if !payload.is_none() {
                o.payload = payload;
            }
            return oid;
        }
        let n = self.site_clones.get(site).copied().unwrap_or(0);
        if n >= MAX_CLONES && !ctx.is_empty() {
            // too many clones of one allocation site: collapse onto the context-free summary object
            let o = self.alloc_raw(kind, site, "", false, label, payload);
            crate::state::mark_merged(o);
            return o;
        }
        self.site_clones.insert(site.to_string(), n + 1);
        let oid = self.objects.len() as ObjId;
        self.objects.push(Obj {
            id: oid,
            kind,
            site: intern::str_get(key.1),
            ctx: intern::str_get(key.2),
            single,
            label: intern::str_get(intern::s(label)),
            payload,
            iter_elem: false,
            derived_from: None,
            depth: 0,
            iterator: site.starts_with("gen:"),
            born: self.serial,
            ty: None,
        });
        self.serial += 1;
        self.index.insert(key, oid);
        oid
    }

    // ------------------------------------------------------------ payloads
    pub fn module_info(&self, moid: ObjId) -> Rc<ModuleInfo> {
        match &self.obj(moid).payload {
            Payload::Module(m) => m.clone(),
            _ => panic!("not a module: {}", moid),
        }
    }
    pub fn func_info(&self, foid: ObjId) -> Rc<FuncInfo> {
        match &self.obj(foid).payload {
            Payload::Func(f) => f.clone(),
            _ => panic!("not a function: {}", foid),
        }
    }
    pub fn func_info_opt(&self, foid: ObjId) -> Option<Rc<FuncInfo>> {
        match &self.obj(foid).payload {
            Payload::Func(f) => Some(f.clone()),
            _ => None,
        }
    }
    pub fn class_info(&self, coid: ObjId) -> Rc<ClassInfo> {
        match &self.obj(coid).payload {
            Payload::Class(c) => c.clone(),
            _ => panic!("not a class: {} ({})", coid, self.obj(coid).label),
        }
    }
    pub fn class_info_opt(&self, coid: ObjId) -> Option<Rc<ClassInfo>> {
        if !self.has_obj(coid) {
            return None;
        }
        match &self.obj(coid).payload {
            Payload::Class(c) => Some(c.clone()),
            _ => None,
        }
    }
    /// The class of an instance object (payload), if it is a user class.
    pub fn instance_class(&self, oid: ObjId) -> Option<ObjId> {
        match &self.obj(oid).payload {
            Payload::Cls(c) => Some(*c),
            _ => None,
        }
    }
    pub fn external_qual(&self, oid: ObjId) -> String {
        match &self.obj(oid).payload {
            Payload::Str(q) => q.clone(),
            _ => String::new(),
        }
    }
    pub fn is_record_like(&self, oid: ObjId) -> bool {
        self.kind(oid).is_record_like()
    }

    // ------------------------------------------------------------ lengths
    /// The length of a list / tuple on this path (positions 0..n are its items), else None.
    pub fn llen(&self, o: ObjId, st: &State) -> Option<usize> {
        st.lens.get(&o).and_then(|v| v.0)
    }

    /// The possible lengths of a list / tuple on this path: the exact one, or the alternatives
    /// a join kept (lengths 0..15 on the joined paths).
    pub fn len_alts(&self, o: ObjId, st: &State) -> Option<Vec<usize>> {
        if let Some(n) = self.llen(o, st) {
            return Some(vec![n]);
        }
        match st.lens.get(&o) {
            Some((None, _, m)) if m != 0 => Some((0..16).filter(|i| m & (1 << i) != 0).collect()),
            _ => None,
        }
    }

    /// The number of leading positions known (the length when known; after appends of unknown
    /// count, the length before them).
    pub fn prefix(&self, o: ObjId, st: &State) -> Option<usize> {
        let p = match st.lens.get(&o) {
            Some((Some(n), _, _)) => n,
            Some((None, p, _)) => p,
            None => 0,
        };
        Some(p).filter(|p| *p > 0)
    }

    pub fn set_llen(&mut self, o: ObjId, st: &mut State, v: Option<usize>) {
        st.set_lens(o, (v, v.unwrap_or(0), 0));
    }

    /// The length is unknown now, its first `n` positions are still known.
    pub fn set_prefix(&mut self, o: ObjId, st: &mut State, n: usize) {
        st.set_lens(o, (None, n, 0));
    }

    /// The literal a tuple object stands for: every position one known literal (nested
    /// tuples too).
    pub fn literal_tuple(&self, o: ObjId, state: &State, depth: u32) -> Option<KeyId> {
        if depth > 4 {
            return None;
        }
        // a (key, value) pair (zip / enumerate / items): a 2-tuple
        if self.kind(o) == Kind::Pair {
            let Payload::Pair(k, v) = &self.obj(o).payload else { return None };
            let one = |x: &Value| -> Option<KeyId> {
                match (x.pts.len(), &x.kv) {
                    (0, KeyVal::Lit(k)) => Some(*k),
                    (1, KeyVal::Bot) if matches!(self.kind(x.pts.first().unwrap()), Kind::Tuple | Kind::Pair) => self.literal_tuple(x.pts.first().unwrap(), state, depth + 1),
                    _ => None,
                }
            };
            return Some(intern::key_tuple(vec![one(k)?, one(v)?]));
        }
        let n = self.llen(o, state)?;
        let mut items = Vec::with_capacity(n);
        for i in 0..n {
            let v = state.slot(o, Ns::Pos, intern::key_int(i as i64));
            let k = match (v.pts.len(), &v.kv) {
                (0, KeyVal::Lit(k)) => *k,
                (1, KeyVal::Bot) => {
                    let x = v.pts.first().unwrap();
                    if !matches!(self.kind(x), Kind::Tuple | Kind::Pair) {
                        return None;
                    }
                    self.literal_tuple(x, state, depth + 1)?
                }
                _ => return None,
            };
            items.push(k);
        }
        Some(intern::key_tuple(items))
    }

    // ------------------------------------------------------------ classes
    /// Breadth-first linearisation of the user classes above `cls` (when C3 fails).
    pub fn mro_bfs(&self, cls: u32) -> Vec<u32> {
        let mut seen: Vec<u32> = Vec::new();
        let mut order = vec![cls];
        let mut out: Vec<u32> = Vec::new();
        while !order.is_empty() {
            let c = order.remove(0);
            if seen.contains(&c) || !self.has_obj(c) {
                continue;
            }
            seen.push(c);
            out.push(c);
            if let Some(ci) = self.class_info_opt(c) {
                order.extend(ci.bases.iter().copied());
            }
        }
        out
    }

    /// The C3 method resolution order of a user class (user classes only).
    pub fn mro(&self, cls: u32) -> Vec<u32> {
        fn c3(an: &Heap, cls: u32, depth: usize) -> Option<Vec<u32>> {
            if depth > 64 {
                return None;
            }
            let bases: Vec<u32> = an.class_info_opt(cls).map(|ci| ci.bases.clone()).unwrap_or_default();
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

    /// The objects made since `mark` refer to the summary objects of `map` instead of the
    /// recent ones (payloads, `derived_from`).
    pub fn rename_payloads(&mut self, mark: ObjId, rename: &dyn Fn(&Value) -> Option<Value>, map: &FxHashMap<ObjId, ObjId>) {
        let n = self.len() as ObjId;
        for oid in mark..n {
            let o = self.obj_mut(oid);
            if let Some(d) = o.derived_from {
                if let Some(&sd) = map.get(&d) {
                    o.derived_from = Some(sd);
                }
            }
            let rv = |v: &Value| rename(v).unwrap_or_else(|| v.clone());
            o.payload = match &o.payload {
                Payload::Method(q, v) => Payload::Method(q.clone(), rv(v)),
                Payload::Bound(f, v) => Payload::Bound(*map.get(f).unwrap_or(f), rv(v)),
                Payload::Super(c, v) => Payload::Super(*c, rv(v)),
                Payload::View(k, v) => Payload::View(*k, rv(v)),
                Payload::Zip(a, b) => Payload::Zip(rv(a), rv(b)),
                Payload::Enumerate(v, s0) => Payload::Enumerate(rv(v), *s0),
                Payload::Pair(a, b) => Payload::Pair(rv(a), rv(b)),
                Payload::Partial(f, args, kw) => Payload::Partial(rv(f), args.iter().map(&rv).collect(), kw.iter().map(|(k, v)| (*k, rv(v))).collect()),
                Payload::Groupby(fs, ks, ai, sel) => Payload::Groupby(fs.iter().map(|f| *map.get(f).unwrap_or(f)).collect(), ks.clone(), *ai, sel.clone()),
                Payload::Locidx(nm, x) => Payload::Locidx(nm.clone(), *map.get(x).unwrap_or(x)),
                other => other.clone(),
            };
        }
    }
}
