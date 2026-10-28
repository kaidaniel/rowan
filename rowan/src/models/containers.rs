//! Methods of the containers: `dict.*` (`get`, `setdefault`, `update`, `pop`, views, …),
//! `list.*` and `set.*` (`append`, `extend`, `pop`, `insert`, slicing, set operations), and
//! ChainMap.

use super::*;

pub(super) fn dict_recv(an: &Analyzer, c: &Call) -> Vec<u32> {
    match &c.recv {
        Some(r) => r.pts.iter().filter(|o| an.heap.kind(*o).is_record()).collect(),
        None => Vec::new(),
    }
}

pub(super) fn m_get(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(key) = c.args.first() else { return Value::data("get") };
    an.check_hashable(key, st);
    // (a key that may be an object: an unknown key)
    let okey = an.opaque_key(key, st);
    let key = &okey;
    let default = c.args.get(1).cloned().unwrap_or_else(|| Value::none());
    let recv = c.recv.clone().unwrap_or_default();
    if let Some(k) = key.kv.as_lit() {
        let recs = an.record_objects(&recv);
        if !recs.is_empty() && recs.len() == recv.pts.len() && recs.iter().all(|o| an.heap.kind(*o) != Kind::Frame) {
            // absent from every closed target: the default alone, no load
            if recs.iter().all(|o| { let r = st.row(*o, Ns::Item); r.lacks(k) }) {
                return default;
            }
        }
    }
    let out = an.load_item(&recv, Ns::Item, key, &c.node, st, false);
    // the default is only possible when the key may be absent on some target
    if let Some(k) = key.kv.as_lit() {
        let recs = an.record_objects(&recv);
        if !recs.is_empty() && recs.len() == recv.pts.len() && recs.iter().all(|o| an.heap.kind(*o) != Kind::Frame && st.row(*o, Ns::Item).definite.contains(k)) {
            return out;
        }
    }
    out.join(&default)
}

pub(super) fn m_setdefault(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(key) = c.args.first() else { return Value::data("setdefault") };
    an.check_hashable(key, st);
    let okey = an.opaque_key(key, st);
    let key = &okey;
    let default = c.args.get(1).cloned().unwrap_or_else(|| Value::none());
    let recv = c.recv.clone().unwrap_or_default();
    let recs = an.record_objects(&recv);
    if let Some(k) = key.kv.as_lit() {
        if !recs.is_empty() && recs.len() == recv.pts.len() {
            let rows: Vec<Row> = recs.iter().map(|o| st.row(*o, Ns::Item)).collect();
            if rows.iter().all(|r| r.definite.contains(k)) {
                // present everywhere: the stored value, nothing is written
                return an.load_item(&recv, Ns::Item, key, &c.node, st, false);
            }
            if rows.iter().all(|r| r.lacks(k)) {
                // absent everywhere: the default is stored and returned
                an.store_item(&recv, Ns::Item, key, &default, &c.node, st);
                return default;
            }
        }
    }
    let cur = an.load_item(&recv, Ns::Item, key, &c.node, st, false);
    let nv = if !cur.empty() { cur.join(&default) } else { default };
    an.store_item(&recv, Ns::Item, key, &nv, &c.node, st);
    an.load_item(&recv, Ns::Item, key, &c.node, st, false)
}

/// `d.update(obj)` / `dict(obj)` with an instance of a user class that is no dict: an object
/// with `keys()` is read as `obj[k]` for each key it lists; otherwise it is an iterable of
/// pairs (unknown here: the row opens). Returns false when `src` holds no such instance.
pub(super) fn update_from_instances(an: &mut Analyzer, st: &mut State, oid: u32, src: &Value, strong: bool, c: &Call) -> bool {
    let src = &an.unkey(src);
    if src.pts.is_empty() {
        return false;
    }
    let insts: Vec<u32> = src.pts.iter().filter(|o| an.heap.kind(*o) == Kind::Instance && an.heap.instance_class(*o).is_some_and(|cl| !an.heap.class_info_opt(cl).map(|ci| ci.lib.dict_base || ci.lib.typed_dict || ci.lib.pydantic).unwrap_or(true))).collect();
    if insts.is_empty() || insts.len() != src.pts.len() {
        return false;
    }
    let strong = strong && insts.len() == 1;
    for inst in insts {
        let cls = an.heap.instance_class(inst).unwrap();
        if an.class_lookup(cls, "keys").is_some() {
            let ks = an.call_method_with(&Value::obj(inst), "keys", vec![], &c.node, st).unwrap_or_else(|| Value::data("keys"));
            let it = an.iterate(&ks, &c.node, st);
            match it.items {
                Some(items) => {
                    for k in items {
                        let v = an.call_method_with(&Value::obj(inst), "__getitem__", vec![k.clone()], &c.node, st).unwrap_or_else(|| Value::data("getitem"));
                        an.store_key(oid, Ns::Item, &k, &v, strong, &c.node, st);
                    }
                }
                None => {
                    let k = it.elem.flat();
                    let v = an.call_method_with(&Value::obj(inst), "__getitem__", vec![k.clone()], &c.node, st).unwrap_or_else(|| Value::data("getitem"));
                    an.store_key(oid, Ns::Item, &Value::data("mapkeys"), &v, false, &c.node, st);
                }
            }
        } else {
            let r = st.row(oid, Ns::Item).data_store(Source::new(&format!("update@L{}", c.line), SourceKind::Data));
            st.set_row(oid, Ns::Item, r);
        }
    }
    true
}

