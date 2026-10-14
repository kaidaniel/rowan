//! enum: members as objects with keys (`Key::Member`), their values and texts, `Flag`
//! combinations, `Cls(value)` lookups.

use ruff_python_ast as ast;

use crate::domain::{join_values, Classification, KeyVal, Ns, Row, Tier, Value};
use crate::frontend::Node;
use crate::intern::{self, key_str_lit, KeyId, KeySet};
use crate::interp::{Analyzer, Payload};
use crate::state::{Kind, State};

impl crate::interp::Analyzer {
    /// A mixed-in member key compares as its value (`Mode.FAST == "fast"`).
    pub fn member_base(&self, v: &Value) -> Value {
        if !v.kv.is_literal() {
            return v.clone();
        }
        let lits = crate::domain::kv_literals(&v.kv);
        if !lits.iter().any(intern::key_is_member) {
            return v.clone();
        }
        Value::with_kv(v.pts.clone(), KeyVal::lits(KeySet::from_iter(lits.iter().map(|k| intern::key_member_value(k).unwrap_or(k)))))
    }

    /// The class and literal `value` of a member object of a Flag / Enum.
    pub fn member_value(&self, o: u32, state: &State) -> Option<(u32, KeyId)> {
        if !self.member_key.contains_key(&o) {
            return None;
        }
        let Payload::Cls(cls) = self.obj(o).payload else { return None };
        let v = state.slot(o, Ns::Attr, key_str_lit("value"));
        v.kv.as_lit().filter(|_| v.pts.is_empty()).map(|k| (cls, k))
    }

    /// `str(m)` / `f"{m}"` of Enum members: the value (StrEnum, IntEnum, IntFlag), else `Cls.NAME`.
    pub fn enum_text(&self, v: &Value, state: &State) -> Option<Value> {
        if v.pts.is_empty() || !v.kv.is_bot() {
            return None;
        }
        // pathlib paths: their text
        if v.pts.iter().all(|o| crate::models::pathtext::is_path(self, o)) {
            let ts: Option<Vec<KeyId>> = v.pts.iter().map(|o| crate::models::pathtext::path_text(self, o).map(|t| key_str_lit(&t))).collect();
            return ts.map(|ks| Value::kv_only(KeyVal::lits(KeySet::from_iter(ks))));
        }
        let mut out: Vec<KeyId> = Vec::new();
        for o in v.pts.iter() {
            let mk = *self.member_key.get(&o)?;
            let Payload::Cls(cls) = self.obj(o).payload else { return None };
            let ek = self.enum_kind.get(&cls).copied()?;
            if ek.repr_value {
                let (_, val) = self.member_value(o, state)?;
                out.push(key_str_lit(&intern::key_py_str(val)));
            } else {
                let q = intern::with_key(mk, |k| if let intern::Key::Member(q, _) = k { Some(q.to_string()) } else { None })?;
                out.push(key_str_lit(&q));
            }
        }
        Some(Value::kv_only(KeyVal::lits(KeySet::from_iter(out))))
    }

    /// `a | b`, `a & b`, `a ^ b` on members of one Flag class with known values: the member
    /// with the resulting value, else a combination (`Perm.R|W`) allocated here.
    pub fn flag_binop(&mut self, op: ast::Operator, a: &Value, b: &Value, node: &Node, state: &mut State) -> Option<Value> {
        if !matches!(op, ast::Operator::BitOr | ast::Operator::BitAnd | ast::Operator::BitXor) || self.member_key.is_empty() {
            return None;
        }
        if a.pts.len() != 1 || b.pts.len() != 1 || !a.kv.is_bot() || !b.kv.is_bot() {
            return None;
        }
        let (ca, va) = self.member_value(a.pts.first()?, state)?;
        let (cb, vb) = self.member_value(b.pts.first()?, state)?;
        if ca != cb || !self.enum_kind.get(&ca).map(|k| k.flag).unwrap_or(false) {
            return None;
        }
        let (x, y) = (intern::key_as_int(va)?, intern::key_as_int(vb)?);
        let r = match op {
            ast::Operator::BitOr => x | y,
            ast::Operator::BitAnd => x & y,
            _ => x ^ y,
        };
        Some(Value::obj(self.flag_member(ca, r, node, state)))
    }

