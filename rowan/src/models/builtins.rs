//! The builtin functions and the `str` methods: `dict()`, `list()` and the other
//! constructors, `isinstance`, `getattr` / `setattr` / `hasattr`, `min` / `max`, `zip`,
//! `enumerate`, `next`, `map` / `filter`, `type`, `str` / `repr` / `int`, `len`, `sum`,
//! `sorted`'s order, `range`, `any` / `all`, and `str.*` on known and unknown text.

use super::*;

pub(super) fn m_dict(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    // (pairs from a number, None or a str: TypeError / ValueError)
    if let Some(src) = c.args.first() {
        match crate::models::containers::pairs_part(an, st, src) {
            None => return Value::new(),
            Some(v) if v.empty() => return m_dict(an, st, &Call { args: c.args[1..].to_vec(), ..c.clone() }),
            Some(v) if v != *src => return m_dict(an, st, &Call { args: [vec![v], c.args[1..].to_vec()].concat(), ..c.clone() }),
            _ => {}
        }
    }
    let oid = an.alloc_simple(Kind::Dict, &c.node, st, &format!("dict@L{}", c.line));
    if let Some(src) = c.args.first() {
        // a parameter: a mapping or pairs, decided per call site
        // (an instance of the program's class without library bases: its pairs, by its
        // `__iter__`; by `keys()` with one; neither, a TypeError)
        let plain = |an: &Analyzer, o: u32| an.heap.kind(o) == Kind::Instance && an.heap.instance_class(o).is_some_and(|c| an.hierarchy_closed(c) && an.heap.mro(c).iter().all(|x| an.heap.class_info_opt(*x).is_some_and(|ci| ci.lib.quals.iter().all(|q| q == "builtins.object"))));
        if !src.pts.is_empty() && src.kv.is_bot() && src.pts.iter().all(|o| plain(an, o) && an.heap.instance_class(o).is_some_and(|cl| ["__iter__", "__getitem__", "keys"].iter().all(|m| an.class_lookup(cl, m).is_none()))) {
            an.raise_here("TypeError", st);
            return Value::new();
        }
        let recs: Vec<u32> = an.record_objects(src).into_iter().filter(|o| !plain(an, *o)).collect();
        if !recs.is_empty() {
            an.copy_into(oid, Ns::Item, &Value::with_kv(crate::domain::ObjSet::from_vec(recs), src.kv.clone()), st, true, &KeySet::new());
            if src.pts.iter().any(|o| plain(an, o)) {
                let r = st.row(oid, Ns::Item).data_store(Source::new(&format!("dict@L{}", c.line), SourceKind::Data));
                st.set_row(oid, Ns::Item, r);
            }
        } else if let Some(items) = src.single().filter(|_| src.kv.is_bot()).and_then(|x| an.m.sorted_items.get(&x).cloned()) {
            // `dict(sorted(d.items()))`: d's keys (the sure ones sure) in sorted order
            let mut d = st.row(oid, Ns::Item).data();
            let mut order = d.order.to_vec();
            for (k, val, sure) in items.iter() {
                d.present = d.present.insert(*k);
                d.stored = d.stored.insert(*k);
                if *sure {
                    d.definite = d.definite.insert(*k);
                    d.likely = d.likely.insert(*k);
                }
                if !order.contains(k) {
                    order.push(*k);
                }
                st.set_slot(oid, Ns::Item, *k, val.clone(), true);
            }
            d.order = order.into();
            st.set_row(oid, Ns::Item, Row::from_data(d));
        } else {
            for x in src.pts.iter() {
                match an.heap.kind(x) {
                    Kind::Zip if an.cursor_of(x, st) == Some(0) => {
                        let Payload::Zip(ks, vs) = an.heap.obj(x).payload.clone() else { continue };
                        // both lengths known: the pairs zip makes (it stops at the shorter)
                        if let Some(items) = an.iterate(&Value::obj(x), &c.node, st).items {
                            for item in items {
                                let parts = an.unpack(&item, 2, st, &c.node);
                                an.store_key(oid, Ns::Item, &parts[0], &parts[1], true, &c.node, st);
                            }
                            continue;
                        }
                        let lits = an.lit_keys(&ks, st);
                        let vel = an.elem_of(&vs, &c.node, st);
                        // values that are one of several sequences of known lengths: the keys
                        // the shortest pairs are definite, those of the longer ones possible
                        let klist = an.iterate(&ks, &c.node, st).items.and_then(|its| its.iter().map(|v| v.kv.as_lit().filter(|_| v.pts.is_empty())).collect::<Option<Vec<KeyId>>>());
                        let lens: Option<Vec<usize>> = if vs.pts.len() > 1 && vs.kv.is_bot() { vs.pts.iter().map(|o| if matches!(an.heap.kind(o), Kind::List | Kind::Tuple) && !an.heap.obj(o).iterator { an.heap.llen(o, st) } else { None }).collect() } else { None };
                        if let (Some(kl), Some(lens)) = (klist, lens) {
                            let (lo, hi) = (lens.iter().copied().min().unwrap_or(0), lens.iter().copied().max().unwrap_or(0));
                            let mut row = st.row(oid, Ns::Item);
                            for (i, k) in kl.into_iter().enumerate().take(hi) {
                                row = row.store(k, i < lo);
                                st.set_slot(oid, Ns::Item, k, Value::with_kv(vel.pts.clone(), DATA), false);
                            }
                            st.set_row(oid, Ns::Item, row);
                        } else if let Some(lits) = lits {
                            // the values' length is unknown: the header/row idiom pairs every
                            // key (design case d30), so the keys are definite
                            let mut row = st.row(oid, Ns::Item);
                            for k in lits.iter() {
                                row = row.store(k, true);
                                st.set_slot(oid, Ns::Item, k, Value::with_kv(vel.pts.clone(), DATA), false);
                            }
                            st.set_row(oid, Ns::Item, row);
                        } else {
                            let kel = an.elem_of(&ks, &c.node, st);
                            an.store_key(oid, Ns::Item, &kel, &vel, false, &c.node, st);
                        }
                    }
                    Kind::List | Kind::Tuple | Kind::Set | Kind::View | Kind::Enumerate | Kind::Zip | Kind::Instance if an.heap.kind(x) != Kind::Instance || an.heap.instance_class(x).is_some_and(|cl| an.class_lookup(cl, "keys").is_none()) => {
                        let it = an.iterate(&Value::obj(x), &c.node, st);
                        match (&it.elem, &it.items) {
                            (_, Some(items)) => {
                                for item in items {
                                    let parts = an.unpack(item, 2, st, &c.node);
                                    an.store_key(oid, Ns::Item, &parts[0], &parts[1], true, &c.node, st);
                                }
                            }
                            (Elem::Pair(k, v), None) => an.store_key(oid, Ns::Item, &k, v, false, &c.node, st),
                            _ => {
                                let r = st.row(oid, Ns::Item).data_store(Source::new(&format!("dict@L{}", c.line), SourceKind::Data));
                                st.set_row(oid, Ns::Item, r);
                            }
                        }
                    }
                    _ => {
                        let r = st.row(oid, Ns::Item).data_store(Source::new(&format!("dict@L{}", c.line), SourceKind::Data));
                        st.set_row(oid, Ns::Item, r);
                    }
                }
            }
            if src.pts.is_empty() {
                let r = st.row(oid, Ns::Item).data_store(Source::new(&format!("dict@L{}", c.line), SourceKind::Data));
                st.set_row(oid, Ns::Item, r);
            }
        }
    }
    for (k, v) in c.kwargs.iter() {
        an.store_key(oid, Ns::Item, &Value::lit(intern::key_of_str(*k)), v, true, &c.node, st);
    }
    for sk in &c.star_kwargs {
        an.copy_into(oid, Ns::Item, sk, st, true, &KeySet::new());
    }
    Value::obj(oid)
}

pub(super) fn m_fromkeys(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let oid = an.alloc_simple(Kind::Dict, &c.node, st, &format!("dict.fromkeys@L{}", c.line));
    let val = c.args.get(1).cloned().unwrap_or_else(|| Value::none());
    let Some(a) = c.args.first() else { return Value::obj(oid) };
    // known items that are literals, in order (a list, a string's characters, a generator)
    let it = an.iterate(a, &c.node, st);
    let lits: Option<Vec<KeyId>> = it.any_order_items().and_then(|its| its.iter().map(|v| v.kv.as_lit().filter(|_| v.pts.is_empty())).collect());
    let mut row = st.row(oid, Ns::Item);
    if let Some(ls) = lits {
        for k in ls {
            row = row.store(k, true);
            st.set_slot(oid, Ns::Item, k, val.clone(), true);
        }
        // (a set's items: in an order Python does not promise)
        if it.unordered {
            row = row.with_order(Vec::new());
        }
        st.set_row(oid, Ns::Item, row);
    } else if let Some(ks) = if a.pts.is_empty() { None } else { an.lit_keys(a, st) } {
        // the literals the iterable may hold (how many is not known): possible keys
        for k in ks.iter() {
            row = row.store(k, false);
            st.set_slot(oid, Ns::Item, k, val.clone(), true);
        }
        st.set_row(oid, Ns::Item, row);
    } else {
        let el = an.elem_of(a, &c.node, st);
        an.store_key(oid, Ns::Item, &el, &val, false, &c.node, st);
    }
    Value::obj(oid)
}

pub(super) fn m_list(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    if an.m.torch_on && matches!(c.qual.as_str(), "builtins.tuple" | "builtins.list") {
        if let Some(a) = c.args.first() {
            if crate::models::torch::size_shape(a).is_some() {
                return a.clone();
            }
        }
    }
    // `iter(it)` of an iterator (a generator, another `iter`'s) is `it`: none of a generator's
    // body runs before its items are taken
    if let (true, [a]) = (c.qual == "builtins.iter" && c.kwargs.is_empty(), c.args.as_slice()) {
        let is_it = |an: &Analyzer, o: ObjId| an.heap.obj(o).iterator && an.heap.kind(o) == Kind::List;
        let its = a.pts.filter(|o| is_it(an, o));
        if !its.is_empty() {
            let rest = Value { pts: a.pts.filter(|o| !is_it(an, o)), kv: a.kv.clone() };
            if rest.pts.is_empty() && rest.kv.is_bot() {
                return a.clone();
            }
            let mut c2 = c.clone();
            c2.args = vec![rest];
            return m_list(an, st, &c2).join(&Value::objs(its));
        }
    }
    let kind = match c.qual.as_str() {
        "builtins.tuple" => Kind::Tuple,
        "builtins.set" | "builtins.frozenset" => Kind::Set,
        _ => Kind::List,
    };
    let oid = an.alloc_simple(kind, &c.node, st, &format!("{}@L{}", last(&c.qual), c.line));
    an.heap.set_llen(oid, st, None);
    an.m.sorted_items.remove(&oid);
    if c.qual == "builtins.sorted" && c.kw("key").is_none() && an.heap.obj(oid).single {
        if let Some(items) = c.args.first().and_then(|a| sorted_dict_items(an, st, a, c)) {
            an.m.sorted_items.insert(oid, std::rc::Rc::new(items));
        }
    }
    let src = c.args.first().map(|a| an.iterable_part(a, st));
    fill_seq(an, st, c, oid, kind, src.as_ref());
    match (c.qual.as_str(), &src) {
        // (one-shot; reading the container as it is at each step)
        ("builtins.iter" | "builtins.reversed", Some(a)) => {
            an.start_iterator(oid, st);
            live_from(an, st, c, oid, a);
        }
        (_, Some(a)) => an.exhaust(a, st),
        _ => {}
    }
    Value::obj(oid)
}

/// The items of `src` (`list(src)`, `iter(src)`, …: `c`) written into `oid`, made for it or a
/// live iterator taking them again (`refill_live`).
fn fill_seq(an: &mut Analyzer, st: &mut State, c: &Call, oid: ObjId, kind: Kind, src: Option<&Value>) {
    if let Some(a) = src {
        let it = an.iterate(a, &c.node, st);
        if kind == Kind::Set {
            match &it.items {
                Some(items) => items.iter().for_each(|x| an.check_hashable(x, st)),
                None => an.check_hashable_if(&it.elem.flat(), false, st),
            }
        }
        match &it.elem {
            Elem::Pair(k, v) => {
                let ctx = an.frame().ctx.clone();
                let pid = an.heap.alloc_raw(Kind::Pair, &format!("pair:{}", c.site), &ctx, true, "pair", Payload::Pair(k.clone(), v.clone()));
                st.set_slot(oid, Ns::Elem, STAR, Value::obj(pid), true);
                st.elems.insert(oid, DATA);
                // `sorted(d.items(), key=f)` with unknown items: f reads every pair
                if c.qual == "builtins.sorted" && it.items.is_none() {
                    if let Some(keyfn) = c.kw("key") {
                        if !keyfn.pts.is_empty() {
                            an.call_value(&keyfn, &c.node, None, st, Some(vec![Value::obj(pid)]));
                        }
                    }
                }
                // known pairs (zip / enumerate / items of known inputs) keep their positions;
                // sorted by known literal keys, in that order
                let plain = matches!(c.qual.as_str(), "builtins.list" | "builtins.tuple" | "builtins.iter" | "builtins.reversed" | "builtins.sorted");
                if let (true, Some(items)) = (plain, &it.items) {
                    if an.heap.obj(oid).single && items.len() <= 64 {
                        let order: Option<Vec<usize>> = match c.qual.as_str() {
                            "builtins.sorted" => literal_sort_order(an, st, c, items),
                            "builtins.reversed" => Some((0..items.len()).rev().collect()),
                            _ => Some((0..items.len()).collect()),
                        };
                        if order.is_none() && c.qual == "builtins.sorted" {
                            // (items in no known order may not compare: a TypeError)
                            an.may_raise_here("TypeError", st);
                        }
                        if let Some(order) = order {
                            an.heap.set_llen(oid, st, Some(items.len()));
                            for (i, j) in order.into_iter().enumerate() {
                                st.set_slot(oid, Ns::Pos, intern::key_int(i as i64), items[j].clone(), true);
                            }
                        } else {
                            an.heap.set_llen(oid, st, Some(items.len()));
                            let all = join_values(items.clone());
                            for i in 0..items.len() {
                                st.set_slot(oid, Ns::Pos, intern::key_int(i as i64), all.clone(), true);
                            }
                        }
                    }
                }
            }
            Elem::One(elem) => {
                st.set_slot(oid, Ns::Elem, STAR, Value::with_kv(elem.pts.clone(), crate::models::torch::elem_kv(&elem.kv)), true);
                // (every key of a dict: `list(d)` holds them all)
                let kv = match &elem.kv {
                    KeyVal::KeysOf(ko) if it.exhaustive => KeyVal::KeysOf(Box::new(crate::domain::KeysOf { exhaustive_depth: crate::domain::ALL_KEYS, ..(**ko).clone() })),
                    kv => kv.clone(),
                };
                st.elems.insert(oid, kv);
                let reorders = matches!(c.qual.as_str(), "builtins.sorted" | "builtins.reversed");
                if reorders {
                    if let Some(keyfn) = c.kw("key") {
                        if !keyfn.pts.is_empty() {
                            an.call_value(&keyfn, &c.node, None, st, Some(vec![elem.clone()]));
                        }
                    }
                }
                // (a set of known literals, `set(text)`: its distinct items, however many the
                // iterable had)
                let text_chars: Option<Vec<Value>> = a.kv.as_lit().and_then(intern::key_as_str).filter(|_| a.pts.is_empty()).map(|t| t.chars().map(|ch| Value::lit(key_str_lit(&ch.to_string()))).collect());
                let distinct: Option<Vec<Value>> = it.any_order_items().or(text_chars.as_ref()).filter(|_| kind == Kind::Set).and_then(|items| {
                    let mut seen: Vec<KeyId> = Vec::new();
                    for v in items {
                        match (&v.kv, v.pts.is_empty()) {
                            (KeyVal::Lit(k), true) if !seen.contains(k) => seen.push(*k),
                            (KeyVal::Lit(_), true) => {}
                            _ => return None,
                        }
                    }
                    Some(seen.into_iter().map(Value::lit).collect())
                });
                let sorts = c.qual == "builtins.sorted";
                if let Some(items) = distinct.as_ref().or(if sorts { it.any_order_items() } else { it.items.as_ref() }) {
                    let reversed = c.qual.as_str() == "builtins.reversed";
                    let sorted = c.qual.as_str() == "builtins.sorted";
                    if an.heap.obj(oid).single && items.len() <= 64 {
                        an.heap.set_llen(oid, st, Some(items.len()));
                        let n = items.len();
                        // sorted: when every sort key is a known literal (or a tuple of
                        // them) the order is known (stable sort); otherwise the count is
                        // known and every position may hold any item
                        let order: Option<Vec<usize>> = if sorted { literal_sort_order(an, st, c, items) } else { None };
                        if sorted && order.is_none() {
                            an.may_raise_here("TypeError", st);
                        }
                        let all = join_values(items.clone());
                        for (i, item) in items.iter().enumerate() {
                            let pos = if reversed { n - 1 - i } else { i };
                            let v = if sorted {
                                match &order {
                                    Some(ord) => items[ord[i]].clone(),
                                    None => all.clone(),
                                }
                            } else {
                                item.clone()
                            };
                            st.set_slot(oid, Ns::Pos, intern::key_int(pos as i64), v, true);
                        }
                        // the items of a set come in an order Python does not promise
                        let from_set = it.unordered || a.pts.iter().any(|o| an.heap.kind(o) == Kind::Set);
                        if from_set && kind != Kind::Set && !sorted {
                            for i in 0..n {
                                st.set_slot(oid, Ns::Pos, intern::key_int(i as i64), all.clone(), true);
                            }
                        }
                        an.normalize_set(oid, st);
                    }
                }
            }
        }
        // a dict with some keys only maybe there: the sure keys before the first maybe one
        // are the first items (the length is not known)
        if matches!(c.qual.as_str(), "builtins.list" | "builtins.tuple" | "builtins.iter") && an.heap.llen(oid, st).is_none() && an.heap.obj(oid).single {
            if let Some(m) = an.maybe_dict_items(a, st) {
                let lead: Vec<Value> = m.iter().take_while(|(_, sure)| *sure).map(|(v, _)| v.clone()).collect();
                if !lead.is_empty() {
                    for (i, v) in lead.iter().enumerate() {
                        st.set_slot(oid, Ns::Pos, intern::key_int(i as i64), v.clone(), true);
                    }
                    st.set_lens(oid, (None, lead.len(), 0));
                }
            }
        }
    } else {
        st.elems.insert(oid, KeyVal::Lits(KeySet::new()));
        an.heap.set_llen(oid, st, Some(0));
    }
    // (a copy of a list / tuple whose length is one of a few: the same alternatives, each
    // with its positions — reversed, from each one's end)
    // (a one-shot iterator standing where it differs by path: as many items as are left on each,
    // `list(it)` after a `next(it)` on one branch)
    if matches!(c.qual.as_str(), "builtins.list" | "builtins.tuple" | "builtins.iter") && an.heap.llen(oid, st).is_none() && an.heap.obj(oid).single {
        if let Some((o, n, alts)) = src.and_then(|a| a.single().filter(|_| a.kv.is_bot())).filter(|o| an.heap.kind(*o) == Kind::List && an.heap.obj(*o).iterator).and_then(|o| Some((o, an.heap.llen(o, st)?, an.cursor_alternatives(o, st)?))) {
            if alts.len() > 1 && n <= 15 {
                let mask = alts.iter().fold(0u16, |m, c| m | (1 << (n - c)));
                for j in 0..n - alts[0] {
                    let v = join_values(alts.iter().filter(|c| *c + j < n).map(|c| st.slot(o, Ns::Pos, intern::key_int((c + j) as i64))).collect::<Vec<_>>());
                    st.set_slot(oid, Ns::Pos, intern::key_int(j as i64), v, true);
                }
                st.set_lens(oid, (None, 0, mask));
            }
        }
    }
    // (an iterator's from its start only)
    if matches!(c.qual.as_str(), "builtins.list" | "builtins.tuple" | "builtins.iter" | "builtins.reversed") && an.heap.llen(oid, st).is_none() {
        if let Some(src) = src.and_then(|a| a.single()).filter(|s| matches!(an.heap.kind(*s), Kind::List | Kind::Tuple) && (!an.heap.obj(*s).iterator || an.cursor_of(*s, st) == Some(0))) {
            if let (Some((None, _, m)), Some(alts)) = (st.lens.get(&src).filter(|l| l.2 != 0), an.heap.len_alts(src, st)) {
                let longest = alts.iter().copied().max().unwrap_or(0);
                let reversed = c.qual == "builtins.reversed";
                for i in 0..longest {
                    let at = |len: usize| if reversed { len - 1 - i } else { i };
                    let v = join_values(alts.iter().filter(|l| **l > i).map(|l| st.slot(src, Ns::Pos, intern::key_int(at(*l) as i64))).collect::<Vec<_>>());
                    st.set_slot(oid, Ns::Pos, intern::key_int(i as i64), v, true);
                }
                st.set_lens(oid, (None, 0, m));
            }
        }
    }
}