/// What of `src` a dict can take its pairs from (`dict(src)`, `d.update(src)`): a number or
/// None raises TypeError, a non-empty str ValueError (its items are no pairs), an empty one
/// gives none; None where nothing else is left (the path ends).
pub(super) fn pairs_part(an: &mut Analyzer, st: &mut State, src: &Value) -> Option<Value> {
    // (an unknown, of no type known: a mapping or pairs, or what gives neither)
    if an.untyped_operand(src) && !src.pts.iter().any(|o| an.heap.obj(o).ty == Some("dict")) {
        an.may_raise_here("TypeError", st);
        an.may_raise_here("ValueError", st);
    }
    if !src.kv.is_literal() {
        return Some(src.clone());
    }
    // (an object's identity key, a tuple of pairs, a frozenset: taken as they are)
    let ks = crate::domain::kv_literals(&src.kv);
    let kept: KeySet = ks.iter().filter(|k| intern::with_key(*k, |k| matches!(k, intern::Key::Member(..) | intern::Key::Inv(..) | intern::Key::Tuple(_) | intern::Key::Frozen(_)))).collect();
    let empty = ks.iter().any(|k| intern::with_key(k, |k| matches!(k, intern::Key::Str(t) if t.is_empty()) || matches!(k, intern::Key::Bytes(b) if b.is_empty())));
    let rejected: Vec<KeyId> = ks.iter().filter(|k| !kept.contains(*k) && !intern::with_key(*k, |k| matches!(k, intern::Key::Str(t) if t.is_empty()) || matches!(k, intern::Key::Bytes(b) if b.is_empty()))).collect();
    if rejected.is_empty() {
        // (only empty texts: no pairs)
        return Some(if src.pts.is_empty() && kept.is_empty() { Value::new() } else { src.clone() });
    }
    if rejected.iter().any(|k| !intern::key_is_str(*k)) {
        an.may_raise_here("TypeError", st);
    }
    if rejected.iter().any(|k| intern::key_is_str(*k)) {
        an.may_raise_here("ValueError", st);
    }
    if src.pts.is_empty() && !empty && kept.is_empty() {
        st.reachable = false;
        return None;
    }
    Some(Value::with_kv(src.pts.clone(), if kept.is_empty() { crate::domain::BOT } else { KeyVal::lits(kept) }))
}

pub(super) fn m_update(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    // (pairs from a number, None or a str: TypeError / ValueError)
    if let Some(src) = c.args.first() {
        match pairs_part(an, st, src) {
            None => return Value::new(),
            // (`d.update("")`: no pairs)
            Some(v) if v.empty() && c.kwargs.is_empty() => return Value::none(),
            Some(v) if v != *src => return m_update(an, st, &Call { args: vec![v], ..c.clone() }),
            _ => {}
        }
    }
    // a parameter's mapping (or keywords): decided per call site
    let recs = dict_recv(an, c);
    let strong = recs.len() == 1 && an.heap.obj(recs[0]).single;
    for oid in recs {
        if let Some(src) = c.args.first() {
            if update_from_instances(an, st, oid, src, strong, c) {
            } else if !an.record_objects(src).is_empty() {
                an.copy_into(oid, Ns::Item, src, st, strong, &KeySet::new());
            } else if !src.pts.is_empty() {
                let it = an.iterate(src, &c.node, st);
                match (&it.elem, &it.items) {
                    (Elem::Pair(k, v), _) => {
                        // (each key is hashed: a list among them raises, if there is one)
                        an.check_hashable_if(k, false, st);
                        an.store_key(oid, Ns::Item, k, v, false, &c.node, st)
                    }
                    (_, Some(items)) => {
                        for item in items {
                            let parts = an.unpack(item, 2, st, &c.node);
                            an.store_key(oid, Ns::Item, &parts[0], &parts[1], strong, &c.node, st);
                        }
                    }
                    _ => {
                        let r = st.row(oid, Ns::Item).data_store(Source::new(&format!("update@L{}", c.line), SourceKind::Data));
                        st.set_row(oid, Ns::Item, r);
                    }
                }
            } else {
                let r = st.row(oid, Ns::Item).data_store(Source::new(&format!("update@L{}", c.line), SourceKind::Data));
                st.set_row(oid, Ns::Item, r);
            }
        }
        for (k, v) in c.kwargs.iter() {
            an.store_key(oid, Ns::Item, &Value::lit(intern::key_of_str(*k)), v, strong, &c.node, st);
        }
        for sk in &c.star_kwargs {
            an.copy_into(oid, Ns::Item, sk, st, strong, &KeySet::new());
        }
    }
    Value::none()
}

/// `xs.pop()` / `xs.pop(i)` on one list: positional when the list's positions are known.
pub(super) fn list_pop_one(an: &mut Analyzer, st: &mut State, oid: u32, c: &Call) -> Value {
    let ll = an.heap.llen(oid, st);
    let single = an.heap.obj(oid).single;
    let idx = c.args.first().and_then(|a| a.kv.as_lit()).and_then(intern::key_as_int);
    let keyed = c.args.first().map(|a| a.kv.as_lit().is_none()).unwrap_or(false);
    if let (Some(n), false) = (ll, keyed) {
        if single && n > 0 && an.frame().loop_depth == 0 {
            let i = match idx {
                None => n as i64 - 1,
                Some(i) if i < 0 => n as i64 + i,
                Some(i) => i,
            };
            if 0 <= i && (i as usize) < n {
                let v = st.slot(oid, Ns::Pos, intern::key_int(i));
                for j in (i as usize)..(n - 1) {
                    let nv = st.slot(oid, Ns::Pos, intern::key_int(j as i64 + 1));
                    st.set_slot(oid, Ns::Pos, intern::key_int(j as i64), nv, true);
                }
                st.pop_slot(oid, Ns::Pos, intern::key_int(n as i64 - 1));
                an.heap.set_llen(oid, st, Some(n - 1));
                return v;
            }
        }
    }
    an.heap.set_llen(oid, st, None);
    an.elem_of(&Value::obj(oid), &c.node, st)
}

