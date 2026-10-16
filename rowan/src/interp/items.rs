//! Item access: reads (verdicts per object, requirements on inputs, placeholders for what an
//! open record holds), stores, copies and deletes of keys, positions and elements.

use indexmap::IndexMap;

use crate::domain::{ObjId, classify, join_rows, join_values, lens_apply, Classification, KeyVal, Ns, ObjSet, Row, Source, SourceKind, Tier, Value, DATA};
use crate::frontend::Node;
use crate::intern::{self, key_str_lit, s, KeyId, KeySet, STAR};
use crate::state::{Kind, State};

use super::{Analyzer, Payload, ViewKind, PATH_DEPTH, WIDE_READ};

/// An item read `obj[key]` (an attribute read when `ns` is `Attr`): what each receiver's part
/// of it needs.
struct ItemRead<'a> {
    obj: &'a Value,
    ns: Ns,
    key: &'a Value,
    node: &'a Node,
    report: bool,
    /// a handler catches its failure (or the read is a model's, unreported)
    guarded: bool,
    /// a real try / except guard: no exception means the key was there
    try_guard: bool,
    /// one receiver, not also a scalar
    single_target: bool,
    line_data: Value,
}

/// The bounds of a slice key as written (`[1:]`, `[…]` when not known); None: not a slice.
fn slice_text(kv: &KeyVal) -> Option<String> {
    let KeyVal::Data(d) = kv else { return None };
    let t = intern::str_get(*d);
    let rest = t.strip_prefix("slice")?;
    let b = rest.rsplit_once('@').map(|(b, _)| b).unwrap_or(rest);
    let b = match b.strip_prefix('[').and_then(|b| b.strip_suffix(']')).map(|b| b.split(':').collect::<Vec<_>>()).as_deref() {
        Some([lo, hi, ""]) => format!("{}:{}", lo, hi),
        Some([lo, hi, st]) => format!("{}:{}:{}", lo, hi, st),
        _ => "…".to_string(),
    };
    Some(format!("[{}]", b))
}

impl Analyzer {
    pub fn load_item(&mut self, obj: &Value, ns: Ns, key: &Value, node: &Node, state: &mut State, report: bool) -> Value {
        if let Some(r) = self.split_mixed(&[obj], state, &mut |an, vs, st| an.load_item(&vs[0], ns, key, node, st, report)) {
            return r;
        }
        if ns == Ns::Item {
            if let Some(v) = self.per_receiver(obj, &|an, o| an.has_user_method(o, "__getitem__"), state, &mut |an, o, st| an.load_item(o, ns, key, node, st, report)) {
                return v;
            }
            if obj.pts.iter().any(|o| self.hashes_keys(o)) {
                self.check_hashable(key, state);
            }
        }
        // (a sequence's position is read without a verdict: a running generator that yielded
        // reads the whole sequence, for code between its items that may change it)
        for o in obj.pts.iter().filter(|o| matches!(self.heap.kind(*o), Kind::List | Kind::Tuple)) {
            self.gens.read(o, None);
        }
        // a numpy structured array's missing field raises ValueError
        let exc = if ns == Ns::Item && !obj.pts.is_empty() && !self.m.np_struct.is_empty() && obj.pts.iter().all(|o| self.m.np_struct.contains(&o)) { Some("ValueError") } else { None };
        let exc = if ns == Ns::Item { self.unwind.item_exc_force.or(exc) } else { exc };
        let saved = std::mem::replace(&mut self.unwind.item_exc, exc);
        let scalar = self.may_be_scalar(obj);
        let saved_scalar = std::mem::replace(&mut self.mode.recv_scalar, scalar);
        let saved_wide = self.wide_read(obj, node);
        let v = self.load_item_inner(obj, ns, key, node, state, report);
        self.mode.recv_scalar = saved_scalar;
        self.end_wide_read(saved_wide, state);
        self.unwind.item_exc = saved;
        v
    }

    pub(super) fn load_item_inner(&mut self, obj: &Value, ns: Ns, key: &Value, node: &Node, state: &mut State, report: bool) -> Value {
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
        if ns == Ns::Item && !obj.pts.is_empty() && obj.pts.iter().all(|o| self.m.enum_members.contains_key(&o)) {
            return crate::models::enums::member_by_name(self, obj, key, node, guarded, state);
        }
        // (all targets' values joined at once: a pairwise join over many targets is quadratic)
        let mut parts: Vec<Value> = vec![std::mem::take(&mut acc)];
        let rd = ItemRead { obj, ns, key, node, report, guarded, try_guard, single_target, line_data };
        // (a receiver of several objects is one of them on a run: what one needs, the input
        // needs only sometimes — each read from the requirements before it)
        // (a scalar part is not counted: indexed by a key it fails, or holds no input's data)
        let several = obj.pts.len() > 1;
        let base = several.then(|| state.reqs.clone());
        let mut outs: Vec<(bool, Reqs)> = Vec::new();
        for oid in obj.pts.iter() {
            // (one that surely lacks the key fails the read: runs that go on took another)
            let fails = several && self.heap.kind(oid).is_record() && key.pts.is_empty() && {
                let (row, ks) = (state.row(oid, ns), crate::domain::kv_literals(&key.kv));
                !ks.is_empty() && ks.iter().all(|k| classify(&row, k).tier == Tier::Missing)
            };
            if let Some(b) = &base {
                state.reqs = b.clone();
            }
            self.item_of(oid, &rd, state, &mut parts);
            if several {
                outs.push((!fails, state.reqs.clone()));
            }
        }
        if let Some(base) = base {
            needs_of_one(&base, &outs, state);
        }
        let line_data = rd.line_data;
        acc = crate::domain::join_values(parts);
        // (a part that is no object: what an unknown, a mapping's key or a str / tuple literal
        // holds is not known here; None or a number only fails the read)
        if obj.pts.is_empty() || obj.kv.may_have_items() {
            acc = if obj.pts.is_empty() { line_data } else { acc.join(&line_data) };
        }
        if let Some(ts) = frame_tiers {
            if ts.iter().all(|t| *t == Tier::Missing) {
                self.raise_here("KeyError", state);
            }
        }
        acc
    }

