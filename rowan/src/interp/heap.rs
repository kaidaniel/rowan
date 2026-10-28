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
use crate::state::{mark_merged, Kind, State};

/// Objects one allocation site may have across contexts; past it the site's objects collapse
/// onto one context-free summary object.
pub const MAX_CLONES: usize = 256;

/// A point in the heap's history (`Heap::mark`): its objects then, and its serial.
#[derive(Clone, Copy, Debug)]
pub struct HeapMark {
    pub len: ObjId,
    pub serial: u64,
}
/// Objects of a site made past `MAX_CLONES` in calls the main file's own code makes
/// (`Heap::from_main`): a registry's helper the main file calls once more keeps its result.
pub const MAIN_CLONES: usize = 16;

/// What a view object (`Kind::View`) shows of the object it views.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ViewKind {
    /// `d.keys()`
    Keys,
    /// `d.values()`
    Values,
    /// `d.items()`
    Items,
    /// a frame's rows (`df.iterrows()`): (index, row) pairs
    Rows,
    /// the elements of the objects it views (a slice of an unknown sequence)
    Elems,
    /// `obj.__dict__`: the instance's attributes as a dict
    Attrs,
    /// `types.MappingProxyType(d)`: a read-only live view of the dict
    Proxy,
}

impl ViewKind {
    /// The view's name (`keys`, `values`, …).
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
    /// The view `d.keys()`, `d.values()` or `d.items()` makes, by the method's name.
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

/// What an object is besides its keys: flow-insensitive, fixed when it is made (or made again).
#[derive(Clone, Debug)]
pub enum Payload {
    None,
    /// a module
    Module(Rc<ModuleInfo>),
    /// a function or lambda of the program
    Func(Rc<FuncInfo>),
    /// a class of the program
    Class(Rc<ClassInfo>),
    /// an instance: its class
    Cls(ObjId),
    /// a library object's qualified name (`builtins.len`); a response's tag
    Str(String),
    /// a library method (its qualified name, `dict.get`) bound to a receiver
    Method(String, Value),
    /// a method of the program (its function) bound to a receiver
    Bound(ObjId, Value),
    /// `super()`: the class it starts after and the receiver
    Super(ObjId, Value),
    /// a view of the objects of `Value`
    View(ViewKind, Value),
    /// `zip(a, b)` (more than two: nested pairs)
    Zip(Value, Value),
    /// `enumerate(it, start)` (the start when known)
    Enumerate(Value, Option<i64>),
    /// a (key, value) pair of `items()`, `zip`, `enumerate`
    Pair(Value, Value),
    /// `functools.partial(f, *args, **kwargs)`
    Partial(Value, Vec<Value>, IndexMap<StrId, Value>),
    /// a pandas groupby: the frames grouped, the key columns (when known), `as_index`, and the
    /// columns selected (`gb["a"]`; whether as one column)
    Groupby(Vec<ObjId>, Option<KeySet>, bool, Option<(KeySet, bool)>),
    /// a pandas accessor or indexer (`.loc`, `.iloc`, `.str`, `.dt`, …) by name, of a frame or
    /// series
    Locidx(String, ObjId),
    /// `range(start, stop, step)` with known bounds
    Range(i64, i64, i64),
    /// a str known to start and end with these texts (an f-string with one unknown hole)
    Affix(String, String),
    /// a definition whose decorators run when it is first read (`deferred.rs`)
    Deferred(Rc<super::deferred::Deferred>),
}

impl Payload {
    pub fn is_none(&self) -> bool {
        matches!(self, Payload::None)
    }
}

/// The syntax a function object was made from.
#[derive(Clone, Copy, Debug)]
pub enum FuncNode {
    Def(&'static ast::StmtFunctionDef),
    Lambda(&'static ast::ExprLambda),
    /// a generator expression (an `Expr::Generator`): a generator function of the iterator
    /// its first iterable gave
    GenExp(&'static ast::Expr),
}

impl FuncNode {
    /// Its parameters (a lambda may have none).
    pub(in crate::interp) fn params(&self) -> Option<&'static ast::Parameters> {
        match self {
            FuncNode::Def(f) => Some(&f.parameters),
            FuncNode::Lambda(l) => l.parameters.as_deref(),
            FuncNode::GenExp(_) => None,
        }
    }
    /// The function's node: its position, and its identity (`FuncNode::addr`).
    pub(in crate::interp) fn node(&self) -> Node {
        match self {
            FuncNode::Def(f) => frontend::node(*f),
            FuncNode::Lambda(l) => frontend::node(*l),
            FuncNode::GenExp(g) => frontend::node(*g),
        }
    }
    /// Its address: the identity of the function's code (all closures of one `def` share it).
    pub(in crate::interp) fn addr(&self) -> usize {
        match self {
            FuncNode::Def(f) => *f as *const _ as usize,
            FuncNode::Lambda(l) => *l as *const _ as usize,
            FuncNode::GenExp(g) => *g as *const _ as usize,
        }
    }
    /// A `def`'s statements (a lambda's body is an expression: none).
    pub(in crate::interp) fn body(&self) -> &'static [ast::Stmt] {
        match self {
            FuncNode::Def(f) => &f.body,
            FuncNode::Lambda(_) | FuncNode::GenExp(_) => &[],
        }
    }
    /// Does a call of it make a coroutine (an `async def` that does not yield)? Its body runs
    /// where the coroutine is awaited (`Analyzer::await_value`).
    pub(in crate::interp) fn is_coroutine(&self) -> bool {
        matches!(self, FuncNode::Def(f) if f.is_async && !crate::syntax::is_generator(&f.body))
    }
    /// Is it a generator expression's?
    pub(in crate::interp) fn is_genexp(&self) -> bool {
        matches!(self, FuncNode::GenExp(_))
    }
    /// Does a call of it make a generator (a `def` that yields, a generator expression)?
    pub(in crate::interp) fn is_generator(&self) -> bool {
        match self {
            FuncNode::Def(f) => crate::syntax::is_generator(&f.body),
            FuncNode::Lambda(_) => false,
            FuncNode::GenExp(_) => true,
        }
    }
}

/// A function object of the program (`Payload::Func`): what its `def` made. The flags are set
/// by its decorators after it is made (`apply_decorators`).
#[derive(Debug, Clone)]
pub struct FuncInfo {
    pub node: FuncNode,
    pub name: String,
    /// its qualified name (`Conn.__init__`; classes marked `class ` inside, `py_qualname`)
    pub qualname: String,
    pub module: ObjId,
    /// its scope in its module's `Scopes`
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
    /// decorated by what rowan takes to give it back unchanged (a decorator not modelled,
    /// `typing.overload`, …): what the name holds may be another object
    pub opaque: Cell<bool>,
}

impl FuncInfo {
    /// The qualified name as Python spells it (no `class ` markers): `Conn.__init__`.
    pub fn py_qualname(&self) -> String {
        self.qualname.replace("class ", "")
    }
}

/// A class object of the program (`Payload::Class`): what its `class` statement made.
#[derive(Debug)]
pub struct ClassInfo {
    pub name: String,
    pub node: &'static ast::StmtClassDef,
    pub bases: Vec<ObjId>,
    pub module: ObjId,
    /// inherits from something we do not see
    pub open_base: std::cell::Cell<bool>,
    /// a dataclass's (NamedTuple's, pydantic model's, …) fields with their defaults, once known
    pub dataclass_fields: RefCell<Option<Vec<(String, Option<&'static ast::Expr>)>>>,
    /// what its direct library bases make of it (`dict`, `TypedDict`, `NamedTuple`, pydantic's
    /// `BaseModel`, `nn.Module`, …)
    pub lib: crate::models::ClassBase,
    /// decorated with `pydantic.dataclasses.dataclass`: validated like a model, no model API
    pub pyd_dataclass: Cell<bool>,
    /// `@dataclass(frozen=True)` / `@dataclass(kw_only=True)`
    pub dc_frozen: Cell<bool>,
    pub dc_kw_only: Cell<bool>,
    /// `@dataclass(order=True)`: instances of the class compare their fields as tuples
    pub dc_order: Cell<bool>,
    /// the attributes its body annotates `Any` / `object` (computed on first use)
    pub any_fields: RefCell<Option<FxHashSet<String>>>,
}

impl ClassInfo {
    /// Does it have generated fields (a dataclass and the like)?
    pub fn has_fields(&self) -> bool {
        self.dataclass_fields.borrow().is_some()
    }
    /// Its generated fields (none if it has none).
    pub fn fields(&self) -> Vec<(String, Option<&'static ast::Expr>)> {
        self.dataclass_fields.borrow().clone().unwrap_or_default()
    }
}

/// A module object (`Payload::Module`): its name, file, parsed tree and scopes.
#[derive(Debug)]
pub struct ModuleInfo {
    /// the dotted name it was imported as
    pub name: String,
    pub path: Option<String>,
    /// the dotted package the module belongs to (its own name for an `__init__.py`), the
    /// base of its relative imports
    pub pkg: String,
    /// its index in the frontend's registry
    pub idx: u32,
    pub tree: &'static ast::ModModule,
    pub scopes: Scopes,
    /// read from a site-packages directory (a followed installed package): its code runs within the
    /// library allowance (`library.rs`)
    pub library: bool,
}

/// What an Enum class's bases make of its members (`models::enums`).
#[derive(Clone, Copy, Default, Debug)]
pub struct EnumKind {
    /// mixed with `str` or `int` (`StrEnum`, `IntEnum`, `class M(str, Enum)`): a member equals
    /// and hashes as its value
    pub mixed: bool,
    /// `str(member)` is its value's (`StrEnum`, `IntEnum`, `ReprEnum`), not `Cls.NAME`
    pub repr_value: bool,
    /// a `Flag` / `IntFlag`: members combine with `|`
    pub flag: bool,
    /// str-based (`auto()` is the lower-cased name in a StrEnum)
    pub is_str: bool,
}

impl EnumKind {
    /// This kind with `o`'s traits added (an Enum class whose bases bring both).
    pub fn join(&mut self, o: EnumKind) {
        (self.mixed, self.repr_value, self.flag, self.is_str) = (self.mixed | o.mixed, self.repr_value | o.repr_value, self.flag | o.flag, self.is_str | o.is_str);
    }
}

/// One abstract object: identified by (kind, allocation site, context); see the module doc.
#[derive(Debug)]
pub struct Obj {
    pub id: ObjId,
    pub kind: Kind,
    /// where it was made (`L5:3-12`, a model's site); site, context and label are shared
    /// strings (one allocation per distinct string: many objects have the same context or label)
    pub site: std::sync::Arc<str>,
    /// the allocation context: the call chain it was made in (`Frame::ctx`)
    pub ctx: std::sync::Arc<str>,
    /// it stands for one runtime object: stores into it are strong
    pub single: bool,
    /// a name for people and the report (`dict@L4`, `Conn@L12`, `result of f(x)`)
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
    /// an object a library call rowan does not model made: its methods are its own (not a
    /// dict's or a str's, as an input's data may have)
    pub opaque: bool,
    /// an instance made for any call that also stands for instances of its class's subclasses
    /// the run did not make (`annotated_instance`): its class is any of them (`type(x)`,
    /// `isinstance(x, Sub)`), what it holds is the class's own
    pub any_sub: bool,
    /// an instance a run for any call (an entry run) takes for what a caller passes — a
    /// receiver, an annotated parameter or field: it may be any instance of its class, one the
    /// program made too, and the object another argument is (identity decides nothing of it)
    pub arg: bool,
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
    /// the allocation is in a call the main file's own code makes (not its imports'): past
    /// `MAX_CLONES`, up to `MAIN_CLONES` more objects of the site are its own
    pub from_main: bool,
    /// those more objects made per site
    main_clones: FxHashMap<String, usize>,
    /// allocations so far (`Obj::born`): the collectors' marks
    serial: u64,
    /// objects made earlier given a payload again, by serial then (`rename_payloads`)
    repaid: Vec<(u64, ObjId)>,
    /// the functions, classes and modules made so far (definitions: the statement making one
    /// pays for it, `Analyzer::library_work`)
    pub defs: usize,
    /// the placeholders of deferred definitions made so far (`deferred.rs`; none: no value
    /// needs looking at for them)
    pub deferred: usize,
    /// their decorators' runs (once per path that reads one)
    pub forced: usize,
    /// keys standing for objects
    pub keys: ObjKeys,
    /// class attributes no path's state holds (flow-insensitive)
    pub cls_slots: FxHashMap<(ObjId, Ns, KeyId), Value>,
    /// per object, the class attributes whose values were given it (`set_cls_slot`; some may
    /// hold it no more): what a loop's back edge renames (`rename_recent`)
    cls_refs: FxHashMap<ObjId, Vec<(ObjId, Ns, KeyId)>>,
    /// class attributes the program stores after the class is made (`Cls.x = 1`): those a
    /// path may lack (`class_lookup_on_path`)
    pub cls_late: rustc_hash::FxHashSet<(ObjId, KeyId)>,
    /// the names any class body bound to a property (not a cached one): what a read of an
    /// instance's own value checks its class for on the path (a data descriptor takes it)
    pub prop_names: rustc_hash::FxHashSet<KeyId>,
    /// `@name.setter` functions by (defining class, attribute)
    pub prop_setters: FxHashMap<(ObjId, KeyId), ObjId>,
    /// a property's deleter by (class, name) (`@name.deleter`): `del obj.name` runs it
    pub prop_deleters: FxHashMap<(ObjId, KeyId), ObjId>,
    /// the program's functions unknown code was given (on any path): it may have set any
    /// attribute on them (`Analyzer::plain_function`)
    pub escaped_fns: rustc_hash::FxHashSet<ObjId>,
    /// a copy's original (through copies made at the same node: `out = deepcopy(out)` in a loop
    /// makes one copy site, not a new one per round)
    pub copy_root: FxHashMap<ObjId, ObjId>,
    /// `copy_root`'s distinct originals: what the copies pin for the collectors
    copy_roots: rustc_hash::FxHashSet<ObjId>,
    /// when an original became young for the collectors (made one, or renewed while one), in
    /// serial order: a collector after mark `m` finds the young originals at the log's end (a
    /// generational collector's remembered set)
    root_log: Vec<(u64, ObjId)>,
    /// the builtin container annotation an unknown follows (`list[dict[str, int]]`) and the
    /// module it is written in: its parts' types (`annotations::part_annotation`)
    pub ann: FxHashMap<ObjId, (&'static ruff_python_ast::Expr, ObjId)>,
    /// container annotations (by node) a part following them was given items in place: what
    /// any part following them holds is not known (`Analyzer::put_items`)
    pub ann_changed: rustc_hash::FxHashSet<usize>,
    /// per class and attribute name: the annotation its instances' attribute is declared with
    /// (`Analyzer::attr_annotation`), found once
    #[allow(clippy::type_complexity)]
    pub aliases: std::cell::RefCell<FxHashMap<(ObjId, crate::intern::StrId), Option<(&'static ruff_python_ast::Expr, ObjId)>>>,
    /// per class and attribute name: (see `attr_anns`); per module and name above: the alias it
    /// binds (`Analyzer::alias_of`), found once
    #[allow(clippy::type_complexity)]
    pub attr_anns: std::cell::RefCell<FxHashMap<(ObjId, crate::intern::StrId), Option<(&'static ruff_python_ast::Expr, ObjId)>>>,
    /// a placeholder read as an attribute of these objects (`o.m` of an unknown `o`): called,
    /// it may be a method bound to one of them, which it is given (`call_one`)
    pub attr_of: FxHashMap<ObjId, Vec<ObjId>>,
    /// per class: whether its library bases are all builtin classes (`Analyzer::builtin_based`)
    pub builtin_based: FxHashMap<ObjId, bool>,
}

impl Default for Heap {
    fn default() -> Heap {
        let none = Obj { id: 0, kind: Kind::Unknown, site: "".into(), ctx: "".into(), single: true, label: "<none>".into(), payload: Payload::None, iter_elem: false, derived_from: None, depth: 0, iterator: false, born: 0, ty: None, opaque: false, any_sub: false, arg: false };
        Heap { objects: vec![none], index: FxHashMap::default(), site_clones: FxHashMap::default(), from_main: false, main_clones: FxHashMap::default(), serial: 0, repaid: Vec::new(), defs: 0, deferred: 0, forced: 0, keys: ObjKeys::default(), cls_slots: FxHashMap::default(), cls_refs: FxHashMap::default(), cls_late: Default::default(), prop_names: Default::default(), prop_setters: FxHashMap::default(), prop_deleters: FxHashMap::default(), escaped_fns: Default::default(), copy_root: FxHashMap::default(), copy_roots: Default::default(), root_log: Vec::new(), ann: FxHashMap::default(), ann_changed: Default::default(), attr_anns: Default::default(), aliases: Default::default(), attr_of: FxHashMap::default(), builtin_based: FxHashMap::default() }
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

    /// Where the heap is now: what is made or given a payload from here on is younger.
    pub fn mark(&self) -> HeapMark {
        HeapMark { len: self.objects.len() as ObjId, serial: self.serial }
    }

    /// Forget the payloads given since the last call of the module's code returned (no mark
    /// of a running call is older).
    pub fn forget_repaid(&mut self) {
        self.repaid.clear();
    }

    /// Was `o` made (or made new) before `mark`?
    /// Records that `copy` is a copy of `root`.
    pub fn set_copy_root(&mut self, copy: ObjId, root: ObjId) {
        self.copy_root.insert(copy, root);
        if self.copy_roots.insert(root) {
            self.root_log.push((self.serial, root));
        }
    }

    /// The originals of copies (what the collectors keep), or only those born since `mark`
    /// (older objects are never collected after it).
    pub fn push_copy_roots(&self, mark: Option<u64>, work: &mut Vec<ObjId>) {
        match mark {
            Some(m) => {
                let from = self.root_log.partition_point(|(s, _)| *s < m);
                work.extend(self.root_log[from..].iter().map(|(_, o)| *o).filter(|o| !self.born_before(*o, m)));
            }
            None => work.extend(self.copy_roots.iter().copied()),
        }
    }

    /// Was `o`'s state made (or last made new) before `mark` (`Obj::born`): the collector after
    /// that mark leaves it alone.
    pub fn born_before(&self, o: ObjId, mark: u64) -> bool {
        self.objects[o as usize].born < mark
    }

    /// A strong re-allocation: the object's state starts over, younger than every mark taken.
    pub fn renew(&mut self, oid: ObjId) {
        self.objects[oid as usize].born = self.serial;
        self.serial += 1;
        if self.copy_roots.contains(&oid) {
            self.root_log.push((self.serial - 1, oid));
        }
    }

    /// The object at (kind, site, context), made if there is none yet; an existing one is weak
    /// if either allocation is, and takes a new payload.
    pub fn alloc_raw(&mut self, kind: Kind, site: &str, ctx: &str, single: bool, label: &str, payload: Payload) -> ObjId {
        let key = (kind, intern::s(site), intern::s(ctx));
        if let Some(&oid) = self.index.get(&key) {
            let o = &mut self.objects[oid as usize];
            o.single = o.single && single;
            if !payload.is_none() {
                // (one holding objects other than it held: the collector after a mark expands
                // it, a rename of recent objects looks at it)
                let (mut objs, mut had) = (Vec::new(), Vec::new());
                payload_objs(&payload, &mut objs);
                payload_objs(&o.payload, &mut had);
                let o = &mut self.objects[oid as usize];
                o.payload = payload;
                if !objs.is_empty() && objs != had && self.repaid.last() != Some(&(self.serial, oid)) {
                    self.repaid.push((self.serial, oid));
                }
            }
            return oid;
        }
        let n = self.site_clones.get(site).copied().unwrap_or(0);
        let more = n >= MAX_CLONES && self.from_main && self.main_clones.get(site).copied().unwrap_or(0) < MAIN_CLONES;
        if more {
            *self.main_clones.entry(site.to_string()).or_default() += 1;
        } else if n >= MAX_CLONES && !ctx.is_empty() {
            // too many clones of one allocation site: collapse onto the context-free summary object
            let o = self.alloc_raw(kind, site, "", false, label, payload);
            mark_merged(o);
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
            opaque: false,
            any_sub: false,
            arg: false,
        });
        self.serial += 1;
        self.defs += usize::from(matches!(kind, Kind::Function | Kind::Class | Kind::Module));
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
    /// A library object's qualified name ("" for others).
    pub fn external_qual(&self, oid: ObjId) -> String {
        match &self.obj(oid).payload {
            Payload::Str(q) => q.clone(),
            _ => String::new(),
        }
    }
    /// Is the object a record (it has keys: a dict, frame, instance, unknown, module, class,
    /// namespace)?
    pub fn is_record(&self, oid: ObjId) -> bool {
        self.kind(oid).is_record()
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

    /// The list's length on this path is `v` (None: not known), every position known.
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
    /// Class `k.0`'s attribute `k.2` holds `v` (flow-insensitive, `cls_slots`).
    pub fn set_cls_slot(&mut self, k: (ObjId, Ns, KeyId), v: Value) {
        let kos = match &v.kv {
            crate::domain::KeyVal::KeysOf(ko) => ko.objs.to_vec(),
            _ => Vec::new(),
        };
        for o in v.pts.iter().chain(kos) {
            let refs = self.cls_refs.entry(o).or_default();
            if !refs.contains(&k) {
                refs.push(k);
            }
        }
        self.cls_slots.insert(k, v);
    }

    /// The class attributes whose values may hold one of `objs`.
    pub fn cls_slots_with(&self, objs: &[ObjId]) -> Vec<(ObjId, Ns, KeyId)> {
        let mut ks: Vec<(ObjId, Ns, KeyId)> = objs.iter().filter_map(|o| self.cls_refs.get(o)).flatten().copied().collect();
        ks.sort_unstable();
        ks.dedup();
        ks
    }

    /// What class `owner` holds under attribute `name` (flow-insensitive, `cls_slots`; the
    /// empty value when nothing).
    pub fn class_attr(&self, owner: ObjId, name: &str) -> Value {
        self.cls_slots.get(&(owner, Ns::Attr, intern::key_str_lit(name))).cloned().unwrap_or_default()
    }

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
        /// C3 linearisation (None: inconsistent bases, or deeper than 64).
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

    /// The objects made before the allocation serial `mark` and given a payload holding objects
    /// since (`alloc_raw` of an existing object).
    pub fn repaid_since(&self, mark: u64) -> Vec<ObjId> {
        let mut v: Vec<ObjId> = self.repaid.iter().rev().take_while(|(s, _)| *s >= mark).map(|(_, o)| *o).filter(|o| self.born_before(*o, mark)).collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// The objects made or given a payload since `mark`.
    pub fn payloads_since(&self, mark: HeapMark) -> Vec<ObjId> {
        let older = self.repaid.iter().rev().take_while(|(s, _)| *s >= mark.serial).map(|(_, o)| *o).filter(|o| *o < mark.len);
        older.chain(mark.len..self.len() as ObjId).collect()
    }

    /// The objects made (or given a payload) since `mark` refer to the summary objects of `map` instead of the
    /// recent ones (payloads, `derived_from`).
    pub fn rename_payloads(&mut self, mark: HeapMark, rename: &dyn Fn(&Value) -> Option<Value>, map: &FxHashMap<ObjId, ObjId>) {
        for oid in self.payloads_since(mark) {
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

/// The objects a payload holds that the collector and the rename of recent objects follow
/// (`Heap::alloc_raw`'s re-payments).
fn payload_objs(payload: &Payload, objs: &mut Vec<ObjId>) {
    match payload {
        Payload::Method(_, v) | Payload::View(_, v) | Payload::Enumerate(v, _) => v.push_objs(objs),
        Payload::Bound(f, v) | Payload::Super(f, v) => {
            objs.push(*f);
            v.push_objs(objs);
        }
        Payload::Zip(a, b) | Payload::Pair(a, b) => {
            a.push_objs(objs);
            b.push_objs(objs);
        }
        Payload::Partial(f, args, kw) => {
            f.push_objs(objs);
            args.iter().chain(kw.values()).for_each(|v| v.push_objs(objs));
        }
        _ => {}
    }
}
