//! Iteration: what a `for`, a comprehension, unpacking or a builtin (`list`, `zip`, `sorted`,
//! …) sees when it iterates a value. `iterate` gives an `Iteration`: the items in order when
//! they are known (a list of known length, a dict's literal keys, a `zip` of known sides, an
//! iterator whose `__next__` is decided call by call), else none; the element (a value, or a
//! (key, value) pair) every item is; and whether the items are every key of one record. A
//! value of several objects iterates as one of them (alternatives, joined position by
//! position).

use crate::domain::{join_values, kv_literals, ALL_KEYS, DATA, KeyVal, Ns, ObjId, ObjSet, Value};
use crate::frontend::Node;
use crate::intern::{self, key_int, key_str_lit, s, KeyId, KeySet, STAR};
use crate::state::{Kind, State};

use super::{Analyzer, Payload, ViewKind, PATH_DEPTH, UNROLL_MAX};

/// The abstract element of an iteration: a value, or a (key, value) pair.
#[derive(Clone, Debug)]
pub enum Elem {
    One(Value),
    Pair(Value, Value),
}

impl Elem {
    /// The value, or a pair's second part.
    pub(in crate::interp) fn value_part(&self) -> Value {
        match self {
            Elem::One(v) => v.clone(),
            Elem::Pair(_, v) => v.clone(),
        }
    }
    /// Both parts joined (what a pair looks like when forced into one value).
    pub fn flat(&self) -> Value {
        match self {
            Elem::One(v) => v.clone(),
            Elem::Pair(k, v) => k.join(v),
        }
    }
}

/// What iterating a value gives (`Analyzer::iterate`).
pub struct Iteration {
    /// the items in order, when known; items in an order Python does not promise (a set's, a
    /// dict's whose insertion order depends on the path) are each any of them here
    pub items: Option<Vec<Value>>,
    /// those items themselves, in no order, for uses their order does not matter to (a set
    /// built from them, `sorted`, a comprehension's distinct items): `any_order_items`
    unordered_items: Option<Vec<Value>>,
    /// what every item is
    pub elem: Elem,
    /// the items are every key of one record (`for k in d`): a loop over them sees them all
    pub exhaustive: bool,
    /// the items come in an order Python does not promise
    pub unordered: bool,
}

impl Iteration {
    fn new(items: Option<Vec<Value>>, elem: Elem, exhaustive: bool, unordered: bool) -> Iteration {
        match items {
            Some(its) if unordered => {
                let any = join_values(its.iter().cloned());
                Iteration { items: Some(vec![any; its.len()]), unordered_items: Some(its), elem, exhaustive, unordered }
            }
            items => Iteration { items, unordered_items: None, elem, exhaustive, unordered },
        }
    }

    /// The items for a use their order does not matter to: a set's each one of them.
    pub fn any_order_items(&self) -> Option<&Vec<Value>> {
        self.unordered_items.as_ref().or(self.items.as_ref())
    }
}

/// The label of `enumerate`'s index when it is not known: an int.
pub const ENUM_INDEX: &str = "index";

/// What `iterate` gathers over the objects a value may be: their items in order (None: not
/// known), their elements and (key, value) pairs, and whether one record's keys are all of it.
struct ItemsAcc {
    items: Option<Vec<Value>>,
    elems: Vec<Value>,
    pairs: Vec<(Value, Value)>,
    pair: bool,
    exhaustive: bool,
    /// a set's items among them (in an order Python does not promise)
    unordered: bool,
}

impl ItemsAcc {
    /// One more element, or (key, value) pair, the items may be.
    fn add_elem(&mut self, e: Elem) {
        match e {
            Elem::One(e) => self.elems.push(e),
            Elem::Pair(k, v) => {
                self.pair = true;
                self.pairs.push((k, v));
            }
        }
    }
}