    /// One receiver's part of an item read (`load_item_inner`), pushed to `parts`.
    fn item_of(&mut self, oid: ObjId, rd: &ItemRead, state: &mut State, parts: &mut Vec<Value>) {
        let (obj, ns, key, node) = (rd.obj, rd.ns, rd.key, rd.node);
        let (report, guarded, single_target, line_data) = (rd.report, rd.guarded, rd.single_target, &rd.line_data);
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
                return;
            }
            Kind::Groupby => {
                // (several groupby objects: each selection is an object of its own)
                let several = obj.pts.iter().filter(|o| self.heap.kind(*o) == Kind::Groupby).count() > 1;
                let saved = if several { self.cx.alloc_extra.replace(format!("gb{}", oid)) } else { self.cx.alloc_extra.clone() };
                let v = crate::models::pandas::groupby_select(self, oid, key, node, state);
                self.cx.alloc_extra = saved;
                parts.push(v);
                return;
            }
            Kind::Instance if ns == Ns::Item => { if self.item_of_instance(oid, key, node, state, single_target, line_data, parts) { return; } }
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
            Kind::List | Kind::Tuple | Kind::Set => { if self.item_of_sequence(oid, kind, key, node, state, single_target, parts) { return; } }
            Kind::Series => parts.push(Value::with_kv(ObjSet::one(self.m.series), DATA)),
            Kind::Frame if ns == Ns::Item => {
                let v = crate::models::pandas::frame_getitem_n(self, oid, key, node, state, guarded, report);
                parts.push(v);
            }
            Kind::Unknown if ns == Ns::Item && (slice_text(&key.kv).is_some() || self.slice_base(oid).is_some()) => parts.push(self.item_of_slice(oid, rd, state)),
            k if k.is_record() => parts.push(self.item_of_record(oid, rd, state)),
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

    /// `rec[key]` on a record: the key made canonical (`1` for `True`), reported as written.
    fn item_of_record(&mut self, oid: ObjId, rd: &ItemRead, state: &mut State) -> Value {
        let (ns, key, node) = (rd.ns, rd.key, rd.node);
        let ckv = self.canon_kv(oid, ns, &key.kv, state);
        if ckv != key.kv {
            let saved = self.mode.key_alias.take();
            if let (KeyVal::Lit(c), KeyVal::Lit(o)) = (&ckv, &key.kv) {
                self.mode.key_alias = Some((*c, *o));
            }
            let ck = Value::with_kv(key.pts.clone(), ckv);
            let v = self.load_record(oid, ns, &ck, node, state, rd.guarded, rd.single_target, rd.try_guard);
            self.mode.key_alias = saved;
            v
        } else {
            self.load_record(oid, ns, key, node, state, rd.guarded, rd.single_target, rd.try_guard)
        }
    }

    /// `seq[a:b]` on an unknown sequence: a sequence of its elements (`slice_base`, whatever the
    /// bounds); an element of such a slice is an element of the sequence, at a position not known.
    fn item_of_slice(&mut self, oid: ObjId, rd: &ItemRead, state: &mut State) -> Value {
        let bases = self.slice_base(oid).unwrap_or_else(|| ObjSet::one(oid));
        match slice_text(&rd.key.kv) {
            Some(bounds) => {
                // (an object that cannot be sliced: a TypeError)
                self.may_raise_here("TypeError", state);
                let sid = self.elems_of(&bases, &bounds, state);
                let one = (bases.len() == 1).then(|| bases.to_vec()[0]);
                if let Some((b, sym)) = one.and_then(|b| self.names.get(b).map(|n| (b, n))).filter(|_| !self.names.has(sid)).map(|(b, n)| (b, format!("{}{}", n, bounds))) {
                    let scope = self.names.scope(b);
                    self.names.set(sid, sym, scope);
                }
                Value::obj(sid)
            }
            None => {
                let mut out: Vec<Value> = Vec::new();
                for b in bases.iter() {
                    out.push(self.load_record(b, Ns::Item, &Value::data("index"), rd.node, state, rd.guarded, rd.single_target, rd.try_guard));
                }
                crate::domain::join_values(out)
            }
        }
    }

    /// An unknown sequence of the elements of the unknown sequences `bases` (a slice of one,
    /// `bounds` `[1:]`; a concatenation of several, `+`), whatever its own content.
    pub(in crate::interp) fn elems_of(&mut self, bases: &ObjSet, bounds: &str, state: &mut State) -> ObjId {
        let ids: Vec<String> = bases.iter().map(|b| b.to_string()).collect();
        let label = bases.iter().map(|b| self.heap.obj(b).label.to_string()).collect::<Vec<_>>().join(" + ") + bounds;
        let kind = if bounds.is_empty() { "concat" } else { "slice" };
        let sid = self.heap.alloc_raw(Kind::Unknown, &format!("{}:{}", kind, ids.join(",")), "", false, &label, Payload::View(ViewKind::Elems, Value::objs(bases.clone())));
        // (its own content is not known: what a copy of it holds, say)
        if !state.has_row(sid, Ns::Item) {
            state.set_row(sid, Ns::Item, Row::top());
            state.set_row(sid, Ns::Attr, Row::top());
        }
        sid
    }

