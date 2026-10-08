# Analysis input only: a module of the former Python prototype, kept as a large realistic
# fixture for tests/run_snapshots.py. It is not used as code.
"""Abstract domains for rowan (SPECIFICATION.md §5.4, §7).

Everything here is immutable and hashable so that states can be compared for
fixpoint convergence.

  KeyVal   – abstract value of an expression *when used as a key*
  Lens     – chain of steps relating an open source's keys to an object's keys
  Row      – provision facts for one (object, namespace)
  ReqInfo  – requirement facts accumulated per (source, key term)
  Value    – abstract value of an expression: points-to set + KeyVal
"""
from __future__ import annotations

from dataclasses import dataclass, field, replace
from typing import Any, FrozenSet, Iterable, Optional, Tuple, Union

MAX_LITS = 64          # cap on finite key sets before degrading to Data
MAX_LENSES = 6         # cap on alternative lenses per source


# ---------------------------------------------------------------------------
# key terms
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class Inv:
    """Inverse image of a named transformer: the source key x with fn(x) == key."""
    fn: str
    key: Any

    def __str__(self) -> str:
        return f"{self.fn}⁻¹({self.key!r})"


def key_str(k: Any) -> str:
    if isinstance(k, Inv):
        return str(k)
    return repr(k)


def keys_str(ks: Iterable[Any]) -> str:
    items = sorted((key_str(k) for k in ks), key=str)
    return "{" + ", ".join(items) + "}"


# ---------------------------------------------------------------------------
# lens steps
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class Kill:
    keys: FrozenSet[Any]

    def __str__(self) -> str:
        return f"∖{keys_str(self.keys)}"


@dataclass(frozen=True)
class RenameMap:
    pairs: Tuple[Tuple[Any, Any], ...]      # ((old, new), ...), identity elsewhere

    def apply(self, k: Any) -> Any:
        return dict(self.pairs).get(k, k)

    def inverse(self, k: Any) -> FrozenSet[Any]:
        d = dict(self.pairs)
        out = {o for o, n in self.pairs if n == k}
        if k not in d:
            out.add(k)
        return frozenset(out)

    def __str__(self) -> str:
        return "rename{" + ", ".join(f"{o!r}→{n!r}" for o, n in self.pairs) + "}"


@dataclass(frozen=True)
class RenameFn:
    fn: str                 # upper | lower | strip | title | prefix | suffix | str | replace
    arg: Any = None

    def apply(self, k: Any) -> Any:
        if self.fn == "str":
            return str(k)
        if not isinstance(k, str):
            return k
        if self.fn == "upper":
            return k.upper()
        if self.fn == "lower":
            return k.lower()
        if self.fn == "strip":
            return k.strip()
        if self.fn == "title":
            return k.title()
        if self.fn == "prefix":
            return f"{self.arg}{k}"
        if self.fn == "suffix":
            return f"{k}{self.arg}"
        if self.fn == "replace":
            a, b = self.arg
            return k.replace(a, b)
        return k

    def inverse(self, k: Any) -> FrozenSet[Any]:
        if self.fn == "prefix":
            p = str(self.arg)
            return frozenset({k[len(p):]}) if isinstance(k, str) and k.startswith(p) else frozenset()
        if self.fn == "suffix":
            s = str(self.arg)
            return frozenset({k[: len(k) - len(s)]}) if isinstance(k, str) and k.endswith(s) and s else frozenset()
        if self.fn == "upper":
            return frozenset({Inv("upper", k)}) if isinstance(k, str) and k == k.upper() else frozenset()
        if self.fn == "lower":
            return frozenset({Inv("lower", k)}) if isinstance(k, str) and k == k.lower() else frozenset()
        if self.fn == "title":
            return frozenset({Inv("title", k)}) if isinstance(k, str) and k == k.title() else frozenset()
        if self.fn == "strip":
            return frozenset({Inv("strip", k)}) if isinstance(k, str) and k == k.strip() else frozenset()
        if self.fn == "str":
            out = {Inv("str", k)}
            if isinstance(k, str) and k.lstrip("-").isdigit():
                out.add(int(k))
            return frozenset(out)
        if self.fn == "replace":
            a, b = self.arg
            if not isinstance(k, str):
                return frozenset({k})
            if a in k and a not in b:
                return frozenset()          # the output never contains `a`
            if b in k:
                return frozenset({k, Inv(f"replace({a!r},{b!r})", k)})
            return frozenset({k})
        return frozenset({Inv(self.fn, k)})

    def __str__(self) -> str:
        return self.fn if self.arg is None else f"{self.fn}({self.arg!r})"