/// `oid`, an iterator over `a` (`c`), reads the containers among it as they are at each step
/// (`Analyzer::sync_live`); one over a tuple, a str or unknown values has no other items.
fn live_from(an: &mut Analyzer, st: &mut State, c: &Call, oid: ObjId, a: &Value) {
    if !a.pts.iter().any(|o| an.changing_container(o)) {
        return;
    }
    let made = Call { args: Vec::new(), kwargs: IndexMap::new(), star_args: Vec::new(), star_kwargs: Vec::new(), recv: None, ..c.clone() };
    an.m.live_iters.insert(oid, Rc::new(LiveIter { call: made, rerun: None }));
    an.watch_live(oid, a, st);
}

/// Lazy iterators of several inputs (`zip`, `islice`, `chain`, …) made by model `m`: live
/// iterators made again from their inputs as they are, where a container among them changed.
pub(crate) fn live_after_model(an: &mut Analyzer, m: Model, call: &Call, r: &Value, st: &mut State) {
    const RERUN: &[&str] = &[
        "builtins.zip", "builtins.map", "itertools.islice", "itertools.chain", "itertools.chain.from_iterable", "itertools.starmap", "itertools.takewhile",
        "itertools.dropwhile", "itertools.accumulate", "itertools.compress", "itertools.pairwise", "itertools.batched", "itertools.filterfalse", "itertools.zip_longest",
    ];
    if !RERUN.contains(&call.qual.as_str()) {
        return;
    }
    // (made at this site before, in another run: live on this path too, its containers as here)
    let Some(o) = r.single().filter(|o| an.heap.kind(*o) == Kind::List && an.m.live_iters.get(o).is_none_or(|l| l.rerun.is_some())) else { return };
    let args: Vec<Value> = call.args.iter().chain(call.star_args.iter()).chain(call.kwargs.values()).cloned().collect();
    if !args.iter().any(|a| a.pts.iter().any(|x| an.changing_container(x))) {
        return;
    }
    an.m.live_iters.insert(o, Rc::new(LiveIter { call: call.clone(), rerun: Some(m) }));
    an.watch_live(o, &join_values(args), st);
}

/// A live iterator `o` (`live`) takes its items again from `srcs` as they are now.
pub fn refill_live(an: &mut Analyzer, st: &mut State, o: ObjId, live: &LiveIter, srcs: &Value) {
    let c = &live.call;
    st.drop_obj_ns(o, Ns::Pos);
    st.drop_obj_ns(o, Ns::Elem);
    an.heap.set_llen(o, st, None);
    // (made again, at its own site: its items copied)
    if let Some(m) = live.rerun {
        let v = m(an, st, c);
        match v.single().filter(|n| *n != o) {
            Some(n) => {
                for ((ns, k), x) in st.slots_of(n) {
                    if matches!(ns, Ns::Pos | Ns::Elem) {
                        st.set_slot(o, ns, k, x, true);
                    }
                }
                if let Some(e) = st.elems.get(&n) {
                    st.elems.insert(o, e);
                }
                if let Some(l) = st.lens.get(&n) {
                    st.set_lens(o, l);
                }
            }
            None => {
                let it = an.iterate(&v, &c.node, st);
                let e = an.elem_value(&it.elem, &c.node, "live");
                an.add_elem(o, &e, st);
            }
        }
        return;
    }
    let as_iter = |srcs: &Value| Call { qual: "builtins.iter".into(), args: vec![srcs.clone()], ..c.clone() };
    match c.qual.as_str() {
        "builtins.map" | "builtins.filter" => {
            let f = st.slot(o, Ns::Attr, key_str_lit("\u{1}fn"));
            // (the items taken already are not computed again: no call for them)
            let from = if c.qual.ends_with("map") { an.cursor_of(o, st).unwrap_or(0) } else { 0 };
            fill_map(an, st, &Call { args: vec![f, srcs.clone()], ..c.clone() }, o, from);
        }
        // (a list's: from the index it started at, `n0` - 1, down; an index past the list's
        // end now ends it)
        "builtins.reversed" if srcs.single().is_some_and(|l| an.heap.kind(l) == Kind::List) => {
            fill_seq(an, st, &as_iter(srcs), o, Kind::List, Some(srcs));
            let n0 = st.slot_opt(o, Ns::Attr, key_str_lit("\u{1}n0")).and_then(|v| v.kv.as_lit()).and_then(intern::key_as_int).map(|n| n.max(0) as usize);
            match (n0, an.heap.llen(o, st)) {
                (Some(n0), Some(m)) => {
                    let fwd: Vec<Value> = (0..m).map(|i| st.slot(o, Ns::Pos, intern::key_int(i as i64))).collect();
                    st.drop_obj_ns(o, Ns::Pos);
                    for j in 0..n0 {
                        if let Some(v) = fwd.get(n0 - 1 - j) {
                            st.set_slot(o, Ns::Pos, intern::key_int(j as i64), v.clone(), true);
                        }
                    }
                    an.heap.set_llen(o, st, Some(n0));
                    match an.cursor_of(o, st) {
                        _ if m == 0 => an.end_iterator(o, st),
                        Some(cur) if m < n0 && cur < n0 - m => an.end_iterator(o, st),
                        // (where it stands not known: it may have run past the list's end)
                        None if m < n0 => {
                            st.drop_obj_ns(o, Ns::Pos);
                            an.heap.set_llen(o, st, None);
                        }
                        _ => {}
                    }
                }
                // (its element the list's)
                _ => {
                    st.drop_obj_ns(o, Ns::Pos);
                    an.heap.set_llen(o, st, None);
                }
            }
        }
        "builtins.reversed" => fill_seq(an, st, &Call { args: vec![srcs.clone()], ..c.clone() }, o, Kind::List, Some(srcs)),
        _ => fill_seq(an, st, &as_iter(srcs), o, Kind::List, Some(srcs)),
    }
}

/// `callable(x)`: decided where every value `x` may be is: functions, classes and bound
/// methods are, literals are not, an instance is when its class defines `__call__`.
pub(super) fn m_callable(an: &mut Analyzer, _st: &mut State, c: &Call) -> Value {
    let Some(v) = c.args.first() else { return Value::data("callable") };
    let mut seen: Vec<bool> = Vec::new();
    if !v.kv.is_bot() {
        if !v.kv.is_literal() || crate::domain::kv_literals(&v.kv).is_empty() {
            return Value::data("callable");
        }
        seen.push(false);
    }
    for o in v.pts.iter() {
        let b = match an.heap.kind(o) {
            Kind::Function | Kind::Bound | Kind::Class | Kind::Partial | Kind::Method => true,
            Kind::Instance => match an.heap.instance_class(o) {
                Some(cls) if !an.heap.mro(cls).iter().any(|x| an.heap.class_info_opt(*x).is_none_or(|ci| ci.open_base.get())) => an.class_lookup(cls, "__call__").is_some(),
                _ => return Value::data("callable"),
            },
            Kind::Dict | Kind::List | Kind::Tuple | Kind::Set => false,
            // (a builtin function or class; other library names may be constants)
            Kind::External if an.heap.external_qual(o).strip_prefix("builtins.").is_some_and(|n| crate::interp::is_builtin_name(n) && !n.starts_with("__") && !matches!(n, "NotImplemented" | "Ellipsis" | "None" | "True" | "False")) => true,
            _ => return Value::data("callable"),
        };
        seen.push(b);
    }
    match (seen.iter().all(|b| *b), seen.iter().all(|b| !*b)) {
        (true, false) => Value::lit(intern::key_bool(true)),
        (false, true) => Value::lit(intern::key_bool(false)),
        _ => Value::data("callable"),
    }
}

/// `isinstance(x, C)`: decided when every object of `x` has a known kind / class; a
/// placeholder records an event so the call site decides.
pub(super) fn m_isinstance(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let (Some(v), Some(cv)) = (c.args.first(), c.args.get(1)) else { return Value::data("bool") };
    // isinstance(x, torch.Tensor): decided for tensors and for containers / literals
    if an.m.torch_on {
        if let Some(q) = cv.single().filter(|o| an.heap.kind(*o) == Kind::External).map(|o| an.heap.external_qual(o)) {
            if q == "torch.Tensor" || q == "torch.nn.Parameter" || q == "torch.nn.parameter.Parameter" {
                let tensor = crate::models::torch::is_tensor(v);
                let not_tensor = !tensor
                    && !v.pts.is_empty()
                    && v.pts.iter().all(|o| matches!(an.heap.kind(o), Kind::List | Kind::Tuple | Kind::Dict | Kind::Set | Kind::Namespace | Kind::Function | Kind::Class) || (an.heap.kind(o) == Kind::Instance && !crate::models::torch::t_is_module_instance(an, o)))
                    && (v.kv.is_bot() || v.kv.is_literal())
                    || v.pts.is_empty() && v.kv.is_literal();
                if tensor && q == "torch.Tensor" {
                    return Value::lit(intern::key_bool(true));
                }
                if not_tensor {
                    return Value::lit(intern::key_bool(false));
                }
                return Value::data("bool");
            }
        }
    }
    let Some(classes) = an.isinstance_classes(cv, st) else { return Value::data("bool") };
    let builtin = |an: &Analyzer, cls: u32| -> Option<String> { if an.heap.kind(cls) == Kind::External { Some(an.heap.external_qual(cls).rsplit('.').next().unwrap_or("").to_string()) } else { None } };
    let mut verdicts: Vec<bool> = Vec::new();
    // (an unknown beside objects: of the program's classes it is no instance — unknown values
    // are not taken for the program's instances — of a builtin one perhaps, by its type)
    if !v.pts.is_empty() && !v.kv.is_bot() && !v.kv.is_literal() {
        let user = classes.iter().all(|c| an.heap.kind(*c) == Kind::Class);
        let ty = v.kv.data_site().and_then(|l| crate::domain::label_type(&intern::str_get(l)));
        match (user, ty) {
            (true, _) => verdicts.push(false),
            (false, Some(t)) if classes.iter().all(|c| builtin(an, *c).is_some()) => match typed_isinstance(an, t, &classes) {
                Some(b) => verdicts.push(b),
                None => return Value::data("bool"),
            },
            // (a library class's: decided by the objects, as for the program's classes)
            _ if classes.iter().all(|c| an.heap.kind(*c) != Kind::External || !an.heap.external_qual(*c).starts_with("builtins.")) => verdicts.push(false),
            _ => return Value::data("bool"),
        }
    }
    // a typed unknown alone (`str(?)`, `None | int(?)`): by its type
    if v.pts.is_empty() {
        if let Some(parts) = typed_parts(&v.kv) {
            for (_, kind) in parts {
                match typed_isinstance(an, kind, &classes) {
                    Some(b) => verdicts.push(b),
                    None => return Value::data("bool"),
                }
            }
        }
    }
    // literal values (with or without objects: both parts count)
    if (v.pts.is_empty() && typed_parts(&v.kv).is_none()) || (v.kv.is_literal() && !crate::domain::kv_literals(&v.kv).is_empty()) {
        let lits = crate::domain::kv_literals(&v.kv);
        if !v.kv.is_literal() || lits.is_empty() {
            return Value::data("bool");
        }
        for k in lits.iter() {
            match kind_isinstance(an, lit_kind_name(k), &classes) {
                Some(b) => verdicts.push(b),
                None => return Value::data("bool"),
            }
        }
    }
    for o in v.pts.iter() {
        match an.instance_of(o, &classes, st) {
            Some(b) => verdicts.push(b),
            None => return Value::data("bool"),
        }
    }
    if verdicts.iter().all(|b| *b) {
        Value::lit(intern::key_bool(true))
    } else if verdicts.iter().all(|b| !*b) {
        Value::lit(intern::key_bool(false))
    } else {
        Value::data("bool")
    }
}

/// The type of a literal as `type(k).__name__` says (`"?"`: none `kind_isinstance` knows).
fn lit_kind_name(k: KeyId) -> &'static str {
    intern::with_key(k, |kk| match kk {
        intern::Key::Str(_) => "str",
        intern::Key::Bool(_) => "bool",
        intern::Key::Int(_) => "int",
        intern::Key::Float(_) => "float",
        intern::Key::None => "NoneType",
        intern::Key::Tuple(_) => "tuple",
        intern::Key::Bytes(_) => "bytes",
        intern::Key::Dec(..) => "Decimal",
        intern::Key::Frac(..) => "Fraction",
        intern::Key::Frozen(_) => "frozenset",
        _ => "?",
    })
}

/// A typed unknown's parts and their types: `int(?)` an int; `None | int(?)` an int and None.
fn typed_parts(kv: &KeyVal) -> Option<Vec<(KeyVal, &'static str)>> {
    let KeyVal::Data(l, _) = kv else { return None };
    let (t, opt) = crate::domain::label_base(&intern::str_get(*l))?;
    let mut out = vec![(crate::domain::kv_not_none(kv), t)];
    if opt {
        out.push((KeyVal::Lit(intern::key_none()), "NoneType"));
    }
    Some(out)
}

/// Whether a value of the builtin type `kind` (as `type(x).__name__` says) is an instance of
/// one of `classes`; None when that is not known (an abstract class, a protocol, a library
/// class `kind_isinstance` does not know). Of the program's classes it is none.
fn kind_isinstance(an: &Analyzer, kind: &str, classes: &[u32]) -> Option<bool> {
    // (an abstract class like numbers.Number or collections.abc.Hashable: not decided)
    const KNOWN: &[&str] = &["str", "bool", "int", "float", "NoneType", "tuple", "bytes", "Decimal", "Fraction", "frozenset", "object", "list", "dict", "set", "complex", "bytearray", "type", "range", "defaultdict", "OrderedDict", "Counter", "deque"];
    let builtin = |cls: u32| -> Option<String> { if an.heap.kind(cls) == Kind::External { Some(an.heap.external_qual(cls).rsplit('.').next().unwrap_or("").to_string()) } else { None } };
    if kind == "?" || classes.iter().any(|cls| builtin(*cls).map(|b| !KNOWN.contains(&b.as_str())).unwrap_or(false) || protocol_members(an, *cls).is_some()) {
        return None;
    }
    Some(classes.iter().any(|cls| match builtin(*cls) {
        Some(b) => b == kind || (b == "int" && kind == "bool") || (b == "object"),
        None => false,
    }))
}

/// `kind_isinstance` for a typed unknown, which may be of a subclass: an `int(?)` a bool
/// (`key_type`), an annotated `str` a `StrEnum`'s member. Decided only for builtin classes.
pub fn typed_isinstance(an: &Analyzer, kind: &str, classes: &[u32]) -> Option<bool> {
    let builtin = |c: u32| an.heap.kind(c) == Kind::External && an.heap.external_qual(c).starts_with("builtins.");
    let names_bool = || classes.iter().any(|c| an.heap.external_qual(*c) == "builtins.bool");
    if !classes.iter().all(|c| builtin(*c)) || kind == "int" && names_bool() {
        return None;
    }
    kind_isinstance(an, kind, classes)
}

/// The scalar part of a value on the path where `isinstance(value, classes)` is `truth`: its
/// literals and typed parts (`None | int(?)`) of the other outcome left out; None when nothing
/// is known to go.
pub fn kv_isinstance_narrowed(an: &Analyzer, kv: &KeyVal, classes: &[u32], truth: bool) -> Option<KeyVal> {
    let typed = typed_parts(kv);
    let parts: Vec<(KeyVal, &'static str)> = match typed.clone() {
        Some(p) => p,
        None if kv.is_literal() => crate::domain::kv_literals(kv).iter().map(|k| (KeyVal::Lit(k), lit_kind_name(k))).collect(),
        None => return None,
    };
    let mut out = KeyVal::Bot;
    let mut dropped = false;
    for (part, kind) in parts {
        let verdict = if typed.is_some() && part.data_site().is_some() { typed_isinstance(an, kind, classes) } else { kind_isinstance(an, kind, classes) };
        if verdict == Some(!truth) {
            dropped = true;
        } else {
            out = crate::domain::kv_join(&out, &part);
        }
    }
    dropped.then_some(out)
}