    /// The sequences an unknown slice or concatenation holds the elements of (`elems_of`).
    pub(in crate::interp) fn slice_base(&self, oid: ObjId) -> Option<ObjSet> {
        match &self.heap.obj(oid).payload {
            Payload::View(ViewKind::Elems, v) if self.heap.kind(oid) == Kind::Unknown && !v.pts.is_empty() => Some(v.pts.clone()),
            _ => None,
        }
    }

    /// `seq[i]` / `seq[a:b]` on a list, tuple or set: the position (from either end; a prefix still
    /// known after appends of unknown count, the last appended), a slice (known positions or the same
    /// elements), else any element (may raise IndexError). True: the read is done.
    fn item_of_sequence(&mut self, oid: ObjId, kind: Kind, key: &Value, node: &Node, state: &mut State, single_target: bool, parts: &mut Vec<Value>) -> bool {
        let is_slice = matches!(&key.kv, KeyVal::Data(s) if intern::str_get(*s).starts_with("slice"));
        // known positions: a slice with known bounds (`xs[1:3]`, `xs[slice(0, n)]`), or one from
        // the start to a bound not known (`xs[:n]`: each position it has is xs's, its length one
        // of 0..=len(xs))
        let known = match (is_slice && kind != Kind::Set && single_target, self.heap.llen(oid, state)) {
            (true, Some(n)) => match Self::slice_label_bounds(&key.kv) {
                Some((lo, hi, step)) if step != Some(0) => Some((crate::interp::exprs::slice_positions(n as i64, lo, hi, step.unwrap_or(1)), None)),
                None if n < 16 && matches!(&key.kv, KeyVal::Data(s) if intern::str_get(*s).starts_with("slice-prefix@")) => Some(((0..n as i64).collect(), Some(((1u32 << (n + 1)) - 1) as u16))),
                _ => None,
            },
            _ => None,
        };
        if let Some((idx, mask)) = known {
            let vals: Vec<Value> = idx.iter().map(|i| state.slot(oid, Ns::Pos, intern::key_int(*i))).collect();
            let kv = state.elems.get(&oid).unwrap_or(DATA);
            let v = self.alloc_positional(kind, node, Some(vals), Value::new(), kv, state);
            if let (Some(m), Some(nid)) = (mask, v.single()) {
                state.set_lens(nid, (None, 0, m));
            }
            parts.push(v);
            return true;
        }
        if is_slice && kind != Kind::Set {
            // a slice is a container of the same elements (positions unknown)
            let elem = state.slot(oid, Ns::Elem, STAR);
            let kv = state.elems.get(&oid).unwrap_or(DATA);
            // (one object per sequence sliced: of several, each its own)
            let part = (!single_target).then(|| format!("part{}", parts.len()));
            let nid = self.alloc_at(kind, node, state, &format!("{}@L{}", kind.name(), node.line), Payload::None, None, Ns::Item, None, part.as_deref());
            self.heap.set_llen(nid, state, None);
            state.set_slot(nid, Ns::Elem, STAR, elem, true);
            state.elems.insert(nid, kv);
            parts.push(Value::obj(nid));
            return true;
        }
        let ll = self.heap.llen(oid, state);
        let idx = key.kv.as_lit().and_then(intern::key_as_int);
        // the element appended last
        if let (Some(-1), None, Some(v)) = (idx, ll, state.last_of(oid)) {
            parts.push(v);
            return true;
        }
        // positions below a known prefix stay valid after appends of unknown count
        if let (Some(i), None, Some(p)) = (idx, ll, self.heap.prefix(oid, state)) {
            if 0 <= i && (i as usize) < p {
                parts.push(state.slot(oid, Ns::Pos, intern::key_int(i)));
                return true;
            }
        }
        // (one of a few lengths: the position where it is there; a negative one from each end)
        if let (Some(i), None, Some(alts)) = (idx, ll, self.heap.len_alts(oid, state).filter(|_| kind != Kind::Set)) {
            let at: Vec<i64> = alts.iter().map(|n| if i < 0 { *n as i64 + i } else { i }).collect();
            let inside: Vec<i64> = alts.iter().zip(&at).filter(|(n, p)| **p >= 0 && **p < **n as i64).map(|(_, p)| *p).collect();
            if !inside.is_empty() && inside.iter().all(|p| state.has_slot(oid, Ns::Pos, intern::key_int(*p))) {
                if inside.len() < alts.len() {
                    self.may_raise_here("IndexError", state);
                }
                parts.push(join_values(inside.iter().map(|p| state.slot(oid, Ns::Pos, intern::key_int(*p))).collect::<Vec<Value>>()));
                return false;
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
        false
    }

    /// `obj[key]` on an instance: its class's `__getitem__`, `__missing__` for keys the dict lacks, a
    /// NamedTuple's positions, else nothing (a plain class: not subscriptable). True: the read is
    /// done (the parts pushed).
    fn item_of_instance(&mut self, oid: ObjId, key: &Value, node: &Node, state: &mut State, single_target: bool, line_data: &Value, parts: &mut Vec<Value>) -> bool {
        let cls = self.heap.instance_class(oid);
        if let Some(cls) = cls {
            if let Some(m) = self.class_lookup(cls, "__getitem__") {
                let fv = self.heap.cls_slots.get(&(m, Ns::Attr, key_str_lit("__getitem__"))).cloned().unwrap_or_default();
                for f in fv.pts.iter() {
                    if self.heap.kind(f) == Kind::Function {
                        let r = self.call_user(f, Some(Value::obj(oid)), vec![key.clone()], IndexMap::new(), vec![], vec![], state, node, false);
                        parts.push(r);
                    }
                }
                return true;
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
                        return true;
                    }
                }
            }
            // a NamedTuple by position: its fields in order, from either end
            if crate::models::dataclasses::is_named_tuple_class(self, cls) {
                let v = crate::models::dataclasses::named_tuple_item(self, oid, cls, key, single_target, node, state);
                parts.push(v);
                return true;
            }
            if let Some(ci) = self.heap.class_info_opt(cls) {
                if !ci.lib.typed_dict && !ci.lib.named_tuple && !ci.open_base {
                    parts.push(line_data.clone());
                    return true;
                }
            }
        }
        false
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn load_record(&mut self, oid: u32, ns: Ns, key: &Value, node: &Node, state: &mut State, guarded: bool, single_target: bool, try_guard: bool) -> Value {
        // (a load a `try` handler guards: its exception still fires when the key is missing, so
        // the key is required of the input all the same — the runtime oracle's rule)
        let caught = try_guard;
        let saved = std::mem::replace(&mut self.unwind.caught_load, caught);
        let v = self.load_record_inner(oid, ns, key, node, state, guarded, single_target, try_guard);
        self.unwind.caught_load = saved;
        v
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn load_record_inner(&mut self, oid: u32, ns: Ns, key: &Value, node: &Node, state: &mut State, guarded: bool, single_target: bool, try_guard: bool) -> Value {
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
                    self.guarded_read_raises(oid, ns, k, kind, &cls, &mut row, node, state, single_target, try_guard, is_lit);
                }
                if guarded {
                    self.verdict_guarded(node, k, &cls, oid, ns);
                }
                // (a handler that re-raises, a thread / future: the input still needs the key)
                if guarded && cls.tier == Tier::Input && (self.unwind.catching_depth > 0 || self.guard_reraises(ns) || self.unwind.caught_load) {
                    self.require_providers_k(&cls, &row, true, state, Some(k), ns);
                }
                if !guarded {
                    self.unguarded_read(oid, ns, k, &cls, &mut row, node, state, single_target, is_lit);
                }
                let v = self.literal_key_value(oid, ns, k, &cls, &row, &row0, state, is_lit);
                acc = acc.join(&v);
            }
            return acc;
        }
        if let KeyVal::KeysOf(ko) = kv {
            return self.keys_of_key_read(oid, ns, ko, &row, node, state, guarded);
        }
        // Data key: it may be one the record lacks (KeyError), unless the read cannot fail
        if !guarded || try_guard {
            self.unwind.cur_exc = None;
            self.may_raise_here(if ns == Ns::Item { self.unwind.item_exc.unwrap_or("KeyError") } else { "AttributeError" }, state);
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
        if row.open() {
            let p = self.path_placeholder_value(oid, ns, STAR, &row, state);
            v = v.join(&p);
        }
        Value::with_kv(v.pts, DATA)
    }