Step = Union[Kill, RenameMap, RenameFn]
Lens = Tuple[Step, ...]


def lens_provides(lens: Lens, k: Any) -> FrozenSet[Any]:
    """Source key terms that would appear as `k` after the lens; empty = cannot."""
    cands: set = {k}
    for step in reversed(lens):
        if isinstance(step, Kill):
            cands = {c for c in cands if isinstance(c, Inv) or c not in step.keys}
        else:
            new: set = set()
            for c in cands:
                if isinstance(c, Inv):
                    new.add(c)          # approximate: keep symbolic
                else:
                    new |= step.inverse(c)
            cands = new
        if not cands:
            break
    return frozenset(cands)


def lens_apply(lens: Lens, k: Any) -> Optional[Any]:
    """Forward image of a single source key; None if killed."""
    for step in lens:
        if isinstance(step, Kill):
            if k in step.keys:
                return None
        else:
            k = step.apply(k)
    return k


def lens_str(lens: Lens) -> str:
    return "".join(f"·{s}" for s in lens)


def lens_image(lens: Lens, keys: Iterable[Any]) -> FrozenSet[Any]:
    out = set()
    for k in keys:
        r = lens_apply(lens, k)
        if r is not None:
            out.add(r)
    return frozenset(out)


def _lens_segments(lens: Lens):
    """[(kills_before, rename), ..., (kills_after, None)]: kills grouped between renames."""
    segs = []
    kills: set = set()
    for step in lens:
        if isinstance(step, Kill):
            kills |= step.keys
        else:
            segs.append((frozenset(kills), step))
            kills = set()
    segs.append((frozenset(kills), None))
    return segs


def lens_join_definite(a: Lens, b: Lens) -> Optional[Lens]:
    """Lens whose definite image is contained in both: same renames, kills united;
    None when the renames differ (spec §7.3: 'renames must agree')."""
    if a == b:
        return a
    sa, sb = _lens_segments(a), _lens_segments(b)
    if len(sa) != len(sb) or any(x[1] != y[1] for x, y in zip(sa, sb)):
        return None
    out: list = []
    for (ka, r), (kb, _) in zip(sa, sb):
        ks = ka | kb
        if ks:
            out.append(Kill(ks))
        if r is not None:
            out.append(r)
    return tuple(out)


def lens_add_kill(lens: Lens, k: Any) -> Lens:
    if lens and isinstance(lens[-1], Kill):
        return lens[:-1] + (Kill(lens[-1].keys | {k}),)
    return lens + (Kill(frozenset({k})),)


# ---------------------------------------------------------------------------
# sources (attributions of openness)
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class Source:
    """A reason an object may have keys we do not see.

    kind: external (read_csv/json/environ...), call (opaque producer), param
          (entry-point parameter), name (undefined global), data (data-keyed
          store), class (open class instance)
    """
    site: str
    kind: str
    path: Tuple[Any, ...] = ()

    def extend(self, k: Any) -> "Source":
        return replace(self, path=self.path + (k,))

    @property
    def is_data(self) -> bool:
        return self.kind == "data"

    def __str__(self) -> str:
        p = "".join(f"[{k!r}]" for k in self.path)
        return f"{self.site}{p}"


# ---------------------------------------------------------------------------
# Row
# ---------------------------------------------------------------------------

def _frozen(x: Iterable[Any] = ()) -> FrozenSet[Any]:
    return x if isinstance(x, frozenset) else frozenset(x)