pub(super) fn m_pop(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    st.drop_name_keys(None);
    let recv = c.recv.clone().unwrap_or_default();
    let int_key = c.args.first().and_then(|a| a.kv.as_lit()).map(|k| intern::key_as_int(k).is_some()).unwrap_or(false);
    let listy = recv.pts.iter().all(|o| matches!(an.heap.kind(o), Kind::List | Kind::Tuple));
    if !recv.pts.is_empty() && listy && (c.args.is_empty() || int_key) {
        // list.pop through a placeholder (or with no key)
        return m_lpop(an, st, c);
    }
    let Some(key) = c.args.first() else { return Value::data("pop") };
    if recv.pts.iter().any(|o| an.hashes_keys(o)) {
        an.check_hashable(key, st);
    }
    // (a key that may be an object: an unknown key)
    let okey = an.opaque_key(key, st);
    let key = &okey;
    let has_default = c.args.len() > 1;
    if let Some(k) = key.kv.as_lit() {
        let recs = an.record_objects(&recv);
        if !recs.is_empty() && recs.len() == recv.pts.len() && recs.iter().all(|o| an.heap.kind(*o) != Kind::Frame) {
            let rows: Vec<Row> = recs.iter().map(|o| st.row(*o, Ns::Item)).collect();
            if has_default && rows.iter().all(|r| r.lacks(k)) {
                // absent everywhere: the default alone
                return c.args[1].clone();
            }
            if rows.iter().all(|r| r.definite.contains(k)) {
                // present everywhere: the value alone
                let out = an.load_item(&recv, Ns::Item, key, &c.node, st, !has_default);
                an.delete_item(&recv, Ns::Item, key, &c.node, st, false);
                return out;
            }
        }
    }
    // (pop reports its own raises: without a default a key that may be missing is a KeyError)
    if !has_default {
        an.may_raise_here("KeyError", st);
    }
    let mut out = an.load_item(&recv, Ns::Item, key, &c.node, st, !has_default);
    an.delete_item(&recv, Ns::Item, key, &c.node, st, false);
    if has_default {
        out = out.join(&c.args[1]);
    }
    out
}

pub(super) fn m_popitem(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    st.drop_name_keys(None);
    let recs = dict_recv(an, c);
    // one closed dict whose insertion order is known: the last key is removed (the first
    // with `last=False`)
    let first = c.kw("last").or_else(|| c.args.first().cloned()).map(|v| v.kv.as_lit() == Some(intern::key_bool(false))).unwrap_or(false);
    if let [oid] = recs[..] {
        let row = st.row(oid, Ns::Item);
        if an.heap.obj(oid).single && row.closed() && row.present == row.definite {
            if let Some(last_k) = row.ordered_keys().and_then(|ks| if first { ks.first().copied() } else { ks.last().copied() }) {
                let v = st.slot(oid, Ns::Item, last_k);
                st.set_row(oid, Ns::Item, row.delete(last_k, true));
                let tid = an.alloc_positional(Kind::Tuple, &c.node, Some(vec![Value::lit(last_k), v]), Value::new(), DATA, st);
                return tid;
            }
        }
    }
    for oid in recs {
        let row = st.row(oid, Ns::Item);
        st.set_row(oid, Ns::Item, Row::from_data(RowData { present: row.present.clone(), sources: row.sources.clone(), mutators: row.mutators.clone(), top: row.top, ..Default::default() }));
    }
    Value::data("popitem")
}

/// `od.move_to_end(k, last=True)` on one closed dict with a known order.
pub(super) fn m_move_to_end(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let recs = dict_recv(an, c);
    // the key is looked up: an access (KeyError when it is not there)
    if let (Some(kv), Some(recv)) = (c.args.first(), c.recv.clone()) {
        let row0 = recs.first().map(|o| st.row(*o, Ns::Item));
        an.load_item(&recv, Ns::Item, kv, &c.node, st, true);
        if let (Some(k), Some(r0)) = (kv.kv.as_lit().filter(|_| kv.pts.is_empty()), row0) {
            if recs.len() == 1 && r0.closed() && !r0.present.contains(k) {
                an.raise_here("KeyError", st);
                return Value::new();
            }
        }
    }
    let k = c.args.first().and_then(|a| a.kv.as_lit().filter(|_| a.pts.is_empty()));
    let to_front = c.kw("last").or_else(|| c.args.get(1).cloned()).map(|v| v.kv.as_lit() == Some(intern::key_bool(false))).unwrap_or(false);
    if let ([oid], Some(k)) = (&recs[..], k) {
        let row = st.row(*oid, Ns::Item);
        if let Some(mut ks) = row.ordered_keys().filter(|ks| ks.contains(&k) && row.definite.contains(k)) {
            ks.retain(|x| *x != k);
            if to_front {
                ks.insert(0, k);
            } else {
                ks.push(k);
            }
            st.set_row(*oid, Ns::Item, row.with_order(ks));
            return Value::none();
        }
        if row.closed() && !row.present.contains(k) {
            an.raise_here("KeyError", st);
            return Value::new();
        }
    }
    for o in recs {
        // the key order is no longer known
        let row = st.row(o, Ns::Item);
        st.set_row(o, Ns::Item, row.with_order(Vec::new()));
    }
    Value::none()
}