/// `a op b` (an ordering or equality) where a library value decides it: pathlib paths are
/// equal when their texts are (never equal to a str); Decimal / Fraction / datetime literals
/// compare by value (`Decimal("1.0") == 1`).
pub fn library_compare(an: &Analyzer, op: ast::CmpOp, a: &Value, b: &Value) -> Option<Value> {
    let lit = |t: bool| Value::lit(intern::key_bool(t));
    if matches!(op, ast::CmpOp::Eq | ast::CmpOp::NotEq) {
        let pt = |v: &Value| v.single().filter(|_| v.kv.is_bot()).and_then(|o| crate::models::pathtext::path_text(an, o));
        let (pa, pb) = (pt(a), pt(b));
        let decided = match (&pa, &pb) {
            (Some(x), Some(y)) => Some(x == y),
            (Some(_), None) | (None, Some(_)) if (a.pts.is_empty() && a.kv.is_literal()) || (b.pts.is_empty() && b.kv.is_literal()) => Some(false),
            _ => None,
        };
        if let Some(eq) = decided {
            return Some(lit(if op == ast::CmpOp::Eq { eq } else { !eq }));
        }
    }
    let has_num = |v: &Value| v.kv.is_literal() && crate::domain::kv_literals(&v.kv).iter().any(|k| pynum::is_num_key(k) || chrono::is_chrono_key(k));
    if !(has_num(a) || has_num(b)) || !a.pts.is_empty() || !b.pts.is_empty() {
        return None;
    }
    let opn = match op {
        ast::CmpOp::Eq => "eq",
        ast::CmpOp::NotEq => "ne",
        ast::CmpOp::Lt => "lt",
        ast::CmpOp::LtE => "le",
        ast::CmpOp::Gt => "gt",
        _ => "ge",
    };
    let mut outs: Vec<bool> = Vec::new();
    for x in crate::domain::kv_literals(&a.kv).iter() {
        for y in crate::domain::kv_literals(&b.kv).iter() {
            match pynum::binop(opn, x, y).or_else(|| chrono::binop(opn, x, y)).flatten().and_then(|k| intern::with_key(k, |kk| if let intern::Key::Bool(t) = kk { Some(*t) } else { None })) {
                Some(t) => outs.push(t),
                None => return Some(Value::data("bool")),
            }
        }
    }
    outs.dedup();
    Some(if outs.len() == 1 { lit(outs[0]) } else { Value::data("bool") })
}

/// An attribute of a value without objects (a str, a number, a Decimal / Fraction / datetime,
/// None, a slice, an unknown scalar): its value, or the bound method.
pub fn scalar_attr(an: &mut Analyzer, obj: &Value, name: &str) -> Value {
    if name == "str" && matches!(obj.kv, KeyVal::KeysOf(_)) {
        // `df.columns.str.strip().str.lower()`: the accessor of renamed column names
        return obj.clone();
    }
    // a Decimal / Fraction: its attributes and methods
    let numlike = |k: KeyId| crate::models::pynum::is_num_key(k) || crate::models::chrono::is_chrono_key(k);
    if obj.kv.is_literal() && !crate::domain::kv_literals(&obj.kv).is_empty() && crate::domain::kv_literals(&obj.kv).iter().all(numlike) {
        let ks = crate::domain::kv_literals(&obj.kv);
        let attr_of = |k: KeyId, n: &str| if crate::models::chrono::is_chrono_key(k) { crate::models::chrono::attr(k, n) } else { crate::models::pynum::attr(k, n) };
        let mut out: Option<Vec<Value>> = Some(Vec::new());
        for k in ks.iter() {
            match (attr_of(k, name), out.as_mut()) {
                (Some(Some(v)), Some(o)) => o.push(Value::lit(v)),
                _ => out = None,
            }
        }
        if let Some(o) = out {
            return join_values(o);
        }
        if attr_of(ks.iter().next().unwrap(), name).is_some() {
            return Value::data(name);
        }
        return an.method_value(&format!("num.{}", name), obj);
    }
    if matches!(obj.kv, KeyVal::Lit(_) | KeyVal::Lits(_) | KeyVal::KeysOf(_)) {
        return an.method_value(&format!("str.{}", name), obj);
    }
    if let KeyVal::Data(d, _) = &obj.kv {
        let label = intern::str_get(*d);
        if matches!(&*label, "datetime" | "date" | "time" | "timedelta" | "timezone") && crate::models::chrono::NAMES.contains(&name) {
            return match crate::models::chrono::attr(crate::models::chrono::chrono_key(crate::models::chrono::DELTA, 0, crate::models::chrono::NAIVE), name) {
                _ if matches!(name, "year" | "month" | "day" | "hour" | "minute" | "second" | "microsecond" | "days" | "seconds" | "microseconds" | "tzinfo") => Value::data(name),
                _ => an.method_value(&format!("num.{}", name), obj),
            };
        }
        // a string whose text is not known (a validated `str` field, a str method's result)
        if (&*label == "str" || crate::domain::label_type(&label) == Some("str")) && crate::interp::STR_METHOD_NAMES.contains(&name) {
            return an.method_value(&format!("str.{}", name), obj);
        }
        if matches!(&*label, "Decimal" | "Fraction") && crate::models::pynum::NAMES.contains(&name) {
            return if matches!(name, "numerator" | "denominator") { Value::data(name) } else { an.method_value(&format!("num.{}", name), obj) };
        }
    }
    // a slice object with known bounds
    if let Some((lo, hi, st)) = Analyzer::slice_label_bounds(&obj.kv) {
        let lit = |b: Option<i64>| match b {
            Some(i) => Value::lit(intern::key_int(i)),
            None => Value::none(),
        };
        match name {
            "start" => return lit(lo),
            "stop" => return lit(hi),
            "step" => return lit(st),
            "indices" => return an.method_value("slice.indices", obj),
            _ => {}
        }
    }
    an.method_value(&format!("unknown.{}", name), obj)
}

/// The members of a `typing.Protocol` class (its own and its protocol bases'), as
/// `isinstance` checks them on a runtime-checkable one; None for any other class.
pub fn protocol_members(an: &Analyzer, cls: u32) -> Option<Vec<String>> {
    const SPECIAL: &[&str] = &["__init__", "__new__", "__module__", "__qualname__", "__doc__", "__dict__", "__weakref__", "__slots__", "__annotations__", "__class_getitem__", "__init_subclass__", "__subclasshook__", "__abstractmethods__", "__parameters__", "__orig_bases__", "__match_args__"];
    let ci = an.heap.class_info_opt(cls)?;
    // (`Protocol` / `typing.Protocol` / `Protocol[T]` among the bases as written)
    let is_proto = |e: &ast::Expr| match e {
        ast::Expr::Subscript(sb) => matches!(&*sb.value, ast::Expr::Name(n) if n.id.as_str() == "Protocol") || matches!(&*sb.value, ast::Expr::Attribute(a) if a.attr.as_str() == "Protocol"),
        ast::Expr::Name(n) => n.id.as_str() == "Protocol",
        ast::Expr::Attribute(a) => a.attr.as_str() == "Protocol",
        _ => false,
    };
    if !ci.node.bases().iter().any(is_proto) {
        return None;
    }
    let proto_bases: Vec<u32> = ci.bases.clone();
    let mut ms: Vec<String> = Vec::new();
    for st in ci.node.body.iter() {
        let name = match st {
            ast::Stmt::FunctionDef(f) => Some(f.name.id.to_string()),
            ast::Stmt::AnnAssign(a) => a.target.as_name_expr().map(|n| n.id.to_string()),
            ast::Stmt::Assign(a) => a.targets.first().and_then(|t| t.as_name_expr()).map(|n| n.id.to_string()),
            _ => None,
        };
        if let Some(n) = name.filter(|n| !SPECIAL.contains(&n.as_str()) && !n.starts_with("_abc_")) {
            ms.push(n);
        }
    }
    for b in proto_bases {
        ms.extend(protocol_members(an, b).unwrap_or_default());
    }
    Some(ms)
}

/// `max(xs, key=f)` / `min(xs)`: an element of the container (the key function runs on it);
/// with several arguments, any of them.
pub(super) fn m_minmax(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    if an.m.torch_on {
        if let Some(v) = crate::models::torch::t_minmax(an, c.qual.ends_with("max"), &c.args, st) {
            return v;
        }
    }
    let is_max = c.qual.ends_with("max");
    // known items (or several arguments) whose keys are known literals: the first extreme one
    let cands: Option<Vec<Value>> = if c.args.len() == 1 { an.iterate(&c.args[0], &c.node, st).items } else { Some(c.args.clone()) };
    // (one argument that may have no items: ValueError without a default)
    let may_be_empty = c.args.len() == 1 && !matches!(&cands, Some(its) if !its.is_empty()) && !surely_nonempty(an, &c.args[0], st);
    if c.args.len() == 1 {
        an.exhaust(&c.args[0], st);
    }
    let default = c.kw("default");
    // a known-empty iterable: the default, else ValueError
    if c.args.len() == 1 && matches!(&cands, Some(its) if its.is_empty()) && !c.args[0].empty() {
        return match default {
            Some(d) => d,
            None => {
                an.raise_here("ValueError", st);
                Value::new()
            }
        };
    }
    if let Some(items) = cands.filter(|its| !its.is_empty() && its.len() <= crate::interp::UNROLL_MAX) {
        let keyf = c.kw("key").filter(|k| !k.pts.is_empty());
        let mut keys: Vec<Option<KeyId>> = Vec::new();
        let mut kvals: Vec<Value> = Vec::new();
        for (i, item) in items.iter().enumerate() {
            let kv = match &keyf {
                Some(f) => {
                    an.frame_mut().unroll.push(i);
                    let r = an.call_value(f, &c.node, None, st, Some(vec![item.clone()]));
                    an.frame_mut().unroll.pop();
                    r
                }
                None => item.clone(),
            };
            keys.push(kv.kv.as_lit().filter(|_| kv.pts.is_empty()));
            kvals.push(kv);
        }
        // items that order themselves (`__lt__`): compared one against the best so far
        if keys.iter().any(|k| k.is_none()) && keyf.is_none() && items.iter().all(|x| x.single().map(|o| an.heap.kind(o) == Kind::Instance).unwrap_or(false)) {
            let mut best = 0usize;
            let mut ok = true;
            for i in 1..items.len() {
                let (x, y) = if is_max { (items[best].clone(), items[i].clone()) } else { (items[i].clone(), items[best].clone()) };
                // max: best < item → item; min: item < best → item
                match an.instance_less(&x, &y, &c.node, st) {
                    Some(true) => best = i,
                    Some(false) => {}
                    None => {
                        ok = false;
                        break;
                    }
                }
            }
            if ok {
                return items[best].clone();
            }
        }
        if keys.iter().all(|k| k.is_some()) {
            let mut best = 0usize;
            let mut ok = true;
            for i in 1..items.len() {
                match crate::interp::compare::literal_order(keys[i].unwrap(), keys[best].unwrap()) {
                    Some(std::cmp::Ordering::Greater) if is_max => best = i,
                    Some(std::cmp::Ordering::Less) if !is_max => best = i,
                    Some(_) => {}
                    None => {
                        ok = false;
                        break;
                    }
                }
            }
            if ok {
                return items[best].clone();
            }
        }
        // keys that are tuples / pairs of known items: compared item by item
        if keys.iter().any(|k| k.is_none()) {
            let mut best = 0usize;
            let mut ok = true;
            for i in 1..items.len() {
                match crate::models::stdfuncs::value_order(an, st, &kvals[i], &kvals[best]) {
                    Some(std::cmp::Ordering::Greater) if is_max => best = i,
                    Some(std::cmp::Ordering::Less) if !is_max => best = i,
                    Some(_) => {}
                    None => {
                        ok = false;
                        break;
                    }
                }
            }
            if ok {
                return items[best].clone();
            }
        }
        if c.args.len() > 1 {
            return join_values(c.args.clone());
        }
    }
    if c.args.len() == 1 {
        let it = an.iterate(&c.args[0], &c.node, st);
        let elem = an.elem_value(&it.elem, &c.node, "el");
        if let Some(kf) = c.kw("key") {
            if !kf.pts.is_empty() {
                an.call_value(&kf, &c.node, None, st, Some(vec![elem.clone()]));
            }
        }
        // any number of items: one of them, or the default when there are none
        return match default {
            Some(d) => elem.join(&d),
            None => {
                if may_be_empty {
                    an.may_raise_here("ValueError", st);
                }
                elem
            }
        };
    }
    join_values(c.args.clone())
}

/// Has `v` surely an item: each of its containers a list, tuple or set of a known length (or
/// lengths, or a known prefix) of at least one, each literal a non-empty str?
fn surely_nonempty(an: &Analyzer, v: &Value, st: &State) -> bool {
    let objs = v.pts.iter().all(|o| {
        matches!(an.heap.kind(o), Kind::List | Kind::Tuple | Kind::Set)
            && !an.heap.obj(o).iterator
            && (an.heap.llen(o, st).is_some_and(|n| n > 0) || an.heap.len_alts(o, st).is_some_and(|ns| ns.iter().all(|n| *n > 0)) || an.heap.prefix(o, st).is_some_and(|n| n > 0))
    });
    let lits = v.kv.is_bot() || (v.kv.is_literal() && crate::domain::kv_literals(&v.kv).iter().all(|k| intern::key_as_str(k).is_some_and(|t| !t.is_empty())));
    !v.empty() && objs && lits
}

pub(super) fn m_zip(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    // `zip(*rows)` / `zip(a, b, c)`: every input with known items gives the tuples directly
    if !c.star_args.is_empty() || c.args.len() != 2 {
        let mut inputs: Option<Vec<Value>> = Some(c.args.clone());
        for sa in &c.star_args {
            match (an.iterate(sa, &c.node, st).items, inputs.as_mut()) {
                (Some(its), Some(ins)) => ins.extend(its),
                _ => inputs = None,
            }
        }
        let cols: Option<Vec<Vec<Value>>> = inputs.and_then(|ins| ins.iter().map(|x| an.iterate(x, &c.node, st).items).collect());
        if let Some(cols) = cols {
            let n = cols.iter().map(|c0| c0.len()).min().unwrap_or(0);
            let mut tuples: Vec<Value> = Vec::new();
            for j in 0..n {
                an.frame_mut().unroll.push(j);
                let vals: Vec<Value> = cols.iter().map(|c0| c0[j].clone()).collect();
                let kv = KeyVal::lits(vals.iter().filter_map(|v| v.kv.as_lit()).collect());
                tuples.push(an.alloc_positional(Kind::Tuple, &c.node, Some(vals), Value::new(), kv, st));
                an.frame_mut().unroll.pop();
            }
            let v = an.alloc_positional(Kind::List, &c.node, Some(tuples), Value::new(), DATA, st);
            return one_shot(an, v, st);
        }
        // (not every input's items known: tuples of their items, how many not known; with
        // `*rows` of items not known, of any length)
        let elems: Vec<Value> = c.args.clone().iter().map(|x| {
            let e = an.iterate(x, &c.node, st).elem;
            an.elem_value(&e, &c.node, "zn")
        }).collect();
        let t = if c.star_args.is_empty() {
            an.alloc_positional_sfx(Kind::Tuple, &c.node, "#t", Some(elems), Value::new(), DATA, st)
        } else {
            let mut parts = elems;
            for sa in &c.star_args {
                let e = an.iterate(sa, &c.node, st).elem;
                let row = an.elem_value(&e, &c.node, "zr");
                let e = an.iterate(&row, &c.node, st).elem;
                parts.push(an.elem_value(&e, &c.node, "zn"));
            }
            an.alloc_positional_sfx(Kind::Tuple, &c.node, "#t", None, join_values(parts), DATA, st)
        };
        let v = an.alloc_positional(Kind::List, &c.node, None, t, DATA, st);
        return one_shot(an, v, st);
    }
    let a = c.args.first().cloned().unwrap_or_default();
    let b = c.args.get(1).cloned().unwrap_or_default();
    let oid = an.alloc_at(Kind::Zip, &c.node, st, &format!("zip@L{}", c.line), Payload::Zip(a.clone(), b.clone()), None, Ns::Item, None, None);
    an.heap.obj_mut(oid).payload = Payload::Zip(a.clone(), b.clone());
    an.start_iterator(oid, st);
    // `strict=True`: lengths that differ raise ValueError; otherwise both have the known one
    let strict = c.kwargs.get(&intern::s("strict")).and_then(|v| v.kv.as_lit()) == Some(intern::key_bool(true));
    if strict {
        an.m.zip_strict.insert(oid);
        let la = an.iterate(&a, &c.node, st).items.map(|x| x.len());
        let lb = an.iterate(&b, &c.node, st).items.map(|x| x.len());
        match (la, lb) {
            (Some(x), Some(y)) if x != y => {
                an.raise_here("ValueError", st);
                return Value::new();
            }
            (Some(_), Some(_)) => {}
            _ => an.may_raise_here("ValueError", st),
        }
    }
    Value::obj(oid)
}

pub(super) fn m_enumerate(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let a = c.args.first().cloned().unwrap_or_default();
    // `start=` (or the second positional): indices are known only when it is a known int
    let start = match c.kw("start").or_else(|| c.args.get(1).cloned()) {
        None => Some(0),
        Some(v) => v.kv.as_lit().and_then(intern::key_as_int).filter(|_| v.pts.is_empty()),
    };
    let oid = an.alloc_at(Kind::Enumerate, &c.node, st, &format!("enumerate@L{}", c.line), Payload::Enumerate(a.clone(), start), None, Ns::Item, None, None);
    an.heap.obj_mut(oid).payload = Payload::Enumerate(a, start);
    an.start_iterator(oid, st);
    Value::obj(oid)
}

/// `v`'s objects one-shot iterators (`zip`'s list of tuples).
fn one_shot(an: &mut Analyzer, v: Value, st: &mut State) -> Value {
    for o in v.pts.iter() {
        an.start_iterator(o, st);
    }
    v
}

pub(super) fn m_next(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let r = next_item(an, st, c);
    // (what else asking for an item may raise: anything, but of a generator whose raises are
    // taken where it is asked, or of an iterator over a builtin container)
    let known = c.args.first().is_some_and(|a| a.kv.is_bot() && !a.pts.is_empty() && a.pts.iter().all(|o| an.raises_where_asked(o)));
    if !known {
        an.flag_may_raise(st);
    }
    r
}