@dataclass(frozen=True, eq=False)
class Row:
    """Provision facts for one (object, namespace) at one program point.

    present        keys with positive evidence (may)
    definite       keys present on all paths (proven)
    likely         keys believed present despite later unknown mutators (⊇ definite)
    absent         keys proven absent even though the row may be open
    absent_likely  likewise, optimistic
    sources        frozenset of (Source, Lens): open sources and how their keys map here
    mutators       sites of unknown code that may have added or removed keys
    top            nothing known at all

    Symbolic part (function summaries, spec §7.3), referring to entry rows of objects ρ:
    terms          (ρ, lens): present ⊇ lens(entry(ρ).present); sources/mutators flow through
    dterms         (ρ, lens): definite ⊇ lens(entry(ρ).definite)
    lterms         (ρ, lens): likely ⊇ lens(entry(ρ).definite)  (survives unknown mutators)
    pstores        (ρ, strong, lens): a store whose key is the value of placeholder ρ
    """
    present: FrozenSet[Any] = frozenset()
    definite: FrozenSet[Any] = frozenset()
    likely: FrozenSet[Any] = frozenset()
    absent: FrozenSet[Any] = frozenset()
    absent_likely: FrozenSet[Any] = frozenset()
    sources: FrozenSet[Tuple[Source, Lens]] = frozenset()
    mutators: FrozenSet[str] = frozenset()
    top: bool = False
    terms: FrozenSet[Tuple[int, Lens]] = frozenset()
    dterms: FrozenSet[Tuple[int, Lens]] = frozenset()
    lterms: FrozenSet[Tuple[int, Lens]] = frozenset()
    pstores: FrozenSet[Tuple[int, bool, Lens]] = frozenset()
    _hash: Optional[int] = field(default=None, init=False, repr=False, compare=False)
    _refs: Optional[FrozenSet[int]] = field(default=None, init=False, repr=False, compare=False)

    # rows are hashed and compared constantly (caches, joins, dedupe): the hash is cached
    def _key(self) -> tuple:
        return (self.present, self.definite, self.likely, self.absent, self.absent_likely, self.sources,
                self.mutators, self.top, self.terms, self.dterms, self.lterms, self.pstores)

    def __hash__(self) -> int:
        h = self._hash
        if h is None:
            h = hash(self._key())
            object.__setattr__(self, "_hash", h)
        return h

    def __eq__(self, o: object) -> bool:
        if self is o:
            return True
        if not isinstance(o, Row):
            return NotImplemented
        if self._hash is not None and o._hash is not None and self._hash != o._hash:
            return False
        return self._key() == o._key()

    @property
    def refs(self) -> FrozenSet[int]:
        """The entry rows ρ this row's symbolic part refers to."""
        r = self._refs
        if r is None:
            r = frozenset(x for x, _l in self.terms) | frozenset(x for x, _l in self.dterms) | \
                frozenset(x for x, _l in self.lterms) | frozenset(x for x, _s, _l in self.pstores)
            object.__setattr__(self, "_refs", r)
        return r

    # -- queries -----------------------------------------------------------
    @property
    def symbolic(self) -> bool:
        return bool(self.terms or self.dterms or self.lterms or self.pstores)

    @property
    def open(self) -> bool:
        return bool(self.sources) or bool(self.mutators) or self.top or self.symbolic

    @property
    def closed(self) -> bool:
        return not self.open

    @property
    def is_map(self) -> bool:
        return any(s.is_data for s, _ in self.sources)

    def external_sources(self) -> FrozenSet[Tuple[Source, Lens]]:
        return frozenset((s, l) for s, l in self.sources if not s.is_data)

    def provides(self, k: Any) -> dict:
        """Source -> frozenset of source key terms that may surface as k."""
        out: dict = {}
        for s, lens in self.sources:
            terms = lens_provides(lens, k)
            if terms:
                out.setdefault(s, frozenset())
                out[s] = out[s] | terms
        return out

    # -- lattice -----------------------------------------------------------
    def join(self, o: "Row") -> "Row":
        if self is o or self == o:
            return self
        if o.sources <= self.sources:
            sources = self.sources
        elif self.sources <= o.sources:
            sources = o.sources
        else:
            sources = _cap_sources(self.sources | o.sources)
        return Row(
            present=self.present | o.present,
            definite=self.definite & o.definite,
            likely=self.likely & o.likely,
            absent=self.absent & o.absent,
            absent_likely=self.absent_likely & o.absent_likely,
            sources=sources,
            mutators=self.mutators | o.mutators,
            top=self.top or o.top,
            terms=_cap_terms(self.terms | o.terms) if self.terms != o.terms else self.terms,
            dterms=_join_dterms(self.dterms, o.dterms),
            lterms=_join_dterms(self.lterms, o.lterms),
            pstores=_join_pstores(self.pstores, o.pstores),
        )

    # -- transfers (§7.3) --------------------------------------------------
    def store(self, k: Any, strong: bool) -> "Row":
        if strong:
            return replace(self, present=self.present | {k}, definite=self.definite | {k},
                           likely=self.likely | {k}, absent=self.absent - {k},
                           absent_likely=self.absent_likely - {k})
        return replace(self, present=self.present | {k}, absent=self.absent - {k},
                       absent_likely=self.absent_likely - {k})

    def store_many(self, ks: Iterable[Any]) -> "Row":
        """One of several keys was stored (weak)."""
        ks = _frozen(ks)
        return replace(self, present=self.present | ks, absent=self.absent - ks,
                       absent_likely=self.absent_likely - ks)

    def param_store(self, ph: int, strong: bool) -> "Row":
        """Store under the (unknown) key held by placeholder ph."""
        return replace(self, pstores=self.pstores | {(ph, strong, ())})

    def delete(self, k: Any, strong: bool) -> "Row":
        if strong:
            return replace(self, present=self.present - {k}, definite=self.definite - {k},
                           likely=self.likely - {k}, absent=self.absent | {k},
                           absent_likely=self.absent_likely | {k},
                           sources=frozenset((s, lens_add_kill(l, k)) for s, l in self.sources),
                           terms=frozenset((r, lens_add_kill(l, k)) for r, l in self.terms),
                           dterms=frozenset((r, lens_add_kill(l, k)) for r, l in self.dterms),
                           lterms=frozenset((r, lens_add_kill(l, k)) for r, l in self.lterms),
                           pstores=frozenset((r, st, lens_add_kill(l, k)) for r, st, l in self.pstores))
        # weak delete: k is no longer definite, whatever its origin
        return replace(self, definite=self.definite - {k}, likely=self.likely - {k},
                       dterms=frozenset((r, lens_add_kill(l, k)) for r, l in self.dterms),
                       lterms=frozenset((r, lens_add_kill(l, k)) for r, l in self.lterms),
                       pstores=self.pstores)

    def data_store(self, src: Source) -> "Row":
        return replace(self, sources=_cap_sources(self.sources | {(src, ())}))

    def rename(self, step: Union[RenameMap, RenameFn]) -> "Row":
        f = step.apply
        return Row(
            present=frozenset(f(k) for k in self.present),
            definite=frozenset(f(k) for k in self.definite),
            likely=frozenset(f(k) for k in self.likely),
            absent=frozenset(), absent_likely=frozenset(),
            sources=frozenset((s, l + (step,)) for s, l in self.sources),
            mutators=self.mutators, top=self.top,
            terms=frozenset((r, l + (step,)) for r, l in self.terms),
            dterms=frozenset((r, l + (step,)) for r, l in self.dterms),
            lterms=frozenset((r, l + (step,)) for r, l in self.lterms),
            pstores=frozenset((r, st, l + (step,)) for r, st, l in self.pstores),
        )

    def project(self, keys: Iterable[Any]) -> "Row":
        ks = _frozen(keys)
        return Row(present=ks, definite=ks, likely=ks)

    def copy_from(self, o: "Row", strong: bool = True, kill: FrozenSet[Any] = frozenset(),
                  chain: Lens = ()) -> "Row":
        """self ∪= chain(o ∖ kill).  strong: definite keys of o are definitely copied."""
        def img(ks: FrozenSet[Any]) -> FrozenSet[Any]:
            out = set()
            for k in ks:
                if k in kill:
                    continue
                r = lens_apply(chain, k)
                if r is not None:
                    out.add(r)
            return frozenset(out)
        pre: Lens = ((Kill(kill),) if kill else ()) + chain
        present = self.present | img(o.present)
        definite = self.definite | (img(o.definite) if strong else frozenset())
        likely = self.likely | (img(o.likely) if strong else frozenset())
        absent = self.absent - present
        absent_likely = self.absent_likely - present
        if o.open:
            absent = frozenset()
            absent_likely = frozenset()
        return Row(present=present, definite=definite, likely=likely, absent=absent,
                   absent_likely=absent_likely,
                   sources=_cap_sources(self.sources | frozenset((s, l + pre) for s, l in o.sources)),
                   mutators=self.mutators | o.mutators, top=self.top or o.top,
                   terms=_cap_terms(self.terms | frozenset((r, l + pre) for r, l in o.terms)),
                   dterms=self.dterms | (frozenset((r, l + pre) for r, l in o.dterms) if strong else frozenset()),
                   lterms=self.lterms | (frozenset((r, l + pre) for r, l in o.lterms) if strong else frozenset()),
                   pstores=self.pstores | frozenset((r, st and strong, l + pre) for r, st, l in o.pstores))

    def mutate(self, site: str) -> "Row":
        return replace(self, likely=self.likely | self.definite, definite=frozenset(),
                       absent_likely=self.absent_likely | self.absent, absent=frozenset(),
                       mutators=self.mutators | {site},
                       lterms=self.lterms | self.dterms, dterms=frozenset(),
                       pstores=frozenset((r, False, l) for r, st, l in self.pstores))

    def narrow_in(self, k: Any, strong: bool = True) -> "Row":
        if strong:
            return replace(self, present=self.present | {k}, definite=self.definite | {k},
                           likely=self.likely | {k}, absent=self.absent - {k},
                           absent_likely=self.absent_likely - {k})
        return replace(self, present=self.present | {k}, absent=self.absent - {k},
                       absent_likely=self.absent_likely - {k})

    def narrow_out(self, k: Any) -> "Row":
        return replace(self, present=self.present - {k}, definite=self.definite - {k},
                       likely=self.likely - {k}, absent=self.absent | {k},
                       absent_likely=self.absent_likely | {k},
                       sources=frozenset((s, lens_add_kill(l, k)) for s, l in self.sources),
                       terms=frozenset((r, lens_add_kill(l, k)) for r, l in self.terms),
                       dterms=frozenset((r, lens_add_kill(l, k)) for r, l in self.dterms),
                       lterms=frozenset((r, lens_add_kill(l, k)) for r, l in self.lterms),
                       pstores=frozenset((r, st, lens_add_kill(l, k)) for r, st, l in self.pstores))

    def make_top(self) -> "Row":
        return replace(self, definite=frozenset(), likely=frozenset(), absent=frozenset(),
                       absent_likely=frozenset(), top=True)

    @staticmethod
    def entry(ph: int) -> "Row":
        """The symbolic row of an object as it is at function entry."""
        t = frozenset({(ph, ())})
        return Row(terms=t, dterms=t, lterms=t)

    @property
    def is_entry(self) -> bool:
        """Exactly `Row.entry(ρ)` for some ρ: nothing happened to the object yet."""
        t = self.terms
        return (len(t) == 1 and t == self.dterms and t == self.lterms and not self.pstores
                and not self.present and not self.absent and not self.absent_likely and not self.sources
                and not self.mutators and not self.top and next(iter(t))[1] == ())

    def concretise(self, entry_row_of, kv_of) -> "Row":
        """Replace the symbolic part by actual entry rows (`entry_row_of(ρ)`) and actual key
        values of placeholders (`kv_of(ρ)`)."""
        if not self.symbolic:
            return self
        present = set(self.present)
        definite = set(self.definite)
        likely = set(self.likely)
        sources = set(self.sources)
        mutators = set(self.mutators)
        top = self.top
        terms: set = set()
        dterms: set = set()
        lterms: set = set()
        pstores: set = set()
        for r, lens in self.terms:
            e = entry_row_of(r)
            present |= lens_image(lens, e.present)
            sources |= {(s, l + lens) for s, l in e.sources}
            mutators |= e.mutators
            top = top or e.top
            terms |= {(r2, l2 + lens) for r2, l2 in e.terms}
            pstores |= {(r2, False, l2 + lens) for r2, st_, l2 in e.pstores}
        for r, lens in self.dterms:
            e = entry_row_of(r)
            definite |= lens_image(lens, e.definite)
            dterms |= {(r2, l2 + lens) for r2, l2 in e.dterms}
            pstores |= {(r2, st_, l2 + lens) for r2, st_, l2 in e.pstores}
        for r, lens in self.lterms:
            e = entry_row_of(r)
            likely |= lens_image(lens, e.likely | e.definite)
            lterms |= {(r2, l2 + lens) for r2, l2 in e.lterms | e.dterms}
        present |= definite
        likely |= definite
        absent = self.absent - present
        absent_likely = self.absent_likely - present
        out = Row(present=frozenset(present), definite=frozenset(definite), likely=frozenset(likely),
                  absent=frozenset(absent), absent_likely=frozenset(absent_likely),
                  sources=_cap_sources(frozenset(sources)), mutators=frozenset(mutators), top=top,
                  terms=_cap_terms(frozenset(terms)), dterms=frozenset(dterms), lterms=frozenset(lterms))
        remaining: set = set()
        for r, strong, lens in self.pstores | frozenset(pstores):
            kv = kv_of(r)
            if isinstance(kv, Param):
                remaining.add((kv.ph, strong, lens))
                continue
            lits = kv_literals(kv)
            if lits:
                for k in lits:
                    k2 = lens_apply(lens, k)
                    if k2 is not None:
                        out = out.store(k2, strong and len(lits) == 1)
            elif isinstance(kv, KeysOf):
                out = out.data_store(Source(f"keys-of-store@{r}", "data"))
            elif isinstance(kv, Bot):
                continue
            else:
                out = out.data_store(Source(f"store@param{r}", "data"))
        if remaining:
            out = replace(out, pstores=frozenset(remaining))
        return out

    # -- printing ----------------------------------------------------------
    def __str__(self) -> str:
        parts = [f"present={keys_str(self.present)}", f"definite={keys_str(self.definite)}"]
        if self.likely != self.definite:
            parts.append(f"likely={keys_str(self.likely)}")
        if self.absent:
            parts.append(f"absent={keys_str(self.absent)}")
        if self.sources:
            parts.append("sources={" + ", ".join(sorted(f"{s}{lens_str(l)}" for s, l in self.sources)) + "}")
        if self.mutators:
            parts.append("mutators={" + ", ".join(sorted(self.mutators)) + "}")
        if self.terms:
            parts.append("terms={" + ", ".join(sorted(f"ρ{r}{lens_str(l)}" for r, l in self.terms)) + "}")
        if self.dterms and self.dterms != self.terms:
            parts.append("dterms={" + ", ".join(sorted(f"ρ{r}{lens_str(l)}" for r, l in self.dterms)) + "}")
        if self.pstores:
            parts.append("pstores={" + ", ".join(sorted(f"ρ{r}{'!' if st else '?'}{lens_str(l)}" for r, st, l in self.pstores)) + "}")
        if self.top:
            parts.append("TOP")
        parts.append("closed" if self.closed else "open")
        return "⟨" + ", ".join(parts) + "⟩"


