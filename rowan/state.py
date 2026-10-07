"""Abstract objects and the flow-sensitive analysis state.

The heap parts of a state (rows, slots, elems, reqs) live in layered persistent maps
(`layered.py`): copying a state is O(1) and joining two states costs time
proportional to what changed since they were forked, not to the heap size.
"""
from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Dict, Optional, Tuple

from .domain import DATA, KeyVal, ReqInfo, Row, Source, Value, kv_join
from .layered import LayerMap, TwoLevel

RECORD_KINDS = frozenset({"dict", "frame", "instance", "unknown", "module", "class", "namespace", "ph"})
CONTAINER_KINDS = frozenset({"list", "tuple", "set"})


@dataclass
class Obj:
    id: int
    kind: str
    site: str
    ctx: str
    single: bool
    label: str
    payload: Any = None
    literal_len: Optional[int] = None    # containers built from a literal: positional slots are valid
    iter_elem: bool = False              # summary object standing for "the current element" of a loop
    derived_from: Optional[int] = None   # record this object is a view of (virtual row -> frame)
    depth: int = 0                       # access-path depth under an open source (placeholders, elements)

    def __repr__(self) -> str:
        return f"o{self.id}:{self.label}"


def _demote(info: ReqInfo) -> ReqInfo:
    return info if not info.always else ReqInfo(False, info.sites, info.alternatives, info.caveats, info.alt_keys)


def _join_req(a: ReqInfo, b: ReqInfo) -> ReqInfo:
    return a.join(b)


