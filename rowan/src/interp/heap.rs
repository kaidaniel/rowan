//! The object heap: every abstract object, identified by its allocation site and context, and
//! the allocation counter the collectors' birth marks compare with (`Obj::born`).
//!
//! The heap is flow-insensitive: an object, once made, exists on every path; what a path knows
//! of it (rows, slots, elements, lengths) is in its `State`. Only an object's identity and its
//! payload (the function, class or module it is, a bound method's parts, …) live here, plus
//! two per-object fallbacks for lengths (`literal_len`, `known_prefix`) that a state not
//! descended from the allocation reads. Allocation *policy* (contexts, strong / recent /
//! variant / merged objects) is `Analyzer::alloc` (`interp/alloc.rs`); this is the table.

use std::rc::Rc;

use indexmap::IndexMap;
use rustc_hash::FxHashMap;

use super::{ClassInfo, FuncInfo, ModuleInfo};
use crate::domain::{KeyVal, Ns, ObjId, Value};
use crate::intern::{self, KeyId, KeySet, StrId};
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
}

impl Payload {
    pub fn is_none(&self) -> bool {
        matches!(self, Payload::None)
    }

    /// The same payload (shared infos by identity).
    pub fn same(&self, o: &Payload) -> bool {
        use Payload as P;
        match (self, o) {
            (P::None, P::None) => true,
            (P::Module(a), P::Module(b)) => Rc::ptr_eq(a, b),
            (P::Func(a), P::Func(b)) => Rc::ptr_eq(a, b),
            (P::Class(a), P::Class(b)) => Rc::ptr_eq(a, b),
            (P::Cls(a), P::Cls(b)) => a == b,
            (P::Str(a), P::Str(b)) => a == b,
            (P::Method(a, x), P::Method(b, y)) => a == b && x == y,
            (P::Bound(a, x), P::Bound(b, y)) | (P::Super(a, x), P::Super(b, y)) => a == b && x == y,
            (P::View(a, x), P::View(b, y)) => a == b && x == y,
            (P::Zip(a, x), P::Zip(b, y)) | (P::Pair(a, x), P::Pair(b, y)) => a == b && x == y,
            (P::Enumerate(a, x), P::Enumerate(b, y)) => a == b && x == y,
            (P::Partial(a, x, k), P::Partial(b, y, l)) => a == b && x == y && k == l,
            (P::Groupby(a, x, p, q), P::Groupby(b, y, r, s)) => a == b && x == y && p == r && q == s,
            (P::Locidx(a, x), P::Locidx(b, y)) => a == b && x == y,
            (P::Range(a, x, p), P::Range(b, y, q)) => a == b && x == y && p == q,
            _ => false,
        }
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
    /// containers built from a literal: positional slots are valid
    pub literal_len: Option<usize>,
    /// after appends of unknown count: positions below this are still valid
    pub known_prefix: Option<usize>,
    /// the object standing for "the current element" of a loop
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
    /// `@name.setter` functions by (defining class, attribute)
    pub prop_setters: FxHashMap<(ObjId, KeyId), ObjId>,
    /// a copy's original (through copies made at the same node: `out = deepcopy(out)` in a loop
    /// makes one copy site, not a new one per round)
    pub copy_root: FxHashMap<ObjId, ObjId>,
    pub muts: u64,
    pub last_mut: Option<&'static std::panic::Location<'static>>,
}

impl Default for Heap {
    fn default() -> Heap {
        let none = Obj { id: 0, kind: Kind::Unknown, site: "".into(), ctx: "".into(), single: true, label: "<none>".into(), payload: Payload::None, literal_len: None, known_prefix: None, iter_elem: false, derived_from: None, depth: 0, iterator: false, born: 0 };
        Heap { objects: vec![none], index: FxHashMap::default(), site_clones: FxHashMap::default(), serial: 0, keys: ObjKeys::default(), cls_slots: FxHashMap::default(), prop_setters: FxHashMap::default(), copy_root: FxHashMap::default(), muts: 0, last_mut: None }
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
    #[track_caller]
    pub fn obj_mut(&mut self, oid: ObjId) -> &mut Obj {
        self.muts += 1;
        self.last_mut = Some(std::panic::Location::caller());
        &mut self.objects[oid as usize]
    }
    /// Set `oid`'s payload (a change only if it differs).
    #[track_caller]
    pub fn set_payload(&mut self, oid: ObjId, p: Payload) {
        if !self.objects[oid as usize].payload.same(&p) {
            self.obj_mut(oid).payload = p;
        }
    }
    /// Mark `oid` a one-shot iterator.
    pub fn set_iterator(&mut self, oid: ObjId) {
        if !self.objects[oid as usize].iterator {
            self.obj_mut(oid).iterator = true;
        }
    }
    /// Mark `oid` the current element of a loop.
    pub fn set_iter_elem(&mut self, oid: ObjId) {
        if !self.objects[oid as usize].iter_elem {
            self.obj_mut(oid).iter_elem = true;
        }
    }
    /// Set `oid`'s access-path depth.
    pub fn set_depth(&mut self, oid: ObjId, d: u32) {
        if self.objects[oid as usize].depth != d {
            self.obj_mut(oid).depth = d;
        }
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
    #[track_caller]
    pub fn alloc_raw(&mut self, kind: Kind, site: &str, ctx: &str, single: bool, label: &str, payload: Payload) -> ObjId {
        let key = (kind, intern::s(site), intern::s(ctx));
        if let Some(&oid) = self.index.get(&key) {
            let o = &mut self.objects[oid as usize];
            if o.single && !single || (!payload.is_none() && !o.payload.same(&payload)) {
                self.muts += 1;
                self.last_mut = Some(std::panic::Location::caller());
            }
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
            literal_len: None,
            known_prefix: None,
            iter_elem: false,
            derived_from: None,
            depth: 0,
            iterator: site.starts_with("gen:"),
            born: self.serial,
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
        match st.lens.get(&o) {
            Some(v) => v.0,
            None => self.objects[o as usize].literal_len,
        }
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
            None => self.objects[o as usize].literal_len.or(self.objects[o as usize].known_prefix).unwrap_or(0),
        };
        Some(p).filter(|p| *p > 0)
    }

    /// (also the object's own field: what a state not descended from the one it was made in
    /// reads)
    pub fn set_llen(&mut self, o: ObjId, st: &mut State, v: Option<usize>) {
        st.lens.insert(o, (v, v.unwrap_or(0), 0));
        self.objects[o as usize].literal_len = v;
        self.objects[o as usize].known_prefix = None;
    }

    /// The length is unknown now, its first `n` positions are still known.
    pub fn set_prefix(&mut self, o: ObjId, st: &mut State, n: usize) {
        st.lens.insert(o, (None, n, 0));
        self.objects[o as usize].literal_len = None;
        self.objects[o as usize].known_prefix = Some(n);
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
            let o = &self.objects[oid as usize];
            if let Some(&sd) = o.derived_from.and_then(|d| map.get(&d)) {
                self.obj_mut(oid).derived_from = Some(sd);
            }
            let o = &self.objects[oid as usize];
            let rv = |v: &Value| rename(v).unwrap_or_else(|| v.clone());
            let p = match &o.payload {
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
                _ => continue,
            };
            self.set_payload(oid, p);
        }
    }
}