/// `next`'s item (StopIteration raised where it may have ended without a default).
fn next_item(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(a) = c.args.first() else { return Value::data("next") };
    an.sync_live(a, st, true);
    // a generator with known yields: the item at the cursor, which moves on
    if let Some(g) = a.single() {
        // (a live iterator used up: it gives nothing more)
        if an.at_end(g, st) && an.m.live_iters.contains_key(&g) {
            return match c.args.get(1) {
                Some(d) => d.clone(),
                None => {
                    an.raise_here("StopIteration", st);
                    Value::new()
                }
            };
        }
        if matches!(an.heap.kind(g), Kind::List) && an.heap.obj(g).single {
            // (an endless generator: the positions it is known to reach)
            if let (None, Some(p), Some(cur)) = (an.heap.llen(g, st), an.heap.prefix(g, st), an.cursor_of(g, st)) {
                if cur < p && an.heap.obj(g).iterator {
                    an.set_cursor(g, Some(cur as usize + 1), st);
                    return st.slot(g, Ns::Pos, intern::key_int(cur as i64));
                }
            }
            if let Some(n) = an.heap.llen(g, st) {
                let cur = an.cursor_of(g, st).map(|c| c as i64).unwrap_or(-1);
                if cur >= 0 && (cur as usize) < n {
                    an.set_cursor(g, Some(cur as usize + 1), st);
                    return st.slot(g, Ns::Pos, intern::key_int(cur));
                }
                if cur >= 0 && (cur as usize) >= n {
                    // exhausted: the default, else StopIteration (a live iterator: for good;
                    // a generator: run to its end)
                    an.end_iterator(g, st);
                    return match c.args.get(1) {
                        Some(d) => d.clone(),
                        None => {
                            an.raise_here("StopIteration", st);
                            Value::new()
                        }
                    };
                }
            }
        }
    }
    let it = an.iterate(a, &c.node, st);
    // (an item is surely there: known items, or a list longer than its cursor on every path)
    let has_item = matches!(&it.items, Some(items) if !items.is_empty())
        || a.single().filter(|g| matches!(an.heap.kind(*g), Kind::List | Kind::Tuple)).is_some_and(|g| match an.cursor_of(g, st) {
            Some(cur) => an.heap.prefix(g, st).is_some_and(|p| p > cur) || an.heap.len_alts(g, st).is_some_and(|ls| ls.iter().all(|l| *l > cur)),
            None => false,
        });
    // (a generator at a position not known: it ran to one of its yields at least)
    if let Some(g) = a.single() {
        an.gen_took_item(g, st);
    }
    // (a zip or enumerate: one item further, where it stood known and an item there)
    if let Some(g) = a.single().filter(|g| an.heap.obj(*g).iterator && matches!(an.heap.kind(*g), Kind::Zip | Kind::Enumerate)) {
        let next = an.cursor_of(g, st).filter(|_| matches!(&it.items, Some(items) if !items.is_empty())).map(|c| c + 1);
        an.set_cursor(g, next, st);
    }
    let item = match it.elem {
        Elem::Pair(..) if matches!(&it.items, Some(items) if !items.is_empty()) => it.items.as_ref().unwrap()[0].clone(),
        Elem::Pair(k, v) => {
            let ctx = an.frame().ctx.clone();
            let pid = an.heap.alloc_raw(Kind::Pair, &format!("pair:{}", c.site), &ctx, true, "pair", Payload::Pair(k, v));
            Value::obj(pid)
        }
        Elem::One(elem) => match it.items {
            Some(items) if !items.is_empty() => items[0].clone(),
            _ => elem,
        },
    };
    // (it may have ended: the default, else StopIteration — raised where a one-shot iterator
    // is used up, a generator run to its end)
    match c.args.get(1) {
        _ if has_item => item,
        Some(d) => item.join(d),
        None => {
            let mut rs = st.copy();
            if let Some(g) = a.single().filter(|g| a.kv.is_bot() && an.heap.obj(*g).iterator) {
                an.end_iterator(g, &mut rs);
            }
            an.may_raise_here("StopIteration", &mut rs);
            item
        }
    }
}

pub(super) fn m_getattr(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(obj) = c.args.first().cloned() else { return Value::data("getattr") };
    let name_v = c.args.get(1).cloned().unwrap_or_default();
    // (one name: with a default, the default where the attribute is surely not there, both
    // where it may not be)
    // (one of several names: no verdict — the read is only on the paths where the name is
    // that one —, an AttributeError where it may be missing)
    let by_name = |an: &mut Analyzer, st: &mut State, name: &str, report: bool| {
        let Some(d) = c.args.get(2) else {
            if report {
                return an.load_attr(&obj, name, &c.node, st, true);
            }
            let has = m_hasattr(an, st, &Call { args: vec![obj.clone(), Value::lit_str(name)], ..c.clone() });
            if has.kv.as_lit() != Some(intern::key_bool(true)) || !has.pts.is_empty() {
                an.may_raise_here("AttributeError", st);
            }
            return an.load_attr(&obj, name, &c.node, st, false);
        };
        let has = m_hasattr(an, st, &Call { args: vec![obj.clone(), Value::lit_str(name)], ..c.clone() });
        let found = |an: &mut Analyzer, st: &mut State| an.load_attr(&obj, name, &c.node, st, false);
        match has.kv.as_lit().filter(|_| has.pts.is_empty()).map(|b| b == intern::key_bool(true)) {
            Some(false) => d.clone(),
            Some(true) => found(an, st),
            None => found(an, st).join(d),
        }
    };
    if let (Some(name), true) = (name_v.kv.as_lit().and_then(intern::key_as_str), name_v.pts.is_empty()) {
        return by_name(an, st, &name, true);
    }
    // (a module's names not known: its deferred definitions run)
    for o in obj.pts.iter() {
        an.force_module(o, st);
    }
    // the names it may be: literals, the keys of closed records (`for k in d`), the attributes
    // whose names fit the text known of it (`f"run_{mode}"`: `run_*`); any other part is a
    // name nothing is known of (`getattr_any_name`)
    let (mut names, mut unknown) = getattr_names(&name_v.kv, st);
    let mut afs: Vec<(String, String)> = Vec::new();
    // (a name made by a template, `"_on_%s" % mode`, `f"_on_{mode}"`, directly or through a
    // local only ever assigned such: the attributes whose names fit it)
    if unknown {
        if let Some(t) = c.call_ast.and_then(|ca| ca.arguments.args.get(1)).and_then(|e| an.template_affixes(e)) {
            afs.extend(t);
            unknown = false;
        }
    }
    for o in name_v.pts.iter() {
        match &an.heap.obj(o).payload {
            Payload::Affix(p, s) => afs.push((p.clone(), s.clone())),
            _ => unknown = true,
        }
    }
    let mut v = Value::new();
    if !afs.is_empty() {
        for n in attr_names_like(an, &obj, &afs, st) {
            let has = m_hasattr(an, st, &Call { args: vec![obj.clone(), Value::lit_str(&n)], ..c.clone() });
            if has.kv.as_lit() != Some(intern::key_bool(false)) || !has.pts.is_empty() {
                v = v.join(&an.load_attr(&obj, &n, &c.node, st, false));
            }
        }
        // (an object of unknown class may have others)
        if obj.pts.iter().any(|o| !matches!(an.heap.kind(o), Kind::Instance | Kind::Class | Kind::Module)) {
            v = v.join(&Value::obj(crate::interp::any_object(&mut an.heap)));
        }
        // (what it holds under names not known — `setattr(o, prefix + x, v)` —, or any
        // attribute where unknown code may have added some; a value not known: any)
        for o in obj.pts.iter() {
            if let Some(sv) = st.slot_opt(o, Ns::Attr, intern::STAR) {
                v = v.join(&sv);
            }
            if !st.row(o, Ns::Attr).closed() {
                v = v.join(&Value::data("getattr"));
            }
        }
        if !obj.kv.is_bot() {
            v = v.join(&Value::data("getattr"));
        }
    }
    names.sort();
    names.dedup();
    let one = names.len() == 1 && !unknown && afs.is_empty();
    for n in names {
        v = v.join(&by_name(an, st, &n, one));
    }
    if unknown {
        v = v.join(&getattr_any_name(an, &obj, &name_v, c, st));
    }
    if obj.pts.iter().any(|o| an.heap.kind(o) == Kind::External) {
        let lib: Vec<Value> = vec![Value::with_kv(obj.pts.filter(|o| an.heap.kind(o) == Kind::External), crate::domain::BOT)];
        v = v.join(&an.unknown_call(&c.qual, None, Args::pos(&lib), st, &c.node));
    }
    if let Some(d) = c.args.get(2).filter(|_| unknown || !afs.is_empty()) {
        v = v.join(d);
    }
    v
}

/// The attribute names a name's scalar part may be, and whether it may be another (`Data`, the
/// keys of a record that may have others).
fn getattr_names(kv: &KeyVal, st: &State) -> (Vec<String>, bool) {
    let strs = |ks: &mut dyn Iterator<Item = KeyId>| ks.filter_map(intern::key_as_str).map(|n| n.to_string()).collect::<Vec<_>>();
    match kv {
        KeyVal::Bot => (Vec::new(), false),
        KeyVal::Lit(k) => (strs(&mut std::iter::once(*k)), false),
        KeyVal::Lits(ks) => (strs(&mut ks.iter()), false),
        KeyVal::KeysOf(ko) if st.keys_row(ko).closed() => {
            let keys = st.keys_row(ko).present.difference(&ko.exclude);
            let keys: Vec<KeyId> = keys.iter().filter_map(|k| crate::source::lens_apply(ko.chain, k)).collect();
            (strs(&mut keys.into_iter()), false)
        }
        _ => (Vec::new(), true),
    }
}

/// `getattr(obj, name)` by a name nothing is known of: any attribute the program's objects
/// (instances, classes, modules) have — their own and their classes', bound as a read binds
/// them, properties run —, what a `__getattr__` makes of the name, and what their keys under
/// unknown names hold (read quietly: the reads are speculative, their verdicts would claim
/// what no run may do); a dict's, list's or set's attribute is a method that runs unknown
/// code on it when called (so are more of the program's objects than a wide read); an
/// unknown object's is what it holds or unknown (its methods take it no more than `u.m()`
/// does). Without a default the read may raise.
fn getattr_any_name(an: &mut Analyzer, obj: &Value, name_v: &Value, c: &Call, st: &mut State) -> Value {
    let mut v = join_values(obj.pts.iter().map(|o| an.all_values(o, Ns::Attr, st)));
    let user = |an: &Analyzer, o: ObjId| matches!(an.heap.kind(o), Kind::Instance | Kind::Class | Kind::Module);
    // (more of the program's objects than a wide read: one method of them all, unknown code
    // run on them when called — a method of each would be an object per receiver and name)
    let wide = obj.pts.iter().filter(|o| user(an, *o)).count() > crate::interp::WIDE_READ;
    let coarse = obj.pts.filter(|o| (wide && user(an, o)) || matches!(an.heap.kind(o), Kind::List | Kind::Dict | Kind::Set));
    if !coarse.is_empty() {
        v = v.join(&getattr_method(an, c, &Value::objs(coarse)));
    }
    for o in obj.pts.iter() {
        let one = Value::obj(o);
        if wide || !user(an, o) {
            continue;
        }
        // (speculative reads: their own accesses give no verdicts)
        an.quietly(|an| {
            for n in attr_names_like(an, &one, &[(String::new(), String::new())], st) {
                v = v.join(&an.load_attr(&one, &n, &c.node, st, false));
            }
            let hook = an.heap.kind(o) == Kind::Instance && an.heap.instance_class(o).is_some_and(|cls| an.class_lookup(cls, "__getattr__").is_some());
            if hook {
                v = v.join(&an.call_method_with(&one, "__getattr__", vec![name_v.clone()], &c.node, st).unwrap_or_default());
            }
        });
    }
    if !obj.kv.is_bot() || obj.pts.iter().any(|o| !matches!(an.heap.kind(o), Kind::Instance | Kind::Class | Kind::Module | Kind::External)) {
        v = v.join(&Value::data("getattr"));
    }
    if c.args.len() < 3 {
        an.may_raise_here("AttributeError", st);
    }
    v
}

/// The method `getattr` at call `c` may give of objects whose attributes are not followed: one
/// object per call site (its receivers all those seen there), unknown code run on them when
/// called.
fn getattr_method(an: &mut Analyzer, c: &Call, recv: &Value) -> Value {
    let key = format!("getattr:M{}:L{}:{}", c.node.module, c.node.line, c.node.col);
    let o = an.heap.alloc_raw(Kind::Method, &key, "", true, "getattr", Payload::Method("unknown.getattr".into(), recv.clone()));
    if let Payload::Method(q, r) = an.heap.obj(o).payload.clone() {
        an.heap.obj_mut(o).payload = Payload::Method(q, r.join(recv));
    }
    Value::obj(o)
}

/// The attribute names of `obj` (its own and its classes') that start and end with one of the
/// affix pairs.
pub(super) fn attr_names_like(an: &Analyzer, obj: &Value, afs: &[(String, String)], st: &State) -> Vec<String> {
    let fits = |n: &str| afs.iter().any(|(p, s)| n.len() >= p.len() + s.len() && n.starts_with(p.as_str()) && n.ends_with(s.as_str()));
    let mut owners: Vec<u32> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    for o in obj.pts.iter() {
        for k in st.row(o, Ns::Attr).present.iter() {
            names.extend(intern::key_as_str(k).map(|n| n.to_string()));
        }
        match an.heap.kind(o) {
            Kind::Instance => owners.extend(an.heap.instance_class(o).map(|c| an.heap.mro(c)).unwrap_or_default()),
            Kind::Class => owners.extend(an.heap.mro(o)),
            _ => {}
        }
    }
    if !owners.is_empty() {
        for (c, ns, k) in an.heap.cls_slots.keys() {
            if *ns == Ns::Attr && owners.contains(c) {
                names.extend(intern::key_as_str(*k).map(|n| n.to_string()));
            }
        }
    }
    names.retain(|n| fits(n));
    names.sort();
    names.dedup();
    names
}

pub(super) fn m_setattr(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    // (setattr reports its own raises: frozen classes, slots, setter-less properties raise in
    // the store; an object of unknown class may)
    if let Some(o) = c.args.first() {
        if o.pts.is_empty() || o.pts.iter().any(|x| !matches!(an.heap.kind(x), Kind::Instance | Kind::Namespace | Kind::Class | Kind::Module | Kind::Function)) || c.args.get(1).map(|k| k.kv.as_lit().is_none()).unwrap_or(true) {
            an.may_raise_here("AttributeError", st);
        }
    }
    if c.args.len() >= 3 {
        match c.args[1].kv.as_lit().and_then(intern::key_as_str) {
            Some(name) => an.store_attr(&c.args[0], &name, &c.args[2], &c.node, st),
            None => {
                // (a function's attributes too)
                let recs: Vec<ObjId> = c.args[0].pts.iter().filter(|o| an.heap.is_record(*o) || an.heap.kind(*o) == Kind::Function).collect();
                let strong = recs.len() == 1 && an.heap.obj(recs[0]).single;
                for o in recs {
                    // a user `__setattr__` takes the store, whatever the name
                    let hook = an.heap.kind(o) == Kind::Instance && !an.setattr_bypassed() && an.heap.instance_class(o).is_some_and(|cls| an.class_lookup(cls, "__setattr__").is_some());
                    if hook {
                        an.call_method_with(&Value::obj(o), "__setattr__", vec![c.args[1].clone(), c.args[2].clone()], &c.node, st);
                        continue;
                    }
                    an.store_key(o, Ns::Attr, &c.args[1], &c.args[2], strong, &c.node, st);
                }
            }
        }
    }
    Value::none()
}

/// `random.shuffle(xs)` of lists of known length: each position may hold any of the items
/// (the same objects); anything else is an unknown call.
pub(super) fn m_shuffle(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let xs = c.args.first().cloned().unwrap_or_default();
    let lists: Vec<(ObjId, usize)> = xs.pts.iter().filter_map(|o| (an.heap.kind(o) == Kind::List && !an.heap.obj(o).iterator).then(|| an.heap.llen(o, st).map(|n| (o, n))).flatten()).collect();
    if c.args.len() != 1 || xs.pts.is_empty() || !xs.kv.is_bot() || lists.len() != xs.pts.len() {
        return an.unknown_call(&c.qual, None, c.arguments(), st, &c.node);
    }
    for (o, n) in lists {
        let all = join_values((0..n).map(|i| st.slot(o, Ns::Pos, intern::key_int(i as i64))).collect::<Vec<_>>());
        for i in 0..n {
            st.set_slot(o, Ns::Pos, intern::key_int(i as i64), all.clone(), xs.pts.len() == 1);
        }
    }
    Value::none()
}

/// `random.choice(seq)` (`secrets.choice`): one of seq's items; `random.sample(seq, k)` /
/// `random.choices(seq, …)`: a new list of them. Of anything but the program's lists and
/// tuples (and literals: a str's characters): what `noeffect` gives.
pub(super) fn m_random_pick(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let seq = c.arg(0, Some(if c.qual == "random.sample" || c.qual == "random.choices" { "population" } else { "seq" })).unwrap_or_default();
    let sequence = seq.pts.iter().all(|o| matches!(an.heap.kind(o), Kind::List | Kind::Tuple) && !an.heap.obj(o).iterator);
    if seq.pts.is_empty() || !sequence {
        return noeffect(an, st, c);
    }
    let it = an.iterate(&seq, &c.node, st);
    let e = an.elem_value(&it.elem, &c.node, "pick");
    // (an empty one: IndexError, `sample`'s k past its length: ValueError)
    an.may_raise_here(if c.qual == "random.sample" { "ValueError" } else { "IndexError" }, st);
    if c.qual.ends_with(".choice") {
        return e;
    }
    crate::models::itertools::it_list(an, None, e, &c.node, st, "pick").single().map(|o| {
        an.heap.obj_mut(o).iterator = false;
        Value::obj(o)
    }).unwrap_or_default()
}

/// `reprlib.repr(x)`: a str; it runs the `__repr__` of what x reaches, so with an instance of
/// a class defining one (or anything unknown, or more than a few hundred objects) it is an
/// unknown call.
pub(super) fn m_reprlib_repr(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let mut work: Vec<ObjId> = c.args.iter().flat_map(|a| a.pts.iter()).collect();
    let mut seen: FxHashSet<ObjId> = FxHashSet::default();
    let mut own_code = !c.kwargs.is_empty() || c.args.len() != 1;
    while let Some(o) = work.pop() {
        if !seen.insert(o) || matches!(an.heap.kind(o), Kind::Class | Kind::Module | Kind::Function | Kind::External) {
            continue;
        }
        own_code |= seen.len() > 512 || matches!(an.heap.kind(o), Kind::Unknown | Kind::Frame | Kind::Series) || an.heap.instance_class(o).is_some_and(|cls| an.class_lookup(cls, "__repr__").is_some());
        if own_code {
            break;
        }
        an.obj_edges(o, st, &mut work);
    }
    if own_code {
        return an.unknown_call(&c.qual, None, c.arguments(), st, &c.node);
    }
    Value::data(&format!("L{}", c.line))
}

/// `operator.setitem(d, k, v)` into dicts by a key of no objects is `d[k] = v`; anything else
/// is an unknown call.
pub(super) fn m_op_setitem(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    match &c.args[..] {
        [d, k, v] if !d.pts.is_empty() && d.kv.is_bot() && k.pts.is_empty() && d.pts.iter().all(|o| an.heap.kind(o) == Kind::Dict) => {
            an.store_item(d, Ns::Item, k, v, &c.node, st);
            Value::none()
        }
        _ => an.unknown_call(&c.qual, None, c.arguments(), st, &c.node),
    }
}

/// `object.__setattr__(obj, "name", v)`: the plain store (no user `__setattr__`).
pub(super) fn m_object_setattr(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    an.plain_setattr(|an| m_setattr(an, st, c));
    Value::none()
}

/// `object.__delattr__(obj, name)`: `delattr` past the class's own `__delattr__`.
pub(super) fn m_object_delattr(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    an.plain_setattr(|an| m_delattr(an, st, c));
    Value::none()
}