    /// A guarded read of key `k` that may fail: the handler starts from this state (with a
    /// KeyError / AttributeError carrying the key); a sure miss ends the try body here; else
    /// the key is there from here on.
    #[allow(clippy::too_many_arguments)]
    fn guarded_read_raises(&mut self, oid: ObjId, ns: Ns, k: KeyId, kind: Kind, cls: &Classification, row: &mut Row, node: &Node, state: &mut State, single_target: bool, try_guard: bool, is_lit: bool) {
        if crate::interp::debug_level() == 2 {
            eprintln!("  guarded access may raise: {:?} tier {:?} at L{}", intern::key_repr(k), cls.tier, node.line);
        }
        // (a model's own read — pydantic collecting its field errors — raises nothing here:
        // the model raises its ValidationError, with every error, afterwards)
        let model_read = ns == Ns::Item && self.unwind.item_exc == Some("ValidationError");
        if model_read {
            if self.unwind.caught_load && cls.tier == Tier::Input {
                self.require_providers_k(cls, row, true, state, Some(k), ns);
            }
        }
        // the handler sees the state as it is here (and a KeyError / AttributeError
        // carrying the key)
        if model_read {
        } else if !self.unwind.tries.is_empty() {
            let cname = if ns == Ns::Item { self.unwind.item_exc.unwrap_or("KeyError") } else { "AttributeError" };
            let e = crate::models::exceptions::exception_object(self, cname, vec![Value::lit(k)], node, state);
            self.unwind.cur_exc = Some(e);
        }
        // (required on the handler's path too: the exception fired for the key)
        if !model_read && self.unwind.caught_load && cls.tier == Tier::Input {
            self.require_providers_k(cls, row, true, state, Some(k), ns);
        }
        if !model_read {
            self.unwind.cur_exc_cls = Some(super::builtin_exc_names(if ns == Ns::Item { self.unwind.item_exc.unwrap_or("KeyError") } else { "AttributeError" }));
            self.record_raise_state(state);
            self.unwind.cur_exc_cls = None;
            // (an object of unknown kind indexed by a number may be a sequence: an
            // IndexError then)
            if ns == Ns::Item && intern::key_as_int(k).is_some() && matches!(kind, Kind::Unknown | Kind::External) {
                self.unwind.cur_exc = None;
                self.may_raise_here("IndexError", state);
            }
        }
        if model_read {
        } else if try_guard && cls.tier == Tier::Missing && is_lit && single_target {
            // the access raises here: the rest of the try body is unreachable
            state.reachable = false;
        }
        if try_guard && !model_read && cls.tier != Tier::Missing && is_lit && single_target && self.heap.obj(oid).single {
            // no exception raised: the key is present from here on
            *row = row.narrow_in(k, true);
            state.set_row_narrowed(oid, ns, row.clone());
        }
    }

