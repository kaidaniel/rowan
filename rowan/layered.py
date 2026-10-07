"""Persistent-style maps for the fork/join pattern of abstract interpretation.

`LayerMap` behaves like a dict but is organised as a chain of frozen layers with a
private mutable top:

    copy()           O(1): the current top is frozen and shared, both maps get a new
                     empty top
    join(o, f)       O(#keys written on either side since the common ancestor)
    same(o)          O(diff) likewise
    diff_keys(base)  O(#keys written since `base` was forked off)
    get / set / del  O(depth); the chain is flattened every FLATTEN_EVERY layers, so
                     depth is bounded

It gives the properties a Patricia-tree / HAMT environment gives an abstract
interpreter (cheap snapshots, joins proportional to the change), with plain dict
operations as the only primitive.  `TwoLevel` adds a copy-on-write inner dict per
key, used for an object's slots and for a source's requirements.
"""
from __future__ import annotations

from typing import Any, Callable, Dict, Iterator, Optional, Set, Tuple

FLATTEN_EVERY = 10

_MISSING = object()
DELETED = object()


class _Node:
    __slots__ = ("parent", "data", "depth", "flat")

    def __init__(self, parent: Optional["_Node"], data: dict, depth: int) -> None:
        self.parent = parent
        self.data = data            # immutable once wrapped in a node
        self.depth = depth
        self.flat: Optional[dict] = None   # materialised contents; set only on flattened (root) nodes