/// `object.__init__(self)` (`super().__init__()` in a class whose bases are the program's):
/// nothing; given more, as CPython decides by the class (unknown code).
pub(super) fn m_object_init(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    if c.args.len() == 1 && c.kwargs.is_empty() && c.star_args.is_empty() && c.star_kwargs.is_empty() {
        return Value::none();
    }
    an.unknown_call(&c.qual, None, c.arguments(), st, &c.node)
}

/// `object.__new__(cls)`: a fresh instance of each user class `cls` may be, without attributes
/// (a NamedTuple's: its fields from the other arguments, as `super().__new__(cls, …)` in a
/// subclass's `__new__` makes it); anything else (a class only a call site knows, an external
/// class) is unknown.
pub(super) fn m_object_new(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(cv) = c.args.first().cloned() else { return Value::new() };
    let mut out = Value::new();
    let mut other = !cv.kv.is_bot();
    for o in cv.pts.iter() {
        if an.heap.kind(o) == Kind::Class && an.heap.class_info_opt(o).is_some() {
            let inst = an.alloc_instance(o, &c.node, st);
            if crate::models::dataclasses::is_named_tuple_class(an, o) {
                crate::models::dataclasses::dataclass_init(an, o, inst, &c.args[1..], &c.star_args, &c.kwargs, &c.star_kwargs, st, &c.node);
            }
            out = out.join(&Value::obj(inst));
        } else {
            other = true;
        }
    }
    if other {
        out = out.join(&an.unknown_call(&c.qual, None, c.arguments(), st, &c.node));
    }
    out
}

pub(super) fn m_delattr(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    if c.args.len() >= 2 {
        match c.args[1].kv.as_lit().filter(|_| c.args[1].pts.is_empty()).and_then(intern::key_as_str) {
            Some(name) => an.delete_item(&c.args[0], Ns::Attr, &Value::lit(key_str_lit(&name)), &c.node, st, true),
            // (a name not known: any attribute may be the one removed)
            None => an.delete_item(&c.args[0], Ns::Attr, &c.args[1], &c.node, st, true),
        }
    }
    Value::none()
}

/// `hasattr(obj, "name")` on instances of known classes: decided by the attribute rows and
/// the class (a class without `__getattr__`).
/// `hasattr(o, name)` for a builtin dict / list / tuple / set: its type's methods are there;
/// what only records of user classes have (dataclass / pydantic / namedtuple API, `__dict__`)
/// is not; anything else is not decided here (Counter, defaultdict, OrderedDict add methods).
pub fn builtin_has_attr(an: &Analyzer, o: u32, name: &str) -> Option<bool> {
    const DICT_M: &[&str] = &["keys", "values", "items", "get", "pop", "popitem", "setdefault", "update", "clear", "copy", "fromkeys"];
    const LIST_M: &[&str] = &["append", "extend", "insert", "remove", "pop", "clear", "index", "count", "sort", "reverse", "copy"];
    const SET_M: &[&str] = &["add", "discard", "remove", "pop", "clear", "copy", "union", "intersection", "difference", "symmetric_difference", "issubset", "issuperset", "isdisjoint", "update"];
    const ABSENT: &[&str] = &["__dataclass_fields__", "__dataclass_params__", "__pydantic_fields__", "model_fields", "model_dump", "model_validate", "__fields__", "_fields", "_asdict", "_replace", "__dict__", "__slots__", "dict", "json"];
    let methods: &[&str] = match an.heap.kind(o) {
        Kind::Dict => DICT_M,
        Kind::List => LIST_M,
        Kind::Tuple => &["index", "count"],
        Kind::Set => SET_M,
        _ => return None,
    };
    if methods.contains(&name) {
        Some(true)
    } else if ABSENT.contains(&name) {
        Some(false)
    } else {
        None
    }
}

/// Run `o.__getattr__(name)` on a copy of the state under a try catching AttributeError:
/// Some(false) when it surely raises it, Some(true) when it surely returns, None otherwise (or
/// when it changes the heap).
pub(super) fn getattr_hook_outcome(an: &mut Analyzer, st: &mut State, o: u32, k: KeyId, node: &crate::frontend::Node) -> Option<bool> {
    let mut s2 = st.copy();
    let mark = an.heap.len() as u32;
    let caught: rustc_hash::FxHashSet<String> = ["AttributeError".to_string()].into_iter().collect();
    an.enter_try(caught, &mut s2);
    an.call_method_with(&Value::obj(o), "__getattr__", vec![Value::lit(k)], node, &mut s2);
    let out = an.leave_try();
    let (flagged, raised) = (out.any(), out.raised());
    let recs = out.recs;
    if flagged {
        return None;
    }
    // (objects it made, the exception among them, do not count)
    let fp = s2.footprint(st);
    if fp.rows.iter().any(|(x, _)| *x < mark) || fp.slots.iter().any(|(x, _)| *x < mark) || fp.elems.iter().any(|x| *x < mark) {
        return None;
    }
    let all_attr = recs.iter().all(|(cls, _, _)| cls.as_ref().is_some_and(|(n, _)| n.iter().any(|x| x == "AttributeError")));
    match (s2.reachable, raised.is_some()) {
        (false, true) if all_attr && !recs.is_empty() => Some(false),
        (true, false) => Some(true),
        _ => None,
    }
}

/// The attributes of `str` (CPython 3.14 `dir(str)`).
pub(super) const STR_ATTRS: &[&str] = &[
    "__add__", "__class__", "__contains__", "__delattr__", "__dir__", "__doc__", "__eq__", "__format__", "__ge__",
    "__getattribute__", "__getitem__", "__getnewargs__", "__getstate__", "__gt__", "__hash__", "__init__", "__init_subclass__",
    "__iter__", "__le__", "__len__", "__lt__", "__mod__", "__mul__", "__ne__", "__new__", "__reduce__", "__reduce_ex__",
    "__repr__", "__rmod__", "__rmul__", "__setattr__", "__sizeof__", "__str__", "__subclasshook__",
    "capitalize", "casefold", "center", "count", "encode", "endswith", "expandtabs", "find", "format", "format_map", "index",
    "isalnum", "isalpha", "isascii", "isdecimal", "isdigit", "isidentifier", "islower", "isnumeric", "isprintable", "isspace",
    "istitle", "isupper", "join", "ljust", "lower", "lstrip", "maketrans", "partition", "removeprefix", "removesuffix", "replace",
    "rfind", "rindex", "rjust", "rpartition", "rsplit", "rstrip", "split", "splitlines", "startswith", "strip", "swapcase", "title",
    "translate", "upper", "zfill",
];

pub(super) fn m_hasattr(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    // (undecided: a bool all the same)
    let unknown = Value::data(&crate::domain::typed_label("bool", ""));
    if c.args.len() < 2 {
        return unknown;
    }
    let Some(name) = c.args[1].kv.as_lit().filter(|_| c.args[1].pts.is_empty()).and_then(intern::key_as_str) else { return unknown };
    let k = key_str_lit(&name);
    let v0 = &c.args[0];
    // (a str literal: its type's attributes are known exactly — CPython 3.14's `dir(str)`)
    if v0.pts.is_empty() && v0.kv.is_literal() && crate::domain::kv_literals(&v0.kv).iter().all(intern::key_is_str) {
        return Value::lit(intern::key_bool(STR_ATTRS.contains(&&*name)));
    }
    // dunder attributes come from places not modelled (decorators, the type itself)
    if name.starts_with("__") && !(c.args[0].kv.is_bot() && !c.args[0].pts.is_empty() && c.args[0].pts.iter().all(|o| builtin_has_attr(an, o, &name) == Some(false))) {
        return unknown;
    }
    let objs: Vec<u32> = c.args[0].pts.iter().collect();
    if objs.is_empty() || !c.args[0].kv.is_bot() {
        return unknown;
    }
    let mut verdicts: Vec<bool> = Vec::new();
    for o in objs {
        if let Some(b) = builtin_has_attr(an, o, &name) {
            verdicts.push(b);
            continue;
        }
        // (the program's function: what the program stored on it on every path, its type's
        // attributes all dunders; an lru_cache's / singledispatch's: not known here)
        if an.heap.kind(o) == Kind::Function && an.plain_function(o) && an.has_facts(o, st) {
            let row = st.row(o, Ns::Attr);
            if row.definite.contains(k) {
                verdicts.push(true);
                continue;
            }
            // (one it surely lacks: a call that did not run gives back the functions its code
            // may, `returned_aliases`)
            if row.lacks(k) {
                verdicts.push(false);
                continue;
            }
            // (stored on some paths only: either)
            if row.closed() {
                verdicts.extend([true, false]);
                continue;
            }
            return unknown;
        }
        if an.heap.kind(o) != Kind::Instance {
            return unknown;
        }
        let Some(cls) = an.heap.instance_class(o) else { return unknown };
        if !an.hierarchy_closed(cls) {
            return unknown;
        }
        if an.class_lookup(cls, "__getattr__").is_some() {
            // the normal lookup first; then hasattr is `__getattr__` under `except AttributeError`
            if an.class_lookup(cls, &name).is_some() || st.row(o, Ns::Attr).definite.contains(k) {
                verdicts.push(true);
                continue;
            }
            let outcome = getattr_hook_outcome(an, st, o, k, &c.node);
            // (the hook runs: its effects, where it returns)
            let mut ran = st.copy();
            an.run_dunder(&Value::obj(o), &["__getattr__"], vec![Value::lit_str(&name)], &c.node, &mut ran);
            if ran.reachable {
                *st = st.join(&mut ran);
            }
            match outcome {
                Some(b) => {
                    verdicts.push(b);
                    continue;
                }
                None => return unknown,
            }
        }
        let (defs, sure) = an.class_lookup_on_path(cls, &name, st);
        if sure {
            verdicts.push(true);
            continue;
        }
        let row = st.row(o, Ns::Attr);
        if row.definite.contains(k) {
            verdicts.push(true);
        } else if !defs.is_empty() {
            return unknown;
        } else if row.closed() && !row.present.contains(k) {
            verdicts.push(false);
        } else {
            return unknown;
        }
    }
    if verdicts.iter().all(|b| *b) {
        Value::lit(intern::key_bool(true))
    } else if verdicts.iter().all(|b| !*b) {
        Value::lit(intern::key_bool(false))
    } else {
        Value::lit(intern::key_bool(true)).join(&Value::lit(intern::key_bool(false)))
    }
}

/// `dataclasses.asdict(obj)`: a dict of the fields, nested dataclass instances (also in
/// lists / tuples) converted the same way.
/// `dataclasses.asdict(v)` at `node` (a dataclass inside a pydantic dump).
pub fn asdict_of(an: &mut Analyzer, st: &mut State, node: &crate::frontend::Node, v: &Value) -> Value {
    let c = Call { qual: "dataclasses.asdict".into(), recv: None, args: vec![v.clone()], kwargs: IndexMap::new(), star_args: vec![], star_kwargs: vec![], node: *node, call_ast: None, site: format!("L{}", node.line), line: node.line };
    m_asdict(an, st, &c)
}

pub(super) fn m_asdict(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(a) = c.args.first().cloned() else { return Value::data("asdict") };
    fn conv(an: &mut Analyzer, st: &mut State, c: &Call, v: &Value, depth: u32) -> Value {
        let mut out = Value::with_kv(crate::domain::ObjSet::new(), v.kv.clone());
        for o in v.pts.iter() {
            let cls_of = match an.heap.kind(o) {
                Kind::Instance => an.heap.instance_class(o),
                _ => None,
            };
            let is_dc = cls_of.map(|cl| !crate::models::dataclasses::dataclass_fields_mro(an, cl).is_empty()).unwrap_or(false);
            if !is_dc || depth > 4 {
                out = out.join(&Value::obj(o));
                continue;
            }
            let cls = cls_of.unwrap();
            // one dict per converted instance (siblings at one depth must not share it)
            let d = an.alloc_at(Kind::Dict, &c.node, st, &format!("asdict@L{}", c.line), Payload::None, Some(Row::new()), Ns::Item, None, Some(&format!("asdict{}:{}", depth, o)));
            st.set_row(d, Ns::Item, Row::new());
            for (f, _) in crate::models::dataclasses::dataclass_fields_mro(an, cls) {
                let fk = key_str_lit(&f);
                let row = st.row(o, Ns::Attr);
                if !row.present.contains(fk) {
                    continue;
                }
                let fv = st.slot(o, Ns::Attr, fk);
                let cv = conv(an, st, c, &fv, depth + 1);
                an.store_key(d, Ns::Item, &Value::lit(fk), &cv, row.definite.contains(fk), &c.node, st);
            }
            out = out.join(&Value::obj(d));
        }
        out
    }
    if a.pts.iter().all(|o| an.heap.kind(o) == Kind::Instance && an.heap.instance_class(o).map(|cl| !crate::models::dataclasses::dataclass_fields_mro(an, cl).is_empty()).unwrap_or(false)) && !a.pts.is_empty() {
        return conv(an, st, c, &a, 0);
    }
    m_vars(an, st, c)
}

/// `model.model_dump()` / `model.dict()`: like `asdict` on the model.
pub(super) fn m_dc_fields(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(a) = c.args.first() else { return Value::data("fields") };
    let cls = a.single().and_then(|o| match an.heap.kind(o) {
        Kind::Class => Some(o),
        Kind::Instance => an.heap.instance_class(o),
        _ => None,
    });
    let Some(cls) = cls else { return Value::data("fields") };
    let names: Vec<String> = crate::models::dataclasses::dataclass_fields_mro(an, cls).into_iter().map(|(f, _)| f).collect();
    let mut vals: Vec<Value> = Vec::new();
    for (i, f) in names.iter().enumerate() {
        an.frame_mut().unroll.push(i);
        let row = Row::with_source(Source::new("dataclasses.Field", SourceKind::Data));
        let fo = an.alloc_at(Kind::Namespace, &c.node, st, &format!("Field {}", f), Payload::None, Some(row), Ns::Attr, None, None);
        an.frame_mut().unroll.pop();
        an.store_key(fo, Ns::Attr, &Value::lit(key_str_lit("name")), &Value::lit(key_str_lit(f)), true, &c.node, st);
        vals.push(Value::obj(fo));
    }
    an.alloc_positional(Kind::Tuple, &c.node, Some(vals), Value::new(), DATA, st)
}

pub(super) fn m_vars(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(a) = c.args.first() else { return Value::data("vars") };
    let oid = an.alloc_simple(Kind::Dict, &c.node, st, &format!("vars@L{}", c.line));
    if c.qual == "builtins.vars" {
        if let Some(inst) = a.single().filter(|o| matches!(an.heap.kind(*o), Kind::Instance | Kind::Function)) {
            an.m.attr_mirrors.insert(oid, inst);
        }
    }
    // (one of several objects: the attributes of each, joined)
    let empty = st.row(oid, Ns::Item);
    let mut rows: Vec<Row> = Vec::new();
    for x in an.record_objects(a) {
        an.force_module(x, st);
        rows.push(empty.copy_from(&st.row(x, Ns::Attr), true, &KeySet::new(), intern::EMPTY_LENS));
        for (k, v) in st.obj_slots(x, Ns::Attr) {
            st.set_slot(oid, Ns::Item, k, v, false);
        }
    }
    if !rows.is_empty() {
        st.set_row(oid, Ns::Item, crate::domain::join_rows(rows));
    }
    Value::obj(oid)
}

pub(super) fn m_map(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let oid = an.alloc_simple(Kind::List, &c.node, st, &format!("{}@L{}", last(&c.qual), c.line));
    fill_map(an, st, c, oid, 0);
    an.start_iterator(oid, st);
    // (over one iterable: the function's results on its items as they are at each step)
    if let [f, a] = c.args.as_slice() {
        st.set_slot(oid, Ns::Attr, key_str_lit("\u{1}fn"), f.clone(), true);
        live_from(an, st, c, oid, a);
    }
    Value::obj(oid)
}

/// `map(f, xs…)` / `filter(f, xs)` (`c`): the function's results (the items it keeps) written
/// into `oid` (a map's from item `from` on: the function is not called for those before).
fn fill_map(an: &mut Analyzer, st: &mut State, c: &Call, oid: ObjId, from: usize) {
    an.clear_item_effects(oid);
    an.heap.set_llen(oid, st, None);
    if c.args.len() >= 2 {
        let it = an.iterate(&c.args[1], &c.node, st);
        if c.qual.ends_with("filter") {
            if let Some(items) = it.items.as_ref().filter(|its| its.len() <= crate::interp::UNROLL_MAX) {
                // known items: the predicate per item (None: the item's truth); decided for all
                // of them, the kept items keep their positions
                let mut kept: Vec<Value> = Vec::new();
                let mut all_decided = true;
                for (i, item) in items.iter().enumerate() {
                    let truth = if c.args[0].pts.is_empty() {
                        an.truthiness(item, st)
                    } else {
                        an.frame_mut().unroll.push(i);
                        let r = an.call_value(&c.args[0], &c.node, None, st, Some(vec![item.clone()]));
                        an.frame_mut().unroll.pop();
                        an.truthiness(&r, st)
                    };
                    match truth {
                        Some(false) => {}
                        Some(true) => kept.push(item.clone()),
                        None => {
                            all_decided = false;
                            kept.push(item.clone());
                        }
                    }
                }
                let res = join_values(kept.clone());
                st.set_slot(oid, Ns::Elem, STAR, Value::with_kv(res.pts.clone(), crate::models::torch::elem_kv(&res.kv)), true);
                st.elems.insert(oid, res.kv);
                if all_decided && an.heap.obj(oid).single {
                    an.heap.set_llen(oid, st, Some(kept.len()));
                    for (i, v) in kept.into_iter().enumerate() {
                        st.set_slot(oid, Ns::Pos, intern::key_int(i as i64), v, true);
                    }
                }
                return;
            }
        }
        // `map(f, a, b, …)`: f gets one item of each (as many as the shortest has)
        let others: Vec<crate::interp::iter::Iteration> = c.args.iter().skip(2).map(|a| an.iterate(a, &c.node, st)).collect();
        if !c.qual.ends_with("filter") && an.heap.obj(oid).single {
            let n_known = std::iter::once(it.items.as_ref().map(|x| x.len())).chain(others.iter().map(|o| o.items.as_ref().map(|x| x.len()))).collect::<Option<Vec<usize>>>().and_then(|v| v.into_iter().min());
            if let (Some(items), Some(n)) = (&it.items, n_known) {
                if items.len() <= crate::interp::UNROLL_MAX {
                    // known items: the function per item, positions kept; what its calls
                    // change comes with each item as it is taken (`effects_per_item`)
                    let (pre, mark) = (st.copy(), an.heap.serial());
                    let mut steps = Vec::new();
                    let mut outs = Vec::new();
                    for (i, item) in items.iter().take(n).enumerate() {
                        if i < from {
                            let r = Value::data("map");
                            st.set_slot(oid, Ns::Pos, intern::key_int(i as i64), r.clone(), true);
                            outs.push(r);
                            steps.push(st.copy());
                            continue;
                        }
                        an.frame_mut().unroll.push(i);
                        let mut call_args = vec![item.clone()];
                        call_args.extend(others.iter().map(|o| o.items.as_ref().unwrap()[i].clone()));
                        let r = an.call_value(&c.args[0], &c.node, None, st, Some(call_args));
                        an.frame_mut().unroll.pop();
                        st.set_slot(oid, Ns::Pos, intern::key_int(i as i64), r.clone(), true);
                        outs.push(r);
                        steps.push(st.copy());
                    }
                    let res = join_values(outs);
                    st.set_slot(oid, Ns::Elem, STAR, Value::with_kv(res.pts.clone(), crate::models::torch::elem_kv(&res.kv)), true);
                    st.elems.insert(oid, res.kv);
                    an.heap.set_llen(oid, st, Some(n.min(items.len())));
                    an.effects_per_item(oid, &pre, steps, mark, st);
                    return;
                }
            }
        }
        let elem = an.elem_value(&it.elem, &c.node, "el");
        let res = if c.qual.ends_with("filter") {
            // the predicate still runs on the items (its reads are the program's)
            if !c.args[0].pts.is_empty() {
                an.call_value(&c.args[0], &c.node, None, st, Some(vec![elem.clone()]));
            }
            elem
        } else {
            let mut call_args = vec![elem];
            call_args.extend(others.iter().map(|o| an.elem_value(&o.elem, &c.node, "el")));
            an.call_value(&c.args[0], &c.node, None, st, Some(call_args))
        };
        st.set_slot(oid, Ns::Elem, STAR, Value::with_kv(res.pts.clone(), crate::models::torch::elem_kv(&res.kv)), true);
        st.elems.insert(oid, res.kv);
    }
}