    /// An unguarded read of key `k`: its verdict, the input's requirement (a TypedDict
    /// parameter's too), and the key narrowed in on success.
    #[allow(clippy::too_many_arguments)]
    fn unguarded_read(&mut self, oid: ObjId, ns: Ns, k: KeyId, cls: &Classification, row: &mut Row, node: &Node, state: &mut State, single_target: bool, is_lit: bool) {
        self.verdict(node, k, cls, oid, ns, None, None);
        if cls.tier == Tier::Input {
            self.require_providers_k(cls, row, true, state, Some(k), ns);
        }
        if cls.tier == Tier::Proven && ns == Ns::Item {
            if let Some(src) = self.m.td_param_src.get(&oid).copied() {
                let fresh = !state.reqs.contains(&src, &k);
                state.require(src, k, intern::IdSet::new(), intern::IdSet::new(), intern::IdSet::new());
                if fresh && (self.cx.branch_depth > 0 || self.cx.cond_depth > 0) {
                    if let Some(cur) = state.reqs.get(&src, &k) {
                        state.reqs.set(&src, k, cur.sometimes());
                    }
                }
            }
        }
        if cls.tier != Tier::Missing && is_lit && single_target && self.heap.obj(oid).single {
            *row = row.narrow_in(k, true);
            state.set_row_narrowed(oid, ns, row.clone());
            self.propagate_narrowing(oid, ns, k, state);
            // (the input's key read through a shallow copy: the original has it too, while it
            // holds the input's there)
            if cls.tier == Tier::Input {
                if let Some(r) = self.shallow_copy_root(oid).filter(|r| self.heap.obj(*r).single && !state.has_slot(*r, ns, k)) {
                    let rr = state.row(r, ns);
                    if rr.open() && !rr.absent.contains(k) {
                        state.set_row_narrowed(r, ns, rr.narrow_in(k, true));
                    }
                }
            }
        }
    }

    /// `seq[i] = v`: the position (from either end, of each length it may have; any known one
    /// when i is not known); `seq[a:b] = items`: positions and length not known any more.
    fn store_in_sequence(&mut self, oid: ObjId, key: &Value, v: &Value, strong: bool, node: &Node, state: &mut State) {
        if matches!(&key.kv, KeyVal::Data(s) if intern::str_get(*s).starts_with("slice")) {
            // known bounds (step 1) on a known length, known items: the list spliced
            let it = self.iterate(v, node, state);
            if let (Some((lo, hi, None | Some(1))), Some(n), Some(items), true) = (Self::slice_label_bounds(&key.kv), self.heap.llen(oid, state), it.items.as_ref(), strong) {
                let norm = |x: i64| if x < 0 { (n as i64 + x).max(0) } else { x.min(n as i64) } as usize;
                let (a, b) = (norm(lo.unwrap_or(0)), norm(hi.unwrap_or(n as i64)));
                let b = b.max(a);
                let mut vals: Vec<Value> = (0..a).map(|i| state.slot(oid, Ns::Pos, intern::key_int(i as i64))).collect();
                vals.extend(items.iter().cloned());
                vals.extend((b..n).map(|i| state.slot(oid, Ns::Pos, intern::key_int(i as i64))));
                if vals.len() < 16 {
                    return self.set_positions(oid, vals, state);
                }
            }
            let e = self.elem_value(&it.elem, node, "el");
            self.add_elem(oid, &e, state);
            state.drop_obj_ns(oid, Ns::Pos);
            self.heap.set_llen(oid, state, None);
            return;
        }
        let idx = key.kv.as_lit().filter(|_| key.pts.is_empty()).and_then(intern::key_as_int);
        let at: Vec<i64> = match (idx, self.heap.len_alts(oid, state)) {
            (Some(i), Some(alts)) => alts.iter().map(|n| if i < 0 { *n as i64 + i } else { i }).zip(alts.iter()).filter(|(p, n)| 0 <= *p && *p < **n as i64).map(|(p, _)| p).collect(),
            _ => state.slots_of(oid).into_iter().filter(|((ns, _), _)| *ns == Ns::Pos).filter_map(|((_, k), _)| intern::key_as_int(k)).collect(),
        };
        let one = idx.is_some() && self.heap.llen(oid, state).is_some();
        for p in at {
            state.set_slot(oid, Ns::Pos, intern::key_int(p), v.clone(), strong && one);
        }
        // (every position known: the elements are exactly what they hold, the value
        // overwritten gone)
        match self.known_positions(oid, state).filter(|_| strong && one) {
            Some(ps) => self.set_positions(oid, ps, state),
            None => self.add_elem(oid, v, state),
        }
    }