def _cap_terms(ts: FrozenSet[Tuple[int, Lens]]) -> FrozenSet[Tuple[int, Lens]]:
    by: dict = {}
    for r, l in ts:
        by.setdefault(r, set()).add(l)
    out = set()
    for r, lenses in by.items():
        if len(lenses) > MAX_LENSES:
            out.add((r, ()))
        else:
            for l in lenses:
                out.add((r, l))
    return frozenset(out)


def _join_dterms(a: FrozenSet[Tuple[int, Lens]], b: FrozenSet[Tuple[int, Lens]]) -> FrozenSet[Tuple[int, Lens]]:
    """Definite terms common to both sides (same renames; kills united)."""
    if a == b:
        return a
    by_b: dict = {}
    for r, l in b:
        by_b.setdefault(r, []).append(l)
    out = set()
    for r, la in a:
        for lb in by_b.get(r, ()):
            m = lens_join_definite(la, lb)
            if m is not None:
                out.add((r, m))
    return frozenset(out)


def _join_pstores(a, b):
    if a == b:
        return a
    out = set()
    keys_a = {(r, l): st for r, st, l in a}
    keys_b = {(r, l): st for r, st, l in b}
    for key in set(keys_a) | set(keys_b):
        sa = keys_a.get(key)
        sb = keys_b.get(key)
        strong = bool(sa) and bool(sb)       # only strong when stored on both paths
        out.add((key[0], strong, key[1]))
    return frozenset(out)