pub(super) fn m_clear(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    st.drop_name_keys(None);
    let recs = dict_recv(an, c);
    for &oid in &recs {
        let strong = an.heap.obj(oid).single && recs.len() == 1;
        if strong {
            st.set_row(oid, Ns::Item, Row::new());
            st.drop_obj_ns(oid, Ns::Item);
        } else {
            let row = st.row(oid, Ns::Item);
            st.set_row(oid, Ns::Item, Row::from_data(RowData { present: row.present.clone(), sources: row.sources.clone(), mutators: row.mutators.clone(), ..Default::default() }));
        }
    }
    Value::none()
}

pub(super) fn m_views(an: &mut Analyzer, _st: &mut State, c: &Call) -> Value {
    let kind = ViewKind::from_name(last(&c.qual));
    let recv = c.recv.clone().unwrap_or_default();
    let ids: Vec<String> = recv.pts.iter().map(|x| x.to_string()).collect();
    let oid = an.heap.alloc_raw(Kind::View, &format!("view:{}:{}", kind.name(), ids.join(",")), "", true, &format!("{} view", kind.name()), Payload::View(kind, recv.clone()));
    an.heap.obj_mut(oid).payload = Payload::View(kind, recv);
    Value::obj(oid)
}

pub(super) fn m_dcopy(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let outs: Vec<Value> = dict_recv(an, c).into_iter().map(|o| Value::obj(an.copy_object(o, &c.node, st, false, &mut IndexMap::new()))).collect();
    if outs.is_empty() {
        Value::data("copy")
    } else {
        join_values(outs)
    }
}

pub(super) fn m_unknown_method(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    an.unknown_call(&c.qual, c.recv.as_ref(), c.arguments(), st, &c.node)
}

/// `lst.append(x)` / `lst.extend(known items)` on a list whose length is one of a few: the
/// alternatives shifted by the number added (a length past 15 voids them all).
pub(super) fn m_append(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let added: Option<usize> = match last(&c.qual) {
        "append" => Some(1),
        "extend" => c.args.first().and_then(|a| a.single()).filter(|o| matches!(an.heap.kind(*o), Kind::List | Kind::Tuple)).and_then(|o| an.heap.llen(o, st)),
        _ => None,
    };
    let before: Vec<(crate::domain::ObjId, u16)> = match (&c.recv, added) {
        (Some(r), Some(_)) => r.pts.iter().filter_map(|o| st.lens.get(&o).filter(|l| l.0.is_none() && l.2 != 0).map(|l| (o, l.2))).collect(),
        _ => Vec::new(),
    };
    // (a length that is a dim: that many more)
    let sym = c.recv.as_ref().and_then(|r| r.single()).and_then(|o| Some((o, st.sym_len(o)?))).filter(|_| added.is_some());
    let r = m_append_inner(an, st, c);
    if let (Some((o, l)), Some(k)) = (sym, added) {
        if let Some(d) = crate::models::shape::parse_dim_label(&intern::str_get(l)) {
            st.set_sym_len(o, intern::s(&crate::models::shape::dim_label(crate::models::shape::add(d, crate::models::shape::lit(k as i64)))));
        }
    }
    // (one of several lists: each may also not have grown)
    let alone = c.recv.as_ref().is_some_and(|r| r.pts.len() == 1 && r.kv.is_bot());
    if let Some(k) = added {
        for (o, m) in before {
            if let Some((None, p, 0)) = st.lens.get(&o) {
                let shifted = ((m as u32) << k) | if alone { 0 } else { m as u32 };
                if shifted <= 0xFFFF {
                    let last = st.last_of(o);
                    st.set_lens(o, (None, p, shifted as u16));
                    if let Some(l) = last {
                        st.set_last(o, l);
                    }
                }
            }
        }
    }
    r
}