impl Analyzer {
    /// Iterating `v` (a generator among it runs first): per kind of object its items and
    /// element (`iter_sequence`, `iter_mapping`, `iter_view`, `iter_zip`, …), and the scalar
    /// part's (`iter_scalar`).
    pub fn iterate(&mut self, v: &Value, node: &Node, state: &mut State) -> Iteration {
        let Some(v) = self.iterable_value(v, state) else {
            return Iteration::new(None, Elem::One(Value::new()), false, false);
        };
        let v = &v;
        self.sync_chainmaps(v, state);
        self.force_gen(v, state, false);
        self.sync_live(v, state, true);
        // (a loop reads the containers it is over at each step, through a live iterator too:
        // later code may change them)
        if !self.gens.reads.is_empty() {
            for o in v.pts.iter().flat_map(|o| self.iter_sources(o, state)).filter(|o| matches!(self.heap.kind(*o), Kind::List | Kind::Dict | Kind::Set) && !self.heap.obj(*o).iterator).collect::<Vec<_>>() {
                self.gens.read_iterated(o);
            }
        }
        for o in v.pts.iter() {
            self.gens.read(o, None);
        }
        // one of several containers (or strings): their items are alternatives, not one
        // sequence — joined position by position when every one is a sequence of one known
        // length, else not known (the elements are still the join)
        let n_alts = v.pts.len() + usize::from(!v.kv.is_bot());
        let multi_lits = v.pts.is_empty() && matches!(&v.kv, KeyVal::Lits(ks) if ks.len() > 1);
        let alts_mode = n_alts > 1 || multi_lits;
        let alt_items: Option<Vec<Value>> = if alts_mode { self.alternative_items(v, state) } else { None };
        let mut acc = ItemsAcc { items: Some(Vec::new()), elems: Vec::new(), pairs: Vec::new(), pair: false, exhaustive: true, unordered: false };
        let line_data = Value::data(&format!("L{}", node.line));
        for oid in v.pts.iter() {
            let kind = self.heap.kind(oid);
            match kind {
                Kind::List | Kind::Tuple | Kind::Set => self.iter_sequence(oid, node, state, &mut acc),
                Kind::Dict if crate::models::pandas::is_series_obj(self, oid) => self.iter_series(oid, node, state, &mut acc),
                Kind::Dict | Kind::Frame => self.iter_mapping(oid, Ns::Item, state, &mut acc),
                Kind::View => self.iter_view(oid, node, state, &mut acc),
                // an sqlite cursor: its remaining rows
                Kind::Namespace if self.m.sql_cursors.contains_key(&oid) => self.iter_cursor(oid, node, state, &mut acc),
                // a file object: its lines (str, or bytes in a binary mode), how many not known
                Kind::Namespace if crate::models::csvio::file_type(self, oid).is_some() => {
                    acc.items = None;
                    acc.exhaustive = false;
                    acc.elems.push(crate::models::csvio::file_text(self, &Value::obj(oid), "line"));
                }
                // itertools.count / cycle / repeat: no end
                Kind::Namespace if self.m.infinites.contains_key(&oid) => {
                    acc.items = None;
                    acc.exhaustive = false;
                    let inf = self.m.infinites[&oid].clone();
                    acc.elems.push(crate::models::itertools::infinite_elem(self, &inf));
                }
                Kind::Zip => self.iter_zip(oid, node, state, &mut acc),
                Kind::Enumerate => self.iter_enumerate(oid, node, state, &mut acc),
                Kind::Class if self.m.enum_members.contains_key(&oid) => {
                    // an Enum class: its members in order
                    let ms: Vec<Value> = self.m.enum_members[&oid].iter().map(|(_, m)| Value::obj(*m)).collect();
                    if let Some(its) = acc.items.as_mut() {
                        its.extend(ms.iter().cloned());
                    }
                    acc.elems.push(join_values(ms));
                }
                Kind::Instance if self.heap.instance_class(oid).and_then(|c| self.class_lookup(c, "__iter__")).is_some() => self.iter_user(oid, node, state, &mut acc),
                // (the program's instance without `__iter__`: `__getitem__` from 0 until it
                // raises, as library code runs its dunders)
                Kind::Instance if self.heap.instance_class(oid).is_some_and(|c| self.plain_program_class(c)) => {
                    self.instance_dunders(vec![oid], &["__getitem__", "__len__"], state, node);
                    acc.items = None;
                    acc.exhaustive = false;
                    acc.elems.push(line_data.clone());
                }
                Kind::Unknown => self.iter_unknown(oid, node, state, &mut acc),
                _ => {
                    acc.items = None;
                    acc.exhaustive = false;
                    acc.elems.push(line_data.clone());
                }
            }
        }
        let ItemsAcc { mut items, mut elems, pairs, pair, mut exhaustive, unordered } = acc;
        // (a running generator iterating what its consumer changes between its items, `Gens::havoc`:
        // its later items are not these, and may be anything)
        if self.consumer_changes(v, state) {
            items = None;
            exhaustive = false;
            elems.push(line_data.clone());
        }
        // (and a part that is no object: an unknown, a str)
        if v.pts.is_empty() || v.kv.may_have_items() {
            self.iter_scalar(v, state, &line_data, &mut items, &mut elems, &mut exhaustive);
        }
        if let Some(its) = &items {
            // no items *and* no containers: nothing is known; empty containers and strings
            // iterate zero times
            if its.is_empty() && v.pts.is_empty() && !(v.kv.is_literal() && !v.kv.is_bot()) {
                items = None;
            }
        }
        if pair {
            let mut kacc = join_values(pairs.iter().map(|p| p.0.clone()));
            let vacc = join_values(pairs.iter().map(|p| p.1.clone()));
            for e in &elems {
                kacc = kacc.join(e);
            }
            // (pairs whose first items are a dict's keys: `d.items()`, not an `enumerate`'s)
            let exh = exhaustive && pairs.len() == 1 && matches!(kacc.kv, KeyVal::KeysOf(_));
            if alts_mode {
                items = alt_items;
            }
            return Iteration::new(items, Elem::Pair(kacc, vacc), exh, unordered);
        }
        let elem = join_values(elems.iter().cloned());
        let exh = exhaustive && elems.len() == 1 && matches!(elem.kv, KeyVal::KeysOf(_));
        if alts_mode {
            items = alt_items;
        }
        Iteration::new(items, Elem::One(elem), exh, unordered)
    }

    /// What of `v` Python can iterate: numbers, None and the program's instances whose class has
    /// neither `__iter__` nor `__getitem__` raise TypeError, an unknown may (None: nothing is
    /// left, the path ends).
    fn iterable_value(&mut self, v: &Value, state: &mut State) -> Option<Value> {
        let v = self.iterable_part(v, state);
        let v = self.without_dunders(&v, &["__iter__", "__getitem__"], state).filter(|v| !v.empty())?;
        if !self.unwind.tries.is_empty() && self.untyped_operand(&v) {
            self.may_raise_here("TypeError", state);
        }
        Some(v)
    }