    /// The member of Flag class `cls` with value `r`: a defined one, else a combination object.
    pub fn flag_member(&mut self, cls: u32, r: i64, node: &Node, state: &mut State) -> u32 {
        let vk = key_str_lit("value");
        let ms = self.enum_members.get(&cls).cloned().unwrap_or_default();
        for (_, m) in &ms {
            if state.slot(*m, Ns::Attr, vk).kv.as_lit() == Some(intern::key_int(r)) {
                return *m;
            }
        }
        // the name: the members it combines, lowest value first (`R|W`); None for 0 / unnamed bits
        let mut parts: Vec<(i64, String)> = Vec::new();
        let mut covered = 0i64;
        for (n, m) in &ms {
            if let Some(v) = state.slot(*m, Ns::Attr, vk).kv.as_lit().and_then(intern::key_as_int) {
                if v != 0 && (v & (v - 1)) == 0 && r & v == v {
                    parts.push((v, n.clone()));
                    covered |= v;
                }
            }
        }
        parts.sort_by(|p, q| p.0.cmp(&q.0));
        let name = if r != 0 && covered == r { Value::lit_str(&parts.iter().map(|p| p.1.as_str()).collect::<Vec<_>>().join("|")) } else { self.none_v.clone() };
        let cname = self.class_info(cls).name.clone();
        let qual = format!("{}({})", cname, r);
        let mid = self.alloc_raw(Kind::Instance, &format!("flag:{}:{}", cls, r), "", true, &qual, Payload::Cls(cls));
        let mut row = Row::new();
        for (k, val) in [("name", name), ("value", Value::lit(intern::key_int(r)))] {
            let kk = key_str_lit(k);
            row = row.store(kk, true);
            state.set_slot(mid, Ns::Attr, kk, val, true);
        }
        state.set_row(mid, Ns::Attr, row);
        let text = match state.slot(mid, Ns::Attr, key_str_lit("name")).kv.as_lit().and_then(intern::key_as_str) {
            Some(n) => format!("{}.{}", cname, n),
            None => qual,
        };
        let ek = self.enum_kind.get(&cls).copied().unwrap_or_default();
        let mk = intern::key_member(&text, if ek.mixed { Some(intern::key_int(r)) } else { None });
        self.member_key.insert(mid, mk);
        self.key_member.insert(mk, mid);
        let _ = node;
        mid
    }

    /// `a in b` on members of one Flag class with known values: `a & b == a`.
    pub fn flag_contains(&self, a: &Value, b: &Value, state: &State) -> Option<bool> {
        if self.member_key.is_empty() || a.pts.len() != 1 || b.pts.len() != 1 || !a.kv.is_bot() || !b.kv.is_bot() {
            return None;
        }
        let (ca, va) = self.member_value(a.pts.first()?, state)?;
        let (cb, vb) = self.member_value(b.pts.first()?, state)?;
        if ca != cb || !self.enum_kind.get(&ca).map(|k| k.flag).unwrap_or(false) {
            return None;
        }
        let (x, y) = (intern::key_as_int(va)?, intern::key_as_int(vb)?);
        Some(x & y == x)
    }
}

