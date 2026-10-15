//! `collections.Counter`: a dict whose values are counts. Counted from literal items (and
//! mappings / keywords), the counts are known; `most_common`, `total`, `elements`, `update`,
//! `subtract` and the operators `+ - & |` (and unary `+` / `-`) follow Counter's rules (the
//! binary operators and unary ones keep only positive counts). Unknown counts keep sound key
//! sets: what may be there, nothing claimed definite that is not.

use indexmap::IndexMap;

use crate::domain::{join_values, KeyVal, Ns, ObjId, Row, Source, SourceKind, Value};
use crate::frontend::Node;
use crate::intern::{self, KeyId};
use crate::interp::{Analyzer, Call, Payload};
use crate::state::{Kind, State};

/// Counter methods (attribute loads on a Counter route here; the rest is the dict model).
pub const COUNTER_METHODS: &[&str] = &["most_common", "total", "elements", "update", "subtract", "copy"];

/// Known counts of a Counter: every key known and definite, every count a literal int.
pub fn counts(an: &Analyzer, oid: ObjId, st: &State) -> Option<Vec<(KeyId, i64)>> {
    let row = st.row(oid, Ns::Item);
    if row.open() || row.present != row.definite {
        return None;
    }
    // (insertion order decides most_common's ties: never guessed)
    let keys = match row.ordered_keys() {
        Some(k) => k,
        None if row.present.len() <= 1 => intern::sorted_by_str(row.present.iter()),
        None => return None,
    };
    let _ = an;
    keys.into_iter()
        .map(|k| {
            let v = st.slot(oid, Ns::Item, k);
            let n = v.kv.as_lit().filter(|_| v.pts.is_empty()).and_then(intern::key_as_int)?;
            Some((k, n))
        })
        .collect()
}

/// A new Counter with exactly these counts (in this order).
pub fn make(an: &mut Analyzer, st: &mut State, node: &Node, entries: &[(KeyId, i64)], ctx: &str) -> Value {
    let oid = an.alloc_at(Kind::Dict, node, st, &format!("Counter@L{}", node.line), Payload::None, Some(Row::new()), Ns::Item, None, Some(ctx));
    let mut row = Row::new();
    let mut order = Vec::new();
    for (k, n) in entries {
        row = row.store(*k, true);
        order.push(*k);
        st.set_slot(oid, Ns::Item, *k, Value::lit(intern::key_int(*n)), true);
    }
    st.set_slot(oid, Ns::Item, intern::STAR, Value::lit(intern::key_int(0)), false);
    st.set_row(oid, Ns::Item, row.with_order(order));
    an.m.counters.insert(oid);
    Value::obj(oid)
}

/// A Counter whose counts are not known: keys `present` (none definite unless `definite`).
fn make_unknown(an: &mut Analyzer, st: &mut State, node: &Node, present: &[KeyId], definite: &[KeyId], open: bool, ctx: &str) -> Value {
    let oid = an.alloc_at(Kind::Dict, node, st, &format!("Counter@L{}", node.line), Payload::None, Some(Row::new()), Ns::Item, None, Some(ctx));
    let mut row = Row::new();
    for k in present {
        row = row.store(*k, definite.contains(k));
        st.set_slot(oid, Ns::Item, *k, Value::data("count"), true);
    }
    if open {
        row = row.data_store(Source::new(&format!("Counter@L{}", node.line), SourceKind::Data));
    }
    st.set_slot(oid, Ns::Item, intern::STAR, Value::lit(intern::key_int(0)), false);
    st.set_row(oid, Ns::Item, row);
    an.m.counters.insert(oid);
    Value::obj(oid)
}

/// What a Counter / mapping / iterable contributes: known counts, or None.
fn contribution(an: &mut Analyzer, st: &mut State, v: &Value, node: &Node) -> Option<Vec<(KeyId, i64)>> {
    let o = v.single();
    if let Some(o) = o {
        if an.heap.kind(o) == Kind::Dict {
            if an.m.counters.contains(&o) {
                return counts(an, o, st);
            }
            // a mapping: its values are the counts
            let row = st.row(o, Ns::Item);
            if row.open() || row.present != row.definite {
                return None;
            }
            let keys = row.ordered_keys().unwrap_or_else(|| intern::sorted_by_str(row.present.iter()));
            return keys
                .into_iter()
                .map(|k| {
                    let x = st.slot(o, Ns::Item, k);
                    Some((k, x.kv.as_lit().filter(|_| x.pts.is_empty()).and_then(intern::key_as_int)?))
                })
                .collect();
        }
    }
    // an iterable of literal items: occurrences
    let it = an.iterate(v, node, st);
    let items = it.items?;
    let mut out: Vec<(KeyId, i64)> = Vec::new();
    for x in items {
        let k = x.kv.as_lit().filter(|_| x.pts.is_empty())?;
        match out.iter_mut().find(|(kk, _)| *kk == k) {
            Some(e) => e.1 += 1,
            None => out.push((k, 1)),
        }
    }
    Some(out)
}