    /// Does the consumer of the running generator change what `v` is or reads (its containers,
    /// a live iterator's) between the generator's items (`Gens::havoc`)?
    fn consumer_changes(&self, v: &Value, state: &State) -> bool {
        let Some(hits) = self.gens.reads.last().and_then(|r| self.gens.havoc.get(&r.0)).filter(|h| !h.is_empty()) else { return false };
        // (a container's: an iterator object changed is where it stands, which its consumer moves)
        v.pts.iter().flat_map(|o| self.iter_sources(o, state)).filter(|o| matches!(self.heap.kind(*o), Kind::List | Kind::Dict | Kind::Set) && !self.heap.obj(*o).iterator).any(|o| hits.iter().any(|h| h.0 == o))
    }

    /// `o` and what it iterates over, at any depth: a live iterator's containers, a lazy
    /// iterator's sources (`zip`, `enumerate`, `map` over them).
    fn iter_sources(&self, o: ObjId, state: &State) -> Vec<ObjId> {
        let (mut out, mut work) = (Vec::new(), vec![o]);
        while let Some(x) = work.pop() {
            if out.contains(&x) || out.len() > 16 {
                continue;
            }
            out.push(x);
            if self.m.live_iters.contains_key(&x) {
                work.extend(state.slot_opt(x, Ns::Attr, intern::key_str_lit("\u{1}src")).iter().flat_map(|s| s.pts.iter()));
            }
            work.extend(self.m.lazy_src.get(&x).into_iter().flatten().map(|(s, _)| *s));
            match &self.heap.obj(x).payload {
                Payload::Enumerate(v, _) => work.extend(v.pts.iter()),
                Payload::Zip(a, b) => work.extend(a.pts.iter().chain(b.pts.iter())),
                _ => {}
            }
        }
        out
    }

    /// The items of one of several containers (or strings), position by position: when every
    /// one is a list / tuple / string of one known length (at most `UNROLL_MAX`), the join of
    /// the alternatives at each position.
    fn alternative_items(&self, v: &Value, state: &State) -> Option<Vec<Value>> {
        let mut lists: Option<Vec<Vec<Value>>> = Some(Vec::new());
        for o in v.pts.iter() {
            let seq = matches!(self.heap.kind(o), Kind::List | Kind::Tuple) && !self.heap.obj(o).iterator;
            match (seq, self.heap.llen(o, state), lists.as_mut()) {
                (true, Some(n), Some(ls)) if n <= UNROLL_MAX => ls.push((0..n).map(|i| state.slot(o, Ns::Pos, key_int(i as i64))).collect()),
                _ => lists = None,
            }
        }
        match (&v.kv, lists.as_mut()) {
            (KeyVal::Bot, _) => {}
            (kv, Some(ls)) if kv.is_literal() => {
                for k in kv_literals(kv).iter() {
                    match intern::key_as_str(k) {
                        Some(s) if s.chars().count() <= UNROLL_MAX => ls.push(s.chars().map(|ch| Value::lit(intern::key_str_lit(&ch.to_string()))).collect()),
                        _ => {
                            lists = None;
                            break;
                        }
                    }
                }
            }
            _ => lists = None,
        }
        lists.filter(|ls| !ls.is_empty() && ls.iter().all(|l| l.len() == ls[0].len())).map(|ls| (0..ls[0].len()).map(|i| join_values(ls.iter().map(|l| l[i].clone()).collect::<Vec<_>>())).collect())
    }

    /// A list, tuple or set: its positions from the cursor on (a generator partly consumed).
    fn iter_sequence(&mut self, oid: ObjId, node: &Node, state: &mut State, acc: &mut ItemsAcc) {
        let ll = self.heap.llen(oid, state);
        acc.unordered |= self.heap.kind(oid) == Kind::Set && ll != Some(1);
        // (a sequence of some of a dict's keys need not hold them all, `[k for k in d if …]`:
        // only `list(d)` as made, as long as the dict's keys are known)
        acc.exhaustive &= match state.elems.get(&oid) {
            Some(KeyVal::KeysOf(ko)) if ko.exhaustive_depth == ALL_KEYS && ko.objs.len() == 1 && ko.exclude.is_empty() => {
                let row = state.row(ko.objs.first().unwrap(), ko.ns);
                !row.open() && row.present == row.definite && ll == Some(row.present.len()) && ko.then.difference(&row.present).is_empty()
            }
            _ => false,
        };
        // a generator partly consumed by next(): iteration starts at its cursor
        let cursor = self.cursor_of(oid, state);
        match (ll, acc.items.as_mut(), cursor) {
            (Some(n), Some(its), Some(cursor)) => {
                for i in cursor.min(n)..n {
                    its.push(state.slot(oid, Ns::Pos, key_int(i as i64)));
                }
            }
            // (a one-shot iterator used up: nothing more)
            (_, Some(_), None) if self.at_end(oid, state) => {}
            _ => acc.items = None,
        }
        acc.elems.push(self.elem_of(&Value::obj(oid), node, state));
    }