pub(super) fn m_append_inner(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = last(&c.qual);
    let recv = c.recv.clone().unwrap_or_default();
    // (a receiver that is one of several lists: each may or may not be the one that grows)
    let alone = recv.pts.len() == 1 && recv.kv.is_bot();
    for oid in recv.pts.iter() {
        if !matches!(an.heap.kind(oid), Kind::List | Kind::Set | Kind::Tuple) {
            continue;
        }
        let mut ext_items: Option<Vec<Value>> = None;
        // (the lengths it may have, before a store of an element voids them)
        let alts = an.heap.len_alts(oid, st);
        let elem = if (name == "extend" || name == "update") && !c.args.is_empty() {
            let it = an.iterate(&c.args[0], &c.node, st);
            if name == "extend" {
                ext_items = it.items.clone();
            }
            an.elem_value(&it.elem, &c.node, "el")
        } else {
            c.args.last().cloned().unwrap_or_default()
        };
        // extend by known items in straight-line code: each item gets its position
        if let (Some(items), Kind::List) = (&ext_items, an.heap.kind(oid)) {
            let ll = an.heap.llen(oid, st);
            if let Some(n) = ll {
                let cap = if an.m.torch_on { 64 } else { 16 };
                if alone && an.heap.obj(oid).single && an.frame().loop_depth == 0 && n + items.len() <= cap {
                    for (i, item) in items.iter().enumerate() {
                        st.set_slot(oid, Ns::Pos, intern::key_int((n + i) as i64), item.clone(), true);
                        an.add_elem(oid, item, st);
                    }
                    an.heap.set_llen(oid, st, Some(n + items.len()));
                    continue;
                }
            }
        }
        an.add_elem(oid, &elem, st);
        let ll = an.heap.llen(oid, st);
        let single = an.heap.obj(oid).single;
        if let Some(n) = ll {
            if alone && single && name == "append" && an.frame().loop_depth == 0 && n < if an.m.torch_on { 64 } else { 16 } {
                st.set_slot(oid, Ns::Pos, intern::key_int(n as i64), elem, true);
                an.heap.set_llen(oid, st, Some(n + 1));
                continue;
            }
            // one of several lists: this one as it was, or longer by the items
            let items = if name == "append" { Some(vec![elem.clone()]) } else { ext_items.clone() };
            if let Some(items) = items.filter(|its| !alone && an.frame().loop_depth == 0 && n + its.len() < 16) {
                for (i, item) in items.iter().enumerate() {
                    st.set_slot(oid, Ns::Pos, intern::key_int((n + i) as i64), item.clone(), false);
                }
                st.set_lens(oid, (None, n, (1u16 << n) | (1u16 << (n + items.len()))));
                continue;
            }
            // appends of unknown count: the positions so far stay valid
            if name == "append" || name == "extend" {
                an.heap.set_prefix(oid, st, n);
                continue;
            }
            st.drop_obj_ns(oid, Ns::Pos);
        } else if name == "append" || name == "extend" {
            // the length was unknown already: the known prefix stays; one of a few lengths: the
            // items at each one's end (as each path has them)
            if let Some(alts) = alts {
                // (the positions a longer alternative has must be known: a write at one joins them)
                let longest = alts.iter().copied().max().unwrap_or(0);
                let known = (0..longest).all(|p| st.has_slot(oid, Ns::Pos, intern::key_int(p as i64)));
                match if name == "append" { Some(vec![elem.clone()]) } else { ext_items.clone() }.filter(|_| known) {
                    Some(items) => {
                        for n in alts {
                            for (i, item) in items.iter().enumerate() {
                                st.set_slot(oid, Ns::Pos, intern::key_int((n + i) as i64), item.clone(), false);
                            }
                        }
                    }
                    None => {
                        let p = (0..alts[0]).take_while(|p| st.has_slot(oid, Ns::Pos, intern::key_int(*p as i64))).count();
                        an.heap.set_prefix(oid, st, p)
                    }
                }
            }
            continue;
        }
        an.heap.set_llen(oid, st, None);
    }
    // the last element after an append (whatever the length)
    if name == "append" {
        if let (Some(oid), Some(x)) = (recv.single(), c.args.last()) {
            if an.heap.kind(oid) == Kind::List && an.heap.obj(oid).single {
                st.set_last(oid, x.clone());
            }
        }
    }
    Value::none()
}

/// `slice(stop)` / `slice(start, stop[, step])`: the label a `a:b:c` subscript makes (known
/// int / None bounds), else an unknown slice.
pub(super) fn m_slice(_an: &mut Analyzer, _st: &mut State, c: &Call) -> Value {
    let none = intern::key_none();
    // a parameter bound: known at the call site
    let b = |v: &Value| -> Option<String> {
        if !v.pts.is_empty() {
            return None;
        }
        let k = v.kv.as_lit()?;
        if k == none {
            return Some(String::new());
        }
        intern::key_as_int(k).filter(|_| !intern::with_key(k, |kk| matches!(kk, intern::Key::Bool(_)))).map(|i| i.to_string())
    };
    let parts: Option<Vec<String>> = match c.args.len() {
        1 => b(&c.args[0]).map(|h| vec![String::new(), h, String::new()]),
        2 | 3 => {
            let mut v: Vec<Option<String>> = c.args.iter().map(b).collect();
            if v.len() == 2 {
                v.push(Some(String::new()));
            }
            v.into_iter().collect()
        }
        _ => None,
    };
    match parts {
        Some(p) => Value::data(&format!("slice[{}]@L{}", p.join(":"), c.line)),
        None => Value::data(&format!("slice@L{}", c.line)),
    }
}

pub(super) fn m_slice_indices(an: &mut Analyzer, _st: &mut State, c: &Call) -> Value {
    let recv = c.recv.clone().unwrap_or_default();
    let n = c.args.first().and_then(|v| v.kv.as_lit().filter(|_| v.pts.is_empty())).and_then(intern::key_as_int);
    match (Analyzer::slice_label_bounds(&recv.kv), n) {
        (Some((lo, hi, st)), Some(n)) if n >= 0 && st != Some(0) => {
            let (a, b, s) = crate::interp::exprs::slice_indices(n, lo, hi, st.unwrap_or(1));
            Value::lit(intern::key_tuple(vec![intern::key_int(a), intern::key_int(b), intern::key_int(s)]))
        }
        _ => {
            let _ = an;
            Value::data("indices")
        }
    }
}