/// The keys a value may contribute (for unknown counts), and whether others may come.
fn maybe_keys(an: &mut Analyzer, st: &mut State, v: &Value, node: &Node) -> (Vec<KeyId>, bool) {
    let mut ks: Vec<KeyId> = Vec::new();
    let mut open = false;
    for o in v.pts.iter() {
        if an.heap.kind(o) == Kind::Dict {
            let row = st.row(o, Ns::Item);
            ks.extend(row.present.iter());
            open |= row.open();
        } else {
            let it = an.iterate(&Value::obj(o), node, st);
            let e = an.elem_value(&it.elem, node, "el");
            match &e.kv {
                kv if kv.is_literal() && e.pts.is_empty() => ks.extend(crate::domain::kv_literals(kv).iter()),
                _ => open = true,
            }
        }
    }
    if v.pts.is_empty() {
        open = true;
    }
    ks.sort();
    ks.dedup();
    (ks, open)
}

/// `Counter(x)`: counted.
pub fn construct(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let mut entries: Vec<(KeyId, i64)> = Vec::new();
    let mut known = true;
    if let Some(a) = c.args.first() {
        match contribution(an, st, a, &c.node) {
            Some(e) => entries = e,
            None => known = false,
        }
    }
    for (k, v) in c.kwargs.iter() {
        let kk = intern::key_str_lit(&intern::str_get(*k));
        match v.kv.as_lit().filter(|_| v.pts.is_empty()).and_then(intern::key_as_int) {
            Some(n) => match entries.iter_mut().find(|(x, _)| *x == kk) {
                Some(e) => e.1 += n,
                None => entries.push((kk, n)),
            },
            None => known = false,
        }
    }
    if known {
        return make(an, st, &c.node, &entries, "counter");
    }
    // counts not known: the keys that may be there
    let mut present: Vec<KeyId> = Vec::new();
    let mut open = false;
    if let Some(a) = c.args.first() {
        let (ks, o) = maybe_keys(an, st, a, &c.node);
        present.extend(ks);
        open |= o;
    }
    let kw: Vec<KeyId> = c.kwargs.keys().map(|k| intern::key_str_lit(&intern::str_get(*k))).collect();
    present.extend(kw.iter().copied());
    make_unknown(an, st, &c.node, &present, &kw, open, "counter")
}

fn combine(op: char, a: &[(KeyId, i64)], b: &[(KeyId, i64)]) -> Vec<(KeyId, i64)> {
    let mut keys: Vec<KeyId> = a.iter().map(|x| x.0).collect();
    for (k, _) in b {
        if !keys.contains(k) {
            keys.push(*k);
        }
    }
    let get = |s: &[(KeyId, i64)], k: KeyId| s.iter().find(|x| x.0 == k).map(|x| x.1).unwrap_or(0);
    keys.into_iter()
        .map(|k| {
            let (x, y) = (get(a, k), get(b, k));
            (k, match op {
                '+' => x + y,
                '-' => x - y,
                '&' => x.min(y),
                _ => x.max(y),
            })
        })
        .filter(|(_, n)| *n > 0)
        .collect()
}