impl crate::interp::Analyzer {
    /// `Color(3)`: the member of Enum class `cls` (members `ms`) with that value — any of the
    /// Flag's bits combined —, else ValueError; a member given is itself.
    pub(crate) fn enum_lookup(&mut self, cls: u32, ms: &[(String, u32)], a: &Value, node: &Node, state: &mut State) -> Value {
        let vk = key_str_lit("value");
        let lits = if a.pts.is_empty() && a.kv.is_literal() { Some(crate::domain::kv_literals(&a.kv)) } else { None };
        let mut out = Value::new();
        match lits {
            Some(ls) => {
                let mut all_hit = true;
                let is_flag = self.enum_kind.get(&cls).map(|k| k.flag).unwrap_or(false);
                let all_bits: i64 = ms.iter().filter_map(|(_, m)| state.slot(*m, Ns::Attr, vk).kv.as_lit().and_then(intern::key_as_int)).fold(0, |a, b| a | b);
                for l in ls.iter() {
                    let hit = ms.iter().find(|(_, m)| state.slot(*m, Ns::Attr, vk).kv.as_lit() == Some(l));
                    match hit {
                        Some((_, m)) => out = out.join(&Value::obj(*m)),
                        // a Flag: any combination of the members' bits (0 too)
                        None => match intern::key_as_int(l).filter(|i| is_flag && *i >= 0 && i & !all_bits == 0) {
                            Some(i) => {
                                let fm = self.flag_member(cls, i, node, state);
                                out = out.join(&Value::obj(fm));
                            }
                            None => all_hit = false,
                        },
                    }
                }
                if out.empty() {
                    self.raise_here("ValueError", state);
                    return Value::new();
                }
                if !all_hit {
                    self.may_raise_here("ValueError", state);
                }
            }
            None => {
                // a member given: itself
                let given: Vec<u32> = a.pts.iter().filter(|o| ms.iter().any(|(_, m)| m == o)).collect();
                if !given.is_empty() && given.len() == a.pts.len() && a.kv.is_bot() {
                    return Value::objs(crate::domain::ObjSet::from_vec(given));
                }
                self.may_raise_here("ValueError", state);
                out = join_values(ms.iter().map(|(_, m)| Value::obj(*m)).collect::<Vec<_>>());
            }
        }
        out
    }
}

