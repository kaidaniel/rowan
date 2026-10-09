//! Record access: items, attributes, stores, deletes, class lookup (port of the
//! "record access" / "attributes" sections of `rowan/interp.py`).

use indexmap::IndexMap;
use ruff_python_ast as ast;
use rustc_hash::FxHashMap;

use crate::domain::{classify, join_rows, join_values, lens_apply, Classification, KeyVal, Ns, ObjSet, Row, Source, SourceKind, Tier, Value, DATA};
use crate::frontend::{self, Node};
use crate::intern::{self, key_str_lit, s, KeyId, KeySet, STAR};
use crate::state::{Kind, State};

use super::{is_scalar_method, Analyzer, ClassInfo, Payload, ViewKind, ATTR_GUARDS, DICT_METHOD_NAMES, ITEM_GUARDS, LIST_METHOD_NAMES, PATH_DEPTH, STR_METHOD_NAMES};

pub fn param_slot_key(ph: u32) -> KeyId {
    intern::key_tuple(vec![key_str_lit("@"), intern::key_int(ph as i64)])
}

/// Slot key of the i-th `append` through a placeholder list (`-1`: an append or extend whose
/// position is unknown); applied to the actual list's positions at instantiation.
pub fn append_slot_key(i: i64) -> KeyId {
    intern::key_tuple(vec![key_str_lit("+"), intern::key_int(i)])
}

pub fn append_slot_of(k: KeyId) -> Option<i64> {
    intern::with_key(k, |kk| match kk {
        intern::Key::Tuple(items) if items.len() == 2 => {
            let a = intern::key_as_str(items[0])?;
            if &*a != "+" {
                return None;
            }
            intern::key_as_int(items[1])
        }
        _ => None,
    })
}

/// `("@", ph)` slot keys: the placeholder, if the key is one.
pub fn param_slot_of(k: KeyId) -> Option<u32> {
    intern::with_key(k, |kk| match kk {
        intern::Key::Tuple(items) if items.len() == 2 => {
            let a = intern::key_as_str(items[0])?;
            if &*a != "@" {
                return None;
            }
            intern::key_as_int(items[1]).map(|i| i as u32)
        }
        _ => None,
    })
}

impl Analyzer {
    pub fn is_guarded(&self, ns: Ns) -> bool {
        let want: &[&str] = if ns == Ns::Item { ITEM_GUARDS } else { ATTR_GUARDS };
        self.guards.iter().any(|g| g.iter().any(|x| want.contains(&x.as_str())))
    }

    pub fn record_objects(&self, v: &Value) -> Vec<u32> {
        v.pts.iter().filter(|o| self.is_record_like(*o)).collect()
    }

    pub fn require_providers(&mut self, cls: &Classification, row: &Row, site: &str, with_alts: bool, state: &mut State) {
        let site_id = s(site);
        let srcs: Vec<crate::domain::SourceId> = cls.providers.iter().map(|(s, _)| *s).collect();
        for (src, terms) in &cls.providers {
            for t in terms.iter() {
                if with_alts {
                    let alts = intern::IdSet::from_iter(srcs.iter().copied().filter(|x| x != src));
                    state.require(*src, t, site_id, alts, row.mutators.clone(), terms.clone());
                } else {
                    state.require(*src, t, site_id, intern::IdSet::new(), row.mutators.clone(), intern::IdSet::new());
                }
            }
        }
    }

    /// `True`, `1` and `1.0` are one dict key: a numeric literal key equal to a key the record
    /// has (or may have) is that key.
    pub fn canon_key(&self, oid: u32, ns: Ns, k: KeyId, state: &State) -> KeyId {
        let Some(x) = numeric_of(k) else { return k };
        let row = state.row(oid, ns);
        if row.present.contains(k) {
            return k;
        }
        for p in row.present.iter() {
            if numeric_of(p) == Some(x) {
                return p;
            }
        }
        k
    }

    pub fn canon_kv(&self, oid: u32, ns: Ns, kv: &KeyVal, state: &State) -> KeyVal {
        if crate::interp::disabled("numkey") {
            return kv.clone();
        }
        match kv {
            KeyVal::Lit(k) => KeyVal::Lit(self.canon_key(oid, ns, *k, state)),
            KeyVal::Lits(ks) => KeyVal::Lits(ks.map(|k| self.canon_key(oid, ns, k, state))),
            other => other.clone(),
        }
    }