/// `a + b`, `a - b`, `a & b`, `a | b` with a Counter operand: Some(result).
pub fn binop(an: &mut Analyzer, st: &mut State, op: char, a: &Value, b: &Value, node: &Node) -> Option<Value> {
    let is_counter = |an: &Analyzer, v: &Value| !v.pts.is_empty() && v.pts.iter().all(|o| an.m.counters.contains(&o));
    if !(is_counter(an, a) && is_counter(an, b)) {
        return None;
    }
    let ca = a.single().and_then(|o| counts(an, o, st));
    let cb = b.single().and_then(|o| counts(an, o, st));
    if let (Some(x), Some(y)) = (ca, cb) {
        return Some(make(an, st, node, &combine(op, &x, &y), &format!("counterop{}", op)));
    }
    // unknown counts: the keys that may be there (a `-` / `&` result's keys are a's)
    let (ka, oa) = maybe_keys(an, st, a, node);
    let (kb, ob) = maybe_keys(an, st, b, node);
    let (present, open) = match op {
        '-' | '&' => (ka, oa),
        _ => {
            let mut k = ka;
            k.extend(kb);
            k.sort();
            k.dedup();
            (k, oa || ob)
        }
    };
    Some(make_unknown(an, st, node, &present, &[], open, &format!("counterop{}", op)))
}

/// Unary `+c` / `-c`: the positive counts (of c / of -c).
pub fn unary(an: &mut Analyzer, st: &mut State, neg: bool, v: &Value, node: &Node) -> Option<Value> {
    let o = v.single().filter(|o| an.m.counters.contains(o))?;
    match counts(an, o, st) {
        Some(cs) => {
            let r: Vec<(KeyId, i64)> = cs.into_iter().map(|(k, n)| (k, if neg { -n } else { n })).filter(|(_, n)| *n > 0).collect();
            Some(make(an, st, node, &r, if neg { "counterneg" } else { "counterpos" }))
        }
        None => {
            let (k, open) = maybe_keys(an, st, v, node);
            Some(make_unknown(an, st, node, &k, &[], open, "counterpos"))
        }
    }
}