impl crate::interp::Analyzer {
    /// An Enum class: each plain class-level `NAME = value` is a member, an instance of the
    /// class with known `name` and `value`, in definition order.
    pub(crate) fn make_enum_members(&mut self, c: &'static ast::StmtClassDef, coid: u32, ek: crate::interp::EnumKind, str_mixin: bool, state: &mut State) {
        let mut members: Vec<(String, u32)> = Vec::new();
        self.enum_kind.insert(coid, ek);
        // `auto()`: the lower-cased name (StrEnum), the next power of two (Flag), else the
        // last int value + 1
        let mut last_int: i64 = 0;
        let mut member_vals: Vec<(crate::intern::KeyId, u32)> = Vec::new();
        for st in &c.body {
            let ast::Stmt::Assign(a) = st else { continue };
            let [ast::Expr::Name(n)] = &a.targets[..] else { continue };
            let name = n.id.to_string();
            if name.starts_with('_') {
                continue;
            }
            let nk = key_str_lit(&name);
            let is_auto = match &*a.value {
                ast::Expr::Call(cl) => match &*cl.func {
                    ast::Expr::Name(f) => f.id.as_str() == "auto",
                    ast::Expr::Attribute(at) => at.attr.as_str() == "auto",
                    _ => false,
                },
                _ => false,
            };
            // (`class M(str, Enum)` with `auto()`: str of an int, deprecated: left unknown)
            if is_auto && !(str_mixin && !ek.is_str) {
                let av = if ek.is_str && !ek.flag {
                    Value::lit(key_str_lit(&name.to_lowercase()))
                } else if ek.flag {
                    let nx = if last_int <= 0 { 1 } else { 1i64 << (64 - (last_int as u64).leading_zeros()) };
                    Value::lit(crate::intern::key_int(nx))
                } else {
                    Value::lit(crate::intern::key_int(last_int + 1))
                };
                self.cls_slots.insert((coid, Ns::Attr, nk), av);
            }
            let Some(v) = self.cls_slots.get(&(coid, Ns::Attr, nk)).cloned() else { continue };
            if v.pts.iter().any(|o| matches!(self.kind(o), Kind::Function | Kind::Class)) {
                continue;
            }
            if let Some(i) = v.kv.as_lit().filter(|_| v.pts.is_empty()).and_then(crate::intern::key_as_int) {
                last_int = last_int.max(i);
            }
            // a value an earlier member has: an alias of that member (not iterated)
            let vlit = v.kv.as_lit().filter(|_| v.pts.is_empty());
            if let Some(prev) = vlit.and_then(|l| member_vals.iter().find(|(x, _)| *x == l)).map(|(_, m)| *m) {
                self.cls_slots.insert((coid, Ns::Attr, nk), Value::obj(prev));
                state.set_slot(coid, Ns::Attr, nk, Value::obj(prev), true);
                self.enum_aliases.entry(coid).or_default().push((name, prev));
                continue;
            }
            let mid = self.alloc_raw(Kind::Instance, &format!("enum:{}:{}", coid, name), "", true, &format!("{}.{}", c.name.id, name), Payload::Cls(coid));
            let mut row = Row::new();
            for (k, val) in [("name", Value::lit(nk)), ("value", v.clone())] {
                let kk = key_str_lit(k);
                row = row.store(kk, true);
                state.set_slot(mid, Ns::Attr, kk, val, true);
            }
            state.set_row(mid, Ns::Attr, row);
            self.cls_slots.insert((coid, Ns::Attr, nk), Value::obj(mid));
            state.set_slot(coid, Ns::Attr, nk, Value::obj(mid), true);
            let mval = if ek.mixed { v.kv.as_lit().filter(|_| v.pts.is_empty()) } else { None };
            let mk = crate::intern::key_member(&format!("{}.{}", c.name.id, name), mval);
            self.member_key.insert(mid, mk);
            self.key_member.insert(mk, mid);
            if let Some(l) = vlit {
                member_vals.push((l, mid));
            }
            self.enum_aliases.entry(coid).or_default().push((name.clone(), mid));
            members.push((name, mid));
        }
        self.enum_members.insert(coid, members);
    }
}

/// `Color["RED"]`: an Enum's member by name (KeyError for a name that is not one).
pub fn member_by_name(an: &mut Analyzer, obj: &Value, key: &Value, node: &Node, guarded: bool, state: &mut State) -> Value {
    let mut out = Value::new();
    for oid in obj.pts.iter() {
        let ms = an.enum_members[&oid].clone();
        let aliases = an.enum_aliases.get(&oid).cloned().unwrap_or_default();
        match key.kv.as_lit().filter(|_| key.pts.is_empty()).and_then(intern::key_as_str) {
            Some(nm) => match aliases.iter().chain(ms.iter()).find(|(n, _)| *n == *nm) {
                Some((_, m)) => {
                    let cls = Classification::new(Tier::Proven);
                    if !guarded { an.verdict(node, key_str_lit(&nm), &cls, oid, Ns::Item, None, None); } else { an.verdict_guarded(node, key_str_lit(&nm), &cls, oid, Ns::Item); }
                    out = out.join(&Value::obj(*m));
                }
                None => {
                    let cls = Classification::with_note(Tier::Missing, "is not a member of the Enum");
                    if !guarded { an.verdict(node, key_str_lit(&nm), &cls, oid, Ns::Item, None, None); } else { an.verdict_guarded(node, key_str_lit(&nm), &cls, oid, Ns::Item); }
                    if obj.pts.len() == 1 {
                        an.raise_here("KeyError", state);
                    }
                }
            },
            None => {
                an.may_raise_here("KeyError", state);
                out = out.join(&join_values(ms.iter().map(|(_, m)| Value::obj(*m)).collect::<Vec<_>>()));
            }
        }
    }
    out
}