/// `reduce(f, xs[, init])`: over known items the function item by item; otherwise one
/// application on any element, joined with the start value.
pub(super) fn m_reduce(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let (Some(f), Some(xs)) = (c.args.first().cloned(), c.args.get(1).cloned()) else { return Value::data("reduce") };
    let init = c.args.get(2).cloned();
    let it = an.iterate(&xs, &c.node, st);
    if let Some(items) = it.items.filter(|its| its.len() <= crate::interp::UNROLL_MAX) {
        let mut rest = items.into_iter();
        let mut acc = match init {
            Some(v) => v,
            None => match rest.next() {
                Some(v) => v,
                None => {
                    an.raise_here("TypeError", st);
                    return Value::new();
                }
            },
        };
        for (i, item) in rest.enumerate() {
            an.frame_mut().unroll.push(i);
            acc = an.call_value(&f, &c.node, None, st, Some(vec![acc, item]));
            an.frame_mut().unroll.pop();
        }
        return acc;
    }
    let elem = an.elem_value(&it.elem, &c.node, "el");
    let start = init.unwrap_or_else(|| elem.clone());
    let r = an.call_value(&f, &c.node, None, st, Some(vec![start.clone(), elem]));
    r.join(&start)
}

/// `type(x)` with one argument: the class of an instance of a known class (else unknown).
pub(super) fn m_type(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    // `type(name, bases, {...})`: a class whose attributes are the namespace's items, made
    // by a synthetic module per call site and choice of bases (`class N(__b0, __b1): pass`
    // with `__bi` bound to the bases); a base that is one of several classes makes one class
    // per choice
    if c.args.len() == 3 {
        let lit_name = c.args[0].kv.as_lit().filter(|_| c.args[0].pts.is_empty()).and_then(intern::key_as_str).filter(|n| !n.is_empty() && n.chars().all(|ch| ch.is_alphanumeric() || ch == '_'));
        let bases: Option<Vec<Vec<ObjId>>> = c.args[1].single().filter(|o| an.heap.kind(*o) == Kind::Tuple && c.args[1].kv.is_bot()).and_then(|t| {
            let n = an.heap.llen(t, st)?;
            (0..n)
                .map(|i| {
                    let b = st.slot(t, Ns::Pos, intern::key_int(i as i64));
                    (b.kv.is_bot() && !b.pts.is_empty() && b.pts.iter().all(|o| matches!(an.heap.kind(o), Kind::Class | Kind::External))).then(|| b.pts.to_vec())
                })
                .collect()
        });
        let ns = c.args[2].single().filter(|o| an.heap.kind(*o) == Kind::Dict).map(|o| (o, st.row(o, Ns::Item)));
        let mut choices: Vec<Vec<ObjId>> = vec![Vec::new()];
        for alts in bases.iter().flatten() {
            choices = choices.iter().flat_map(|ch| alts.iter().map(move |b| ch.iter().copied().chain([*b]).collect::<Vec<ObjId>>())).collect();
        }
        if let (Some(_), Some((ns, row))) = (bases.as_ref().filter(|_| choices.len() <= 32), ns.filter(|(_, r)| r.closed() && r.present == r.definite)) {
            let name: String = lit_name.as_deref().unwrap_or("type").to_string();
            let mut out = Value::new();
            for ch in choices {
                let names: Vec<String> = (0..ch.len()).map(|i| format!("__b{}", i)).collect();
                let modname = format!("<type {} {:?}>", crate::interp::site_string(&c.node), ch);
                let moid = match an.module(&modname) {
                    Some(m) => m,
                    None => {
                        let Ok((idx, tree)) = crate::frontend::parse(&modname, None, format!("class {}({}):\n    pass\n", name, names.join(", "))) else { return Value::data("type") };
                        an.make_module(&modname, None, idx, tree)
                    }
                };
                // (made in another state, or not yet: run it in this one)
                if st.slot_opt(moid, Ns::Attr, key_str_lit(&name)).is_none() {
                    for (n, b) in names.iter().zip(&ch) {
                        st.set_slot(moid, Ns::Attr, key_str_lit(n), Value::obj(*b), true);
                    }
                    an.run_module(moid, st);
                }
                let cv = st.slot(moid, Ns::Attr, key_str_lit(&name));
                if let Some(cls) = cv.single() {
                    for k in row.present.iter().filter(|k| intern::key_as_str(*k).is_some()) {
                        let v = st.slot(ns, Ns::Item, k);
                        an.heap.set_cls_slot((cls, Ns::Attr, k), v.clone());
                        st.set_slot(cls, Ns::Attr, k, v, true);
                        let r = st.row(cls, Ns::Attr).store(k, row.definite.contains(k));
                        st.set_row(cls, Ns::Attr, r);
                    }
                    if lit_name.is_none() {
                        for k in ["__name__", "__qualname__"] {
                            an.heap.set_cls_slot((cls, Ns::Attr, key_str_lit(k)), c.args[0].clone());
                        }
                    }
                }
                out = out.join(&cv);
            }
            return out;
        }
        return an.unknown_call(&c.qual, None, c.arguments(), st, &c.node);
    }
    if c.args.len() == 1 {
        let a = &c.args[0];
        let classes: Option<Vec<u32>> = if a.pts.is_empty() { None } else { a.pts.iter().map(|o| if an.heap.kind(o) == Kind::Instance { an.heap.instance_class(o) } else { None }).collect() };
        if let Some(cs) = classes {
            return join_values(cs.into_iter().map(Value::obj).collect::<Vec<Value>>());
        }
        // builtin values: their builtin class (the object the builtin name is)
        let builtin = |an: &mut Analyzer, n: &str| -> Value { Value::obj(an.heap.alloc_raw(Kind::External, &format!("ext:builtins.{}", n), "", true, n, Payload::Str(format!("builtins.{}", n)))) };
        let mut outs: Vec<Value> = Vec::new();
        let mut known = true;
        for o in a.pts.iter() {
            // a builtin exception object: its class
            if let Some(en) = an.m.exc_objs.get(&o).cloned().filter(|_| an.heap.kind(o) == Kind::Namespace) {
                outs.push(builtin(an, &en));
                continue;
            }
            let n = match an.heap.kind(o) {
                // (a torch container — ModuleList, Sequential, ModuleDict — is not a list or dict)
                _ if crate::models::torch::t_layer_type(an, o).is_some() => {
                    known = false;
                    continue;
                }
                Kind::Dict if !crate::models::pandas::is_series_obj(an, o) => "dict",
                Kind::List => "list",
                Kind::Tuple => "tuple",
                Kind::Set => "set",
                _ => {
                    known = false;
                    continue;
                }
            };
            outs.push(builtin(an, n));
        }
        match &a.kv {
            KeyVal::Bot => {}
            kv if kv.is_literal() => {
                for k in crate::domain::kv_literals(kv).iter() {
                    let n = intern::with_key(k, |kk| match kk {
                        intern::Key::Str(_) => "str",
                        intern::Key::Int(_) => "int",
                        intern::Key::Float(_) => "float",
                        intern::Key::Bool(_) => "bool",
                        intern::Key::None => "NoneType",
                        intern::Key::Bytes(_) => "bytes",
                        intern::Key::Tuple(_) => "tuple",
                        _ => "",
                    });
                    if n.is_empty() {
                        known = false;
                    } else {
                        outs.push(builtin(an, n));
                    }
                }
            }
            _ => known = false,
        }
        if known && !outs.is_empty() {
            return join_values(outs);
        }
    }
    Value::data(&format!("type@L{}", c.line))
}

/// The builtin type a library function's result has, whatever its arguments (`len`: an int;
/// `abs` of an int: an int), when it returns: None when that depends on more than this knows.
pub fn result_type(an: &Analyzer, qual: &str, args: &[Value]) -> Option<&'static str> {
    let arg_type = || args.first().and_then(|a| an.value_type(a));
    match qual.strip_prefix("builtins.").unwrap_or(qual) {
        "len" | "ord" | "hash" | "id" | "int" => Some("int"),
        "str" | "repr" | "ascii" | "chr" | "format" | "json.dumps" => Some("str"),
        "float" => Some("float"),
        "bool" | "isinstance" | "issubclass" | "callable" | "hasattr" => Some("bool"),
        "input" | "os.getcwd" => Some("str"),
        "time.time" | "time.perf_counter" | "time.monotonic" | "time.process_time" | "time.thread_time" | "random.random" => Some("float"),
        "time.time_ns" | "time.perf_counter_ns" | "time.monotonic_ns" | "time.process_time_ns" | "os.getpid" | "os.getppid" | "random.randint" | "random.randrange" | "random.getrandbits" => Some("int"),
        // (`math`'s functions of real numbers: a float, whatever number they are given)
        "math.sqrt" | "math.exp" | "math.expm1" | "math.log" | "math.log2" | "math.log10" | "math.log1p" | "math.sin" | "math.cos" | "math.tan" | "math.asin" | "math.acos" | "math.atan" | "math.atan2" | "math.sinh" | "math.cosh"
        | "math.tanh" | "math.hypot" | "math.pow" | "math.fabs" | "math.radians" | "math.degrees" | "math.fsum" | "math.copysign" | "math.fmod" | "math.dist" => Some("float"),
        "math.isnan" | "math.isinf" | "math.isfinite" | "math.isclose" | "os.path.exists" | "os.path.isfile" | "os.path.isdir" | "os.path.isabs" | "os.path.islink" => Some("bool"),
        "math.factorial" | "math.gcd" | "math.lcm" | "math.comb" | "math.perm" | "math.isqrt" => Some("int"),
        "math.floor" | "math.ceil" | "math.trunc" if matches!(arg_type()?, "int" | "bool" | "float") => Some("int"),
        "abs" => match arg_type()? {
            "bool" | "int" => Some("int"),
            "float" => Some("float"),
            _ => None,
        },
        // (`round(x)` of a number: an int; `round(x, n)`: of x's type)
        "round" if matches!(arg_type()?, "int" | "bool" | "float") => Some(if args.len() == 1 || arg_type() != Some("float") { "int" } else { "float" }),
        _ => None,
    }
}

/// The builtin type of what a method of a str (`ty`: or bytes) returns (`count`: an int).
pub fn str_method_type(qual: &str, ty: &'static str) -> Option<&'static str> {
    let name = last(qual);
    if matches!(name, "count" | "find" | "rfind" | "index" | "rindex") {
        return Some("int");
    }
    if name == "startswith" || name == "endswith" || name.starts_with("is") {
        return Some("bool");
    }
    const SAME: &[&str] = &["upper", "lower", "strip", "lstrip", "rstrip", "title", "capitalize", "casefold", "swapcase", "removeprefix", "removesuffix", "zfill", "center", "ljust", "rjust", "replace", "expandtabs", "join", "format", "format_map", "translate"];
    if SAME.contains(&name) {
        return Some(if matches!(name, "format" | "format_map" | "casefold") { "str" } else { ty });
    }
    match (name, ty) {
        ("encode", "str") => Some("bytes"),
        ("decode", "bytes") => Some("str"),
        _ => None,
    }
}

pub(super) fn m_str(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    // `str(b, encoding[, errors])` decodes (known bytes: models::pytext)
    if c.args.len() > 1 || c.kwargs.keys().any(|k| matches!(&*intern::str_get(*k), "encoding" | "errors")) {
        return Value::data("str");
    }
    if let Some(a) = c.args.first() {
        if let Some(t) = crate::models::enums::enum_text(an, a, st) {
            return t;
        }
        if let Some(t) = crate::models::exceptions::exc_text(an, a, false, st) {
            return t;
        }
        let mut out = if a.kv.is_bot() { Value::new() } else { Value::kv_only(an.rename_kv(&a.kv, &RenameFn::new(FnKind::Str))) };
        // (a text not known: a string all the same, its str methods apply)
        if matches!(out.kv, KeyVal::Data(_, _)) {
            out = Value::data("str");
        }
        // the objects' part (an unknown value, an instance's __str__, …): some string
        if !a.pts.is_empty() {
            out = out.join(&Value::data("str"));
        }
        return out;
    }
    Value::lit_str("")
}

pub(super) fn m_repr(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    if let Some(t) = c.args.first().and_then(|a| crate::models::exceptions::exc_text(an, a, true, st)) {
        return t;
    }
    if let Some(r) = c.args.first().and_then(|a| a.single()).and_then(|o| pathtext::path_repr(an, o)) {
        return Value::lit_str(&r);
    }
    noeffect(an, st, c)
}

/// `int(x)` / `float(x)`: on a known number or numeric string the value; on a known string
/// that is not a number a ValueError raised here; on anything else a value that may raise.
pub(super) fn m_int(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let is_float = c.qual == "builtins.float";
    let Some(a) = c.args.first() else { return Value::lit(intern::key_int(0)) };
    // int(d / k) of a (non-negative) dim: d // k
    if !is_float {
        if let Some((d, k)) = crate::models::torch::quotient_of(a) {
            return crate::models::torch::dim_value(crate::models::shape::norm(crate::models::shape::floordiv(d, crate::models::shape::lit(k)), &st.tf));
        }
        if crate::models::torch::dim_of_label(a).is_some() {
            return a.clone();
        }
    }
    if c.args.len() > 1 || !c.kwargs.is_empty() {
        an.may_raise_here("ValueError", st);
        return Value::data(&format!("L{}", c.line));
    }
    let num = |k: KeyId| -> Option<Result<intern::Key, ()>> {
        intern::with_key(k, |kk| match kk {
            intern::Key::Int(i) => Some(Ok(if is_float { intern::Key::Float((*i as f64).to_bits()) } else { intern::Key::Int(*i) })),
            intern::Key::Bool(b) => Some(Ok(intern::Key::Int(*b as i64))),
            intern::Key::Float(f) => Some(Ok(if is_float { intern::Key::Float(*f) } else { intern::Key::Int(f64::from_bits(*f).trunc() as i64) })),
            intern::Key::Str(s) => {
                let t = s.trim().replace('_', "");
                if is_float {
                    Some(t.parse::<f64>().map(|f| intern::Key::Float(f.to_bits())).map_err(|_| ()))
                } else {
                    Some(t.parse::<i64>().map(intern::Key::Int).map_err(|_| ()))
                }
            }
            _ => None,
        })
    };
    if a.pts.is_empty() {
        if let Some(k) = a.kv.as_lit() {
            match num(k) {
                Some(Ok(key)) => return Value::lit(intern::key(key)),
                Some(Err(())) => {
                    an.raise_here("ValueError", st);
                    return Value::new();
                }
                None => {}
            }
        }
    }
    an.may_raise_here("ValueError", st);
    Value::data(&format!("L{}", c.line))
}

pub(super) fn m_super(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(foid) = an.frame().func else { return Value::data("super") };
    let fi = an.heap.func_info(foid);
    let Some(mut cls) = fi.cls else { return Value::data("super") };
    // (`super(C, self)`: the classes after C in self's class's order)
    if let Some(c0) = c.args.first().and_then(|a| a.single()).filter(|o| an.heap.kind(*o) == Kind::Class && c.args[0].kv.is_bot()) {
        cls = c0;
    }
    // the function's first parameter (`self`, or `cls` in a classmethod / __init_subclass__)
    let first = match fi.node {
        crate::interp::FuncNode::Def(f) => crate::scopes::all_params(&f.parameters).first().map(|p| p.name.id.to_string()).unwrap_or_else(|| "self".into()),
        _ => "self".into(),
    };
    let mut self_v = st.env.get(&s(&first)).cloned().unwrap_or_default();
    let cells = an.frame().cells.clone();
    for (name, cid) in cells.iter() {
        if *intern::str_get(*name) == *first {
            self_v = st.slot(*cid, Ns::Cell, key_str_lit("v"));
        }
    }
    if !c.args.is_empty() {
        if c.args.len() > 1 {
            self_v = c.args[1].clone();
        }
    }
    let ctx = an.frame().ctx.clone();
    let oid = an.heap.alloc_raw(Kind::Super, &format!("super:{}:{}", foid, c.site), &ctx, true, "super()", Payload::Super(cls, self_v.clone()));
    an.heap.obj_mut(oid).payload = Payload::Super(cls, self_v);
    Value::obj(oid)
}

pub(super) fn m_str_fn(an: &mut Analyzer, _st: &mut State, c: &Call) -> Value {
    let fname = last(&c.qual);
    if let Some(a) = c.args.first() {
        let f = FnKind::from_name(fname).filter(|f| matches!(f, FnKind::Upper | FnKind::Lower | FnKind::Strip | FnKind::Title)).unwrap_or(FnKind::Strip);
        return Value::kv_only(an.rename_kv(&a.kv, &RenameFn::new(f)));
    }
    Value::data("str")
}

/// `"{} {name:>4}".format(...)` with a known template and known arguments.
/// `t.format(*args, **kwargs)` / `t.format_map(mapping)`: every field is looked up (names
/// not given as keywords from `**mapping` / the mapping, `.attr` / `[key]` accessors read
/// there, so a missing key is reported); the text when every piece is a known literal.
pub(super) fn format_fields(an: &mut Analyzer, st: &mut State, t: &str, c: &Call, mapping: Option<&Value>) -> Option<String> {
    let lit_of = |v: &Value| -> Option<KeyId> { v.kv.as_lit().filter(|_| v.pts.is_empty()) };
    let chars: Vec<char> = t.chars().collect();
    let mut out = Some(String::new());
    let push = |out: &mut Option<String>, p: Option<String>| {
        match (out.as_mut(), p) {
            (Some(o), Some(p)) => o.push_str(&p),
            _ => *out = None,
        }
    };
    let mut auto = 0usize;
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '{' && chars.get(i + 1) == Some(&'{') {
            push(&mut out, Some("{".into()));
            i += 2;
            continue;
        }
        if ch == '}' && chars.get(i + 1) == Some(&'}') {
            push(&mut out, Some("}".into()));
            i += 2;
            continue;
        }
        if ch == '}' {
            return None;
        }
        if ch != '{' {
            push(&mut out, Some(ch.to_string()));
            i += 1;
            continue;
        }
        // the field runs to the matching `}` (a nested `{w}` in the spec counts)
        let mut depth = 0;
        let mut end = None;
        for (j, x) in chars.iter().enumerate().skip(i + 1) {
            match x {
                '{' => depth += 1,
                '}' if depth == 0 => {
                    end = Some(j);
                    break;
                }
                '}' => depth -= 1,
                _ => {}
            }
        }
        let end = end?;
        let field: String = chars[i + 1..end].iter().collect();
        i = end + 1;
        // field_name [!conv] [:spec]; `[` … `]` may hold `:` / `!`
        let mut name_end = field.len();
        let mut in_br = false;
        for (j, x) in field.char_indices() {
            match x {
                '[' => in_br = true,
                ']' => in_br = false,
                '!' | ':' if !in_br => {
                    name_end = j;
                    break;
                }
                _ => {}
            }
        }
        let name = &field[..name_end];
        let rest = &field[name_end..];
        let (conv, spec) = if let Some(r) = rest.strip_prefix('!') {
            let (cv, sp) = r.split_at(r.find(':').unwrap_or(r.len()));
            (Some(cv.to_string()), sp.strip_prefix(':').unwrap_or("").to_string())
        } else {
            (None, rest.strip_prefix(':').unwrap_or("").to_string())
        };
        let head_end = name.find(|x| x == '.' || x == '[').unwrap_or(name.len());
        let head = &name[..head_end];
        let mut v: Value = if head.is_empty() || head.chars().all(|x| x.is_ascii_digit()) {
            let n = if head.is_empty() {
                auto += 1;
                auto - 1
            } else {
                head.parse::<usize>().ok()?
            };
            match c.args.get(n) {
                Some(v) if c.star_args.is_empty() || n < c.args.len() => v.clone(),
                _ => {
                    push(&mut out, None);
                    continue;
                }
            }
        } else if let Some(v) = c.kwargs.get(&s(head)) {
            v.clone()
        } else if let Some(m) = mapping {
            an.load_item(m, Ns::Item, &Value::lit(key_str_lit(head)), &c.node, st, true)
        } else if !c.star_kwargs.is_empty() {
            let mut acc = Value::new();
            for sk in c.star_kwargs.clone() {
                acc = acc.join(&an.load_item(&sk, Ns::Item, &Value::lit(key_str_lit(head)), &c.node, st, true));
            }
            acc
        } else {
            push(&mut out, None);
            continue;
        };
        // accessors
        let mut acc = &name[head_end..];
        while !acc.is_empty() {
            if let Some(r) = acc.strip_prefix('.') {
                let e = r.find(|x| x == '.' || x == '[').unwrap_or(r.len());
                v = an.load_attr(&v, &r[..e], &c.node, st, true);
                acc = &r[e..];
            } else if let Some(r) = acc.strip_prefix('[') {
                let e = r.find(']')?;
                let k = &r[..e];
                let key = match k.parse::<i64>() {
                    Ok(n) if k.chars().all(|x| x.is_ascii_digit()) => intern::key_int(n),
                    _ => key_str_lit(k),
                };
                v = an.load_item(&v, Ns::Item, &Value::lit(key), &c.node, st, true);
                acc = &r[e + 1..];
            } else {
                return None;
            }
        }
        if out.is_none() || spec.contains('{') {
            out = None;
            continue;
        }
        let Some(k) = lit_of(&v) else {
            out = None;
            continue;
        };
        let k = match conv.as_deref() {
            None | Some("s") => k,
            Some("r") => match crate::models::pyvalues::builtin("repr", &[k], &[]) {
                Some(x) => x,
                None => {
                    out = None;
                    continue;
                }
            },
            Some("a") => match crate::models::pyvalues::builtin("ascii", &[k], &[]) {
                Some(x) => x,
                None => {
                    out = None;
                    continue;
                }
            },
            _ => return None,
        };
        let piece = if spec.is_empty() { Some(intern::key_py_str(k)) } else { crate::interp::ops::format_spec_literal(k, &spec) };
        push(&mut out, piece);
    }
    out
}