/// Counter methods: `counter.<name>` (receiver: the Counter).
pub fn m_counter(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = c.qual.rsplit('.').next().unwrap_or("").to_string();
    let recv = c.recv.clone().unwrap_or_default();
    let Some(oid) = recv.single() else { return Value::data("counter") };
    match name.as_str() {
        "most_common" => {
            let n = c.arg(0, Some("n")).and_then(|v| v.kv.as_lit()).and_then(intern::key_as_int);
            match counts(an, oid, st) {
                Some(mut cs) => {
                    // by count, ties in insertion order (a stable sort)
                    cs.sort_by(|a, b| b.1.cmp(&a.1));
                    if let Some(n) = n {
                        cs.truncate(n.max(0) as usize);
                    }
                    let items: Vec<Value> = cs.iter().enumerate().map(|(i, (k, n))| an.alloc_positional_sfx(Kind::Tuple, &c.node, &format!("#mc{}", i), Some(vec![Value::lit(*k), Value::lit(intern::key_int(*n))]), Value::new(), crate::domain::DATA, st)).collect();
                    an.alloc_positional(Kind::List, &c.node, Some(items), Value::new(), crate::domain::DATA, st)
                }
                None => {
                    let row = st.row(oid, Ns::Item);
                    let kv = if row.open() { crate::domain::DATA } else { KeyVal::lits(row.present.clone()) };
                    let t = an.alloc_positional(Kind::Tuple, &c.node, Some(vec![Value::kv_only(kv), Value::data("count")]), Value::new(), crate::domain::DATA, st);
                    an.alloc_positional(Kind::List, &c.node, None, t, crate::domain::DATA, st)
                }
            }
        }
        "total" => match counts(an, oid, st) {
            Some(cs) => Value::lit(intern::key_int(cs.iter().map(|x| x.1).sum())),
            None => Value::data("total"),
        },
        "elements" => match counts(an, oid, st) {
            Some(cs) => {
                let items: Vec<Value> = cs.iter().flat_map(|(k, n)| std::iter::repeat(Value::lit(*k)).take((*n).max(0) as usize)).collect();
                if items.len() <= crate::interp::UNROLL_MAX {
                    an.alloc_positional(Kind::List, &c.node, Some(items), Value::new(), crate::domain::DATA, st)
                } else {
                    let ks = intern::KeySet::from_iter(cs.iter().map(|x| x.0));
                    an.alloc_positional(Kind::List, &c.node, None, Value::kv_only(KeyVal::lits(ks)), crate::domain::DATA, st)
                }
            }
            None => {
                let row = st.row(oid, Ns::Item);
                let kv = if row.open() { crate::domain::DATA } else { KeyVal::lits(row.present.clone()) };
                an.alloc_positional(Kind::List, &c.node, None, Value::kv_only(kv), crate::domain::DATA, st)
            }
        },
        "copy" => match counts(an, oid, st) {
            Some(cs) => make(an, st, &c.node, &cs, "countercopy"),
            None => {
                let v = Value::obj(an.copy_object(oid, &c.node, st, false, &mut IndexMap::new()));
                for o in v.pts.iter() {
                    an.m.counters.insert(o);
                }
                v
            }
        },
        "update" | "subtract" => {
            let strong = an.heap.obj(oid).single;
            let mut add: Option<Vec<(KeyId, i64)>> = Some(Vec::new());
            for a in c.args.iter() {
                match (contribution(an, st, a, &c.node), add.as_mut()) {
                    (Some(e), Some(acc)) => acc.extend(e),
                    _ => add = None,
                }
            }
            for (k, v) in c.kwargs.iter() {
                let kk = intern::key_str_lit(&intern::str_get(*k));
                match (v.kv.as_lit().filter(|_| v.pts.is_empty()).and_then(intern::key_as_int), add.as_mut()) {
                    (Some(n), Some(acc)) => acc.push((kk, n)),
                    _ => add = None,
                }
            }
            let sign = if name == "subtract" { -1 } else { 1 };
            match (counts(an, oid, st), add) {
                (Some(mut cur), Some(add)) if strong => {
                    for (k, n) in add {
                        match cur.iter_mut().find(|x| x.0 == k) {
                            Some(e) => e.1 += sign * n,
                            None => cur.push((k, sign * n)),
                        }
                    }
                    let mut row = st.row(oid, Ns::Item);
                    let mut order: Vec<KeyId> = Vec::new();
                    for (k, n) in &cur {
                        row = row.store(*k, true);
                        order.push(*k);
                        st.set_slot(oid, Ns::Item, *k, Value::lit(intern::key_int(*n)), true);
                    }
                    st.set_row(oid, Ns::Item, row.with_order(order));
                }
                (_, add) => {
                    // counts not known: the keys that may be added, the counts unknown
                    let mut row = st.row(oid, Ns::Item);
                    let present: Vec<KeyId> = row.present.iter().collect();
                    for k in present {
                        st.set_slot(oid, Ns::Item, k, Value::data("count"), strong);
                    }
                    match add {
                        Some(add) => {
                            for (k, _) in add {
                                row = row.store(k, strong);
                                st.set_slot(oid, Ns::Item, k, Value::data("count"), strong);
                            }
                        }
                        None => {
                            for a in c.args.iter() {
                                let (ks, open) = maybe_keys(an, st, a, &c.node);
                                for k in ks {
                                    row = row.store(k, false);
                                    st.set_slot(oid, Ns::Item, k, Value::data("count"), false);
                                }
                                if open {
                                    row = row.data_store(Source::new(&format!("Counter.{}@L{}", name, c.line), SourceKind::Data));
                                }
                            }
                        }
                    }
                    st.set_row(oid, Ns::Item, row);
                }
            }
            an.none_v.clone()
        }
        _ => {
            let _ = join_values(Vec::<Value>::new());
            Value::data("counter")
        }
    }
}

// ------------------------------------------------------------------ collections.deque

/// deque methods (attribute loads on a deque route here; the rest is the list model).
pub const DEQUE_METHODS: &[&str] = &["append", "appendleft", "pop", "popleft", "extend", "extendleft", "rotate", "clear", "maxlen", "copy"];

/// The known items of a list-like object (positions of a single object), or None.
fn known_items(an: &Analyzer, o: ObjId, st: &State) -> Option<Vec<Value>> {
    let n = an.heap.llen(o, st)?;
    Some((0..n).map(|i| st.slot(o, Ns::Pos, intern::key_int(i as i64))).collect())
}

fn set_items(an: &mut Analyzer, o: ObjId, items: Vec<Value>, st: &mut State) {
    st.drop_obj_ns(o, Ns::Pos);
    let all = join_values(items.clone());
    for (i, v) in items.iter().enumerate() {
        st.set_slot(o, Ns::Pos, intern::key_int(i as i64), v.clone(), true);
    }
    an.heap.set_llen(o, st, Some(items.len()));
    st.set_slot(o, Ns::Elem, intern::STAR, Value::with_kv(all.pts.clone(), crate::models::torch::elem_kv(&all.kv)), true);
    st.elems.insert(o, if items.is_empty() { KeyVal::Lits(intern::KeySet::new()) } else { all.kv.clone() });
}