    /// The object a shallow copy (`copy.copy`, `dict.copy`) was made of.
    pub(in crate::interp) fn shallow_copy_root(&self, oid: ObjId) -> Option<ObjId> {
        self.heap.copy_root.get(&oid).copied().filter(|_| self.heap.obj(oid).site.starts_with("copy:"))
    }

    /// The value under key `k`: the slot's, a map's stored values, or the open record's
    /// placeholder for what the input holds there.
    #[allow(clippy::too_many_arguments)]
    fn literal_key_value(&mut self, oid: ObjId, ns: Ns, k: KeyId, cls: &Classification, row: &Row, row0: &Row, state: &mut State, is_lit: bool) -> Value {
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
        // the input's, or what unknown code put there, which a slot one path made does not cover)
        let partly_stored = !v.empty() && (row0.has_input() || !row0.mutators.is_empty()) && !(row0.definite.contains(k) && row0.stored.contains(k));
        if !from_map && (partly_stored || (v.pts.is_empty() && matches!(v.kv, KeyVal::Data(_) | KeyVal::Bot) && (row.open() || cls.tier == Tier::Map))) {
            let p = self.path_placeholder_value(oid, ns, k, row, state);
            v = v.join(&p);
            if is_lit {
                state.set_slot_found(oid, ns, k, v.clone(), false);
            }
        }
        v
    }

    /// A read by a key that is one of another object's keys: possibly missing where that
    /// object's keys are not all this one's; any of the values.
    #[allow(clippy::too_many_arguments)]
    fn keys_of_key_read(&mut self, oid: ObjId, ns: Ns, ko: &crate::domain::KeysOf, row: &Row, node: &Node, state: &mut State, guarded: bool) -> Value {
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
        self.all_values(oid, ns, state).join(&Value::kv_only(DATA))
    }

    /// A read on `obj` at `node` starts (`Modes::wide_read`); the previous setting, to restore.
    /// (A value of many objects — a loop over every module of a model taking whatever
    /// `getattr(m, name)` gives — made a placeholder per object and key, and the next round
    /// read those too: objects grew with the keys to the power of the path depth.)
    pub(in crate::interp) fn wide_read(&mut self, obj: &Value, node: &Node) -> Option<(usize, String)> {
        let wide = (obj.pts.len() > WIDE_READ).then(|| (self.cx.frames.len(), self.site_of(node)));
        std::mem::replace(&mut self.mode.wide_read, wide)
    }

    /// A read started by `wide_read` ends: its placeholders' rows are the join of the rows of
    /// what they stand for; `saved` is the setting before it.
    pub(in crate::interp) fn end_wide_read(&mut self, saved: Option<(usize, String)>, state: &mut State) {
        self.mode.wide_read = saved;
        let mut pending = std::mem::take(&mut self.mode.wide_rows);
        pending.sort_by_key(|(p, _)| *p);
        for group in pending.chunk_by(|a, b| a.0 == b.0) {
            let pid = group[0].0;
            let old = state.has_row(pid, Ns::Item).then(|| state.row(pid, Ns::Item));
            let r = crate::domain::join_rows(old.into_iter().chain(group.iter().map(|(_, r)| r.clone())));
            state.set_row_narrowed(pid, Ns::Item, r.clone());
            state.set_row_narrowed(pid, Ns::Attr, r);
        }
    }

    /// Is a read on many objects running in this frame (`wide_read`)?
    pub(in crate::interp) fn in_wide_read(&self) -> bool {
        self.mode.wide_read.as_ref().is_some_and(|(d, _)| *d == self.cx.frames.len())
    }

    /// `_path_placeholder`: the (unknown) content of an open object's slot.
    pub fn path_placeholder_value(&mut self, oid: u32, ns: Ns, k: KeyId, row: &Row, state: &mut State) -> Value {
        if self.heap.obj(oid).depth >= PATH_DEPTH {
            return Value::data("deep");
        }
        if let Some(site) = self.mode.wide_read.as_ref().filter(|_| self.in_wide_read()).map(|(_, s)| s.clone()) {
            // (one placeholder of the read for every object it reads: weak, its row the join)
            let ctx = self.frame().ctx.clone();
            let pid = self.heap.alloc_raw(Kind::Unknown, &format!("path@{}:{}:{}", site, ns, intern::key_repr(k)), &ctx, false, &format!("{}[{}]", self.heap.obj(oid).label, intern::key_repr(k)), Payload::None);
            crate::state::mark_merged(pid);
            self.fill_placeholder(pid, oid, ns, k, row, false, state);
            return Value::obj(pid);
        }
        let label = format!("{}[{}]", self.heap.obj(oid).label, intern::key_repr(k));
        // the content under an unknown key stands for many values: stores on it are weak
        let single = k != STAR;
        // (a loop's item holds there what the records it is one of hold: their placeholder)
        let base = crate::state::view_of(oid).unwrap_or(oid);
        // (a shallow copy holds the original's values: under a key neither stored since, the
        // same part of the input — which the original holds there too while it has no other)
        let root = self.shallow_copy_root(base);
        let base = root.unwrap_or(base);
        let pid = self.heap.alloc_raw(Kind::Unknown, &format!("path:{}:{}:{}", base, ns, intern::key_repr(k)), "", single, &label, Payload::None);
        self.fill_placeholder(pid, oid, ns, k, row, false, state);
        if let Some(r) = root.filter(|r| k != STAR && !state.has_slot(*r, ns, k)) {
            let rr = state.row(r, ns);
            if rr.open() && !rr.absent.contains(k) {
                state.set_slot_found(r, ns, k, Value::obj(pid), false);
            }
        }
        Value::obj(pid)
    }