    /// A dict or frame: its keys (in order when literal); `obj.__dict__`: obj's attribute names
    /// (`ns` Attr).
    fn iter_mapping(&mut self, oid: ObjId, ns: Ns, state: &mut State, acc: &mut ItemsAcc) {
        // (a dict with literal keys, a module constant: its items)
        let lit = self.literal_items_in(oid, ns, state);
        match (lit, acc.items.as_mut()) {
            (Some(l), Some(its)) => its.extend(l.into_iter().map(|(k, _)| Value::lit(k))),
            // (known keys in no known order: each item is any of them, as a set's)
            (None, Some(its)) => match self.unordered_keys_in(oid, ns, state) {
                Some(ks) => {
                    its.extend(ks.into_iter().map(Value::lit));
                    acc.unordered = true;
                }
                None => acc.items = None,
            },
            _ => acc.items = None,
        }
        acc.elems.push(Value::kv_only(KeyVal::KeysOf(state.keys_of(ObjSet::one(oid), ns))));
    }

    /// A pandas Series: its values (not its labels, as a dict's keys).
    fn iter_series(&mut self, oid: ObjId, node: &Node, state: &mut State, acc: &mut ItemsAcc) {
        let (items, e) = crate::models::pandas::series_values(state, oid, node.line);
        match (items, acc.items.as_mut()) {
            (Some(items), Some(its)) => its.extend(items),
            _ => acc.items = None,
        }
        acc.elems.push(e);
    }

    /// A view: keys, values or (key, value) items of its dict; a frame's rows; an object's attributes.
    fn iter_view(&mut self, oid: ObjId, _node: &Node, state: &mut State, acc: &mut ItemsAcc) {
        let Payload::View(vk, recv) = self.heap.obj(oid).payload.clone() else { return };
        // `obj.__dict__`: obj's attribute names (an instance's, a function's, a namespace's
        // whose rows hold every name it has)
        if vk == ViewKind::Attrs {
            match recv.single().filter(|i| self.attrs_are_dict(*i)) {
                Some(i) => self.iter_mapping(i, Ns::Attr, state, acc),
                None => {
                    acc.items = None;
                    acc.exhaustive = false;
                    acc.elems.push(Value::data("attribute name"));
                }
            }
            return;
        }
        if matches!(vk, ViewKind::Rows | ViewKind::Elems) {
            acc.items = None;
            acc.exhaustive = false;
            if vk == ViewKind::Rows {
                acc.pair = true;
                acc.pairs.push((Value::data("index"), recv));
            } else {
                acc.elems.push(recv);
            }
            return;
        }
        // (a view of `obj.__dict__`: of obj's attributes)
        let Some((recv, ns)) = self.mapping_parts(&recv) else {
            acc.items = None;
            acc.exhaustive = false;
            let any = Value::data("attribute");
            match vk {
                ViewKind::Keys | ViewKind::Proxy | ViewKind::Values => acc.elems.push(any),
                _ => {
                    acc.pair = true;
                    acc.pairs.push((any.clone(), any));
                }
            }
            return;
        };
        // (of a dict annotated `dict[K, V]` for any call: its keys as the annotation and the
        // program's stores say)
        let annotated = recv.single().filter(|r| ns == Ns::Item && recv.kv.is_bot() && self.heap.kind(*r) == Kind::Unknown && self.heap.obj(*r).depth < PATH_DEPTH);
        let keys = match annotated.and_then(|r| self.annotated_keys(r, state)) {
            Some(k) => k,
            None => Value::kv_only(KeyVal::KeysOf(state.keys_of(recv.pts.clone(), ns))),
        };
        let vals = join_values(recv.pts.iter().map(|r| self.record_values(r, ns, state)).collect::<Vec<_>>());
        let lit = if recv.pts.len() == 1 { self.literal_items_in(recv.pts.first().unwrap(), ns, state) } else { None };
        match (lit, acc.items.as_mut()) {
            (Some(l), Some(its)) => {
                for (k, val) in l {
                    match vk {
                        ViewKind::Keys | ViewKind::Proxy => its.push(Value::lit(k)),
                        ViewKind::Values => its.push(val),
                        _ => {
                            let pid = self.heap.alloc_raw(Kind::Pair, &format!("pair:{}:{}", oid, intern::key_repr(k)), "", true, "pair", Payload::Pair(Value::lit(k), val.clone()));
                            self.heap.obj_mut(pid).payload = Payload::Pair(Value::lit(k), val);
                            its.push(Value::obj(pid));
                        }
                    }
                }
            }
            // (known keys in no known order: each item is any of them, as a set's)
            (None, Some(its)) if matches!(vk, ViewKind::Keys | ViewKind::Proxy) => match recv.single().and_then(|r| self.unordered_keys_in(r, ns, state)) {
                Some(ks) => {
                    its.extend(ks.into_iter().map(Value::lit));
                    acc.unordered = true;
                }
                None => acc.items = None,
            },
            _ => acc.items = None,
        }
        match vk {
            ViewKind::Keys | ViewKind::Proxy => acc.elems.push(keys),
            ViewKind::Values => acc.elems.push(vals),
            _ => {
                acc.pair = true;
                acc.pairs.push((keys, vals));
            }
        }
    }