/// `deque(iterable=(), maxlen=None)`.
pub fn deque_new(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let maxlen = c.arg(1, Some("maxlen")).and_then(|v| v.kv.as_lit().filter(|_| v.pts.is_empty())).and_then(intern::key_as_int);
    let oid = an.alloc_at(Kind::List, &c.node, st, &format!("deque@L{}", c.line), Payload::None, None, Ns::Item, None, None);
    an.m.deques.insert(oid, maxlen);
    let items = match c.arg(0, Some("iterable")) {
        Some(it) => an.iterate(&it, &c.node, st).items,
        None => Some(Vec::new()),
    };
    match items {
        Some(mut items) if an.heap.obj(oid).single => {
            if let Some(m) = maxlen {
                let m = m.max(0) as usize;
                if items.len() > m {
                    items = items.split_off(items.len() - m);
                }
            }
            set_items(an, oid, items, st);
        }
        _ => {
            let e = c.arg(0, Some("iterable")).map(|it| { let it_ = an.iterate(&it, &c.node, st); an.elem_value(&it_.elem, &c.node, "el") }).unwrap_or_default();
            st.set_slot(oid, Ns::Elem, intern::STAR, Value::with_kv(e.pts.clone(), crate::models::torch::elem_kv(&e.kv)), false);
            st.elems.insert(oid, e.kv.clone());
            an.heap.set_llen(oid, st, None);
        }
    }
    Value::obj(oid)
}

/// deque methods: `deque.<name>`.
pub fn m_deque(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = c.qual.rsplit('.').next().unwrap_or("").to_string();
    let Some(oid) = c.recv.as_ref().and_then(|r| r.single()) else { return Value::data("deque") };
    let maxlen = an.m.deques.get(&oid).copied().flatten();
    let strong = an.heap.obj(oid).single && an.frame().loop_depth == 0 && an.cx.cond_depth == 0;
    let cur = known_items(an, oid, st).filter(|_| strong);
    let arg = c.args.first().cloned().unwrap_or_default();
    let fit = |mut items: Vec<Value>, left: bool| -> Vec<Value> {
        if let Some(m) = maxlen {
            let m = m.max(0) as usize;
            while items.len() > m {
                if left {
                    items.pop();
                } else {
                    items.remove(0);
                }
            }
        }
        items
    };
    match (name.as_str(), cur) {
        ("maxlen", _) => match maxlen {
            Some(m) => Value::lit(intern::key_int(m)),
            None => an.none_v.clone(),
        },
        ("append", Some(mut items)) => {
            items.push(arg);
            let items = fit(items, false);
            set_items(an, oid, items, st);
            an.none_v.clone()
        }
        ("appendleft", Some(mut items)) => {
            items.insert(0, arg);
            let items = fit(items, true);
            set_items(an, oid, items, st);
            an.none_v.clone()
        }
        ("extend" | "extendleft", Some(mut items)) => {
            match an.iterate(&arg, &c.node, st).items {
                Some(new) => {
                    if name == "extend" {
                        items.extend(new);
                        items = fit(items, false);
                    } else {
                        for x in new {
                            items.insert(0, x);
                        }
                        items = fit(items, true);
                    }
                    set_items(an, oid, items, st);
                }
                None => forget(an, oid, &arg, c, st),
            }
            an.none_v.clone()
        }
        ("pop" | "popleft", Some(mut items)) => {
            if items.is_empty() {
                an.raise_here("IndexError", st);
                return Value::new();
            }
            let v = if name == "pop" { items.pop().unwrap() } else { items.remove(0) };
            set_items(an, oid, items, st);
            v
        }
        ("rotate", Some(items)) => {
            let n = c.args.first().and_then(|v| v.kv.as_lit()).and_then(intern::key_as_int).unwrap_or(1);
            if !items.is_empty() {
                let len = items.len() as i64;
                let k = ((n % len) + len) % len;
                let mut r = items.clone();
                r.rotate_right(k as usize);
                set_items(an, oid, r, st);
            }
            an.none_v.clone()
        }
        ("clear", _) => {
            set_items(an, oid, Vec::new(), st);
            an.none_v.clone()
        }
        ("copy", Some(items)) => {
            let nid = an.alloc_at(Kind::List, &c.node, st, &format!("deque@L{}", c.line), Payload::None, None, Ns::Item, None, Some("dequecopy"));
            an.m.deques.insert(nid, maxlen);
            set_items(an, nid, items, st);
            Value::obj(nid)
        }
        ("pop" | "popleft", None) => {
            an.may_raise_here("IndexError", st);
            let e = st.slot(oid, Ns::Elem, intern::STAR);
            an.heap.set_llen(oid, st, None);
            st.drop_obj_ns(oid, Ns::Pos);
            e
        }
        (_, None) => {
            forget(an, oid, &arg, c, st);
            an.none_v.clone()
        }
        _ => Value::data("deque"),
    }
}