/// Set methods: decided on sets with known (literal) members, else the element model.
pub(super) fn m_set(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = last(&c.qual).to_string();
    let recv = c.recv.clone().unwrap_or_default();
    let mine = an.known_members(&recv, st);
    let args: Vec<Option<KeySet>> = c.args.iter().map(|a| an.known_members(a, st)).collect();
    let all_args = args.iter().all(|a| a.is_some());
    let fold = |base: &KeySet, f: &dyn Fn(&KeySet, &KeySet) -> KeySet| -> KeySet { args.iter().flatten().fold(base.clone(), |acc, a| f(&acc, a)) };
    let union = |a: &KeySet, b: &KeySet| a.union(b);
    let inter = |a: &KeySet, b: &KeySet| a.intersection(b);
    let diff = |a: &KeySet, b: &KeySet| a.difference(b);
    let sym = |a: &KeySet, b: &KeySet| a.difference(b).union(&b.difference(a));
    let op: Option<&dyn Fn(&KeySet, &KeySet) -> KeySet> = match name.as_str() {
        "union" | "update" => Some(&union),
        "intersection" | "intersection_update" => Some(&inter),
        "difference" | "difference_update" => Some(&diff),
        "symmetric_difference" | "symmetric_difference_update" => Some(&sym),
        _ => None,
    };
    match name.as_str() {
        "union" | "intersection" | "difference" | "symmetric_difference" | "copy" => {
            if let (Some(m), true) = (&mine, all_args) {
                let r = match op {
                    Some(f) => fold(m, f),
                    None => m.clone(),
                };
                return an.literal_set(&c.node, r, st);
            }
            // a new set: the elements it may hold
            let mut kv = KeyVal::Bot;
            let mut elem = Value::new();
            let srcs: Vec<Value> = std::iter::once(recv.clone()).chain(if name == "union" { c.args.clone() } else { vec![] }).collect();
            for v in &srcs {
                let it = an.iterate(v, &c.node, st);
                let e = an.elem_value(&it.elem, &c.node, "el");
                kv = kv_join(&kv, &e.kv);
                elem = elem.join(&e);
            }
            an.alloc_positional(Kind::Set, &c.node, None, elem, kv, st)
        }
        "issubset" | "issuperset" | "isdisjoint" => {
            if let (Some(m), Some(Some(o))) = (&mine, args.first()) {
                let b = match name.as_str() {
                    "issubset" => m.is_subset(o),
                    "issuperset" => o.is_subset(m),
                    _ => !m.intersects(o),
                };
                return Value::lit(intern::key_bool(b));
            }
            Value::data("set")
        }
        "add" | "discard" | "remove" | "update" | "intersection_update" | "difference_update" | "symmetric_difference_update" | "clear" => {
            if matches!(name.as_str(), "add" | "discard" | "remove") {
                if let Some(a) = c.args.first() {
                    an.check_hashable(a, st);
                }
            }
            // (a tuple of known values: its key)
            let item = c.args.first().and_then(|a| match a.single() {
                Some(o) if a.kv.is_bot() && an.heap.kind(o) == Kind::Tuple => an.value_key(o, st, 0),
                _ => a.kv.as_lit().filter(|_| a.pts.is_empty()),
            });
            for oid in recv.pts.iter().filter(|o| an.heap.kind(*o) == Kind::Set).collect::<Vec<_>>() {
                let strong = recv.pts.len() == 1 && an.heap.obj(oid).single;
                let known = an.known_members(&Value::obj(oid), st);
                let next: Option<KeySet> = match (name.as_str(), &known, item) {
                    ("clear", _, _) if strong => Some(KeySet::new()),
                    ("add", Some(m), Some(k)) => Some(m.insert(k)),
                    ("discard", Some(m), Some(k)) => Some(m.remove(k)),
                    ("remove", Some(m), Some(k)) => {
                        if !m.contains(k) {
                            an.raise_here("KeyError", st);
                            return Value::new();
                        }
                        Some(m.remove(k))
                    }
                    (_, Some(m), _) if all_args && op.is_some() => Some(fold(m, op.unwrap())),
                    _ => None,
                };
                match next {
                    Some(r) if strong => an.set_members(oid, r, st),
                    _ => {
                        // members not known any more: the elements it may hold
                        let mut kv = st.elems.get(&oid).unwrap_or(KeyVal::Bot);
                        if matches!(name.as_str(), "add" | "update") {
                            for a in &c.args {
                                let e = if name == "add" { a.clone() } else { { let it_ = an.iterate(a, &c.node, st); an.elem_value(&it_.elem, &c.node, "el") } };
                                kv = kv_join(&kv, &e.kv);
                                let cur = st.slot(oid, Ns::Elem, STAR);
                                st.set_slot(oid, Ns::Elem, STAR, cur.join(&Value::with_kv(e.pts.clone(), crate::models::torch::elem_kv(&e.kv))), true);
                            }
                        }
                        st.elems.insert(oid, kv);
                        an.heap.set_llen(oid, st, None);
                        st.drop_obj_ns(oid, Ns::Pos);
                    }
                }
            }
            Value::none()
        }
        _ => Value::data("set"),
    }
}