    /// `zip(a, b)`: (a item, b item) pairs, as many as the shorter side has.
    fn iter_zip(&mut self, oid: ObjId, node: &Node, state: &mut State, acc: &mut ItemsAcc) {
        let Payload::Zip(ks, vs) = self.heap.obj(oid).payload.clone() else { return };
        let mut ki = self.iterate(&ks, node, state);
        let mut vi = self.iterate(&vs, node, state);
        // an infinite side (or a generator known that far) yields as many items as the
        // other has
        if let (None, Some(n)) = (&ki.items, vi.items.as_ref().map(|x| x.len())) {
            if let Some(its) = crate::models::itertools::take_known(self, &ks, n, state) {
                ki.items = Some(its);
            }
        }
        if let (None, Some(n)) = (&vi.items, ki.items.as_ref().map(|x| x.len())) {
            if let Some(its) = crate::models::itertools::take_known(self, &vs, n, state) {
                vi.items = Some(its);
            }
        }
        // a strict zip that did not raise: both sides as long as the known one
        let strict = self.m.zip_strict.contains(&oid);
        let (ki_items, vi_items) = match (ki.items.clone(), vi.items.clone()) {
            (Some(k), None) if strict => {
                let e = self.elem_value(&vi.elem, node, "zv");
                let n = k.len();
                (Some(k), Some(vec![e; n]))
            }
            (None, Some(v)) if strict => {
                let e = self.elem_value(&ki.elem, node, "zk");
                let n = v.len();
                (Some(vec![e; n]), Some(v))
            }
            (k, v) => (k, v),
        };
        // (zip stops at the shorter input; a one-shot iterator: from where it stands)
        let pairs = match (ki_items, vi_items) {
            (Some(kit), Some(vit)) => self.items_from_cursor(oid, kit.into_iter().zip(vit).collect(), state),
            _ => None,
        };
        match (pairs, acc.items.as_mut()) {
            (Some(pairs), Some(its)) => {
                let ctx = self.frame().ctx.clone();
                for (k, val) in pairs {
                    let pid = self.heap.alloc_raw(Kind::Pair, &format!("pair:{}:{}", self.site_of(node), its.len()), &ctx, true, "pair", Payload::Pair(k.clone(), val.clone()));
                    self.heap.obj_mut(pid).payload = Payload::Pair(k, val);
                    its.push(Value::obj(pid));
                }
            }
            _ => acc.items = None,
        }
        acc.pair = true;
        let (kv1, vv1) = (self.elem_value(&ki.elem, node, "zk"), self.elem_value(&vi.elem, node, "zv"));
        acc.pairs.push((kv1, vv1));
    }

    /// `enumerate(it, start)`: (index, item) pairs.
    fn iter_enumerate(&mut self, oid: ObjId, node: &Node, state: &mut State, acc: &mut ItemsAcc) {
        let Payload::Enumerate(inner, start) = self.heap.obj(oid).payload.clone() else { return };
        let ii = self.iterate(&inner, node, state);
        // (known items: (index, item) pairs with literal indices, from where it stands)
        let pairs = match (ii.items, start) {
            (Some(inner_items), Some(start)) => self.items_from_cursor(oid, inner_items.into_iter().enumerate().map(|(i, x)| (start + i as i64, x)).collect(), state),
            _ => None,
        };
        match (pairs, acc.items.as_mut()) {
            (Some(pairs), Some(its)) => {
                let ctx = self.frame().ctx.clone();
                for (i, item) in pairs {
                    let idx = Value::lit(key_int(i));
                    let pid = self.heap.alloc_raw(Kind::Pair, &format!("pair:{}:{}", self.site_of(node), its.len()), &ctx, true, "pair", Payload::Pair(idx.clone(), item.clone()));
                    self.heap.obj_mut(pid).payload = Payload::Pair(idx, item);
                    its.push(Value::obj(pid));
                }
            }
            _ => acc.items = None,
        }
        acc.pair = true;
        let iv = self.elem_value(&ii.elem, node, "en");
        // (an int whatever it enumerates: `start` must be one)
        acc.pairs.push((Value::data(&crate::domain::typed_label("int", ENUM_INDEX)), iv));
    }

    /// An sqlite cursor: its remaining rows.
    fn iter_cursor(&mut self, oid: ObjId, node: &Node, state: &mut State, acc: &mut ItemsAcc) {
        if let Some(rows) = crate::models::sqlite::cursor_rows(self, state, node, oid) {
            state.set_slot(oid, Ns::Attr, key_str_lit("\u{1}pos"), Value::data("pos"), true);
            let it = self.iterate(&rows, node, state);
            match (it.items, acc.items.as_mut()) {
                (Some(xs), Some(its)) => its.extend(xs),
                _ => acc.items = None,
            }
            acc.elems.push(it.elem.flat());
        }
    }