class LayerMap:
    __slots__ = ("node", "data")

    def __init__(self, node: Optional[_Node] = None, data: Optional[dict] = None) -> None:
        self.node = node
        self.data: dict = data if data is not None else {}

    # ------------------------------------------------------------------ reads
    def get(self, k: Any, default: Any = None) -> Any:
        v = self.data.get(k, _MISSING)
        if v is _MISSING:
            n = self.node
            while n is not None:
                if n.flat is not None:
                    v = n.flat.get(k, _MISSING)
                    break
                v = n.data.get(k, _MISSING)
                if v is not _MISSING:
                    break
                n = n.parent
        if v is _MISSING or v is DELETED:
            return default
        return v

    def __getitem__(self, k: Any) -> Any:
        v = self.get(k, _MISSING)
        if v is _MISSING:
            raise KeyError(k)
        return v

    def __contains__(self, k: Any) -> bool:
        return self.get(k, _MISSING) is not _MISSING

    def materialise(self) -> dict:
        """A plain dict with the full contents (O(n))."""
        chain = []
        n = self.node
        base: dict = {}
        while n is not None:
            if n.flat is not None:
                base = n.flat
                break
            chain.append(n)
            n = n.parent
        out = dict(base)
        for node in reversed(chain):
            for k, v in node.data.items():
                if v is DELETED:
                    out.pop(k, None)
                else:
                    out[k] = v
        for k, v in self.data.items():
            if v is DELETED:
                out.pop(k, None)
            else:
                out[k] = v
        return out

    def items(self) -> Iterator[Tuple[Any, Any]]:
        return iter(self.materialise().items())

    def keys(self) -> Iterator[Any]:
        return iter(self.materialise().keys())

    __iter__ = keys

    def __len__(self) -> int:
        return len(self.materialise())

    # ----------------------------------------------------------------- writes
    def __setitem__(self, k: Any, v: Any) -> None:
        self.data[k] = v

    def __delitem__(self, k: Any) -> None:
        if self.get(k, _MISSING) is _MISSING:
            raise KeyError(k)
        self.data[k] = DELETED

    def pop(self, k: Any, default: Any = None) -> Any:
        v = self.get(k, _MISSING)
        if v is _MISSING:
            return default
        self.data[k] = DELETED
        return v

    def setdefault(self, k: Any, default: Any) -> Any:
        v = self.get(k, _MISSING)
        if v is _MISSING:
            self.data[k] = default
            return default
        return v

    # ------------------------------------------------------------ fork / freeze
    def _freeze(self) -> Optional[_Node]:
        """Turn the private top into a frozen node; returns the node to fork from."""
        if not self.data:
            return self.node
        depth = self.node.depth + 1 if self.node is not None else 1
        n = _Node(self.node, self.data, depth)
        self.node = n
        self.data = {}
        if depth % FLATTEN_EVERY == 0:
            self._flatten(n)
        return n

    def _flatten(self, n: _Node) -> None:
        # materialise n's contents, make it a root
        chain = []
        m = n
        base: dict = {}
        while m is not None:
            if m.flat is not None:
                base = m.flat
                break
            chain.append(m)
            m = m.parent
        out = dict(base)
        for node in reversed(chain):
            for k, v in node.data.items():
                if v is DELETED:
                    out.pop(k, None)
                else:
                    out[k] = v
        n.flat = out
        n.parent = None
        n.depth = 0

    def copy(self) -> "LayerMap":
        return LayerMap(self._freeze(), {})

    # ----------------------------------------------------------- join / compare
    @staticmethod
    def _ancestor(a: Optional[_Node], b: Optional[_Node]) -> Tuple[Optional[_Node], bool]:
        """Common ancestor node of two chains, or (None, False) when they have none.
        (None, True) means both chains are empty (trivially common)."""
        if a is None and b is None:
            return None, True
        ids: Dict[int, _Node] = {}
        n = a
        while n is not None:
            ids[id(n)] = n
            n = n.parent
        n = b
        while n is not None:
            if id(n) in ids:
                return n, True
            n = n.parent
        return None, False

    def _keys_above(self, anc: Optional[_Node]) -> Set[Any]:
        ks: Set[Any] = set(self.data)
        n = self.node
        while n is not None and n is not anc:
            ks.update(n.data)
            n = n.parent
        return ks

    def _values_above(self, anc: Optional[_Node]) -> dict:
        """{key: value} for every key written above `anc` (topmost write wins; DELETED kept)."""
        layers = [self.data]
        n = self.node
        while n is not None and n is not anc:
            layers.append(n.data)
            n = n.parent
        if len(layers) == 1:
            return dict(self.data)
        out: dict = {}
        for d in reversed(layers):
            out.update(d)
        return out

    @staticmethod
    def _get_from(n: Optional[_Node], k: Any) -> Any:
        while n is not None:
            if n.flat is not None:
                return n.flat.get(k, _MISSING)
            v = n.data.get(k, _MISSING)
            if v is not _MISSING:
                return v
            n = n.parent
        return _MISSING

    def join(self, o: "LayerMap", f: Callable[[Any, Any], Any]) -> "LayerMap":
        """Keys on one side only are kept; keys on both sides are combined with f (unless identical)."""
        anc, ok = self._ancestor(self.node, o.node)
        if not ok:
            a, b = self.materialise(), o.materialise()
            out = dict(a)
            for k, v in b.items():
                cur = out.get(k, _MISSING)
                if cur is _MISSING:
                    out[k] = v
                elif cur is not v and cur != v:
                    out[k] = f(cur, v)
            return LayerMap(None, out)
        va_map = self._values_above(anc)
        vb_map = o._values_above(anc)
        new = LayerMap(anc, {})
        d = new.data
        get_from = self._get_from
        for k in va_map.keys() | vb_map.keys():
            va = va_map.get(k, _MISSING)
            vb = vb_map.get(k, _MISSING)
            if va is _MISSING:
                va = get_from(anc, k)
            if vb is _MISSING:
                vb = get_from(anc, k)
            if va is DELETED:
                va = _MISSING
            if vb is DELETED:
                vb = _MISSING
            if va is _MISSING:
                d[k] = DELETED if vb is _MISSING else vb
            elif vb is _MISSING or va is vb:
                d[k] = va
            elif va == vb:
                d[k] = va
            else:
                d[k] = f(va, vb)
        return new

    def same(self, o: "LayerMap") -> bool:
        anc, ok = self._ancestor(self.node, o.node)
        if not ok:
            return self.materialise() == o.materialise()
        va_map = self._values_above(anc)
        vb_map = o._values_above(anc)
        get_from = self._get_from
        for k in va_map.keys() | vb_map.keys():
            va = va_map.get(k, _MISSING)
            vb = vb_map.get(k, _MISSING)
            if va is _MISSING:
                va = get_from(anc, k)
            if vb is _MISSING:
                vb = get_from(anc, k)
            if va is DELETED:
                va = _MISSING
            if vb is DELETED:
                vb = _MISSING
            if va is vb:
                continue
            if va is _MISSING or vb is _MISSING or va != vb:
                return False
        return True

    def diff_keys(self, base: "LayerMap") -> Set[Any]:
        """Keys whose value differs between self and base."""
        anc, ok = self._ancestor(self.node, base.node)
        if not ok:
            a, b = self.materialise(), base.materialise()
            return {k for k in set(a) | set(b) if a.get(k, _MISSING) is not b.get(k, _MISSING) and a.get(k, _MISSING) != b.get(k, _MISSING)}
        out: Set[Any] = set()
        va_map = self._values_above(anc)
        vb_map = base._values_above(anc)
        get_from = self._get_from
        for k in va_map.keys() | vb_map.keys():
            va = va_map.get(k, _MISSING)
            vb = vb_map.get(k, _MISSING)
            if va is _MISSING:
                va = get_from(anc, k)
            if vb is _MISSING:
                vb = get_from(anc, k)
            if va is DELETED:
                va = _MISSING
            if vb is DELETED:
                vb = _MISSING
            if va is vb:
                continue
            if va is _MISSING or vb is _MISSING or va != vb:
                out.add(k)
        return out

    def __eq__(self, o: object) -> bool:
        return isinstance(o, LayerMap) and self.same(o)

    def __ne__(self, o: object) -> bool:
        return not self.__eq__(o)

    __hash__ = None  # type: ignore[assignment]