/// `ChainMap` attributes and methods: `maps`, `parents`, `new_child(m)`.
pub(super) fn m_chainmap(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = last(&c.qual).to_string();
    let Some(oid) = c.recv.as_ref().and_then(|r| r.single()) else { return Value::data("chainmap") };
    let maps = an.m.chainmaps.get(&oid).cloned().unwrap_or_default();
    match name.as_str() {
        "maps" => an.alloc_positional(Kind::List, &c.node, Some(maps), Value::new(), DATA, st),
        "parents" | "new_child" => {
            let mut ms: Vec<Value> = if name == "parents" { maps.iter().skip(1).cloned().collect() } else { maps.clone() };
            if name == "new_child" {
                let first = match c.arg(0, Some("m")) {
                    Some(m) => m,
                    None => {
                        let d = an.alloc_at(Kind::Dict, &c.node, st, &format!("dict@L{}", c.line), Payload::None, Some(Row::new()), Ns::Item, None, Some("chainchild"));
                        st.set_row(d, Ns::Item, Row::new());
                        Value::obj(d)
                    }
                };
                ms.insert(0, first);
            }
            let call = Call { qual: "collections.ChainMap".into(), recv: None, args: ms, kwargs: IndexMap::new(), star_args: vec![], star_kwargs: vec![], node: c.node, call_ast: None, site: format!("{}:{}", c.site, name), line: c.line };
            an.frame_mut().unroll.push(7000);
            let v = m_defaultdict(an, st, &call);
            an.frame_mut().unroll.pop();
            v
        }
        _ => Value::data("chainmap"),
    }
}

/// `xs.pop()` / `xs.pop(i)` / `s.pop()`: the item; an empty list or an index outside it is an
/// IndexError (an empty set a KeyError), on every path when no receiver can have the item.
pub(super) fn m_lpop(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let recv = c.recv.clone().unwrap_or_default();
    let idx = c.args.first().and_then(|a| a.kv.as_lit().filter(|_| a.pts.is_empty())).and_then(intern::key_as_int);
    let (mut outs, mut may_fail, mut all_fail) = (Vec::new(), recv.pts.is_empty(), !recv.pts.is_empty());
    for oid in recv.pts.iter() {
        let n = an.heap.llen(oid, st);
        let (ok, fails) = match (n, idx, c.args.is_empty()) {
            (Some(n), _, true) => (n > 0, n == 0),
            (Some(n), Some(i), _) => {
                let inside = (i >= 0 && (i as usize) < n) || (i < 0 && ((-i) as usize) <= n);
                (inside, !inside)
            }
            _ => (false, false),
        };
        may_fail |= !ok;
        all_fail &= fails;
        if fails {
            continue;
        }
        // (a set of several members gives any of them: its order is not promised)
        match n.filter(|n| *n > 1 && an.heap.kind(oid) == Kind::Set) {
            Some(n) => {
                outs.push(join_values((0..n).map(|i| st.slot(oid, Ns::Pos, intern::key_int(i as i64))).collect::<Vec<_>>()));
                an.heap.set_llen(oid, st, None);
                st.drop_obj_ns(oid, Ns::Pos);
            }
            None => outs.push(list_pop_one(an, st, oid, c)),
        }
    }
    let exc = if c.qual.starts_with("set.") { "KeyError" } else { "IndexError" };
    if all_fail {
        an.raise_here(exc, st);
        return Value::new();
    }
    if may_fail {
        an.may_raise_here(exc, st);
    }
    if outs.is_empty() {
        Value::data("pop")
    } else {
        join_values(outs)
    }
}

pub(super) fn m_list_other(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = last(&c.qual);
    let recv = c.recv.clone().unwrap_or_default();
    match name {
        // (a generator's `__next__` / `send`: the next item, as `next(g)`)
        "__next__" | "send" if recv.pts.iter().all(|o| an.heap.obj(o).iterator) => m_next(an, st, &Call { args: vec![recv], ..c.clone() }),
        "copy" => join_values(recv.pts.iter().map(|o| Value::obj(an.copy_object(o, &c.node, st, false, &mut IndexMap::new()))).collect::<Vec<_>>()),
        "remove" | "discard" | "clear" | "sort" | "reverse" | "insert" => {
            // (`sort` compares the items: ones of kinds that do not compare may raise TypeError)
            if name == "sort" && c.kw("key").is_none() {
                if crate::models::builtins::incomparable(an, &crate::models::builtins::order_items(an, &recv, st)) {
                    an.may_raise_here("TypeError", st);
                }
            }
            // `clear` on one list, whatever its length: empty
            if name == "clear" {
                if let Some(oid) = recv.single().filter(|o| an.heap.kind(*o) == Kind::List && an.heap.obj(*o).single && !crate::state::is_merged(*o)) {
                    st.drop_obj_ns(oid, Ns::Pos);
                    an.set_positions(oid, Vec::new(), st);
                    st.clear_last(oid);
                    return Value::none();
                }
            }
            // one list with known positions: updated in place
            if let Some(oid) = recv.single() {
                if let Some(mut ps) = an.known_positions(oid, st) {
                    let done = match name {
                        "clear" => {
                            ps.clear();
                            true
                        }
                        "reverse" => {
                            ps.reverse();
                            true
                        }
                        "insert" if c.args.len() == 2 => match c.args[0].kv.as_lit().filter(|_| c.args[0].pts.is_empty()).and_then(intern::key_as_int) {
                            Some(i) => {
                                let n = ps.len() as i64;
                                let at = if i < 0 { (n + i).max(0) } else { i.min(n) } as usize;
                                ps.insert(at, c.args[1].clone());
                                true
                            }
                            None => false,
                        },
                        "sort" => match literal_sort_order(an, st, c, &ps) {
                            Some(order) => {
                                ps = order.into_iter().map(|j| ps[j].clone()).collect();
                                true
                            }
                            None => false,
                        },
                        _ => false,
                    };
                    if done {
                        an.set_positions(oid, ps, st);
                        return Value::none();
                    }
                }
            }
            for oid in recv.pts.iter() {
                an.heap.set_llen(oid, st, None);
                st.drop_obj_ns(oid, Ns::Pos);
                if name == "insert" {
                    if let Some(x) = c.args.get(1) {
                        an.add_elem(oid, x, st);
                    }
                }
            }
            Value::none()
        }
        // (the statements' dunders called by name: as the statements)
        "__setitem__" if c.args.len() == 2 => {
            an.store_item(&recv, Ns::Item, &c.args[0], &c.args[1], &c.node, st);
            Value::none()
        }
        "__delitem__" if c.args.len() == 1 => {
            an.delete_subscript(&recv, &c.args[0], &c.node, st);
            Value::none()
        }
        "__iadd__" | "__ior__" | "__iand__" | "__isub__" | "__ixor__" if c.args.len() == 1 && recv.kv.is_bot() => {
            let kind = |k: Kind| !recv.pts.is_empty() && recv.pts.iter().all(|o| an.heap.kind(o) == k);
            let m = match name {
                "__iadd__" if kind(Kind::List) => "list.extend",
                "__ior__" if kind(Kind::Set) => "set.update",
                "__iand__" if kind(Kind::Set) => "set.intersection_update",
                "__isub__" if kind(Kind::Set) => "set.difference_update",
                "__ixor__" if kind(Kind::Set) => "set.symmetric_difference_update",
                _ => return an.unknown_call(&c.qual, c.recv.as_ref(), c.arguments(), st, &c.node),
            };
            let mv = an.method_value(m, &recv);
            an.call_value(&mv, &c.node, None, st, Some(c.args.clone()));
            recv
        }
        // (any other method of a list or set, called by name, may change it: unknown code,
        // unless it only reads)
        _ if !LIST_READERS.contains(&name) && recv.pts.iter().any(|o| matches!(an.heap.kind(o), Kind::List | Kind::Set)) => an.unknown_call(&c.qual, c.recv.as_ref(), c.arguments(), st, &c.node),
        _ => Value::data(&format!("L{}", c.line)),
    }
}