    /// The placeholder `pid` for what `oid` holds under `k` (`elem`: the element a loop takes,
    /// also of an input's own data): named after its parent, a level deeper, its rows open
    /// through the parent's inputs at `k`.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::interp) fn fill_placeholder(&mut self, pid: ObjId, oid: ObjId, ns: Ns, k: KeyId, row: &Row, elem: bool, state: &mut State) {
        if let Some(sym) = self.names.access(&self.heap, oid, ns, k).filter(|_| !self.names.has(pid)) {
            let scope = self.names.scope(oid);
            self.names.set(pid, sym, scope);
        }
        let d = self.heap.obj(oid).depth + 1;
        // (a part of a value annotated `list[int]`, `dict[str, float]`: of that type)
        let part = self.heap.ann.get(&oid).and_then(|a| crate::interp::entry::part_annotation(a, k));
        let shared = self.heap.obj(pid).site.starts_with("path@");
        if let (Some(p), false) = (part, shared) {
            if let Some((t, false)) = crate::interp::entry::builtin_annotation(p) {
                self.heap.obj_mut(pid).ty = Some(t);
                if let Some(c) = crate::interp::entry::container_annotation(p) {
                    self.heap.ann.insert(pid, c);
                }
            }
        }
        let o = self.heap.obj_mut(pid);
        o.depth = if shared { o.depth.max(d) } else { d };
        o.iter_elem |= elem;
        if state.has_row(pid, Ns::Item) && !shared {
            return;
        }
        // (an input's parts below PATH_DEPTH + 1 keys are nothing more of it: data copied out of
        // a placeholder and read again, as a recursion over unknown values does, made paths
        // without end — `args['*'][0][0][0]…` — in contracts no one can read)
        let srcs = row.sources.filter(|(src, _)| (elem || !crate::domain::source_is_data(src)) && crate::domain::source_depth(src) <= PATH_DEPTH as usize).map(|(src, _)| (crate::domain::source_extend(src, k), intern::EMPTY_LENS));
        // (the parent's value was read on this path: its requirements are known here, none so
        // far — a path without the entry never read it)
        for (cs, _) in srcs.iter() {
            state.reqs.mark(&cs, crate::domain::ReqInfo::default());
        }
        let r = if srcs.is_empty() && row.mutators.is_empty() {
            Row::top()
        } else {
            Row::from_data(crate::domain::RowData { sources: srcs, mutators: row.mutators.clone(), top: elem && row.top, ..Default::default() })
        };
        // (a placeholder of several objects' parts: what each holds there, joined when the
        // read ends)
        if shared {
            self.mode.wide_rows.push((pid, r));
            return;
        }
        state.set_row_narrowed(pid, Ns::Item, r.clone());
        state.set_row_narrowed(pid, Ns::Attr, r);
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
        if ns == Ns::Item {
            let split = self.per_receiver(obj, &|an, o| an.has_user_method(o, "__setitem__"), state, &mut |an, o, st| {
                an.store_item(o, ns, key, v, node, st);
                Value::new()
            });
            if split.is_some() {
                return;
            }
            if obj.pts.iter().any(|o| self.hashes_keys(o)) {
                self.check_hashable(key, state);
            }
        }
        let okey = if ns == Ns::Item { self.opaque_key(key, state) } else { key.clone() };
        let key = &okey;
        let targets: Vec<u32> = obj.pts.iter().filter(|o| !matches!(self.heap.kind(*o), Kind::Series | Kind::External | Kind::Function | Kind::Bound | Kind::Class | Kind::Method)).collect();
        for &oid in &targets {
            let kind = self.heap.kind(oid);
            let strong = targets.len() == 1 && self.heap.obj(oid).single;
            match kind {
                Kind::Locidx => {
                    crate::models::pandas::loc_set(self, oid, key, v, node, state);
                    continue;
                }
                Kind::List | Kind::Tuple | Kind::Set => {
                    self.store_in_sequence(oid, key, v, strong, node, state);
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
                            let s1 = strong && insts.len() == 1 && self.heap.obj(*i).single;
                            self.store_key(*i, Ns::Attr, &key.kv, v, s1, node, state);
                        }
                    }
                    continue;
                }
                Kind::Dict if self.m.chainmaps.contains_key(&oid) => {
                    // a ChainMap: the write lands in its first map too
                    let first = self.m.chainmaps[&oid].first().cloned().unwrap_or_default();
                    for m in self.record_objects(&first) {
                        let s1 = strong && first.pts.len() == 1 && self.heap.obj(m).single;
                        self.store_key(m, Ns::Item, &key.kv, v, s1, node, state);
                    }
                    if self.m.chainmaps[&oid].len() > 1 {
                        self.refresh_chainmap(oid, state);
                    } else {
                        let s1 = strong && self.heap.obj(oid).single;
                        self.store_key(oid, Ns::Item, &key.kv, v, s1, node, state);
                    }
                }
                Kind::Dict if self.m.attr_mirrors.contains_key(&oid) => {
                    // a `vars(obj)` dict: obj's attributes change with it
                    let inst = self.m.attr_mirrors[&oid];
                    let s1 = strong && self.heap.obj(inst).single;
                    self.store_key(inst, Ns::Attr, &key.kv, v, s1, node, state);
                }
                Kind::Instance if ns == Ns::Item => {
                    if let Some(cls) = self.heap.instance_class(oid) {
                        if let Some(m) = self.class_lookup(cls, "__setitem__") {
                            let fv = self.heap.cls_slots.get(&(m, Ns::Attr, key_str_lit("__setitem__"))).cloned().unwrap_or_default();
                            for f in fv.pts.iter() {
                                if self.heap.kind(f) == Kind::Function {
                                    self.call_user(f, Some(Value::obj(oid)), vec![key.clone(), v.clone()], IndexMap::new(), vec![], vec![], state, node, false);
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
                state.store_key_row(oid, ns, *k, strong);
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
                let exhaustive = strong && ko.exhaustive_depth == self.cx.cond_depth;
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
    pub(in crate::interp) fn refresh_chainmap(&mut self, oid: u32, state: &mut State) {
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

    /// `del obj[key]` (and `obj.__delitem__(key)`): on a list, later positions move down.
    pub fn delete_subscript(&mut self, obj: &Value, key: &Value, node: &Node, state: &mut State) {
        for o in obj.pts.iter() {
            if self.heap.kind(o) != Kind::List {
                continue;
            }
            let idx = key.kv.as_lit().filter(|_| key.pts.is_empty()).and_then(intern::key_as_int);
            match (self.known_positions(o, state), idx) {
                (Some(mut ps), Some(i)) if obj.pts.len() == 1 => {
                    let len = ps.len() as i64;
                    let at = if i < 0 { len + i } else { i };
                    if 0 <= at && at < len {
                        ps.remove(at as usize);
                        self.set_positions(o, ps, state);
                    }
                }
                _ => {
                    self.heap.set_llen(o, state, None);
                    state.drop_obj_ns(o, Ns::Pos);
                }
            }
        }
        self.delete_item(obj, Ns::Item, key, node, state, true);
    }

    pub fn delete_item(&mut self, obj: &Value, ns: Ns, key: &Value, node: &Node, state: &mut State, report: bool) {
        if ns == Ns::Item {
            let split = self.per_receiver(obj, &|an, o| an.has_user_method(o, "__delitem__"), state, &mut |an, o, st| {
                an.delete_item(o, ns, key, node, st, report);
                Value::new()
            });
            if split.is_some() {
                return;
            }
            if obj.pts.iter().any(|o| self.hashes_keys(o)) {
                self.check_hashable(key, state);
            }
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
                        let fv = self.heap.cls_slots.get(&(m, Ns::Attr, key_str_lit("__delitem__"))).cloned().unwrap_or_default();
                        for f in fv.pts.iter() {
                            if self.heap.kind(f) == Kind::Function {
                                self.call_user(f, Some(Value::obj(oid)), vec![key.clone()], IndexMap::new(), vec![], vec![], state, node, false);
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
                // (one of several objects, or one of several keys: the value may still be there)
                if strong && is_lit {
                    state.pop_slot(oid, ns, k);
                }
            }
            // (an instance attribute's `del` raises AttributeError the same way, a class attribute
            // of the name notwithstanding, unless the class takes it: `__delattr__`, a descriptor)
            let cls = self.heap.instance_class(oid).filter(|_| ns == Ns::Attr && kind == Kind::Instance);
            let attr_raises = cls.is_some_and(|c| self.class_lookup(c, "__delattr__").is_none() && self.slots_of(c).is_none());
            let raise = raise.map(|(surely, k)| (surely && !cls.is_some_and(|c| self.class_lookup(c, &intern::key_py_str(k)).is_some_and(|d| !self.plain_class_attr(d, k, state))), k));
            if let Some((surely, k)) = raise.filter(|_| (ns == Ns::Item && matches!(kind, Kind::Dict | Kind::Unknown)) || attr_raises) {
                state.set_row(oid, ns, row.clone());
                let exc = if ns == Ns::Item { "KeyError" } else { "AttributeError" };
                if !self.unwind.tries.is_empty() {
                    let e = crate::models::exceptions::exception_object(self, exc, vec![Value::lit(k)], node, state);
                    self.unwind.cur_exc = Some(e);
                }
                if surely {
                    self.raise_here(exc, state);
                } else {
                    self.may_raise_here(exc, state);
                }
                self.unwind.cur_exc = None;
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
}

/// A read on one of several objects (`outs`: the requirements after each one's part, from
/// `base`, and whether runs may take it): all are made, but one not made on every part runs
/// may take is needed only sometimes, unless it was before or is an element's.
type Reqs = crate::layered::TwoLevel<crate::domain::SourceId, KeyId, crate::domain::ReqInfo>;
fn needs_of_one(base: &Reqs, outs: &[(bool, Reqs)], state: &mut State) {
    let mut reqs = base.clone();
    let mut changed: Vec<(crate::domain::SourceId, KeyId)> = Vec::new();
    for (_, out) in outs {
        reqs.copy_changes(out, base);
        changed.extend(out.diff_keys(base));
    }
    for (src, k) in changed {
        // (an element's are per element: a list of two inputs' elements has each read)
        if intern::with_source(src, |s| s.path.last() == Some(&STAR)) {
            continue;
        }
        let all = outs.iter().filter(|(taken, _)| *taken).all(|(_, o)| o.get(&src, &k).is_some_and(|r| r.always));
        if all || base.get(&src, &k).is_some_and(|r| r.always) {
            continue;
        }
        if let Some(cur) = reqs.get(&src, &k).filter(|r| r.always) {
            reqs.set(&src, k, cur.sometimes());
        }
    }
    state.reqs = reqs;
}