/// Positions no longer known: the elements it may hold.
fn forget(an: &mut Analyzer, oid: ObjId, arg: &Value, c: &Call, st: &mut State) {
    let name = c.qual.rsplit('.').next().unwrap_or("");
    let e = if matches!(name, "extend" | "extendleft") { { let it_ = an.iterate(arg, &c.node, st); an.elem_value(&it_.elem, &c.node, "el") } } else { arg.clone() };
    let cur = st.slot(oid, Ns::Elem, intern::STAR);
    st.set_slot(oid, Ns::Elem, intern::STAR, cur.join(&Value::with_kv(e.pts.clone(), crate::models::torch::elem_kv(&e.kv))), false);
    let kv = crate::domain::kv_join(&st.elems.get(&oid).unwrap_or(KeyVal::Bot), &e.kv);
    st.elems.insert(oid, kv);
    an.heap.set_llen(oid, st, None);
    st.drop_obj_ns(oid, Ns::Pos);
}

/// `dd[k]` / `counter[k]`: a defaultdict inserts its factory's value for a key it lacks (a
/// fresh one per key); a Counter reads a missing key as 0 without inserting it. Returns the
/// row to put back after the (never failing) read: a Counter's, or a weak insertion's.
pub fn default_on_read(an: &mut Analyzer, oid: ObjId, key: &Value, node: &Node, single_target: bool, state: &mut State) -> Option<Row> {
    let ns = Ns::Item;
    let mut counter_restore: Option<Row> = None;
    // `dd[k]` inserts the factory's value when `k` is absent (a fresh one per key)
    let row = state.row(oid, ns);
    if an.heap.obj(oid).label.starts_with("Counter@") {
        counter_restore = Some(row.clone());
    }
    if key.kv.is_literal() {
        let ks = crate::domain::kv_literals(&key.kv);
        let strong = single_target && ks.len() == 1 && an.strongable(oid);
        let mut r = row.clone();
        for k in ks.iter() {
            if !r.definite.contains(k) {
                let dflt = match an.m.dd_factories.get(&oid).cloned() {
                    Some(f) => an.call_value(&f, node, None, state, Some(vec![])),
                    None => state.slot(oid, Ns::Item, intern::STAR),
                };
                // (a key the closed row surely lacks: no earlier value to keep; in an
                // open one a key not known may be there already, with any value)
                let absent = r.closed() && !r.present.contains(k);
                let dflt = if r.open() && !r.present.contains(k) && state.slot_opt(oid, ns, k).is_none() { dflt.join(&Value::data(&format!("{}[*]", an.heap.obj(oid).label))) } else { dflt };
                r = r.store(k, strong);
                state.set_slot(oid, ns, k, dflt, absent);
            }
        }
        // (a weak insertion — one of several keys, a weak object — still never
        // fails the read: it is made against the keys inserted, then the weak row
        // is put back)
        if !strong && counter_restore.is_none() {
            let mut sr = r.clone();
            for k in ks.iter() {
                if !sr.definite.contains(k) {
                    sr = sr.store(k, true);
                }
            }
            counter_restore = Some(r);
            state.set_row(oid, ns, sr);
        } else {
            state.set_row(oid, ns, r);
        }
    } else {
        // an unknown key: some key gets the factory's value
        if let Some(f) = an.m.dd_factories.get(&oid).cloned() {
            let dflt = an.call_value(&f, node, None, state, Some(vec![]));
            state.set_slot(oid, ns, intern::STAR, Value::with_kv(dflt.pts, crate::domain::DATA), false);
        }
        let src = Source::new(&an.heap.obj(oid).label, SourceKind::Data);
        state.set_row(oid, ns, row.data_store(src));
    }
    counter_restore
}