    pub fn load_item(&mut self, obj: &Value, ns: Ns, key: &Value, node: &Node, state: &mut State, report: bool) -> Value {
        let mut acc = Value::new();
        let guarded = self.is_guarded(ns) || !report;
        // a real try/except guard (as opposed to a model's unreported load): no exception
        // means the key was present from here on
        let try_guard = report && self.is_guarded(ns);
        let single_target = obj.pts.len() == 1;
        let line_data = Value::data(&format!("L{}", node.line));
        for oid in obj.pts.iter() {
            let kind = self.kind(oid);
            // a Counter reads a missing key as 0 without inserting it: the row is put back
            // after the (never failing) access
            let mut counter_restore: Option<Row> = None;
            if kind == Kind::Ph && ns == Ns::Item {
                // a placeholder declared as a defaultdict: insertion is decided per call site
                if let Payload::Ph(p) = &self.obj(oid).payload {
                    if let Some(c) = p.cls.get() {
                        if self.class_info_opt(c).is_none() && self.obj(c).label.ends_with("defaultdict") && !super::disabled("ddnarrow") {
                            self.record_event("narrow", oid);
                        }
                    }
                }
            }
            if kind == Kind::Dict && ns == Ns::Item && (self.obj(oid).label.starts_with("defaultdict@") || self.obj(oid).label.starts_with("Counter@")) {
                // `dd[k]` inserts the factory's value when `k` is absent (a fresh one per key)
                let row = state.row(oid, ns);
                if self.obj(oid).label.starts_with("Counter@") && !super::disabled("counterread") {
                    counter_restore = Some(row.clone());
                }
                if key.kv.is_literal() {
                    let ks = crate::domain::kv_literals(&key.kv);
                    let strong = single_target && ks.len() == 1 && self.strongable(oid);
                    let mut r = row.clone();
                    for k in ks.iter() {
                        if !r.definite.contains(k) {
                            let dflt = match self.dd_factories.get(&oid).cloned() {
                                Some(f) => self.call_value(&f, node, None, state, Some(vec![])),
                                None => state.slot(oid, Ns::Item, STAR),
                            };
                            r = r.store(k, strong);
                            state.set_slot(oid, ns, k, dflt, false);
                        }
                    }
                    state.set_row(oid, ns, r);
                } else if key.kv.as_param().is_none() {
                    // an unknown key: some key gets the factory's value
                    if let Some(f) = self.dd_factories.get(&oid).cloned() {
                        let dflt = self.call_value(&f, node, None, state, Some(vec![]));
                        state.set_slot(oid, ns, STAR, Value::with_kv(dflt.pts, DATA), false);
                    }
                    let src = Source::new(&self.obj(oid).label, SourceKind::Data);
                    state.set_row(oid, ns, row.data_store(src));
                }
            }
            match kind {
                Kind::Locidx => {
                    let v = crate::models::pandas::loc_get(self, oid, key, node, state);
                    acc = acc.join(&v);
                    continue;
                }
                Kind::Groupby => {
                    let v = crate::models::pandas::groupby_select(self, oid, key, node, state);
                    acc = acc.join(&v);
                    continue;
                }
                Kind::Instance if ns == Ns::Item => {
                    let cls = self.instance_class(oid);
                    if let Some(cls) = cls {
                        if let Some(m) = self.class_lookup(cls, "__getitem__") {
                            let fv = self.cls_slots.get(&(m, Ns::Attr, key_str_lit("__getitem__"))).cloned().unwrap_or_default();
                            for f in fv.pts.iter() {
                                if self.kind(f) == Kind::Function {
                                    let r = self.call_user(f, Some(Value::obj(oid)), vec![key.clone()], IndexMap::new(), vec![], vec![], state, node, None, false);
                                    acc = acc.join(&r);
                                }
                            }
                            continue;
                        }
                        // `__missing__` answers the keys the dict does not have
                        if self.class_lookup(cls, "__missing__").is_some() && !super::disabled("dunmissing") {
                            if let Some(k) = key.kv.as_lit().filter(|_| key.pts.is_empty()) {
                                let row = state.row(oid, Ns::Item);
                                if !row.definite.contains(k) {
                                    let r = self.call_method_with(&Value::obj(oid), "__missing__", vec![key.clone()], node, state).unwrap_or_default();
                                    acc = acc.join(&r);
                                    if row.present.contains(k) {
                                        acc = acc.join(&state.slot(oid, Ns::Item, k));
                                    }
                                    continue;
                                }
                            }
                        }
                        if let Some(ci) = self.class_info_opt(cls) {
                            if !ci.typed_dict && !ci.named_tuple && !ci.open_base {
                                acc = acc.join(&line_data);
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
                    match (idx, self.obj(oid).payload.clone()) {
                        (Some(0 | -2), Payload::Pair(k, _)) => acc = acc.join(&k),
                        (Some(1 | -1), Payload::Pair(_, v)) => acc = acc.join(&v),
                        _ => acc = acc.join(&line_data),
                    }
                }
                Kind::View => {
                    if let Payload::View(ViewKind::Attrs, inst) = self.obj(oid).payload.clone() {
                        // `obj.__dict__[k]`: an attribute load
                        let v = self.load_item(&inst, Ns::Attr, key, node, state, report);
                        acc = acc.join(&v);
                    } else {
                        acc = acc.join(&line_data);
                    }
                }
                Kind::Zip | Kind::Enumerate => {
                    acc = acc.join(&line_data);
                }
                Kind::List | Kind::Tuple | Kind::Set => {
                    let is_slice = matches!(&key.kv, KeyVal::Data(s) if intern::str_get(*s).starts_with("slice"));
                    if is_slice && kind != Kind::Set {
                        // a slice is a container of the same elements (positions unknown)
                        let elem = state.slot(oid, Ns::Elem, STAR);
                        let kv = state.elems.get(&oid).unwrap_or(DATA);
                        let nid = self.alloc_at(kind, node, state, &format!("{}@L{}", kind.name(), node.line), Payload::None, None, Ns::Item, None, None);
                        self.obj_mut(nid).literal_len = None;
                        state.set_slot(nid, Ns::Elem, STAR, elem, true);
                        state.elems.insert(nid, kv);
                        acc = acc.join(&Value::obj(nid));
                        continue;
                    }
                    let ll = self.obj(oid).literal_len;
                    let idx = key.kv.as_lit().and_then(|k| intern::with_key(k, |kk| if let intern::Key::Int(i) = kk { Some(*i) } else { None }));
                    // positions below a known prefix stay valid after appends of unknown count
                    if let (Some(i), None, Some(p)) = (idx, ll, self.obj(oid).known_prefix) {
                        if 0 <= i && (i as usize) < p {
                            acc = acc.join(&state.slot(oid, Ns::Pos, intern::key_int(i)));
                            continue;
                        }
                    }
                    match (idx, ll) {
                        (Some(i), Some(n)) => {
                            let n = n as i64;
                            if 0 <= i && i < n {
                                acc = acc.join(&state.slot(oid, Ns::Pos, intern::key_int(i)));
                            } else if -n <= i && i < 0 {
                                acc = acc.join(&state.slot(oid, Ns::Pos, intern::key_int(n + i)));
                            } else {
                                acc = acc.join(&state.slot(oid, Ns::Elem, STAR));
                            }
                        }
                        _ => {
                            let e = state.slot(oid, Ns::Elem, STAR);
                            acc = acc.join(&Value::with_kv(e.pts, DATA));
                        }
                    }
                }
                Kind::Series => acc = acc.join(&Value::with_kv(ObjSet::one(self.series), DATA)),
                Kind::Frame if ns == Ns::Item => {
                    let v = crate::models::pandas::frame_getitem(self, oid, key, node, state, guarded);
                    acc = acc.join(&v);
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
                    acc = acc.join(&v);
                }
                _ => acc = acc.join(&line_data),
            }
            if let Some(r) = counter_restore {
                state.set_row(oid, ns, r);
            }
        }
        if obj.pts.is_empty() {
            acc = line_data;
        }
        acc
    }

    #[allow(clippy::too_many_arguments)]
    fn load_record(&mut self, oid: u32, ns: Ns, key: &Value, node: &Node, state: &mut State, guarded: bool, single_target: bool, try_guard: bool) -> Value {
        let kind = self.kind(oid);
        let mut row = state.row(oid, ns);
        let kv = &key.kv;
        let site = self.access_site(node);
        if let Some(ph) = kv.as_param() {
            if !guarded {
                self.record_symreq(node, kv.clone(), &row, oid, ns, guarded, "load");
            } else if let Some(m) = self.may_raise.last_mut() {
                *m = true;
            }
            let mut v = self.all_values(oid, ns, state);
            // the key's value is unknown: the content may also come from the object's open
            // sources / entry content — not from a closed record the function built itself
            let content_unknown = !row.sources.is_empty() || !row.mutators.is_empty() || row.top || !row.terms.is_empty() || !row.dterms.is_empty() || !row.lterms.is_empty();
            if content_unknown {
                let p = self.path_placeholder_value(oid, ns, param_slot_key(ph), &row, state);
                v = v.join(&p);
            }
            // which entry is read is decided per call site, when the key is a literal there
            if (content_unknown || !row.present.is_empty()) && !super::disabled("paramnarrow") {
                self.record_event("narrow", ph);
            }
            let kv2 = match v.single() {
                Some(o) if self.is_ph(o) => KeyVal::Param(o),
                _ => DATA,
            };
            return Value::with_kv(v.pts, kv2);
        }
        if kv.is_literal() {
            let mut acc = Value::new();
            let ks = crate::domain::kv_literals(kv);
            let is_lit = matches!(kv, KeyVal::Lit(_));
            for k in intern::sorted_by_str(ks.iter()) {
                let cls = classify(&row, k);
                if cls.tier == Tier::Sym {
                    if guarded {
                        if let Some(m) = self.may_raise.last_mut() {
                            *m = true;
                        }
                        self.record_event("guarded", oid);
                    } else {
                        self.record_symreq(node, KeyVal::Lit(k), &row, oid, ns, guarded, "load");
                    }
                    // the element placeholder of a loop stands for every element: what one
                    // iteration's access proved is not known of the next one
                    let elem_ph = self.kind(oid) == Kind::Ph && self.ph_info(oid).key == Some(STAR) && self.frame().loop_depth > 0 && !super::disabled("elemnarrow");
                    if is_lit && single_target && self.strongable(oid) && !elem_ph {
                        row = row.narrow_in(k, true);
                        state.set_row(oid, ns, row.clone());
                    }
                } else if guarded && cls.tier != Tier::Proven {
                    if crate::interp::debug_level() == 2 {
                        eprintln!("  guarded access may raise: {:?} tier {:?} at L{}", intern::key_repr(k), cls.tier, node.line);
                    }
                    // the handler sees the state as it is here (and a KeyError / AttributeError
                    // carrying the key)
                    if !self.raise_states.is_empty() && !super::disabled("excvalue") {
                        let cname = if ns == Ns::Item { "KeyError" } else { "AttributeError" };
                        let e = self.exception_object(cname, vec![Value::lit(k)], node, state);
                        self.cur_exc = Some(e);
                    }
                    if !self.record_raise_state(state) {
                        if let Some(m) = self.may_raise.last_mut() {
                            *m = true;
                        }
                    }
                    if try_guard && cls.tier == Tier::Missing && is_lit && single_target && !row.symbolic() {
                        // the access raises here: the rest of the try body is unreachable
                        state.reachable = false;
                    }
                    if try_guard && cls.tier != Tier::Missing && is_lit && single_target && self.strongable(oid) {
                        // no exception raised: the key is present from here on
                        row = row.narrow_in(k, true);
                        state.set_row(oid, ns, row.clone());
                    }
                }
                if !guarded && cls.tier != Tier::Sym {
                    self.verdict(node, k, &cls, oid, ns, None, None);
                    if cls.tier == Tier::Input {
                        self.require_providers(&cls, &row, &site, true, state);
                    }
                    if cls.tier != Tier::Missing && is_lit && single_target && self.strongable(oid) {
                        row = row.narrow_in(k, true);
                        state.set_row(oid, ns, row.clone());
                        self.propagate_narrowing(oid, ns, k, state);
                    }
                }
                let mut v = state.slot(oid, ns, k);
                let star = state.slot(oid, ns, STAR);
                if !star.pts.is_empty() {
                    v = v.join(&Value::with_kv(star.pts, DATA));
                }
                if v.pts.is_empty() && matches!(v.kv, KeyVal::Data(_) | KeyVal::Bot) && (row.open() || cls.tier == Tier::Map) {
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
                    self.verdict(node, k, &Classification::with_note(Tier::Possibly, "key from another record's key set"), oid, ns, None, None);
                }
            }
            return self.all_values(oid, ns, state).join(&Value::kv_only(DATA));
        }
        // Data key
        if !guarded && row.symbolic() {
            self.record_symreq(node, kv.clone(), &row, oid, ns, guarded, "load");
        } else if !guarded && row.closed() && !row.is_map() && !row.present.is_empty() && matches!(kind, Kind::Dict | Kind::Frame) {
            let k = key_str_lit(&format!("<data {}>", kv));
            self.verdict(node, k, &Classification::with_note(Tier::DataKey, "data-derived key into a closed record"), oid, ns, None, None);
        }
        let mut v = self.all_values(oid, ns, state);
        if v.pts.is_empty() && row.open() {
            let p = self.path_placeholder_value(oid, ns, STAR, &row, state);
            v = v.join(&p);
        }
        Value::with_kv(v.pts, DATA)
    }

    /// `_path_placeholder`: the (unknown) content of an open object's slot.
    pub fn path_placeholder_value(&mut self, oid: u32, ns: Ns, k: KeyId, row: &Row, state: &mut State) -> Value {
        if self.obj(oid).depth >= super::limit("path", PATH_DEPTH as usize) as u32 {
            return Value::data("deep");
        }
        if !self.sum_stack.is_empty() && self.is_entry_object(oid) {
            let pid = self.path_placeholder(oid, ns, k, state, None, "");
            return Value::with_kv(ObjSet::one(pid), KeyVal::Param(pid));
        }
        if !self.sum_stack.is_empty() && row.sources.is_empty() && row.mutators.is_empty() && !row.top {
            if let Some(src) = self.obj(oid).copied_from {
                if self.is_entry_object(src) {
                    // a copy of an entry object, not updated from elsewhere since: the content
                    // comes from the original
                    let pid = self.path_placeholder(src, ns, k, state, None, "");
                    return Value::with_kv(ObjSet::one(pid), KeyVal::Param(pid));
                }
            }
        }
        let label = format!("{}[{}]", self.obj(oid).label, intern::key_repr(k));
        // the content under an unknown key stands for many values: stores on it are weak
        let single = !(k == STAR || param_slot_of(k).is_some());
        let pid = self.alloc_raw(Kind::Unknown, &format!("path:{}:{}:{}", oid, ns, intern::key_repr(k)), "", single, &label, Payload::None);
        if let Some(sym) = self.sym_access(oid, ns, k) {
            if !self.symbols.contains_key(&pid) {
                let scope = self.symbol_scope.get(&oid).copied().unwrap_or(0);
                self.set_symbol(pid, sym, scope);
            }
        }
        let d = self.obj(oid).depth + 1;
        self.obj_mut(pid).depth = d;
        if !state.has_row(pid, Ns::Item) {
            let srcs = row.sources.filter(|(src, _)| !crate::domain::source_is_data(src)).map(|(src, _)| (crate::domain::source_extend(src, k), intern::EMPTY_LENS));
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

    pub fn store_item(&mut self, obj: &Value, ns: Ns, key: &Value, v: &Value, node: &Node, state: &mut State) {
        let targets: Vec<u32> = obj.pts.iter().filter(|o| !matches!(self.kind(*o), Kind::Series | Kind::External | Kind::Function | Kind::Bound | Kind::Class | Kind::Method)).collect();
        for &oid in &targets {
            let kind = self.kind(oid);
            let strong = targets.len() == 1 && self.strongable(oid);
            match kind {
                Kind::Locidx => {
                    crate::models::pandas::loc_set(self, oid, key, v, node, state);
                    continue;
                }
                Kind::List | Kind::Tuple | Kind::Set => {
                    state.set_slot(oid, Ns::Elem, STAR, Value::with_kv(v.pts.clone(), DATA), false);
                    if let (Some(k), Some(n)) = (key.kv.as_lit(), self.obj(oid).literal_len) {
                        if let Some(i) = intern::with_key(k, |kk| if let intern::Key::Int(i) = kk { Some(*i) } else { None }) {
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
                    // `obj.__dict__[k] = v`: an attribute store
                    if let Payload::View(ViewKind::Attrs, inst) = self.obj(oid).payload.clone() {
                        let insts: Vec<u32> = inst.pts.iter().collect();
                        for i in &insts {
                            let s1 = strong && insts.len() == 1 && self.strongable(*i);
                            self.store_key(*i, Ns::Attr, &key.kv, v, s1, node, state);
                        }
                    }
                    continue;
                }
                Kind::Dict if self.attr_mirrors.contains_key(&oid) => {
                    // a `vars(obj)` dict: obj's attributes change with it
                    let inst = self.attr_mirrors[&oid];
                    let s1 = strong && self.strongable(inst);
                    self.store_key(inst, Ns::Attr, &key.kv, v, s1, node, state);
                }
                Kind::Instance if ns == Ns::Item => {
                    if let Some(cls) = self.instance_class(oid) {
                        if let Some(m) = self.class_lookup(cls, "__setitem__") {
                            let fv = self.cls_slots.get(&(m, Ns::Attr, key_str_lit("__setitem__"))).cloned().unwrap_or_default();
                            for f in fv.pts.iter() {
                                if self.kind(f) == Kind::Function {
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
        let kv = &self.canon_kv(oid, ns, kv, state);
        let row = state.row(oid, ns);
        match kv {
            KeyVal::Param(ph) => {
                state.set_row(oid, ns, row.param_store(*ph, strong));
                state.set_slot(oid, ns, param_slot_key(*ph), v.clone(), strong);
            }
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
                if exhaustive {
                    if let Some(log) = self.exh_log.as_mut() {
                        log.push((oid, ns, srcrow.clone(), ko.exclude.clone(), ko.chain));
                    }
                }
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
                state.set_slot(oid, ns, STAR, Value::with_kv(v.pts.clone(), DATA), false);
            }
            _ => {
                let site = format!("store@L{}", node.line);
                state.set_row(oid, ns, row.data_store(Source::new(&site, SourceKind::Data)));
                state.set_slot(oid, ns, STAR, Value::with_kv(v.pts.clone(), DATA), false);
            }
        }
    }

    /// oid[ns] ∪= keys of every record in src (dict spread / update / dict(x)).
    pub fn copy_into(&mut self, oid: u32, ns: Ns, src: &Value, state: &mut State, strong: bool, kill: &KeySet) {
        let recs = self.record_objects(src);
        if recs.is_empty() {
            if !src.pts.is_empty() {
                let n = state.rows.len();
                let r = state.row(oid, ns).data_store(Source::new(&format!("copy@{}", n), SourceKind::Data));
                state.set_row(oid, ns, r);
            }
            return;
        }
        let ns_of = |an: &Analyzer, x: u32| if an.kind(x) != Kind::Instance { Ns::Item } else { Ns::Attr };
        let srcrow = join_rows(recs.iter().map(|x| state.row(*x, ns_of(self, *x))));
        let r = state.row(oid, ns).copy_from(&srcrow, strong, kill, intern::EMPTY_LENS);
        state.set_row(oid, ns, r);
        for x in recs {
            let xns = ns_of(self, x);
            for (k, val) in state.obj_slots(x, xns) {
                if !kill.contains(k) {
                    state.set_slot(oid, ns, k, val, false);
                }
            }
        }
    }

    pub fn delete_item(&mut self, obj: &Value, ns: Ns, key: &Value, node: &Node, state: &mut State, report: bool) {
        let objs: Vec<u32> = obj.pts.iter().collect();
        self.kill_facts_on(&objs, state);
        state.drop_name_keys(None);
        let targets = self.record_objects(obj);
        for &oid in &targets {
            let kind = self.kind(oid);
            let strong = targets.len() == 1 && self.obj(oid).single;
            if kind == Kind::Frame && ns == Ns::Item {
                crate::models::pandas::frame_delitem(self, oid, key, node, state, strong, report);
                continue;
            }
            if kind == Kind::Instance && ns == Ns::Item {
                if let Some(cls) = self.instance_class(oid) {
                    if let Some(m) = self.class_lookup(cls, "__delitem__") {
                        let fv = self.cls_slots.get(&(m, Ns::Attr, key_str_lit("__delitem__"))).cloned().unwrap_or_default();
                        for f in fv.pts.iter() {
                            if self.kind(f) == Kind::Function {
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
            if let KeyVal::Param(ph) = &key.kv {
                // `del d[k]` with k a parameter: decided per call site when k is a literal there
                self.record_event("narrow", *ph);
            }
            for k in intern::sorted_by_str(crate::domain::kv_literals(&key.kv).iter()) {
                if report && !self.is_guarded(ns) {
                    let cls = classify(&row, k);
                    self.verdict(node, k, &cls, oid, ns, None, None);
                    if cls.tier == Tier::Input {
                        let site = self.access_site(node);
                        self.require_providers(&cls, &row, &site, false, state);
                    }
                }
                row = row.delete(k, strong && is_lit);
                state.pop_slot(oid, ns, k);
            }
            state.set_row(oid, ns, row);
        }
    }

    // ---------------------------------------------------------- attributes
    pub fn load_attr(&mut self, obj: &Value, name: &str, node: &Node, state: &mut State, report: bool) -> Value {
        let mut acc = Value::new();
        let guarded = self.is_guarded(Ns::Attr) || !report;
        let single = obj.pts.len() == 1;
        let nk = key_str_lit(name);
        for oid in obj.pts.iter() {
            let k = self.kind(oid);
            let v: Value = match k {
                Kind::Module => match state.slot_opt(oid, Ns::Attr, nk) {
                    Some(v) => v,
                    None => {
                        if !guarded {
                            self.verdict(node, nk, &Classification::with_note(Tier::Missing, "module has no such attribute"), oid, Ns::Attr, None, None);
                        }
                        Value::data(&format!("L{}", node.line))
                    }
                },
                Kind::External => {
                    let q = format!("{}.{}", self.external_qual(oid), name);
                    match crate::models::external_record(self, &q, state) {
                        Some(rec) => Value::obj(rec),
                        None => Value::obj(self.alloc_raw(Kind::External, &format!("ext:{}", q), "", true, &q, Payload::Str(q.clone()))),
                    }
                }
                Kind::Parser | Kind::Groupby | Kind::Partial => self.method_value(&format!("{}.{}", k.name(), name), &Value::obj(oid)),
                Kind::Instance if name == "__class__" => match self.instance_class(oid) {
                    Some(c) => Value::obj(c),
                    None => Value::data(&format!("L{}", node.line)),
                },
                Kind::Namespace | Kind::Instance | Kind::Ph if name == "__dict__" && !super::disabled("dunderdict") => {
                    let vid = self.alloc_raw(Kind::View, &format!("view:attrs:{}", oid), "", true, "__dict__", Payload::View(ViewKind::Attrs, Value::obj(oid)));
                    self.obj_mut(vid).payload = Payload::View(ViewKind::Attrs, Value::obj(oid));
                    Value::obj(vid)
                }
                Kind::Namespace | Kind::Instance => self.load_instance_attr(oid, name, node, state, guarded, single),
                Kind::Class if matches!(name, "__name__" | "__qualname__") && self.class_lookup(oid, name).is_none() => {
                    let n = self.class_info_opt(oid).map(|ci| ci.name.clone()).unwrap_or_default();
                    Value::lit(key_str_lit(&n))
                }
                Kind::Class => {
                    if self.class_lookup(oid, name).is_some() {
                        // a classmethod reached through a class binds that class (not the
                        // class that defines it): `Sub.make()` constructs `Sub`
                        let v = self.class_attr_value(oid, name);
                        let mut out = Value::new();
                        let mut changed = false;
                        for f in v.pts.iter() {
                            if self.kind(f) == Kind::Function && self.func_info(f).is_classmethod.get() {
                                let label = format!("{} bound", self.obj(f).label);
                                out = out.join(&Value::obj(self.alloc_raw(Kind::Bound, &format!("bound:{}:cls{}", f, oid), "", true, &label, Payload::Bound(f, Value::obj(oid)))));
                                changed = true;
                            } else {
                                out = out.join(&Value::obj(f));
                            }
                        }
                        if changed { Value::with_kv(out.pts, v.kv.clone()) } else { v }
                    } else if name == "__init__" && self.dataclass_init_owner(oid).is_some() {
                        let dc = self.dataclass_init_owner(oid).unwrap();
                        Value::obj(self.alloc_raw(Kind::Method, &format!("dcinit:{}", oid), "", true, &format!("{}.__init__", self.obj(oid).label), Payload::Method(format!("dcinit.class.{}", dc), Value::obj(oid))))
                    } else {
                        let label = format!("{}.{}", self.obj(oid).label, name);
                        Value::obj(self.alloc_raw(Kind::Method, &format!("classattr:{}.{}", oid, name), "", true, &label, Payload::Method(format!("class.{}", name), Value::obj(oid))))
                    }
                }
                Kind::Dict => self.method_value(&format!("dict.{}", name), &Value::obj(oid)),
                Kind::List | Kind::Tuple | Kind::Set => self.method_value(&format!("{}.{}", k.name(), name), &Value::obj(oid)),
                Kind::Frame => crate::models::pandas::frame_getattr(self, oid, name, node, state, guarded),
                Kind::Series => {
                    let q = format!("Series.{}", name);
                    Value::obj(self.alloc_raw(Kind::External, &format!("ext:{}", q), "", true, &q, Payload::Str(q.clone())))
                }
                Kind::Unknown => self.load_unknown_attr(oid, name, node, state, guarded),
                Kind::Ph => self.load_ph_attr(oid, name, node, state, guarded, single),
                Kind::Function => state.slot_opt(oid, Ns::Attr, nk).unwrap_or_else(|| Value::data(&format!("L{}", node.line))),
                Kind::Bound => Value::data(&format!("L{}", node.line)),
                Kind::View => {
                    if let Payload::View(ViewKind::Keys, recv) = &self.obj(oid).payload {
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
                    let Payload::Super(cls, self_v) = self.obj(oid).payload.clone() else { continue };
                    let ci = self.class_info(cls);
                    let mut found: Option<u32> = None;
                    // the receiver's own class decides the order: the classes after `cls` in
                    // its MRO (one receiver class; otherwise the defining class's bases)
                    let recv_cls: Vec<u32> = self_v
                        .pts
                        .iter()
                        .filter_map(|o| match self.kind(o) {
                            Kind::Instance => self.instance_class(o),
                            Kind::Class => Some(o),
                            Kind::Ph => self.ph_info(o).cls.get(),
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
                    match found {
                        Some(fc) => {
                            let fv = self.cls_slots.get(&(fc, Ns::Attr, nk)).cloned().unwrap_or_default();
                            let mut out = Value::new();
                            for f in fv.pts.iter() {
                                if self.kind(f) == Kind::Function {
                                    let ids: Vec<String> = self_v.pts.iter().map(|x| x.to_string()).collect();
                                    let label = format!("{} via super", self.obj(f).label);
                                    let b = self.alloc_raw(Kind::Bound, &format!("bound:{}:{}", f, ids.join(",")), "", true, &label, Payload::Bound(f, self_v.clone()));
                                    out = out.join(&Value::obj(b));
                                }
                            }
                            out
                        }
                        None if name == "__init__" && ci.bases.iter().any(|b| self.dataclass_init_owner(*b).is_some()) => {
                            let dc = ci.bases.iter().find_map(|b| self.dataclass_init_owner(*b)).unwrap();
                            let ids: Vec<String> = self_v.pts.iter().map(|x| x.to_string()).collect();
                            Value::obj(self.alloc_raw(Kind::Method, &format!("dcinit:{}:{}", dc, ids.join(",")), "", true, "dataclass __init__ via super", Payload::Method(format!("dcinit.{}", dc), self_v.clone())))
                        }
                        None => Value::data("super"),
                    }
                }
                _ => Value::data(&format!("L{}", node.line)),
            };
            acc = acc.join(&v);
        }
        if obj.pts.is_empty() {
            if matches!(obj.kv, KeyVal::Lit(_) | KeyVal::Lits(_) | KeyVal::KeysOf(_)) {
                return self.method_value(&format!("str.{}", name), obj);
            }
            return self.method_value(&format!("unknown.{}", name), obj);
        }
        acc
    }

    pub fn method_value(&mut self, qual: &str, recv: &Value) -> Value {
        let ids: Vec<String> = recv.pts.iter().map(|x| x.to_string()).collect();
        let key = format!("method:{}:{}:{}", qual, ids.join(","), recv.kv);
        Value::obj(self.alloc_raw(Kind::Method, &key, "", true, qual, Payload::Method(qual.to_string(), recv.clone())))
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

    /// Return the class oid (in MRO) that defines `name`, or None.
    /// The C3 method resolution order of a user class (user classes only).
    pub fn mro(&self, cls: u32) -> Vec<u32> {
        fn c3(an: &Analyzer, cls: u32, depth: usize) -> Option<Vec<u32>> {
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
        if crate::interp::disabled("c3") {
            return self.mro_bfs(cls);
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
            if let Some(ci) = self.class_info_opt(c) {
                order.extend(ci.bases.iter().copied());
            }
        }
        None
    }

    /// The dataclass whose generated `__init__` `cls` inherits (none when a user `__init__`
    /// comes first in the MRO).
    pub fn dataclass_init_owner(&self, cls: u32) -> Option<u32> {
        for c in self.mro(cls) {
            if self.cls_slots.contains_key(&(c, Ns::Attr, key_str_lit("__init__"))) {
                return None;
            }
            if self.class_info_opt(c).map(|x| x.has_fields()).unwrap_or(false) {
                return Some(c);
            }
        }
        None
    }

    pub fn class_attr_value(&self, cls: u32, name: &str) -> Value {
        match self.class_lookup(cls, name) {
            None => Value::new(),
            Some(c) => self.cls_slots.get(&(c, Ns::Attr, key_str_lit(name))).cloned().unwrap_or_default(),
        }
    }

    fn bound_outs(&mut self, cv: &Value, oid: u32, cls: u32, name: &str, node: &Node, state: &mut State) -> Value {
        let mut outs: Vec<Value> = Vec::new();
        // a descriptor object in the class (its class has `__get__`): what `__get__` returns
        if !super::disabled("descriptors") {
            let descs: Vec<u32> = cv.pts.iter().filter(|d| self.kind(*d) == Kind::Instance && self.instance_class(*d).and_then(|dc| self.class_lookup(dc, "__get__")).is_some()).collect();
            if !descs.is_empty() && descs.len() == cv.pts.len() {
                for d in descs {
                    if let Some(r) = self.call_method_with(&Value::obj(d), "__get__", vec![Value::obj(oid), Value::obj(cls)], node, state) {
                        outs.push(r);
                    }
                }
                return join_values(outs);
            }
        }
        for f in cv.pts.iter() {
            if self.kind(f) == Kind::Function {
                let fi = self.func_info(f);
                if fi.is_property.get() {
                    let r = self.call_user(f, Some(Value::obj(oid)), vec![], IndexMap::new(), vec![], vec![], state, node, None, false);
                    if fi.is_cached.get() && matches!(self.kind(oid), Kind::Instance | Kind::Ph) {
                        // stored on the instance: later accesses find this value
                        let strong = self.strongable(oid);
                        self.store_key(oid, Ns::Attr, &KeyVal::Lit(key_str_lit(name)), &r, strong, node, state);
                    }
                    outs.push(r);
                } else if fi.is_static.get() {
                    outs.push(Value::obj(f));
                } else if fi.is_classmethod.get() {
                    let label = format!("{} bound", self.obj(f).label);
                    outs.push(Value::obj(self.alloc_raw(Kind::Bound, &format!("bound:{}:cls{}", f, cls), "", true, &label, Payload::Bound(f, Value::obj(cls)))));
                } else {
                    let label = format!("{} bound to {}", self.obj(f).label, self.obj(oid).label);
                    outs.push(Value::obj(self.alloc_raw(Kind::Bound, &format!("bound:{}:{}", f, oid), "", true, &label, Payload::Bound(f, Value::obj(oid)))));
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

    /// A class whose hierarchy includes a `NamedTuple` class.
    pub fn is_named_tuple_class(&self, cls: u32) -> bool {
        let mut work = vec![cls];
        let mut seen: Vec<u32> = Vec::new();
        while let Some(c) = work.pop() {
            if seen.contains(&c) {
                continue;
            }
            seen.push(c);
            let Some(ci) = self.class_info_opt(c) else { continue };
            if ci.named_tuple {
                return true;
            }
            work.extend(ci.bases.iter().copied());
        }
        false
    }

    pub fn load_instance_attr(&mut self, oid: u32, name: &str, node: &Node, state: &mut State, guarded: bool, single_target: bool) -> Value {
        let nk = key_str_lit(name);
        // pydantic's serialisers on a model: a dict of the fields
        if matches!(name, "model_dump" | "dict") && !super::disabled("pydantic") {
            if let Some(cls) = self.instance_class(oid) {
                if self.class_lookup(cls, name).is_none() && self.mro(cls).iter().any(|c| self.class_info_opt(*c).map(|x| x.pydantic).unwrap_or(false)) {
                    return self.method_value("pydantic.model_dump", &Value::obj(oid));
                }
            }
        }
        // the methods every NamedTuple has
        if matches!(name, "_asdict" | "_replace" | "_fields") && !super::disabled("ntmethods") {
            if let Some(cls) = self.instance_class(oid) {
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
        if let Some(cls) = self.instance_class(oid) {
            if !name.starts_with("__") && self.class_lookup(cls, name).is_none() && self.class_lookup(cls, "__getattr__").is_some() && !super::disabled("getattrdunder") {
                let row = state.row(oid, Ns::Attr);
                if !row.definite.contains(nk) {
                    let fallback = self.call_method_with(&Value::obj(oid), "__getattr__", vec![Value::lit(nk)], node, state).unwrap_or_default();
                    if !row.present.contains(nk) {
                        return fallback;
                    }
                    let own = state.slot(oid, Ns::Attr, nk);
                    return own.join(&fallback);
                }
            }
        }
        let v = state.slot_opt(oid, Ns::Attr, nk);
        let row = state.row(oid, Ns::Attr);
        let cls_of = self.instance_class(oid);
        if let Some(mut v) = v {
            if row.present.contains(nk) {
                let cls = classify(&row, nk);
                if !guarded {
                    self.verdict(node, nk, &cls, oid, Ns::Attr, None, None);
                }
                if let Some(c) = cls_of {
                    if self.field_is_any(c, name) {
                        let label = format!("{}.{}", self.obj(oid).label, name);
                        v = v.join(&self.dynamic_value(&label, state));
                    }
                }
                return v;
            }
        }
        if let Some(c) = cls_of {
            let is_field = self.dataclass_fields_mro(c).iter().any(|(f, _)| f == name);
            if let (Some(defc), false) = (self.class_lookup(c, name), is_field) {
                let cv = self.cls_slots.get(&(defc, Ns::Attr, nk)).cloned().unwrap_or_default();
                return self.bound_outs(&cv, oid, c, name, node, state);
            }
        }
        let cls = classify(&row, nk);
        if cls.tier == Tier::Sym {
            if !guarded {
                self.record_symreq(node, KeyVal::Lit(nk), &row, oid, Ns::Attr, guarded, "attr");
            }
            if single_target && self.strongable(oid) {
                state.set_row(oid, Ns::Attr, row.narrow_in(nk, true));
            }
            return self.path_placeholder_value(oid, Ns::Attr, nk, &row, state);
        }
        if !guarded {
            self.verdict(node, nk, &cls, oid, Ns::Attr, None, None);
            if cls.tier == Tier::Input {
                let site = self.access_site(node);
                self.require_providers(&cls, &row, &site, false, state);
            }
            if cls.tier != Tier::Missing && single_target && self.strongable(oid) {
                state.set_row(oid, Ns::Attr, row.narrow_in(nk, true));
                self.propagate_narrowing(oid, Ns::Attr, nk, state);
            }
        }
        if row.open() {
            return self.path_placeholder_value(oid, Ns::Attr, nk, &row, state);
        }
        Value::data(&format!("L{}", node.line))
    }

    /// Does any user subclass of `base` (other than `except`) define `name`?
    fn subclass_overrides(&self, base: u32, except: Option<u32>, name: &str) -> bool {
        let nk = key_str_lit(name);
        self.user_classes.iter().any(|&sc| Some(sc) != except && self.is_subclass(sc, base) && self.cls_slots.contains_key(&(sc, Ns::Attr, nk)))
    }

    pub fn load_ph_attr(&mut self, oid: u32, name: &str, node: &Node, state: &mut State, guarded: bool, single_target: bool) -> Value {
        let ph = self.ph_info(oid);
        let nk = key_str_lit(name);
        if let Some(v) = state.slot_opt(oid, Ns::Attr, nk) {
            return v;
        }
        // declared class (annotation / self): resolve methods and class attributes (a
        // dataclass field's default is instance data, not a class attribute)
        let is_field = !super::disabled("dcfield") && ph.cls.get().map(|c| self.dataclass_fields_mro(c).iter().any(|(f, _)| f == name)).unwrap_or(false);
        if let (Some(pcls), false) = (ph.cls.get(), is_field) {
            if let Some(c) = self.class_lookup(pcls, name) {
                let cv = self.cls_slots.get(&(c, Ns::Attr, nk)).cloned().unwrap_or_default();
                let out = self.bound_outs(&cv, oid, pcls, name, node, state);
                if !ph.exact.get() && self.subclass_overrides(pcls, Some(c), name) {
                    self.record_event("dispatch", oid);
                }
                return out;
            }
            if !ph.exact.get() && self.subclass_overrides(pcls, Some(pcls), name) {
                self.record_event("dispatch", oid);
            }
        }
        // a declared builtin class (`isinstance(x, dict)`, `self.xs = []`) selects a model
        // family; only a user class counts as "has a class" for method lookup
        let builtin_cls: Option<String> = if super::disabled("builtincls") { None } else { ph.cls.get().filter(|c| self.class_info_opt(*c).is_none()).map(|c| self.obj(c).label.rsplit('.').next().unwrap_or("").to_string()) };
        let has_cls = ph.cls.get().is_some() && builtin_cls.is_none();
        let dictish = builtin_cls.as_deref().map(|b| matches!(b, "dict" | "defaultdict" | "OrderedDict" | "Counter")).unwrap_or(true);
        let listish = builtin_cls.as_deref().map(|b| matches!(b, "list" | "set" | "frozenset" | "deque")).unwrap_or(true);
        // a builtin container model for the name, and / or the user classes defining it: a
        // class-less placeholder may be either, so both are kept and the call site decides
        let model: Option<Value> = if !has_cls && DICT_METHOD_NAMES.contains(&name) && dictish {
            Some(self.method_value(&format!("dict.{}", name), &Value::obj(oid)))
        } else if !has_cls && (LIST_METHOD_NAMES.contains(&name) || matches!(name, "add" | "discard")) && listish {
            Some(self.method_value(&format!("list.{}", name), &Value::obj(oid)))
        } else {
            None
        };
        if let Some(m) = &model {
            // a dict / list method name on a class-less placeholder: the model, with a dispatch
            // event so a call site whose receiver turns out to be a user instance tabulates
            // (user classes defining the same name are not run here: that spread `Config.get`
            // over every `row.get` in a multi-module program and made it recurse)
            if !super::disabled("usermodel") && self.user_classes.iter().any(|c| self.cls_slots.contains_key(&(*c, Ns::Attr, nk))) {
                self.record_event("dispatch", oid);
            }
            return m.clone();
        }
        if model.is_none() && is_scalar_method(name) && !has_cls {
            let q = if STR_METHOD_NAMES.contains(&name) { format!("str.{}", name) } else { format!("unknown.{}", name) };
            return self.method_value(&q, &Value::kv_only(KeyVal::Param(oid)));
        }
        // (a declared builtin container is that container: no user-class candidates)
        if !has_cls && builtin_cls.is_none() {
            let cands: Vec<u32> = self.user_classes.iter().copied().filter(|c| self.cls_slots.contains_key(&(*c, Ns::Attr, nk))).collect();
            if !cands.is_empty() {
                let mut outs: Vec<Value> = Vec::new();
                for c in &cands {
                    let fv = self.cls_slots.get(&(*c, Ns::Attr, nk)).cloned().unwrap_or_default();
                    for f in fv.pts.iter() {
                        if self.kind(f) == Kind::Function {
                            let label = format!("{} bound to {}", self.obj(f).label, self.obj(oid).label);
                            outs.push(Value::obj(self.alloc_raw(Kind::Bound, &format!("bound:{}:{}", f, oid), "", true, &label, Payload::Bound(f, Value::obj(oid)))));
                        }
                    }
                }
                if !outs.is_empty() {
                    // the receiver's class decides (also between the model and the user method)
                    if cands.len() > 1 || model.is_some() || !super::disabled("dispatch1") {
                        self.record_event("dispatch", oid);
                    }
                    if model.is_some() {
                        self.record_event("narrow", oid);
                    }
                    let mut v = join_values(outs);
                    if let Some(m) = model {
                        v = v.join(&m);
                    }
                    return v;
                }
            }
        }
        if let Some(m) = model {
            return m;
        }
        if is_scalar_method(name) && !has_cls {
            let q = if STR_METHOD_NAMES.contains(&name) { format!("str.{}", name) } else { format!("unknown.{}", name) };
            return self.method_value(&q, &Value::kv_only(KeyVal::Param(oid)));
        }
        let row = state.row(oid, Ns::Attr);
        if !guarded {
            self.record_symreq(node, KeyVal::Lit(nk), &row, oid, Ns::Attr, guarded, "attr");
        }
        if single_target {
            state.set_row(oid, Ns::Attr, row.narrow_in(nk, true));
        }
        let mut v = self.path_placeholder_value(oid, Ns::Attr, nk, &row, state);
        if let Some(pcls) = ph.cls.get() {
            if let Some(p) = v.single() {
                if self.is_ph(p) {
                    if let Some(fc) = self.field_class(pcls, name, state) {
                        self.ph_info(p).cls.set(Some(fc));
                    }
                }
            }
            if self.field_is_any(pcls, name) {
                let label = format!("{}.{}", self.obj(oid).label, name);
                v = v.join(&self.dynamic_value(&label, state));
            }
        }
        v
    }

    /// A value declared dynamically typed (`Any`): accesses on it are not checkable (§8.5).
    pub fn dynamic_value(&mut self, label: &str, state: &mut State) -> Value {
        let oid = self.alloc_raw(Kind::Unknown, &format!("any:{}", label), "", true, &format!("Any {}", label), Payload::None);
        if !state.has_row(oid, Ns::Item) {
            state.set_row(oid, Ns::Item, Row::top());
            state.set_row(oid, Ns::Attr, Row::top());
        }
        Value::obj(oid)
    }

    /// Is the attribute declared `Any` (or `object`) in the class body or its bases?
    pub fn field_is_any(&self, cls: u32, name: &str) -> bool {
        for c in self.mro(cls) {
            let Some(ci) = self.class_info_opt(c) else { continue };
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

    /// The one user class an attribute of instances of `cls` is known to hold.
    pub fn field_class(&mut self, cls: u32, name: &str, state: &State) -> Option<u32> {
        // the classes of constructor arguments stored into the field (one class: that one)
        for c in self.mro(cls) {
            if let Some(set) = self.ctor_field_classes.get(&(c, name.to_string())) {
                if set.len() == 1 && !set.contains(&u32::MAX) {
                    return set.iter().next().copied();
                }
                if !set.is_empty() {
                    return None;
                }
            }
        }
        for c in self.mro(cls) {
            let Some(ci) = self.class_info_opt(c) else { continue };
            if ci.field_classes.borrow().is_none() {
                let fc = self.infer_field_classes(&ci, state);
                *ci.field_classes.borrow_mut() = Some(fc);
            }
            let b = ci.field_classes.borrow();
            if let Some(found) = b.as_ref().unwrap().get(name) {
                if !found.is_empty() {
                    return if found.len() == 1 { Some(found[0]) } else { None };
                }
            }
        }
        None
    }

    fn infer_field_classes(&mut self, ci: &ClassInfo, state: &State) -> FxHashMap<String, Vec<u32>> {
        // (field, expression it is declared or assigned with) pairs from the class body and
        // its methods' `self.f = …` / `self.f: T = …` statements
        let mut pairs: Vec<(String, &'static ast::Expr)> = Vec::new();
        for st in &ci.node.body {
            match st {
                ast::Stmt::AnnAssign(a) => {
                    if let ast::Expr::Name(t) = &*a.target {
                        pairs.push((t.id.to_string(), &a.annotation));
                    }
                }
                ast::Stmt::FunctionDef(f) if !f.parameters.args.is_empty() => {
                    let me = f.parameters.args[0].parameter.name.id.as_str();
                    for sub in walk_stmts(&f.body) {
                        match sub {
                            ast::Stmt::Assign(asg) => {
                                for t in &asg.targets {
                                    if let ast::Expr::Attribute(ta) = t {
                                        if let ast::Expr::Name(bn) = &*ta.value {
                                            if bn.id.as_str() == me {
                                                pairs.push((ta.attr.id.to_string(), &asg.value));
                                            }
                                        }
                                    }
                                }
                            }
                            ast::Stmt::AnnAssign(aa) => {
                                if let ast::Expr::Attribute(ta) = &*aa.target {
                                    if let ast::Expr::Name(bn) = &*ta.value {
                                        if bn.id.as_str() == me {
                                            pairs.push((ta.attr.id.to_string(), &aa.annotation));
                                            if let Some(v) = aa.value.as_deref() {
                                                pairs.push((ta.attr.id.to_string(), v));
                                            }
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
        let mut out: FxHashMap<String, Vec<u32>> = FxHashMap::default();
        for (name, expr) in pairs {
            if let Some(c) = self.field_class_of_expr(expr, ci.module, state) {
                let e = out.entry(name).or_default();
                if !e.contains(&c) {
                    e.push(c);
                }
            }
        }
        out
    }

    /// The class an annotation / assigned expression declares for a field: a user class
    /// (`x: Cls`, `self.x = Cls(…)`) or a builtin container class (`self.x = {}`,
    /// `self.x = defaultdict(list)`, `x: dict`), which selects a model family.
    fn field_class_of_expr(&mut self, expr: &'static ast::Expr, module: u32, state: &State) -> Option<u32> {
        // literal containers first (they are not annotations)
        match expr {
            ast::Expr::List(_) | ast::Expr::ListComp(_) => return Some(self.builtin_class_obj("list")),
            ast::Expr::Dict(_) | ast::Expr::DictComp(_) => return Some(self.builtin_class_obj("dict")),
            ast::Expr::Set(_) | ast::Expr::SetComp(_) => return Some(self.builtin_class_obj("set")),
            _ => {}
        }
        let mut e = Analyzer::strip_annotation(expr)?;
        if let ast::Expr::Call(c) = e {
            e = &c.func;
        }
        let v = match e {
            ast::Expr::Name(n) => state.slot(module, Ns::Attr, key_str_lit(n.id.as_str())),
            ast::Expr::Attribute(a) => self.static_attr_value(a, module, state),
            _ => return None,
        };
        if let Some(c) = v.pts.iter().find(|o| self.kind(*o) == Kind::Class) {
            return Some(c);
        }
        if let Some(b) = v.pts.iter().find(|o| self.kind(*o) == Kind::External && BUILTIN_CONTAINER_CLASSES.contains(&self.external_qual(*o).rsplit('.').next().unwrap_or(""))) {
            return Some(b);
        }
        if let ast::Expr::Name(n) = e {
            if v.pts.is_empty() && BUILTIN_CONTAINER_CLASSES.contains(&n.id.as_str()) {
                return Some(self.builtin_class_obj(n.id.as_str()));
            }
        }
        None
    }

    /// The external object standing for a builtin class (`list`, `dict`, …).
    pub fn builtin_class_obj(&mut self, nm: &str) -> u32 {
        self.alloc_raw(Kind::External, &format!("ext:builtins.{}", nm), "", true, nm, Payload::Str(format!("builtins.{}", nm)))
    }

    fn static_attr_value(&self, e: &ast::ExprAttribute, module: u32, state: &State) -> Value {
        if let ast::Expr::Name(n) = &*e.value {
            let base = state.slot(module, Ns::Attr, key_str_lit(n.id.as_str()));
            let mut acc = Value::new();
            for o in base.pts.iter() {
                if self.kind(o) == Kind::Module {
                    acc = acc.join(&state.slot(o, Ns::Attr, key_str_lit(e.attr.id.as_str())));
                }
            }
            return acc;
        }
        Value::new()
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
        let cands: Vec<u32> = self.user_classes.iter().copied().filter(|c| self.cls_slots.contains_key(&(*c, Ns::Attr, nk))).collect();
        if !cands.is_empty() {
            let mut outs: Vec<Value> = Vec::new();
            for c in &cands {
                let fv = self.cls_slots.get(&(*c, Ns::Attr, nk)).cloned().unwrap_or_default();
                for f in fv.pts.iter() {
                    if self.kind(f) == Kind::Function {
                        let label = format!("{} bound to {}", self.obj(f).label, self.obj(oid).label);
                        outs.push(Value::obj(self.alloc_raw(Kind::Bound, &format!("bound:{}:{}", f, oid), "", true, &label, Payload::Bound(f, Value::obj(oid)))));
                    }
                }
            }
            if !outs.is_empty() {
                return join_values(outs);
            }
        }
        let row = state.row(oid, Ns::Attr);
        if let Some(v) = state.slot_opt(oid, Ns::Attr, nk) {
            return v;
        }
        let cls = classify(&row, nk);
        if !guarded && cls.tier != Tier::Map {
            self.verdict(node, nk, &cls, oid, Ns::Attr, None, None);
            if cls.tier == Tier::Input {
                let site = self.access_site(node);
                self.require_providers(&cls, &row, &site, false, state);
            }
        }
        let ph = self.path_placeholder_value(oid, Ns::Attr, nk, &row, state);
        if !ph.pts.is_empty() {
            return ph;
        }
        self.method_value(&format!("unknown.{}", name), &Value::obj(oid))
    }

    pub fn store_attr(&mut self, obj: &Value, name: &str, v: &Value, node: &Node, state: &mut State) {
        if !state.narrowed.is_empty() {
            let needle = format!(".{}", name);
            state.narrowed.retain(|k, _| !intern::str_get(*k).contains(&needle));
        }
        let nk = key_str_lit(name);
        let targets: Vec<u32> = obj.pts.iter().filter(|o| matches!(self.kind(*o), Kind::Instance | Kind::Module | Kind::Class | Kind::Unknown | Kind::Frame | Kind::Namespace | Kind::Ph | Kind::Function)).collect();
        for &oid in &targets {
            let kind = self.kind(oid);
            let strong = targets.len() == 1 && self.obj(oid).single;
            if kind == Kind::Frame {
                crate::models::pandas::frame_setattr(self, oid, name, v, node, state, strong);
                continue;
            }
            // a property with a setter: the setter runs instead of the store
            let cls_of = match kind {
                Kind::Instance => self.instance_class(oid),
                Kind::Ph => match &self.obj(oid).payload {
                    Payload::Ph(p) => p.cls.get(),
                    _ => None,
                },
                _ => None,
            };
            if let Some(cls) = cls_of {
                if let Some(defc) = self.class_lookup(cls, name) {
                    if let Some(&setter) = self.prop_setters.get(&(defc, nk)) {
                        self.call_user(setter, Some(Value::obj(oid)), vec![v.clone()], IndexMap::new(), vec![], vec![], state, node, None, false);
                        continue;
                    }
                    // a data descriptor (an object whose class has `__set__`) takes the store
                    if !super::disabled("descriptors") {
                        let cv = self.cls_slots.get(&(defc, Ns::Attr, nk)).cloned().unwrap_or_default();
                        let descs: Vec<u32> = cv.pts.iter().filter(|d| self.kind(*d) == Kind::Instance && self.instance_class(*d).and_then(|dc| self.class_lookup(dc, "__set__")).is_some()).collect();
                        if !descs.is_empty() {
                            for d in descs {
                                self.call_method_with(&Value::obj(d), "__set__", vec![Value::obj(oid), v.clone()], node, state);
                            }
                            continue;
                        }
                    }
                }
                // a user `__setattr__` takes every assignment (`object.__setattr__` bypasses it)
                if !self.bypass_setattr && !super::disabled("setattrdunder") && self.class_lookup(cls, "__setattr__").is_some() {
                    self.call_method_with(&Value::obj(oid), "__setattr__", vec![Value::lit(nk), v.clone()], node, state);
                    continue;
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
pub const BUILTIN_CONTAINER_CLASSES: &[&str] = &["dict", "defaultdict", "OrderedDict", "Counter", "list", "set", "frozenset", "deque", "tuple"];

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