    /// An instance of a class with `__iter__`: what its iterator's `__next__` gives (in order, when
    /// decided call by call), or what the iterable `__iter__` returns gives.
    fn iter_user(&mut self, oid: ObjId, node: &Node, state: &mut State, acc: &mut ItemsAcc) {
        // a user iterable: what its `__iter__` returns
        let r = self.call_method_with(&Value::obj(oid), "__iter__", Vec::new(), node, state).unwrap_or_default();
        // an iterator (`return self`, an object with `__next__`): its items are what
        // `__next__` returns (iterating it again would ask `__iter__` forever)
        let iters: Vec<u32> = r.pts.iter().filter(|p| *p == oid || (self.heap.kind(*p) == Kind::Instance && self.heap.instance_class(*p).and_then(|c| self.class_lookup(c, "__next__")).is_some())).collect();
        // (one iterator whose `__next__` is decided call by call — an item, or surely
        // StopIteration — and changes nothing but its own state and the items it makes:
        // its items, in order, up to UNROLL_MAX)
        if iters.len() == 1 && r.pts.len() == 1 && r.kv.is_bot() && acc.items.is_some() {
            let it = iters[0];
            if let Some(seq) = self.unroll_next(it, node, state) {
                if let Some(its) = acc.items.as_mut() {
                    its.extend(seq.iter().cloned());
                }
                acc.elems.push(join_values(seq));
                return;
            }
        }
        if !iters.is_empty() {
            acc.items = None;
            acc.exhaustive = false;
            // `__next__` once per item, any number of times (none: StopIteration at once)
            let its = Value::objs(iters.iter().copied().collect());
            let mut nexts = Value::new();
            let (head, _, _) = self.loop_fixpoint(state, &mut |an, s| {
                let (v, rest) = an.run_dunder(&its, &["__next__"], Vec::new(), node, s).unwrap_or((Value::new(), its.clone()));
                nexts = nexts.join(&v);
                if !rest.empty() {
                    nexts = nexts.join(&Value::data("next"));
                }
            });
            state.replace_with(head);
            acc.elems.push(nexts);
            let rest = Value::with_kv(r.pts.filter(|p| !iters.contains(&p)), r.kv.clone());
            if rest.empty() {
                return;
            }
            let sub = self.iterate(&rest, node, state);
            acc.add_elem(sub.elem);
            return;
        }
        let sub = self.iterate(&r, node, state);
        match (sub.items, acc.items.as_mut()) {
            (Some(si), Some(its)) => its.extend(si),
            _ => acc.items = None,
        }
        acc.exhaustive = false;
        acc.add_elem(sub.elem);
    }

    /// The items of an iterator object through `__next__`, when every call is decided (None:
    /// not decided, or it changes other objects than itself and the items it returns).
    fn unroll_next(&mut self, it: ObjId, node: &Node, state: &mut State) -> Option<Vec<Value>> {
        let m = self.load_attr(&Value::obj(it), "__next__", node, state, false);
        if m.pts.is_empty() {
            return None;
        }
        // (a `__next__` found not unrollable once is not probed again: the probes cost work)
        let fkey: Vec<ObjId> = m.pts.iter().map(|b| match &self.heap.obj(b).payload {
            Payload::Bound(f, _) => *f,
            _ => b,
        }).collect();
        if fkey.iter().any(|f| self.gens.no_unroll_next.contains(f)) {
            return None;
        }
        let r = self.unroll_next_inner(it, &m, node, state);
        if r.is_none() {
            self.gens.no_unroll_next.extend(fkey);
        }
        r
    }

    /// `unroll_next`'s probe: `__next__` called up to `UNROLL_MAX` times, each in a try catching
    /// StopIteration, as long as each call is decided (an item, or surely StopIteration) and
    /// changes only the iterator and what it holds.
    fn unroll_next_inner(&mut self, it: ObjId, m: &Value, node: &Node, state: &mut State) -> Option<Vec<Value>> {
        let m = m.clone();
        let mut cur = state.copy();
        let mut seq: Vec<Value> = Vec::new();
        for _ in 0..super::UNROLL_MAX {
            let mark = self.heap.len() as ObjId;
            let mut probe = cur.copy();
            // (a frame that catches StopIteration only, as the for loop does: other raises and
            // the reads in the call behave as in the loop; the classes raised as a try records
            // them)
            let mut pre = probe.copy();
            self.enter_try(rustc_hash::FxHashSet::from_iter(["StopIteration".to_string()]), &mut pre);
            let v = self.call_value(&m, node, None, &mut probe, Some(Vec::new()));
            let out = self.leave_try();
            let (flagged, raised_at) = (out.any(), out.raised());
            let recs = out.recs;
            let only_stop = !recs.is_empty() && recs.iter().all(|(c, _, _)| c.as_ref().is_some_and(|(names, _)| names.iter().any(|n| n == "StopIteration")));
            let level = if flagged { 1 } else if raised_at.is_none() && probe.reachable { 0 } else if !probe.reachable && raised_at.is_some() { 2 } else { 1 };
            // (effects only on the iterator, the objects its attributes hold — its own state,
            // like the list it pops from — and on objects the call made)
            let fp = probe.footprint(&cur);
            let held: Vec<ObjId> = cur.slots_of(it).iter().filter(|((ns, _), _)| *ns == Ns::Attr).flat_map(|(_, v)| v.pts.to_vec()).collect();
            let own = |o: ObjId| o == it || o >= mark || held.contains(&o);
            if fp.rows.iter().any(|(o, _)| !own(*o)) || fp.slots.iter().any(|(o, _)| !own(*o)) || fp.elems.iter().any(|o| !own(*o)) {
                return None;
            }
            match level {
                0 => {
                    // (the items so far live in the state while the next call runs: a record
                    // made at the same site again is then a new object, not the earlier item)
                    probe.env.insert(s(&format!("\u{1}next{}", seq.len())), v.clone());
                    seq.push(v);
                    cur = probe;
                }
                2 if only_stop => {
                    // (the loop goes on from where StopIteration was raised)
                    if let Some(mut rs) = raised_at.and_then(|_| crate::interp::join_raise_recs(&recs)) {
                        rs.reachable = true;
                        cur = rs;
                    }
                    for i in 0..seq.len() {
                        cur.env.shift_remove(&s(&format!("\u{1}next{}", i)));
                    }
                    state.replace_with(cur);
                    return Some(seq);
                }
                _ => return None,
            }
        }
        None
    }

