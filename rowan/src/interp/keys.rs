//! Keys: the key a value stands for in a record (numbers equal across int / float / bool,
//! Enum members, identity-hashed objects, frozen dataclass instances), and back (`unkey`).

use crate::domain::{KeyVal, Ns, Value};
use crate::intern::{self, key_int, key_str_lit, KeyId, KeySet};
use crate::state::{Kind, State};

use super::{Analyzer, Payload};

impl Analyzer {
    /// `True`, `1` and `1.0` are one dict key: a numeric literal key equal to a key the record
    /// has (or may have) is that key.
    /// A mixed-in Enum member (`StrEnum`, `IntEnum`) and its value are one key too.
    pub(in crate::interp) fn canon_key(&self, oid: u32, ns: Ns, k: KeyId, state: &State) -> KeyId {
        if !self.m.ci_dicts.is_empty() && self.m.ci_dicts.contains(&oid) {
            if let Some(s) = intern::key_as_str(k) {
                return key_str_lit(&s.to_lowercase());
            }
        }
        let members = !self.heap.keys.member_key.is_empty();
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
    pub(in crate::interp) fn keyify(&self, v: &Value) -> Value {
        if v.pts.is_empty() {
            return v.clone();
        }
        let mut ks: Vec<KeyId> = Vec::new();
        let mut rest: Vec<u32> = Vec::new();
        for o in v.pts.iter() {
            match self.heap.keys.member_key.get(&o).copied().or_else(|| self.identity_key(o)) {
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
            return crate::models::dataclasses::frozen_instance_key(self, o, state, depth);
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
    pub(in crate::interp) fn keyify_in(&self, v: &Value, state: &State) -> Value {
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
    pub(in crate::interp) fn identity_key(&self, o: u32) -> Option<KeyId> {
        if let Some(k) = self.heap.keys.identity_keys.borrow().get(&o) {
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
                    !ci.open_base && ci.dataclass_fields.borrow().is_none() && !ci.lib.pydantic && !crate::models::pydantic::is_model(self, c) && !ci.lib.named_tuple && !ci.lib.dict_base && !ci.lib.typed_dict && !ci.pyd_dataclass.get() && self.class_lookup(c, "__eq__").is_none() && self.class_lookup(c, "__hash__").is_none() && !self.m.enum_members.contains_key(&c)
                }
                None => false,
            },
            _ => false,
        };
        if !ok {
            return None;
        }
        let k = intern::key_member(&format!("<{} #{}>", ob.label, o), None);
        self.heap.keys.identity_keys.borrow_mut().insert(o, k);
        self.heap.keys.identity_objs.borrow_mut().insert(k, o);
        Some(k)
    }

    /// Member keys (read back from a record's keys) are the member objects.
    pub fn unkey(&self, v: &Value) -> Value {
        if self.heap.keys.key_member.is_empty() && self.heap.keys.identity_objs.borrow().is_empty() {
            return v.clone();
        }
        let lits = crate::domain::kv_literals(&v.kv);
        if !lits.iter().any(intern::key_is_member) || !v.kv.is_literal() {
            return v.clone();
        }
        let mut pts = v.pts.clone();
        let mut rest: Vec<KeyId> = Vec::new();
        for k in lits.iter() {
            match self.heap.keys.key_member.get(&k).copied().or_else(|| self.heap.keys.identity_objs.borrow().get(&k).copied()) {
                Some(m) => pts = pts.union(&crate::domain::ObjSet::one(m)),
                None => rest.push(k),
            }
        }
        let kv = if rest.is_empty() { KeyVal::Bot } else { KeyVal::lits(KeySet::from_iter(rest)) };
        Value::with_kv(pts, kv)
    }

    pub(in crate::interp) fn canon_kv(&self, oid: u32, ns: Ns, kv: &KeyVal, state: &State) -> KeyVal {
        match kv {
            KeyVal::Lit(k) => KeyVal::Lit(self.canon_key(oid, ns, *k, state)),
            KeyVal::Lits(ks) => KeyVal::Lits(ks.map(|k| self.canon_key(oid, ns, k, state))),
            other => other.clone(),
        }
    }
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

/// Whether hashing a value raises TypeError.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Hashing {
    Hashable,
    Maybe,
    Unhashable,
}

impl Analyzer {
    /// Hashing `v` (a dict key, a set item, `in` a dict or set, `hash(v)`) raises TypeError for a
    /// list, dict or set, and for a tuple holding one: on every path when all of `v` is one, a
    /// possible raise when part of it is, or may be (in a running try: an unknown of no known
    /// type, an instance of a class with `__eq__` and no `__hash__`, a dataclass or model not
    /// frozen).
    pub fn check_hashable(&mut self, v: &Value, state: &mut State) {
        self.check_hashable_if(v, true, state)
    }

    /// `check_hashable`, of a value that is hashed only if `hashed` (else maybe: the items of
    /// an iterable that may be empty).
    pub fn check_hashable_if(&mut self, v: &Value, hashed: bool, state: &mut State) {
        if v.pts.is_empty() && !(self.in_try() && kv_hashing(&v.kv) == Hashing::Maybe) {
            return;
        }
        let hs: Vec<Hashing> = v.pts.iter().map(|o| self.obj_hashing(o, state, 0)).collect();
        let worst = hs.iter().copied().max().unwrap_or(Hashing::Hashable).max(kv_hashing(&v.kv));
        if worst == Hashing::Hashable || (worst == Hashing::Maybe && !self.in_try()) {
            return;
        }
        if hashed && v.kv.is_bot() && hs.iter().all(|h| *h == Hashing::Unhashable) {
            self.raise_here("TypeError", state);
        } else {
            self.may_raise_here("TypeError", state);
        }
    }

    fn obj_hashing(&self, o: u32, state: &State, depth: usize) -> Hashing {
        let ob = self.heap.obj(o);
        match ob.kind {
            Kind::List | Kind::Dict if !ob.iterator => Hashing::Unhashable,
            Kind::Set if !ob.iterator && !ob.label.starts_with("frozenset") => Hashing::Unhashable,
            Kind::Tuple if depth < 3 => {
                let item = |v: &Value| {
                    let hs: Vec<Hashing> = v.pts.iter().map(|x| self.obj_hashing(x, state, depth + 1)).collect();
                    match hs.iter().copied().max() {
                        Some(Hashing::Unhashable) if v.kv.is_bot() && hs.iter().all(|h| *h == Hashing::Unhashable) => Hashing::Unhashable,
                        Some(Hashing::Hashable) | None => kv_hashing(&v.kv),
                        _ => Hashing::Maybe,
                    }
                };
                match self.heap.llen(o, state) {
                    Some(n) if ob.single && n <= 16 => (0..n).map(|i| item(&state.slot(o, Ns::Pos, intern::key_int(i as i64)))).max().unwrap_or(Hashing::Hashable),
                    _ => item(&state.slot(o, Ns::Elem, intern::STAR)).min(Hashing::Maybe),
                }
            }
            Kind::Unknown => Hashing::Maybe,
            Kind::Instance => match self.heap.instance_class(o).and_then(|c| Some((c, self.heap.class_info_opt(c)?))) {
                Some((c, ci)) => {
                    let unfrozen = (ci.dataclass_fields.borrow().is_some() && !ci.dc_frozen.get()) || self.m.pyd_classes.get(&c).is_some_and(|p| !p.frozen);
                    let eq_only = self.class_lookup(c, "__eq__").is_some() && self.class_lookup(c, "__hash__").is_none();
                    if unfrozen || eq_only { Hashing::Maybe } else { Hashing::Hashable }
                }
                None => Hashing::Maybe,
            },
            _ => Hashing::Hashable,
        }
    }
}

/// An unknown of no known type may be anything, a list too; literals and keys hash.
fn kv_hashing(kv: &KeyVal) -> Hashing {
    match kv {
        KeyVal::Data(l) if crate::domain::label_type(&intern::str_get(*l)).is_none() => Hashing::Maybe,
        _ => Hashing::Hashable,
    }
}

impl Analyzer {
    /// A dict or set: its keys are hashed (a key that cannot be raises TypeError).
    pub fn hashes_keys(&self, o: u32) -> bool {
        let ob = self.heap.obj(o);
        matches!(ob.kind, Kind::Dict | Kind::Set) && !ob.iterator
    }

    /// The literals a value may be or a container may hold (a list's, tuple's or set's scalar
    /// elements, a closed dict's keys); None when they are not all known.
    pub fn lit_keys(&mut self, v: &Value, state: &State) -> Option<KeySet> {
        if v.kv.is_literal() && v.pts.is_empty() {
            return Some(crate::domain::kv_literals(&v.kv));
        }
        let mut out = KeySet::new();
        for oid in v.pts.iter() {
            match self.heap.kind(oid) {
                Kind::List | Kind::Tuple | Kind::Set => match state.elems.get(&oid) {
                    Some(kv) if kv.is_literal() => out = out.union(&crate::domain::kv_literals(&kv)),
                    _ => {
                        return None;
                    }
                },
                Kind::Dict => {
                    let row = state.row(oid, Ns::Item);
                    if row.closed() && row.present == row.definite {
                        out = out.union(&row.present);
                    } else {
                        return None;
                    }
                }
                _ => return None,
            }
        }
        if v.pts.is_empty() {
            None
        } else {
            Some(out)
        }
    }

    /// The literals a container surely holds, whichever of its objects it is: of a list,
    /// tuple or set of known length the positions that are one literal, of a dict its keys
    /// there on every path (the literals `lit_keys` gives may be there; these are).
    pub fn sure_lit_keys(&mut self, v: &Value, state: &State) -> Option<KeySet> {
        if v.pts.is_empty() || !v.kv.is_bot() {
            return None;
        }
        let mut out: Option<KeySet> = None;
        for oid in v.pts.iter() {
            let ks: KeySet = match self.heap.kind(oid) {
                Kind::List | Kind::Tuple | Kind::Set if self.heap.obj(oid).single && !crate::state::is_merged(oid) => {
                    let n = self.heap.llen(oid, state)?;
                    (0..n).filter_map(|i| { let sv = state.slot(oid, Ns::Pos, key_int(i as i64)); sv.kv.as_lit().filter(|_| sv.pts.is_empty()) }).collect()
                }
                Kind::Dict => state.row(oid, Ns::Item).definite.clone(),
                _ => return None,
            };
            out = Some(match out {
                None => ks,
                Some(o) => o.filter(|k| ks.contains(k)),
            });
        }
        out
    }

    /// The (key, literal value) items of closed dicts whose keys are all there and whose values
    /// are each one literal (pandas: `df.rename(columns={…})`, a replace mapping).
    pub fn literal_mapping(&mut self, v: &Value, state: &State) -> Option<Vec<(KeyId, KeyId)>> {
        let mut out: Vec<(KeyId, KeyId)> = Vec::new();
        for oid in v.pts.iter() {
            if self.heap.kind(oid) != Kind::Dict {
                return None;
            }
            let row = state.row(oid, Ns::Item);
            if row.open() || row.present != row.definite {
                return None;
            }
            for k in row.present.iter() {
                let sv = state.slot(oid, Ns::Item, k);
                match sv.kv.as_lit() {
                    Some(l) => out.push((k, l)),
                    None => return None,
                }
            }
        }
        if v.pts.is_empty() {
            None
        } else {
            Some(out)
        }
    }
}