class TwoLevel:
    """key -> {inner key -> value}, with copy-on-write inner dicts over a LayerMap.

    Also exposes the flat triple interface `(key, a, b)` used for object slots.
    """
    __slots__ = ("m", "owned")

    def __init__(self, m: Optional[LayerMap] = None) -> None:
        self.m = m if m is not None else LayerMap()
        self.owned: Set[Any] = set()

    def copy(self) -> "TwoLevel":
        new = TwoLevel(self.m.copy())      # freezes our top: inner dicts referenced there become shared
        self.owned = set()
        return new

    # -- inner dict access ---------------------------------------------------
    def inner(self, key: Any) -> Optional[dict]:
        return self.m.get(key)

    def writable(self, key: Any) -> dict:
        d = self.m.get(key)
        if d is None:
            d = {}
            self.m[key] = d
            self.owned.add(key)
        elif key not in self.owned:
            d = dict(d)
            self.m[key] = d
            self.owned.add(key)
        return d

    def drop(self, key: Any) -> None:
        self.m.pop(key, None)
        self.owned.discard(key)

    def outer_keys(self) -> Iterator[Any]:
        return self.m.keys()

    def items(self) -> Iterator[Tuple[Any, dict]]:
        return iter((k, d) for k, d in self.m.items() if d)

    # -- triple interface for slots: (oid, ns, key) ----------------------------
    def get(self, key: Tuple[Any, Any, Any], default: Any = None) -> Any:
        d = self.m.get(key[0])
        if d is None:
            return default
        return d.get((key[1], key[2]), default)

    def __getitem__(self, key: Tuple[Any, Any, Any]) -> Any:
        d = self.m.get(key[0])
        if d is None:
            raise KeyError(key)
        return d[(key[1], key[2])]

    def __contains__(self, key: Tuple[Any, Any, Any]) -> bool:   # type: ignore[override]
        d = self.m.get(key[0])
        return d is not None and (key[1], key[2]) in d

    def __setitem__(self, key: Tuple[Any, Any, Any], v: Any) -> None:
        self.writable(key[0])[(key[1], key[2])] = v

    def __delitem__(self, key: Tuple[Any, Any, Any]) -> None:
        d = self.writable(key[0])
        del d[(key[1], key[2])]
        if not d:
            self.drop(key[0])

    def pop(self, key: Tuple[Any, Any, Any], default: Any = None) -> Any:
        if key not in self:
            return default
        v = self[key]
        del self[key]
        return v

    def triples(self) -> Iterator[Tuple[Tuple[Any, Any, Any], Any]]:
        for oid, d in self.m.items():
            for (ns, k), v in d.items():
                yield (oid, ns, k), v

    def for_obj(self, oid: Any, ns: Optional[str] = None) -> Iterator[Tuple[Tuple[str, Any], Any]]:
        d = self.m.get(oid)
        if not d:
            return iter(())
        if ns is None:
            return iter(list(d.items()))
        return iter([(nk, v) for nk, v in d.items() if nk[0] == ns])

    def drop_obj(self, oid: Any, ns: Optional[str] = None) -> None:
        if ns is None:
            self.drop(oid)
            return
        d = self.m.get(oid)
        if not d:
            return
        d = self.writable(oid)
        for nk in [nk for nk in d if nk[0] == ns]:
            del d[nk]
        if not d:
            self.drop(oid)

    def objects(self) -> Iterator[Any]:
        return self.m.keys()

    # -- lattice --------------------------------------------------------------
    def _share(self) -> None:
        """Freeze the private top so that inner dicts referenced from a join result are never
        mutated in place afterwards (copy-on-write from here on)."""
        self.m._freeze()
        self.owned = set()

    def join(self, o: "TwoLevel", f: Callable[[Any, Any], Any]) -> "TwoLevel":
        self._share()
        o._share()

        def merge(d1: dict, d2: dict) -> dict:
            merged = None
            for nk, v in d2.items():
                cur = d1.get(nk, _MISSING)
                if cur is v:
                    continue
                if cur is _MISSING:
                    if merged is None:
                        merged = dict(d1)
                    merged[nk] = v
                elif cur != v:
                    if merged is None:
                        merged = dict(d1)
                    merged[nk] = f(cur, v)
            return merged if merged is not None else d1
        return TwoLevel(self.m.join(o.m, merge))

    def join_with(self, o: "TwoLevel", both: Callable[[Any, Any], Any], one_side: Callable[[Any], Any]) -> "TwoLevel":
        """Join where inner entries present on one side only are transformed by `one_side`
        (requirements: demoted to 'sometimes')."""
        self._share()
        o._share()
        a, b = self.m, o.m
        anc, ok = LayerMap._ancestor(a.node, b.node)
        if ok:
            va_map = a._values_above(anc)
            vb_map = b._values_above(anc)
            keys = va_map.keys() | vb_map.keys()
            new = LayerMap(anc, {})
        else:
            va_map = a.materialise()
            vb_map = b.materialise()
            keys = va_map.keys() | vb_map.keys()
            new = LayerMap(None, {})
        get_from = LayerMap._get_from
        for k in keys:
            da = va_map.get(k, _MISSING)
            db = vb_map.get(k, _MISSING)
            if da is _MISSING:
                da = get_from(anc, k) if ok else _MISSING
            if db is _MISSING:
                db = get_from(anc, k) if ok else _MISSING
            if da is DELETED or da is _MISSING:
                da = None
            if db is DELETED or db is _MISSING:
                db = None
            if da is db:
                if da is not None and not ok:
                    new.data[k] = da
                continue
            if da is None or db is None:
                d = da if da is not None else db
                new.data[k] = {nk: one_side(v) for nk, v in d.items()}
                continue
            merged: dict = {}
            for nk in set(da) | set(db):
                va = da.get(nk, _MISSING)
                vb = db.get(nk, _MISSING)
                if va is _MISSING:
                    merged[nk] = one_side(vb)
                elif vb is _MISSING:
                    merged[nk] = one_side(va)
                elif va is vb or va == vb:
                    merged[nk] = va
                else:
                    merged[nk] = both(va, vb)
            new.data[k] = merged
        return TwoLevel(new)

    def same(self, o: "TwoLevel") -> bool:
        return self.m.same(o.m)

    def diff_keys(self, base: "TwoLevel") -> Set[Tuple[Any, Any, Any]]:
        out: Set[Tuple[Any, Any, Any]] = set()
        for oid in self.m.diff_keys(base.m):
            d = self.m.get(oid) or {}
            b = base.m.get(oid) or {}
            for nk in set(d) | set(b):
                v = d.get(nk, _MISSING)
                bv = b.get(nk, _MISSING)
                if bv is not v and bv != v:
                    out.add((oid, nk[0], nk[1]))
        return out

    def __eq__(self, o: object) -> bool:
        return isinstance(o, TwoLevel) and self.same(o)

    def __ne__(self, o: object) -> bool:
        return not self.__eq__(o)

    __hash__ = None  # type: ignore[assignment]