/// A str method; one returning a string, on a string whose text is not known, gives a
/// string whose text is not known (its str methods apply in turn: `v.strip().title()`).
pub(super) fn m_str_method(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let out = m_str_method_inner(an, st, c);
    // (`format` / `format_map` report their own raises: their field reads do; a result not
    // computed here may raise anywhere)
    if matches!(last(&c.qual), "format" | "format_map") && !(out.pts.is_empty() && out.kv.is_literal()) {
        an.flag_may_raise(st);
    }
    const STR_OUT: &[&str] = &["upper", "lower", "strip", "lstrip", "rstrip", "title", "capitalize", "casefold", "swapcase", "removeprefix", "removesuffix", "zfill", "center", "ljust", "rjust", "replace", "expandtabs", "join", "format"];
    if !STR_OUT.contains(&last(&c.qual)) || !out.pts.is_empty() {
        return out;
    }
    match &out.kv {
        KeyVal::Data(_, _) | KeyVal::Bot => Value::data("str"),
        _ => out,
    }
}

pub(super) fn m_str_method_inner(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let name = last(&c.qual);
    let kv = c.recv.as_ref().map(|r| r.kv.clone()).unwrap_or(DATA);
    match name {
        // (`strip(chars)`, `lstrip()`, `rstrip()` are not `strip()`: a str not known)
        "upper" | "lower" | "title" | "strip" if c.args.is_empty() && c.kwargs.is_empty() => Value::kv_only(an.rename_kv(&kv, &RenameFn::new(FnKind::from_name(name).unwrap()))),
        // positions / counts of a known substring in a known string
        "find" | "index" | "rfind" | "rindex" | "count" if c.args.len() == 1 => {
            let recv_s = kv.as_lit().and_then(intern::key_as_str);
            let arg_s = c.args[0].kv.as_lit().filter(|_| c.args[0].pts.is_empty()).and_then(intern::key_as_str);
            match (recv_s, arg_s) {
                (Some(h), Some(n)) => {
                    let pos = |p: Option<usize>| p.map(|b| h[..b].chars().count() as i64);
                    let r = match name {
                        "find" | "index" => pos(h.find(&*n)),
                        "rfind" | "rindex" => pos(h.rfind(&*n)),
                        _ => Some(if n.is_empty() { h.chars().count() as i64 + 1 } else { h.matches(&*n).count() as i64 }),
                    };
                    match r {
                        Some(i) => Value::lit(intern::key_int(i)),
                        None if name.ends_with("index") => {
                            an.raise_here("ValueError", st);
                            Value::new()
                        }
                        None => Value::lit(intern::key_int(-1)),
                    }
                }
                _ => Value::data(&format!("L{}", c.line)),
            }
        }
        "capitalize" | "casefold" | "swapcase" | "removeprefix" | "removesuffix" | "zfill" | "center" | "ljust" | "rjust" => {
            // on known strings (and known arguments) the result is known
            let arg_s = c.args.first().and_then(|a| a.kv.as_lit().filter(|_| a.pts.is_empty())).and_then(intern::key_as_str);
            let arg_i = c.args.first().and_then(|a| a.kv.as_lit().filter(|_| a.pts.is_empty())).and_then(intern::key_as_int);
            let f = |s: &str| -> Option<String> {
                Some(match name {
                    "capitalize" => {
                        let mut cs = s.chars();
                        match cs.next() {
                            Some(f0) => f0.to_uppercase().collect::<String>() + &cs.as_str().to_lowercase(),
                            None => String::new(),
                        }
                    }
                    "casefold" => s.to_lowercase(),
                    "swapcase" => s.chars().map(|ch| if ch.is_uppercase() { ch.to_lowercase().collect::<String>() } else { ch.to_uppercase().collect() }).collect(),
                    "removeprefix" => s.strip_prefix(&*arg_s.clone()?).unwrap_or(s).to_string(),
                    "removesuffix" => s.strip_suffix(&*arg_s.clone()?).unwrap_or(s).to_string(),
                    _ => {
                        let w = arg_i? as usize;
                        let n = s.chars().count();
                        if n >= w || c.args.len() > 1 {
                            if c.args.len() > 1 {
                                return None;
                            }
                            s.to_string()
                        } else {
                            let pad = w - n;
                            match name {
                                "zfill" => {
                                    let (sign, rest) = if s.starts_with('-') || s.starts_with('+') { s.split_at(1) } else { ("", s) };
                                    format!("{}{}{}", sign, "0".repeat(pad), rest)
                                }
                                "ljust" => format!("{}{}", s, " ".repeat(pad)),
                                "rjust" => format!("{}{}", " ".repeat(pad), s),
                                _ => format!("{}{}{}", " ".repeat(pad / 2), s, " ".repeat(pad - pad / 2)),
                            }
                        }
                    }
                })
            };
            let lits = crate::domain::kv_literals(&kv);
            if kv.is_literal() && !lits.is_empty() && c.recv.as_ref().map(|r| r.pts.is_empty()).unwrap_or(false) {
                let mut out: Vec<KeyId> = Vec::new();
                for k in lits.iter() {
                    match intern::key_as_str(k).and_then(|s| f(&s)) {
                        Some(r) => out.push(key_str_lit(&r)),
                        None => return Value::data("str"),
                    }
                }
                return Value::kv_only(KeyVal::lits(out.into_iter().collect()));
            }
            Value::data("str")
        }
        "startswith" | "endswith" if c.args.len() == 1 && kv.as_lit().and_then(intern::key_as_str).is_some() && c.args[0].kv.as_lit().and_then(intern::key_as_str).is_some() => {
            let s = intern::key_as_str(kv.as_lit().unwrap()).unwrap();
            let p = intern::key_as_str(c.args[0].kv.as_lit().unwrap()).unwrap();
            Value::lit(intern::key_bool(if name == "startswith" { s.starts_with(&*p) } else { s.ends_with(&*p) }))
        }
        "startswith" | "endswith" if c.args.len() == 1 => {
            Value::data("bool")
        }
        "partition" | "rpartition" if c.args.len() == 1 && kv.as_lit().and_then(intern::key_as_str).is_some() && c.args[0].kv.as_lit().and_then(intern::key_as_str).is_some() => {
            let s = intern::key_as_str(kv.as_lit().unwrap()).unwrap();
            let sep = intern::key_as_str(c.args[0].kv.as_lit().unwrap()).unwrap();
            let parts: [String; 3] = if name == "partition" {
                match s.split_once(&*sep) {
                    Some((a, b)) => [a.to_string(), sep.to_string(), b.to_string()],
                    None => [s.to_string(), String::new(), String::new()],
                }
            } else {
                match s.rsplit_once(&*sep) {
                    Some((a, b)) => [a.to_string(), sep.to_string(), b.to_string()],
                    None => [String::new(), String::new(), s.to_string()],
                }
            };
            let oid = an.alloc_simple(Kind::Tuple, &c.node, st, &format!("tuple@L{}", c.line));
            let vals: Vec<Value> = parts.iter().map(|x| Value::lit(key_str_lit(x))).collect();
            for (i, v) in vals.iter().enumerate() {
                st.set_slot(oid, Ns::Pos, intern::key_int(i as i64), v.clone(), true);
            }
            st.set_slot(oid, Ns::Elem, STAR, Value::new(), true);
            st.elems.insert(oid, join_values(vals.clone()).kv);
            an.heap.set_llen(oid, st, Some(3));
            Value::obj(oid)
        }
        "replace" if c.args.len() >= 2 && c.args[0].kv.as_lit().is_some() && c.args[1].kv.as_lit().is_some() => {
            let arg = intern::key_tuple(vec![c.args[0].kv.as_lit().unwrap(), c.args[1].kv.as_lit().unwrap()]);
            Value::kv_only(an.rename_kv(&kv, &RenameFn::with_arg(FnKind::Replace, arg)))
        }
        "format" | "format_map" => {
            let mapping = if name == "format_map" { c.args.first().cloned() } else { None };
            if name == "format_map" && mapping.is_none() {
                return Value::data(&format!("L{}", c.line));
            }
            // a template from a parameter: decided per call site (with its fields)
            if let Some(t) = kv.as_lit().and_then(intern::key_as_str) {
                if let Some(out) = format_fields(an, st, &t, c, mapping.as_ref()) {
                    return Value::lit(key_str_lit(&out));
                }
                // fields from parameters: decided per call site
            }
            if name == "format_map" {
                return Value::data(&format!("L{}", c.line));
            }
            if let Some(k) = kv.as_lit() {
                if let Some(sk) = intern::key_as_str(k) {
                    if sk.matches("{}").count() == 1 && c.args.len() == 1 && c.args[0].pts.is_empty() {
                        let mut parts = sk.splitn(2, "{}");
                        let pre = parts.next().unwrap_or("").to_string();
                        let post = parts.next().unwrap_or("").to_string();
                        let mut out = c.args[0].kv.clone();
                        if let Some(ok) = out.as_lit() {
                            if !intern::key_is_str(ok) {
                                out = KeyVal::Lit(key_str_lit(&intern::key_py_str(ok)));
                            }
                        }
                        if !pre.is_empty() {
                            out = an.rename_kv(&out, &RenameFn::with_arg(FnKind::Prefix, key_str_lit(&pre)));
                        }
                        if !post.is_empty() {
                            out = an.rename_kv(&out, &RenameFn::with_arg(FnKind::Suffix, key_str_lit(&post)));
                        }
                        return Value::kv_only(out);
                    }
                }
            }
            Value::data(&format!("L{}", c.line))
        }
        "join" => {
            // a known separator joining known strings
            if let (Some(sep), Some(arg)) = (kv.as_lit().and_then(intern::key_as_str), c.args.first()) {
                // a literal tuple key's parts; a column key (`for c in df.columns`): the key renamed
                if arg.pts.is_empty() {
                    let step = crate::source::Step::RenameFn(crate::source::RenameFn::with_arg(crate::source::FnKind::Join, key_str_lit(&sep)));
                    if let crate::domain::KeyVal::KeysOf(ko) = &arg.kv {
                        let mut ko = ko.clone();
                        ko.chain = crate::source::lens_push(ko.chain, step);
                        return Value::kv_only(crate::domain::KeyVal::KeysOf(ko));
                    }
                    let ks = crate::domain::kv_literals(&arg.kv);
                    if !ks.is_empty() && ks.iter().all(|k| intern::with_key(k, |kk| matches!(kk, intern::Key::Tuple(_)))) {
                        let out = intern::KeySet::from_iter(ks.iter().map(|k| step.apply(k)));
                        return Value::kv_only(crate::domain::KeyVal::lits(out));
                    }
                }
                let it = an.iterate(arg, &c.node, st);
                // (items in no promised order, at most 4: every order of them)
                let items = if it.unordered { it.any_order_items().filter(|its| its.len() <= 4) } else { it.items.as_ref() };
                let parts: Option<Vec<String>> = items.and_then(|its| its.iter().map(|item| item.kv.as_lit().filter(|_| item.pts.is_empty()).and_then(intern::key_as_str).map(|s| s.to_string())).collect());
                if let Some(parts) = parts {
                    let orders = if it.unordered { permutations(parts.len()) } else { vec![(0..parts.len()).collect()] };
                    let joined = orders.iter().map(|o| key_str_lit(&o.iter().map(|i| parts[*i].as_str()).collect::<Vec<_>>().join(&*sep)));
                    return Value::kv_only(crate::domain::KeyVal::lits(intern::KeySet::from_iter(joined)));
                }
            }
            Value::data(&format!("L{}", c.line))
        }
        // (the parts of a text not known: texts of its type, `str(?)`; a partition's three)
        "partition" | "rpartition" => {
            let part = part_kv(an, c);
            an.alloc_positional(Kind::Tuple, &c.node, Some(vec![Value::kv_only(part.clone()); 3]), Value::new(), part, st)
        }
        "split" | "splitlines" | "rsplit" => {
            let oid = an.alloc_simple(Kind::List, &c.node, st, &format!("split@L{}", c.line));
            an.heap.set_llen(oid, st, None);
            // a literal string split by a literal separator: the parts, in order; a parameter
            // string is decided per call site
            let sep: Option<Option<String>> = match c.args.first() {
                None => Some(None),
                Some(a) => a.kv.as_lit().and_then(intern::key_as_str).map(|s| Some(s.to_string())),
            };
            if let (Some(k), Some(sep), true) = (kv.as_lit(), sep, name == "split" && an.heap.obj(oid).single) {
                if let Some(text) = intern::key_as_str(k) {
                    let parts: Vec<String> = match sep {
                        Some(sep) if !sep.is_empty() => text.split(sep.as_str()).map(|p| p.to_string()).collect(),
                        _ => text.split_whitespace().map(|p| p.to_string()).collect(),
                    };
                    if parts.len() <= 16 && c.args.len() <= 1 {
                        let mut ks = KeySet::new();
                        for (i, p) in parts.iter().enumerate() {
                            let v = Value::lit(key_str_lit(p));
                            st.set_slot(oid, Ns::Pos, intern::key_int(i as i64), v, true);
                            ks = ks.insert(key_str_lit(p));
                        }
                        an.heap.set_llen(oid, st, Some(parts.len()));
                        st.set_slot(oid, Ns::Elem, STAR, Value::new(), true);
                        st.elems.insert(oid, KeyVal::Lits(ks));
                        return Value::obj(oid);
                    }
                }
            }
            st.set_slot(oid, Ns::Elem, STAR, Value::new(), true);
            st.elems.insert(oid, part_kv(an, c));
            Value::obj(oid)
        }
        _ => Value::data(&format!("L{}", c.line)),
    }
}

/// A part of the receiver of a split or partition rowan does not compute: a text of its type
/// (`str(?)`, `bytes(?)`), or anything when that is not known.
fn part_kv(an: &Analyzer, c: &Call) -> KeyVal {
    match c.recv.as_ref().and_then(|r| an.value_type(r)) {
        Some(t @ ("str" | "bytes")) => KeyVal::data(&crate::domain::typed_label(t, "")),
        _ => DATA,
    }
}