def _cap_sources(ss: FrozenSet[Tuple[Source, Lens]]) -> FrozenSet[Tuple[Source, Lens]]:
    # keep at most MAX_LENSES lenses per source; beyond that collapse to the empty lens (may provide anything)
    by: dict = {}
    for s, l in ss:
        by.setdefault(s, set()).add(l)
    out = set()
    for s, lenses in by.items():
        if len(lenses) > MAX_LENSES:
            out.add((s, ()))
        else:
            for l in lenses:
                out.add((s, l))
    return frozenset(out)


def join_rows(rows: Iterable[Row]) -> Row:
    it = iter(rows)
    try:
        acc = next(it)
    except StopIteration:
        return Row()
    for r in it:
        acc = acc.join(r)
    return acc


# ---------------------------------------------------------------------------
# call chains (reporting contexts)
# ---------------------------------------------------------------------------

MAX_CHAIN = 6


def chain_extend(chain: Tuple[int, ...], more: Tuple[int, ...]) -> Tuple[int, ...]:
    """Canonical call chain: a call site repeated in direct recursion is kept once and the
    chain keeps its innermost MAX_CHAIN sites, so the number of distinct contexts stays
    bounded in recursive programs."""
    out = list(chain)
    for x in more:
        if not out or out[-1] != x:
            out.append(x)
    if len(out) > MAX_CHAIN:
        del out[:len(out) - MAX_CHAIN]
    return tuple(out)