/// The methods of a list or set that only read it (those `m_list_other` does not model).
pub(super) const LIST_READERS: &[&str] = &[
    "index", "count", "__len__", "__contains__", "__getitem__", "__iter__", "__reversed__", "__eq__", "__ne__", "__lt__", "__le__", "__gt__", "__ge__", "__hash__", "__repr__", "__str__", "__format__",
    "__sizeof__", "__bool__", "__add__", "__mul__", "__rmul__", "__or__", "__and__", "__sub__", "__xor__", "__ror__", "__rand__", "__rsub__", "__rxor__",
];

pub(super) fn m_view(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = last(&c.qual);
    if name == "map" {
        // `df.columns.map(f)`: the column names renamed by f
        if let Some(v) = pandas::columns_map(an, st, c) {
            return v;
        }
    }
    // `obj.__dict__.update(...)`: stores on the instance's attributes
    let attrs_of: Vec<u32> = c.recv.iter().flat_map(|r| r.pts.iter()).filter_map(|v| match &an.heap.obj(v).payload {
        Payload::View(ViewKind::Attrs, inst) => Some(inst.pts.to_vec()),
        _ => None,
    }).flatten().collect();
    if !attrs_of.is_empty() {
        if name == "get" && !c.args.is_empty() {
            // `obj.__dict__.get(name, default)`: the attribute, or the default when it is not set
            let dflt = c.args.get(1).cloned().unwrap_or_else(|| Value::none());
            let Some(k) = c.args[0].kv.as_lit().filter(|_| c.args[0].pts.is_empty()) else { return an.all_values(attrs_of[0], Ns::Attr, st).join(&dflt) };
            let mut out = Value::new();
            for o in attrs_of {
                let row = st.row(o, Ns::Attr);
                if row.present.contains(k) {
                    out = out.join(&st.slot(o, Ns::Attr, k));
                }
                if !row.definite.contains(k) {
                    out = out.join(&dflt);
                }
            }
            return out;
        }
        if name != "update" {
            return an.unknown_call(&c.qual, c.recv.as_ref(), c.arguments(), st, &c.node);
        }
        let strong = attrs_of.len() == 1 && an.heap.obj(attrs_of[0]).single;
        for oid in attrs_of {
            if let Some(src) = c.args.first() {
                if !an.record_objects(src).is_empty() {
                    an.copy_into(oid, Ns::Attr, src, st, strong, &KeySet::new());
                } else {
                    let r = st.row(oid, Ns::Attr).data_store(Source::new(&format!("update@L{}", c.line), SourceKind::Data));
                    st.set_row(oid, Ns::Attr, r);
                }
            }
            for (k, v) in c.kwargs.iter() {
                an.store_key(oid, Ns::Attr, &Value::lit(intern::key_of_str(*k)), v, strong, &c.node, st);
            }
            for sk in &c.star_kwargs {
                an.copy_into(oid, Ns::Attr, sk, st, strong, &KeySet::new());
            }
        }
        return Value::none();
    }
    if matches!(name, "tolist" | "to_list" | "copy") {
        if let Some(recv) = &c.recv {
            let oid = an.alloc_simple(Kind::List, &c.node, st, &format!("{}@L{}", name, c.line));
            an.heap.set_llen(oid, st, None);
            let it = an.iterate(recv, &c.node, st);
            let elem = an.elem_value(&it.elem, &c.node, "el");
            st.set_slot(oid, Ns::Elem, STAR, Value::with_kv(elem.pts.clone(), crate::models::torch::elem_kv(&elem.kv)), true);
            st.elems.insert(oid, elem.kv);
            return Value::obj(oid);
        }
    }
    Value::data(&format!("L{}", c.line))
}