/// `bool(x)`: the literal when the truthiness of `x` is decided, else unknown data (a pandas
/// Series or DataFrame: a ValueError).
pub(super) fn m_bool(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(v) = c.args.first() else { return Value::lit(intern::key(intern::Key::Bool(false))) };
    match an.truth_of(v, st, &c.node) {
        Some(b) => Value::lit(intern::key(intern::Key::Bool(b))),
        None => Value::data("bool"),
    }
}

/// A literal result, a literal tuple as a tuple object.
pub(super) fn pure_result(an: &mut Analyzer, st: &mut State, c: &Call, k: KeyId) -> Value {
    let items = intern::with_key(k, |kk| match kk {
        intern::Key::Tuple(xs) => Some(xs.clone()),
        _ => None,
    });
    match items {
        Some(xs) => {
            let vals: Vec<Value> = xs.iter().map(|x| Value::lit(*x)).collect();
            an.alloc_positional(Kind::Tuple, &c.node, Some(vals), Value::new(), DATA, st)
        }
        None => Value::lit(k),
    }
}

/// `range(...)` with known int bounds: a list whose items are known when it is short; its
/// bounds decide membership at any length.
pub(super) fn m_range(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let ints: Option<Vec<i64>> = c.args.iter().map(|a| if a.pts.is_empty() { a.kv.as_lit().and_then(intern::key_as_int) } else { None }).collect();
    let bounds = match ints.as_deref() {
        Some([stop]) => Some((0, *stop, 1)),
        Some([start, stop]) => Some((*start, *stop, 1)),
        Some([start, stop, step]) if *step != 0 => Some((*start, *stop, *step)),
        _ => None,
    };
    let Some((lo, hi, step)) = bounds.filter(|_| c.kwargs.is_empty()) else {
        let oid = an.alloc_simple(Kind::List, &c.node, st, &format!("range@L{}", c.line));
        an.heap.set_llen(oid, st, None);
        // (for the tensor model: an int of unknown value)
        st.elems.insert(oid, if an.m.torch_on { KeyVal::data(&crate::models::shape::dim_label(crate::models::shape::unknown())) } else { KeyVal::data(&crate::domain::typed_label("int", "")) });
        return Value::obj(oid);
    };
    let len = if step > 0 { (hi - lo + step - 1).max(0) / step } else { (lo - hi - step - 1).max(0) / -step };
    let vals: Option<Vec<Value>> = if len <= 64 { Some((0..len).map(|i| Value::lit(intern::key_int(lo + i * step))).collect()) } else { None };
    let kv = match &vals {
        Some(vs) => KeyVal::lits(vs.iter().filter_map(|v| v.kv.as_lit()).collect()),
        None => KeyVal::data(&crate::domain::typed_label("int", "")),
    };
    let v = an.alloc_positional(Kind::List, &c.node, vals, Value::new(), kv, st);
    if let Some(oid) = v.single() {
        an.heap.obj_mut(oid).payload = Payload::Range(lo, hi, step);
    }
    v
}

/// `any(xs)` / `all(xs)` over known items: decided when the items' truthiness decides it
/// (`any([])` is False, `all([])` is True).
pub(super) fn m_any_all(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let is_any = c.qual == "builtins.any";
    let Some(a) = c.args.first() else { return Value::data("any") };
    // a one-shot iterator: its items taken until one decides, where it then stands (a held
    // generator runs whole all the same: `stopearly.rs` for one made for the call)
    if let Some(o) = a.single().filter(|o| a.kv.is_bot() && an.heap.obj(*o).iterator && an.heap.obj(*o).single) {
        an.sync_live(a, st, true);
        if let Some((c0, n)) = an.positions_left(o, st) {
            for i in c0..c0 + n {
                let v = st.slot(o, Ns::Pos, intern::key_int(i as i64));
                match an.truthiness(&v, st) {
                    Some(b) if b == is_any => {
                        an.set_cursor(o, Some(i + 1), st);
                        return Value::lit(intern::key(intern::Key::Bool(is_any)));
                    }
                    Some(_) if i + 1 == c0 + n => {
                        an.end_iterator(o, st);
                        return Value::lit(intern::key(intern::Key::Bool(!is_any)));
                    }
                    Some(_) => {}
                    None => break,
                }
            }
            if n == 0 {
                an.end_iterator(o, st);
                return Value::lit(intern::key(intern::Key::Bool(!is_any)));
            }
        }
        let r = m_any_all_items(an, st, c, is_any, a);
        an.set_cursor(o, None, st);
        return r;
    }
    m_any_all_items(an, st, c, is_any, a)
}

/// `any` / `all` over what iterating `a` gives at once.
fn m_any_all_items(an: &mut Analyzer, st: &mut State, c: &Call, is_any: bool, a: &Value) -> Value {
    let Some(items) = an.iterate(a, &c.node, st).items else { return Value::data("any") };
    let mut unknown = false;
    for v in &items {
        match an.truthiness(v, st) {
            Some(b) if b == is_any => return Value::lit(intern::key(intern::Key::Bool(is_any))),
            Some(_) => {}
            None => unknown = true,
        }
    }
    if unknown {
        return Value::data("any");
    }
    Value::lit(intern::key(intern::Key::Bool(!is_any)))
}

/// `len(x)`: the count when it is known (a literal string, a container with a known
/// length, a closed record whose keys are all definite), else unknown data.
pub(super) fn m_len(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(v) = c.args.first() else { return Value::data("len") };
    if an.m.torch_on {
        if let Some(r) = crate::models::torch::t_len(an, v, &c.node, st) {
            return r;
        }
    }
    // (None, a number: no length; beside objects, `may_type_error` says it may raise)
    if v.pts.is_empty() && v.kv.is_literal() && !v.kv.may_have_items() && !crate::domain::kv_literals(&v.kv).is_empty() {
        an.raise_here("TypeError", st);
        return Value::new();
    }
    if !v.pts.is_empty() && v.kv.is_literal() && crate::domain::kv_literals(&v.kv).contains(intern::key_none()) {
        let rest = Value::with_kv(v.pts.clone(), crate::domain::kv_exclude(&v.kv, &KeySet::one(intern::key_none())).unwrap_or(crate::domain::BOT));
        return m_len(an, st, &Call { args: vec![rest], ..c.clone() });
    }
    // (a one-shot iterator has no length)
    if v.pts.iter().any(|o| an.heap.obj(o).iterator) {
        let rest = Value::with_kv(v.pts.filter(|o| !an.heap.obj(o).iterator), v.kv.clone());
        if rest.empty() {
            an.raise_here("TypeError", st);
            return Value::new();
        }
        an.may_raise_here("TypeError", st);
        return m_len(an, st, &Call { args: vec![rest], ..c.clone() });
    }
    // the length of a placeholder: decided per call site where it is a known container
    if v.pts.is_empty() {
        if let Some(k) = v.kv.as_lit() {
            if let Some(s) = intern::key_as_str(k) {
                return Value::lit(intern::key_int(s.chars().count() as i64));
            }
        }
        return Value::data("len");
    }
    // lists / tuples whose possible lengths are known (on this path, or as the alternatives of
    // the paths joined here): one of those
    if v.kv.is_bot() && v.pts.iter().all(|o| matches!(an.heap.kind(o), Kind::List | Kind::Tuple)) {
        let mut all: Vec<usize> = Vec::new();
        let mut known = true;
        for o in v.pts.iter() {
            match an.heap.len_alts(o, st) {
                Some(ls) => all.extend(ls),
                None => {
                    known = false;
                    break;
                }
            }
        }
        if known && !all.is_empty() {
            let ks = intern::KeySet::from_iter(all.into_iter().map(|n| intern::key_int(n as i64)));
            return Value::kv_only(crate::domain::KeyVal::lits(ks));
        }
    }
    if let Some(o) = v.single() {
        // a view of one dict: the dict's length
        if let Payload::View(ViewKind::Keys | ViewKind::Values | ViewKind::Items | ViewKind::Proxy, inner) = an.heap.obj(o).payload.clone() {
            if let Some(d) = inner.single().filter(|d| matches!(an.heap.kind(*d), Kind::Dict | Kind::Frame)) {
                let row = st.row(d, Ns::Item);
                if row.closed() && row.present == row.definite {
                    return Value::lit(intern::key_int(row.definite.len() as i64));
                }
                // a closed frame: between its definite and its possible columns
                if an.heap.kind(d) == Kind::Frame && row.closed() && row.present.len() - row.definite.len() <= 8 {
                    let ks = intern::KeySet::from_iter((row.definite.len()..=row.present.len()).map(|n| intern::key_int(n as i64)));
                    return Value::kv_only(crate::domain::KeyVal::lits(ks));
                }
            }
            return Value::data("len");
        }
        if let Some(n) = an.heap.llen(o, st) {
            return Value::lit(intern::key_int(n as i64));
        }
        // (a loop of a tensor dim's rounds appended to it once each)
        if let Some(l) = st.sym_len(o) {
            return Value::data(&intern::str_get(l));
        }
        if matches!(an.heap.kind(o), Kind::Dict) {
            let row = st.row(o, Ns::Item);
            if row.closed() && row.present == row.definite {
                return Value::lit(intern::key_int(row.definite.len() as i64));
            }
            // keys set on some paths: between the sure and the possible count
            if row.closed() && row.present.len() - row.definite.len() <= 8 {
                let ks = intern::KeySet::from_iter((row.definite.len()..=row.present.len()).map(|n| intern::key_int(n as i64)));
                return Value::kv_only(crate::domain::KeyVal::lits(ks));
            }
        }
    }
    Value::data("len")
}

/// `sum(xs)`: the literal sum when every item is a known number, else unknown data.
pub(super) fn m_sum(an: &mut Analyzer, st: &mut State, c: &Call) -> Value {
    let Some(v) = c.args.first() else { return Value::data("sum") };
    let it = an.iterate(v, &c.node, st);
    an.exhaust(v, st);
    // a sum of tensors: the elements' (broadcast) shape
    if an.m.torch_on {
        let e = it.elem.flat();
        if crate::models::torch::is_tensor(&e) {
            if let Some(s) = crate::models::torch::t_shape(an, &e, st) {
                let start = c.args.get(1).and_then(|x| crate::models::torch::t_shape(an, x, st)).unwrap_or_else(|| crate::models::shape::Shape::known(vec![]));
                return crate::models::torch::t_broadcast(an, &s, &start, &c.node, st, "sum");
            }
        }
    }
    // (ints from an int, 0 by default: an int, also of no items)
    let start_v = c.args.get(1).or(c.kwargs.get(&s("start")));
    let ints = |an: &Analyzer, v: &Value| matches!(an.value_type(v), Some("int" | "bool"));
    let unknown = if !an.m.torch_on && start_v.is_none_or(|v| ints(an, v)) && ints(an, &it.elem.flat()) { Value::data(&crate::domain::typed_label("int", "")) } else { Value::data("sum") };
    if let Some(items) = it.items {
        let lit = |v: &Value| v.kv.as_lit().filter(|_| v.pts.is_empty());
        let start = match start_v {
            Some(v) => match lit(v) {
                Some(k) => k,
                None => return unknown,
            },
            None => intern::key_int(0),
        };
        let mut ks: Vec<KeyId> = Vec::with_capacity(items.len());
        for item in &items {
            match lit(item) {
                Some(k) => ks.push(k),
                None => return unknown,
            }
        }
        return match py_sum(start, &ks) {
            Some(k) => Value::lit(k),
            None => unknown,
        };
    }
    unknown
}

/// CPython's `sum(items, start)` on literals: the exact int path, then the compensated
/// (Neumaier) float path, then plain `+`.
pub fn py_sum(start: KeyId, items: &[KeyId]) -> Option<KeyId> {
    let get = |k: KeyId| intern::key_get(k);
    let mut acc = start;
    let mut i = 0;
    // ints (an exact int start)
    if let intern::Key::Int(mut a) = get(acc) {
        while i < items.len() {
            match get(items[i]) {
                intern::Key::Int(x) => a = a.checked_add(x)?,
                intern::Key::Bool(b) => a = a.checked_add(b as i64)?,
                _ => break,
            }
            i += 1;
        }
        acc = intern::key_int(a);
        if i < items.len() {
            acc = pyvalues::operator_call("add", &[acc, items[i]])?;
            i += 1;
        }
    }
    // floats
    if let intern::Key::Float(bits) = get(acc) {
        let mut f = f64::from_bits(bits);
        let mut c = 0.0f64;
        while i < items.len() {
            match get(items[i]) {
                intern::Key::Float(xb) => {
                    let x = f64::from_bits(xb);
                    let t = f + x;
                    if f.abs() >= x.abs() {
                        c += (f - t) + x;
                    } else {
                        c += (x - t) + f;
                    }
                    f = t;
                }
                intern::Key::Int(x) => f += x as f64,
                intern::Key::Bool(b) => f += b as i64 as f64,
                _ => break,
            }
            i += 1;
        }
        if c != 0.0 && c.is_finite() {
            f += c;
        }
        acc = intern::key_float(f);
    }
    while i < items.len() {
        acc = pyvalues::operator_call("add", &[acc, items[i]])?;
        i += 1;
    }
    Some(acc)
}

/// The sort key of one item as literals: the item itself, or `key(item)`, when it is a
/// single literal or a tuple of single literals.
pub(super) fn literal_sort_key(an: &mut Analyzer, st: &mut State, c: &Call, item: &Value) -> Option<Vec<KeyId>> {
    let v = match c.kw("key") {
        Some(kf) if !kf.pts.is_empty() => an.call_value(&kf, &c.node, None, st, Some(vec![item.clone()])),
        Some(_) => return None,
        None => item.clone(),
    };
    if v.pts.is_empty() {
        return v.kv.as_lit().map(|k| vec![k]);
    }
    let o = v.single()?;
    if an.heap.kind(o) == Kind::Pair {
        // a (key, value) pair compares like a 2-tuple
        let Payload::Pair(a, b) = an.heap.obj(o).payload.clone() else { return None };
        let lit = |x: &Value| x.kv.as_lit().filter(|_| x.pts.is_empty());
        let mut ks = vec![lit(&a)?];
        if let Some(y) = lit(&b) {
            ks.push(y);
        }
        return Some(ks);
    }
    if an.heap.kind(o) != Kind::Tuple {
        return None;
    }
    let n = an.heap.llen(o, st)?;
    let mut ks = Vec::new();
    for i in 0..n {
        let s = st.slot(o, Ns::Pos, intern::key_int(i as i64));
        if !s.pts.is_empty() {
            return None;
        }
        ks.push(s.kv.as_lit()?);
    }
    Some(ks)
}

/// The items of `sorted(d.items())` when `d` is a closed dict whose keys are known,
/// pairwise comparable literals and only some of them maybe there: (key, value, sure) in
/// sorted order (the keys present at run time come in that order).
pub(super) fn sorted_dict_items(an: &Analyzer, st: &State, a: &Value, c: &Call) -> Option<Vec<(KeyId, Value, bool)>> {
    let v = a.single().filter(|_| a.kv.is_bot())?;
    let Payload::View(ViewKind::Items, recv) = &an.heap.obj(v).payload else { return None };
    let d = recv.single().filter(|r| recv.kv.is_bot() && an.heap.kind(*r) == Kind::Dict)?;
    let row = st.row(d, Ns::Item);
    if !row.closed() || row.present == row.definite || row.present.len() > 64 || st.has_slot(d, Ns::Item, STAR) || an.m.counters.contains(&d) {
        return None;
    }
    let mut keys: Vec<KeyId> = row.present.iter().collect();
    for i in 0..keys.len() {
        for j in (i + 1)..keys.len() {
            crate::interp::compare::literal_order(keys[i], keys[j])?;
        }
    }
    keys.sort_by(|x, y| crate::interp::compare::literal_order(*x, *y).unwrap());
    let reverse = match c.kw("reverse") {
        None => false,
        Some(r) => intern::with_key(r.kv.as_lit()?, |kk| match kk { intern::Key::Bool(b) => Some(*b), _ => None })?,
    };
    if reverse {
        keys.reverse();
    }
    Some(keys.into_iter().map(|k| (k, st.slot(d, Ns::Item, k), row.definite.contains(k))).collect())
}

/// The permutation `sorted` applies when every key is known (stable), else None.
pub(super) fn literal_sort_order(an: &mut Analyzer, st: &mut State, c: &Call, items: &[Value]) -> Option<Vec<usize>> {
    // instances ordering themselves (no key=): a stable sort on their `__lt__`
    if c.kw("key").is_none() && !items.is_empty() && items.iter().all(|x| x.single().map(|o| an.heap.kind(o) == Kind::Instance).unwrap_or(false)) {
        let n = items.len();
        let mut less = vec![vec![false; n]; n];
        for i in 0..n {
            for j in 0..n {
                if i != j {
                    less[i][j] = an.instance_less(&items[i], &items[j], &c.node, st)?;
                }
            }
        }
        let reverse = c.kw("reverse").map(|r| r.kv.as_lit() == Some(intern::key_bool(true))).unwrap_or(false);
        let mut idx: Vec<usize> = (0..n).collect();
        idx.sort_by(|&a, &b| {
            let o = if less[a][b] { std::cmp::Ordering::Less } else if less[b][a] { std::cmp::Ordering::Greater } else { std::cmp::Ordering::Equal };
            if reverse { o.reverse() } else { o }
        });
        return Some(idx);
    }
    let mut keys: Vec<Vec<KeyId>> = Vec::new();
    for item in items {
        keys.push(literal_sort_key(an, st, c, item)?);
    }
    let reverse = c.kw("reverse").map(|r| matches!(r.kv.as_lit().map(|k| intern::with_key(k, |kk| matches!(kk, intern::Key::Bool(true)))), Some(true))).unwrap_or(false);
    let cmp = |a: &Vec<KeyId>, b: &Vec<KeyId>| -> Option<std::cmp::Ordering> {
        for (x, y) in a.iter().zip(b.iter()) {
            match crate::interp::compare::literal_order(*x, *y)? {
                std::cmp::Ordering::Equal => continue,
                o => return Some(o),
            }
        }
        Some(a.len().cmp(&b.len()))
    };
    // every pair must be comparable
    for i in 0..keys.len() {
        for j in (i + 1)..keys.len() {
            cmp(&keys[i], &keys[j])?;
        }
    }
    let mut idx: Vec<usize> = (0..items.len()).collect();
    idx.sort_by(|&a, &b| {
        let o = cmp(&keys[a], &keys[b]).unwrap_or(std::cmp::Ordering::Equal);
        if reverse { o.reverse() } else { o }
    });
    Some(idx)
}

/// Every order of `0..n` (n small).
fn permutations(n: usize) -> Vec<Vec<usize>> {
    if n == 0 {
        return vec![Vec::new()];
    }
    let mut out = Vec::new();
    for p in permutations(n - 1) {
        for i in 0..=p.len() {
            let mut q = p.clone();
            q.insert(i, n - 1);
            out.push(q);
        }
    }
    out
}