# ---------------------------------------------------------------------------
# verdict classification (§3.1 / §13)
# ---------------------------------------------------------------------------

PROVEN = "proven"                  # ✓
LIKELY = "likely"                  # ✓ likely (unknown code intervened)
POSSIBLY = "possibly"              # ⚠ set on some paths only
MISSING = "missing"                # ✗ definitely missing
LIKELY_MISSING = "likely-missing"  # ⚠ only unknown code could have added it
INPUT = "input"                    # ◌ depends on external input -> schema
DATAKEY = "datakey"                # ⚠ data-derived key into a closed record
MAP = "map"                        # (not reported) key of a map-role object
UNKNOWN = "unknown"                # nothing known
SYM = "symbolic"                   # decided at summary instantiation (never reported)

TIER_SYMBOL = {
    PROVEN: "✓", LIKELY: "✓~", POSSIBLY: "⚠", MISSING: "✗", LIKELY_MISSING: "⚠~",
    INPUT: "◌", DATAKEY: "⚠data", MAP: "·map", UNKNOWN: "?", SYM: "ρ",
}
TIER_RANK = {PROVEN: 0, LIKELY: 1, MAP: 2, INPUT: 3, POSSIBLY: 4, DATAKEY: 5,
             LIKELY_MISSING: 6, MISSING: 7, UNKNOWN: 8, SYM: 9}