class State:
    """Mutable analysis state at one program point.

    env    current frame's plain locals (small; copied eagerly)
    rows   (oid, ns) -> Row
    slots  (oid, ns, key) -> Value          key '*' = stored under a non-literal key
    elems  oid -> KeyVal                    literal element keys of containers
    reqs   Source -> {key term -> ReqInfo}  requirements accumulated along this path
    ret    joined return value of the current function
    exits  join of the states at every `return` so far
    """
    __slots__ = ("env", "rows", "slots", "elems", "reqs", "ret", "reachable", "exits", "symbolic", "narrowed")

    def __init__(self) -> None:
        self.env: Dict[str, Value] = {}
        # path-sensitive facts about attribute chains (`isinstance(a.b, C)` → `a.b` on this
        # path), keyed by the chain's text; dropped by stores to the root name and by calls
        self.narrowed: Dict[str, Value] = {}
        self.rows: LayerMap = LayerMap()
        self.slots: TwoLevel = TwoLevel()
        self.elems: LayerMap = LayerMap()
        self.reqs: TwoLevel = TwoLevel()
        self.ret: Optional[Value] = None
        self.reachable: bool = True
        self.exits: Optional["State"] = None
        self.symbolic: bool = False       # rows of untouched objects are entry rows (summary analysis)

    # -- copying -----------------------------------------------------------
    def copy(self) -> "State":
        s = State()
        s.env = dict(self.env)
        s.narrowed = dict(self.narrowed) if self.narrowed else {}
        s.rows = self.rows.copy()
        s.slots = self.slots.copy()
        s.elems = self.elems.copy()
        s.reqs = self.reqs.copy()
        s.ret = self.ret
        s.reachable = self.reachable
        s.exits = self.exits
        s.symbolic = self.symbolic
        return s

    def record_exit(self) -> None:
        """Snapshot the heap at a `return` so the caller sees it joined over all return points."""
        snap = self.copy()
        snap.reachable = True
        snap.ret = None
        snap.exits = None
        self.exits = snap if self.exits is None else self.exits.join(snap)

    def set_to(self, o: "State") -> None:
        """Make self equal to o (o stays usable: both get their own private top layers)."""
        self.env = o.env
        self.narrowed = o.narrowed
        self.rows = o.rows.copy()
        self.slots = o.slots.copy()
        self.elems = o.elems.copy()
        self.reqs = o.reqs.copy()
        self.ret, self.reachable, self.exits = o.ret, o.reachable, o.exits
        self.symbolic = o.symbolic

    @staticmethod
    def bottom() -> "State":
        s = State()
        s.reachable = False
        return s

    # -- lattice -----------------------------------------------------------
    def join(self, o: "State") -> "State":
        if self.ret is None:
            ret = o.ret
        elif o.ret is None:
            ret = self.ret
        else:
            ret = self.ret.join(o.ret)
        if self.exits is None:
            exits = o.exits
        elif o.exits is None:
            exits = self.exits
        else:
            exits = self.exits.join(o.exits)
        if not o.reachable:
            s = self.copy()
            s.ret, s.exits = ret, exits
            return s
        if not self.reachable:
            s = o.copy()
            s.ret, s.exits = ret, exits
            return s
        s = State()
        s.env = _join_dict(self.env, o.env, lambda a, b: a.join(b))
        if self.narrowed and o.narrowed:
            s.narrowed = {k: v for k, v in self.narrowed.items() if k in o.narrowed and o.narrowed[k] == v}
        s.rows = self.rows.join(o.rows, lambda a, b: a.join(b))
        s.slots = self.slots.join(o.slots, lambda a, b: a.join(b))
        s.elems = self.elems.join(o.elems, kv_join)
        s.reqs = self.reqs.join_with(o.reqs, _join_req, _demote)
        s.ret = ret
        s.exits = exits
        s.reachable = True
        s.symbolic = self.symbolic or o.symbolic
        return s

    def join_heap(self, o: "State", footprint=None) -> None:
        """Join heap facts (rows/slots/elems/reqs) of `o` into self, in place.

        footprint: optional (row keys, slot keys, elem keys) restricting the join to
        the objects a callee actually touched (its effect summary).
        """
        if not o.reachable:
            return
        if footprint is None:
            self.rows = self.rows.join(o.rows, lambda a, b: a.join(b))
            self.slots = self.slots.join(o.slots, lambda a, b: a.join(b))
            self.elems = self.elems.join(o.elems, kv_join)
            self.reqs = self.reqs.join_with(o.reqs, _join_req, _demote)
            return
        rk, sk, ek = footprint
        for k in rk:
            v = o.rows.get(k)
            if v is None:
                continue
            cur = self.rows.get(k)
            if cur is None:
                self.rows[k] = v
            elif cur is not v and cur != v:
                self.rows[k] = cur.join(v)
        for k in sk:
            v = o.slots.get(k)
            if v is None:
                continue
            cur = self.slots.get(k)
            if cur is None:
                self.slots[k] = v
            elif cur is not v and cur != v:
                self.slots[k] = cur.join(v)
        for k in ek:
            v = o.elems.get(k)
            if v is None:
                continue
            cur = self.elems.get(k)
            if cur is None:
                self.elems[k] = v
            elif cur is not v and cur != v:
                self.elems[k] = kv_join(cur, v)
        for src, d in o.reqs.items():
            cur = self.reqs.inner(src)
            if cur is d:
                continue
            tgt = self.reqs.writable(src)
            for k, info in d.items():
                tgt[k] = tgt[k].join(info) if k in tgt else _demote(info)

    def footprint(self, base: "State"):
        """Keys whose facts differ from `base` (what a callee changed or created)."""
        return self.rows.diff_keys(base.rows), self.slots.diff_keys(base.slots), self.elems.diff_keys(base.elems)

    def same(self, o: "State") -> bool:
        return (self.reachable == o.reachable and self.env == o.env and self.ret == o.ret
                and ((self.exits is None) == (o.exits is None))
                and self.rows.same(o.rows) and self.slots.same(o.slots) and self.elems.same(o.elems)
                and self.reqs.same(o.reqs)
                and (self.exits is None or self.exits.same(o.exits)))

    # -- accessors ---------------------------------------------------------
    def row(self, oid: int, ns: str = "item") -> Row:
        r = self.rows.get((oid, ns))
        if r is not None:
            return r
        if self.symbolic:
            return Row.entry(oid * 2 + (1 if ns == "attr" else 0))
        return Row()

    def set_row(self, oid: int, ns: str, row: Row) -> None:
        self.rows[(oid, ns)] = row

    def slot(self, oid: int, ns: str, key: Any) -> Value:
        v = self.slots.get((oid, ns, key))
        return v if v is not None else Value()

    def set_slot(self, oid: int, ns: str, key: Any, v: Value, strong: bool) -> None:
        if strong:
            self.slots[(oid, ns, key)] = v
        else:
            self.slots[(oid, ns, key)] = self.slot(oid, ns, key).join(v)

    def all_slots(self, oid: int, ns: str) -> Value:
        acc = Value()
        for (n, _k), v in self.slots.for_obj(oid, ns):
            acc = acc.join(v)
        return acc

    def slot_keys(self, oid: int, ns: str):
        return [k for (n, k), _v in self.slots.for_obj(oid, ns) if k != "*"]

    def obj_slots(self, oid: int, ns: str):
        """[(key, Value)] of one object's namespace."""
        return [(k, v) for (n, k), v in self.slots.for_obj(oid, ns)]

    def require(self, src: Source, term: Any, site: str, alternatives=frozenset(), caveats=frozenset(),
                alt_keys=frozenset()) -> None:
        d = self.reqs.writable(src)
        info = ReqInfo(True, frozenset({site}), frozenset(alternatives), frozenset(caveats),
                       frozenset(alt_keys) if len(alt_keys) > 1 else frozenset())
        d[term] = d[term].seq(info) if term in d else info


def _join_dict(a: dict, b: dict, f) -> dict:
    if a is b:
        return dict(a)
    out = dict(a)
    for k, v in b.items():
        cur = out.get(k, _MISSING)
        if cur is _MISSING:
            out[k] = v
        elif cur is not v and cur != v:
            out[k] = f(cur, v)
    return out


_MISSING = object()