    /// An unknown iterable: one element object standing for each of its elements.
    fn iter_unknown(&mut self, oid: ObjId, _node: &Node, state: &mut State, acc: &mut ItemsAcc) {
        // (a slice of an unknown sequence: its elements)
        if let Some(bs) = self.slice_base(oid) {
            for b in bs.iter() {
                self.iter_unknown(b, _node, state, acc);
            }
            return;
        }
        acc.items = None;
        acc.exhaustive = false;
        // (a dict annotated `dict[str, V]`: its keys, and those the program put into it)
        if self.heap.obj(oid).depth < PATH_DEPTH {
            if let Some(v) = self.annotated_keys(oid, state) {
                acc.elems.push(v);
                return;
            }
        }
        // (an annotated container's: what the program put into it too)
        if let Some(put) = self.items_put(oid, state) {
            acc.elems.push(put);
        }
        if self.heap.obj(oid).depth >= PATH_DEPTH {
            acc.elems.push(Value::data("deep"));
            return;
        }
        // (an element annotated with a class of the program: an instance of it)
        if let Some(v) = self.part_instance(oid, STAR, state) {
            acc.elems.push(v);
            return;
        }
        let row = state.row(oid, Ns::Item);
        let label = format!("elem of {}", self.heap.obj(oid).label);
        let eid = self.heap.alloc_raw(Kind::Unknown, &format!("elem:{}", oid), "", false, &label, Payload::None);
        self.fill_placeholder(eid, oid, Ns::Item, STAR, &row, true, state);
        acc.elems.push(Value::obj(eid));
    }

    /// Iterating a value of no object (a str, several literals, a tensor's rows, unknown): its
    /// characters / literals as the elements (and items, when few and known).
    fn iter_scalar(&mut self, v: &Value, state: &State, line_data: &Value, items: &mut Option<Vec<Value>>, elems: &mut Vec<Value>, exhaustive: &mut bool) {
        *items = None;
        *exhaustive = false;
        let one_str = v.kv.as_lit().and_then(intern::key_as_str);
        if let Some(text) = one_str {
            // one known string: its characters
            let chars: Vec<KeyId> = text.chars().map(|ch| intern::key_str_lit(&ch.to_string())).collect();
            if chars.len() <= UNROLL_MAX {
                *items = Some(chars.iter().map(|k| Value::lit(*k)).collect());
            }
            let ks: KeySet = chars.into_iter().collect();
            elems.push(Value::kv_only(KeyVal::lits(ks)));
        } else if v.kv.is_literal() {
            let ks = kv_literals(&v.kv);
            // the items of each literal: a str's characters, bytes' ints, a tuple's or frozenset's
            // items (one literal alone: in order); an object's identity key, any
            // (the key read first, interned after: interning while reading one borrows twice)
            let parts: Vec<Option<Vec<KeyId>>> = ks.iter().map(|k| match intern::key_get(k) {
                intern::Key::Str(t) => Some(t.chars().map(|ch| intern::key_str_lit(&ch.to_string())).collect()),
                intern::Key::Bytes(b) => Some(b.iter().map(|x| intern::key_int(*x as i64)).collect()),
                intern::Key::Tuple(xs) | intern::Key::Frozen(xs) => Some(xs),
                _ => None,
            }).collect();
            if let [Some(one)] = &parts[..] {
                if one.len() <= UNROLL_MAX && !intern::with_key(ks.iter().next().unwrap_or(intern::key_none()), |k| matches!(k, intern::Key::Frozen(_))) {
                    *items = Some(one.iter().map(|k| Value::lit(*k)).collect());
                }
            }
            let mut all: KeySet = KeySet::new();
            for p in parts.iter().flatten() {
                all = all.union(&p.iter().copied().collect());
            }
            if parts.iter().any(|p| p.is_none()) {
                elems.push(line_data.clone());
            }
            if !all.is_empty() {
                elems.push(Value::kv_only(KeyVal::lits(all)));
            }
        } else if let Some(e) = self.m.torch_on.then(|| crate::models::torch::t_iter_elem(self, v, state)).flatten() {
            elems.push(e);
        } else {
            elems.push(line_data.clone());
        }
    }