@dataclass(frozen=True)
class Classification:
    tier: str
    providers: Tuple[Tuple[Source, FrozenSet[Any]], ...] = ()   # external sources that may provide k
    note: str = ""


def classify(row: Row, k: Any) -> Classification:
    if k in row.definite:
        return Classification(PROVEN)
    if k in row.absent:
        return Classification(MISSING, note="removed/excluded on every path")
    if k in row.likely:
        return Classification(LIKELY, note="unknown code ran: " + ", ".join(sorted(row.mutators)))
    if row.symbolic:
        return Classification(SYM)      # depends on the entry row: decided when the summary is instantiated
    if row.top:
        return Classification(UNKNOWN)
    if row.closed:
        if k in row.present:
            return Classification(POSSIBLY, note="set on some paths only")
        return Classification(MISSING, note="closed row")
    prov = row.provides(k)
    ext = tuple(sorted(((s, t) for s, t in prov.items() if not s.is_data), key=lambda x: str(x[0])))
    data = [s for s in prov if s.is_data]
    if ext:
        note = ""
        if row.mutators:
            note = "unless " + ", ".join(sorted(row.mutators)) + " adds it"
        if k in row.present:
            note = (note + "; " if note else "") + "also set on some paths"
        return Classification(INPUT, providers=ext, note=note)
    if data:
        return Classification(MAP)
    if k in row.present:
        return Classification(POSSIBLY, note="set on some paths only")
    if row.mutators:
        return Classification(LIKELY_MISSING, note="only " + ", ".join(sorted(row.mutators)) + " could have added it")
    return Classification(MISSING, note="no source can provide it")


# ---------------------------------------------------------------------------
# key values (§5.4)
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class Lit:
    key: Any

    def __str__(self) -> str:
        return key_str(self.key)


@dataclass(frozen=True)
class Lits:
    keys: FrozenSet[Any]

    def __str__(self) -> str:
        return keys_str(self.keys)


@dataclass(frozen=True)
class KeysOf:
    """The keys of some objects' namespace, minus `exclude`, mapped through `chain`.

    exhaustive_depth: conditional depth at which an iteration over these keys is
    exhaustive (stores at that depth hit every key), or -1.
    """
    objs: FrozenSet[int]
    ns: str = "item"
    exclude: FrozenSet[Any] = frozenset()
    chain: Lens = ()
    exhaustive_depth: int = -1

    def __str__(self) -> str:
        s = f"keys({','.join(f'o{i}' for i in sorted(self.objs))})"
        if self.exclude:
            s += f"∖{keys_str(self.exclude)}"
        if self.chain:
            s += lens_str(self.chain)
        return s


@dataclass(frozen=True)
class Data:
    site: str = ""

    def __str__(self) -> str:
        return f"data({self.site})" if self.site else "data"


@dataclass(frozen=True)
class Bot:
    """No value at all (identity of kv_join)."""

    def __str__(self) -> str:
        return "⊥"


@dataclass(frozen=True)
class Param:
    """The (unknown) value of placeholder object `ph`, resolved at summary instantiation."""
    ph: int

    def __str__(self) -> str:
        return f"param(ρ{self.ph})"


DATA = Data()
BOT = Bot()
KeyVal = Union[Lit, Lits, KeysOf, Data, Bot, Param]


