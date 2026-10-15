//! Keys: the key a value stands for in a record (numbers equal across int / float / bool,
//! Enum members, identity-hashed objects, frozen dataclass instances), and back (`unkey`).


use crate::domain::{KeyVal, Ns, Value};
use crate::intern::{self, key_str_lit, KeyId, KeySet};
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