    /// What any element of `v` is: a container's `Elem` slot and scalars, a dict's or frame's
    /// keys, a Series' values, else an unknown.
    pub fn elem_of(&mut self, v: &Value, node: &Node, state: &State) -> Value {
        let mut acc = Value::new();
        for oid in v.pts.iter() {
            match self.heap.kind(oid) {
                Kind::List | Kind::Tuple | Kind::Set => {
                    let e = state.slot(oid, Ns::Elem, STAR);
                    let kv = state.elems.get(&oid).unwrap_or(DATA);
                    acc = acc.join(&Value::with_kv(e.pts, kv));
                }
                Kind::Dict if crate::models::pandas::is_series_obj(self, oid) => acc = acc.join(&crate::models::pandas::series_values(state, oid, node.line).1),
                Kind::Dict | Kind::Frame => acc = acc.join(&Value::kv_only(KeyVal::KeysOf(state.keys_of(ObjSet::one(oid), Ns::Item)))),
                // (a character of a text not known: a str; an item of bytes: an int)
                Kind::Unknown if matches!(self.heap.obj(oid).ty, Some("str" | "bytes")) => {
                    let t = if self.heap.obj(oid).ty == Some("str") { "str" } else { "int" };
                    acc = acc.join(&Value::data(&crate::domain::typed_label(t, &format!("L{}", node.line))));
                }
                _ => acc = acc.join(&Value::data(&format!("L{}", node.line))),
            }
        }
        if v.pts.is_empty() {
            acc = Value::data(&format!("L{}", node.line));
        }
        acc
    }

    /// An iteration's element as one value: a (key, value) pair is a pair object (what
    /// `enumerate` / `zip` / a starred item hand on, unpacked later).
    pub fn elem_value(&mut self, e: &Elem, node: &Node, tag: &str) -> Value {
        match e {
            Elem::One(v) => v.clone(),
            Elem::Pair(k, v) => {
                let ctx = self.frame().ctx.clone();
                let pid = self.heap.alloc_raw(Kind::Pair, &format!("pair:{}:{}", self.site_of(node), tag), &ctx, false, "pair", Payload::Pair(k.clone(), v.clone()));
                Value::obj(pid)
            }
        }
    }

    /// The (key, value) items of a closed dict whose keys are all there on every path (at most
    /// `UNROLL_MAX`), in insertion order; None when that order is not known.
    pub fn literal_dict_items(&mut self, oid: u32, state: &State) -> Option<Vec<(KeyId, Value)>> {
        self.literal_items_in(oid, Ns::Item, state)
    }

    /// `literal_dict_items` of the keys in namespace `ns` (Attr: an object's attributes as
    /// its `__dict__` holds them).
    fn literal_items_in(&mut self, oid: u32, ns: Ns, state: &State) -> Option<Vec<(KeyId, Value)>> {
        let row = state.row(oid, ns);
        // an empty closed dict iterates zero times (known), like an empty list
        if row.open() || row.present != row.definite || row.present.len() > UNROLL_MAX {
            return None;
        }
        // (a Counter's STAR slot is the count of a missing key, not a key)
        if state.has_slot(oid, ns, STAR) && !self.m.counters.contains(&oid) {
            return None;
        }
        // insertion order when it is known (a guess would make order-dependent results wrong)
        let keys = match row.ordered_keys() {
            Some(k) => k,
            None if row.present.len() <= 1 => intern::sorted_by_str(row.present.iter()),
            None => return None,
        };
        Some(keys.into_iter().map(|k| (k, state.slot(oid, ns, k))).collect())
    }

    /// A loop over a closed dict (or a view of one) whose keys are known in their order but
    /// some only maybe there: its items, each with whether it surely comes (a `for` runs a
    /// maybe item on one path and skips it on the other).
    pub fn maybe_dict_items(&mut self, v: &Value, state: &State) -> Option<Vec<(Value, bool)>> {
        let o = v.single().filter(|_| v.kv.is_bot())?;
        let (d, vk) = match (self.heap.kind(o), &self.heap.obj(o).payload) {
            (Kind::Dict, _) if !crate::models::pandas::is_series_obj(self, o) => (o, ViewKind::Keys),
            (Kind::View, Payload::View(vk, recv)) if matches!(vk, ViewKind::Keys | ViewKind::Values | ViewKind::Items | ViewKind::Proxy) => (recv.single().filter(|r| recv.kv.is_bot() && self.heap.kind(*r) == Kind::Dict)?, *vk),
            _ => return None,
        };
        let row = state.row(d, Ns::Item);
        if !row.closed() || row.present == row.definite || row.present.len() > UNROLL_MAX || state.has_slot(d, Ns::Item, STAR) || self.m.counters.contains(&d) {
            return None;
        }
        let keys = row.ordered_keys()?;
        let mut out = Vec::new();
        for k in keys {
            let sure = row.definite.contains(k);
            let val = state.slot(d, Ns::Item, k);
            let item = match vk {
                ViewKind::Keys | ViewKind::Proxy => Value::lit(k),
                ViewKind::Values => val,
                _ => {
                    let pid = self.heap.alloc_raw(Kind::Pair, &format!("pair:{}:{}", o, intern::key_repr(k)), "", true, "pair", Payload::Pair(Value::lit(k), val.clone()));
                    self.heap.obj_mut(pid).payload = Payload::Pair(Value::lit(k), val);
                    Value::obj(pid)
                }
            };
            out.push((item, sure));
        }
        Some(out)
    }

    /// The keys of a closed dict whose keys are all known but not their order.
    fn unordered_keys_in(&self, oid: u32, ns: Ns, state: &State) -> Option<Vec<KeyId>> {
        let row = state.row(oid, ns);
        let plain = !row.open() && row.present == row.definite && row.present.len() <= UNROLL_MAX && !state.has_slot(oid, ns, STAR) && !self.m.counters.contains(&oid);
        (plain && row.ordered_keys().is_none()).then(|| intern::sorted_by_str(row.present.iter()))
    }
}