def kv_join(a: KeyVal, b: KeyVal) -> KeyVal:
    if a == b:
        return a
    if isinstance(a, Bot) or (isinstance(a, Lits) and not a.keys):
        return b
    if isinstance(b, Bot) or (isinstance(b, Lits) and not b.keys):
        return a
    if isinstance(a, Param) or isinstance(b, Param):
        return DATA
    if isinstance(a, (Lit, Lits)) and isinstance(b, (Lit, Lits)):
        ks = kv_literals(a) | kv_literals(b)
        if len(ks) > MAX_LITS:
            return DATA
        return Lits(ks) if len(ks) != 1 else Lit(next(iter(ks)))
    if isinstance(a, KeysOf) and isinstance(b, KeysOf) and a.exclude == b.exclude and a.chain == b.chain and a.ns == b.ns:
        return KeysOf(a.objs | b.objs, a.ns, a.exclude, a.chain, -1)
    if isinstance(a, Data) and isinstance(b, Data):
        return DATA
    if isinstance(a, Data):
        return a
    if isinstance(b, Data):
        return b
    return DATA


def kv_literals(kv: KeyVal) -> FrozenSet[Any]:
    if isinstance(kv, Lit):
        return frozenset({kv.key})
    if isinstance(kv, Lits):
        return kv.keys
    return frozenset()


def kv_map(kv: KeyVal, step: RenameFn) -> KeyVal:
    if isinstance(kv, Lit):
        return Lit(step.apply(kv.key))
    if isinstance(kv, Lits):
        return Lits(frozenset(step.apply(k) for k in kv.keys))
    if isinstance(kv, KeysOf):
        return replace(kv, chain=kv.chain + (step,))
    return kv


def kv_exclude(kv: KeyVal, keys: FrozenSet[Any]) -> Optional[KeyVal]:
    """Narrow a key value by `k not in keys`; None means the path is infeasible."""
    if isinstance(kv, Param):
        return kv
    if isinstance(kv, Lit):
        return None if kv.key in keys else kv
    if isinstance(kv, Lits):
        rest = kv.keys - keys
        if not rest:
            return None
        return Lits(rest) if len(rest) > 1 else Lit(next(iter(rest)))
    if isinstance(kv, KeysOf):
        return replace(kv, exclude=kv.exclude | keys)
    return kv


def kv_restrict(kv: KeyVal, keys: FrozenSet[Any]) -> Optional[KeyVal]:
    """Narrow a key value by `k in keys`."""
    if not keys:
        return None
    if isinstance(kv, Lit):
        return kv if kv.key in keys else None
    if isinstance(kv, Lits):
        rest = kv.keys & keys
        if not rest:
            return None
        return Lits(rest) if len(rest) > 1 else Lit(next(iter(rest)))
    if isinstance(kv, KeysOf):
        return Lits(keys) if len(keys) > 1 else Lit(next(iter(keys)))
    return Lits(keys) if len(keys) > 1 else Lit(next(iter(keys)))


# ---------------------------------------------------------------------------
# requirements (§7.4)
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class ReqInfo:
    always: bool
    sites: FrozenSet[str] = frozenset()
    alternatives: FrozenSet[Source] = frozenset()   # other sources that could satisfy the same access
    caveats: FrozenSet[str] = frozenset()           # mutators that might have added the key instead
    alt_keys: FrozenSet[Any] = frozenset()          # other key names of the same source that would also satisfy it

    def join(self, o: "ReqInfo") -> "ReqInfo":
        return ReqInfo(self.always and o.always, self.sites | o.sites,
                       self.alternatives | o.alternatives, self.caveats | o.caveats,
                       self.alt_keys | o.alt_keys)

    def seq(self, o: "ReqInfo") -> "ReqInfo":
        """Same path, second requirement on the same key: always if either is."""
        return ReqInfo(self.always or o.always, self.sites | o.sites,
                       self.alternatives & o.alternatives if self.alternatives and o.alternatives else frozenset(),
                       self.caveats | o.caveats,
                       self.alt_keys & o.alt_keys if self.alt_keys and o.alt_keys else frozenset())


# ---------------------------------------------------------------------------
# values
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class Value:
    pts: FrozenSet[int] = frozenset()
    kv: KeyVal = BOT

    def join(self, o: "Value") -> "Value":
        if self == o:
            return self
        return Value(self.pts | o.pts, kv_join(self.kv, o.kv))

    @property
    def empty(self) -> bool:
        return not self.pts and isinstance(self.kv, Bot)

    @staticmethod
    def lit(k: Any) -> "Value":
        return Value(frozenset(), Lit(k))

    @staticmethod
    def obj(oid: int) -> "Value":
        return Value(frozenset({oid}), BOT)

    @staticmethod
    def data(site: str = "") -> "Value":
        return Value(frozenset(), Data(site) if site else DATA)


NOTHING = Value()


def join_values(vals: Iterable[Value]) -> Value:
    acc = NOTHING
    for v in vals:
        acc = acc.join(v)
    return acc
