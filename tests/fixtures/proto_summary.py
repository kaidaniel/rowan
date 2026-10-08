# Analysis input only: a module of the former Python prototype, kept as a large realistic
# fixture for tests/run_snapshots.py. It is not used as code.
"""Symbolic function summaries (SPECIFICATION.md §7.3, §9).

A function body is analysed once against *placeholder* objects — its parameters,
`self`, and (lazily) every pre-existing object it touches.  In that analysis the
state is *symbolic*: the row of an object that was not allocated by the function is
`Row.entry(ρ)`, "whatever the caller has", and every transfer composes on top of it
(terms, kills, renames, parameter-keyed stores).  The result is a `Summary`:

  * the exit rows / slots / elements of every object the body touched (its footprint),
    expressed over entry rows;
  * `SymReq`s — accesses whose verdict depends on an entry row, decided per call site;
  * verdicts that were decided inside the body (re-emitted with the caller's chain);
  * requirements on concrete sources created inside the body;
  * the objects the body allocated (templates, cloned per call site), the return value;
  * *events*: things the symbolic form could not express precisely (a call through a
    parameter, `isinstance` on a parameter, iteration over a parameter's literal list,
    ...).  A call site falls back to per-call-site analysis only when it has the
    concrete information the event is about.

Instantiation substitutes the caller's objects for the placeholders: effects are
applied strongly when a placeholder binds to one single object, weakly otherwise.

What keeps summaries small (the prototype's measured bottlenecks):

  * the footprint is the *difference* between exit and entry state, minus identities
    (an entry row unchanged, a slot holding the placeholder for its own content), minus
    objects the caller can never reach (a reachability pass from the return value, the
    entry objects' slots and the module bindings: most allocations are temporaries);
  * deferred accesses are keyed by (site, key, object, context) and merged across loop
    iterations and nested instantiations; the context kept inside a summary is one call
    site deep (`MAX_REL_CHAIN`), the caller's concrete chain is prepended on use, and a
    recursive instantiation of a partial summary does not nest the recursive site;
  * a closure's summary is specialised on the code its cells hold (sibling closures,
    classes) — data cells stay symbolic;
  * a function is analysed concretely for at most `MAX_TABULATIONS` call sites per kind of
    literal/structural event; calls and dispatch through a placeholder bound to an actual
    object always are (the summary could only treat them as unknown).
"""
from __future__ import annotations

import ast
from dataclasses import dataclass, field, replace as _replace
from typing import Any, Dict, FrozenSet, List, Optional, Set, Tuple

from .domain import (BOT, DATA, INPUT, chain_extend, MAP, MISSING, PROVEN, SYM, DATAKEY, POSSIBLY, Classification, Data,
                     KeysOf, Lit, Lits, Param, Row, Source, Value, classify, join_rows, join_values,
                     kv_literals)
from .state import RECORD_KINDS, State

MAX_SUMMARY_ROUNDS = 3     # rounds for a recursive function's own summary (2–3 suffice in practice, §9.1)
MAX_REL_CHAIN = 1          # call sites kept, inside a summary, below the function's own body
MAX_TABULATIONS = 12       # per function: call sites analysed concretely before the summary is used regardless
MAX_CALL_TABULATIONS = 32  # same, for calls through a parameter / closure cell (callbacks)
MAX_SYMREQ_TARGETS = 8     # a deferred access on more objects than this reports per allocation site
CODE_KINDS = frozenset({"function", "class", "bound", "external", "partial", "method"})


def rho(oid: int, ns: str) -> int:
    return oid * 2 + (1 if ns == "attr" else 0)


def unrho(r: int) -> Tuple[int, str]:
    return r // 2, "attr" if r % 2 else "item"


@dataclass
class PH:
    """Payload of a placeholder object."""
    kind: str                     # param | self | vararg | kwarg | path | narrow (parent when it is a cls)
    func: int                     # function the placeholder belongs to
    name: str = ""
    parent: Optional[int] = None  # path: object whose slot this stands for
    ns: str = "item"
    key: Any = None
    cls: Optional[int] = None     # declared class (annotation), when known


@dataclass
class SymReq:
    node: Any
    key_kv: Any                   # Lit | Lits | Param | Data | KeysOf
    row: Row                      # symbolic row at the access
    oid: int
    ns: str
    guarded: bool
    rel_chain: Tuple[int, ...]
    kind: str                     # load | delete | attr
    file: str
    text: str
    cls: Optional[int] = None     # only for targets that are instances of this class (name-based dispatch)


@dataclass
class Event:
    kind: str                     # call | dispatch | iter | narrow | isinstance | concrete | truth | guarded
    ph: int


@dataclass
class Summary:
    foid: int
    ph_by_name: Dict[str, int]
    ph_self: Optional[int]
    exit: State
    ret: Value
    symreqs: List[SymReq]
    rel_verdicts: tuple
    notes: List[str]
    templates: Set[int]
    events: List[Event]
    rounds: int = 1
    partial: bool = False
    # footprint: the exit facts that differ from the entry facts (computed once)
    rows_fp: List[Tuple[int, str, Row]] = field(default_factory=list)
    slots_fp: List[Tuple[Tuple[int, str, Any], Value]] = field(default_factory=list)
    elems_fp: List[Tuple[int, Any]] = field(default_factory=list)

    def compute_footprint(self, objects: Dict[int, Any], base: State) -> None:
        """Exit facts that differ from the entry state `base` (module bindings, parameters)
        and are not the identity (an entry row unchanged, a slot holding the placeholder that
        stands for its own entry content)."""
        def is_path_of(v: Value, oid: int, ns: str, key: Any) -> bool:
            if len(v.pts) != 1:
                return False
            ph = next(iter(v.pts))
            ob = objects.get(ph)
            if ob is None or ob.kind != "ph" or ob.payload.kind != "path":
                return False
            p = ob.payload
            return p.parent == oid and p.ns == ns and p.key == key and (isinstance(v.kv, Param) and v.kv.ph == ph or v.kv is BOT)
        rows = self.exit.rows
        self.rows_fp = []
        for key in sorted(rows.diff_keys(base.rows), key=repr):
            expr = rows.get(key)
            if expr is None:
                continue          # deleted rows are not re-created in the caller
            oid, ns = key
            if expr != Row.entry(rho(oid, ns)):
                self.rows_fp.append((oid, ns, expr))
        slots = self.exit.slots
        self.slots_fp = []
        for (oid, ns, k) in sorted(slots.diff_keys(base.slots), key=repr):
            v = slots.get((oid, ns, k))
            if v is None or is_path_of(v, oid, ns, k):
                continue
            self.slots_fp.append(((oid, ns, k), v))
        elems = self.exit.elems
        self.elems_fp = []
        for oid in sorted(elems.diff_keys(base.elems), key=repr):
            kv = elems.get(oid)
            if kv is None:
                continue
            if isinstance(kv, Param) and is_path_of(Value(frozenset({kv.ph}), kv), oid, "item", "*"):
                continue
            self.elems_fp.append((oid, kv))
        self._collect_garbage(objects)

    def _collect_garbage(self, objects: Dict[int, Any]) -> None:
        """Objects the body allocated that the caller can never reach — not from the return
        value, not from any entry object's slots, not from module bindings — are dead at exit:
        they are dropped from the footprint (the bulk of a large function's allocations are
        such temporaries)."""
        from .domain import KeysOf as _KeysOf
        slots = self.exit.slots
        elems = self.exit.elems
        templates = self.templates
        roots: Set[int] = set(self.ret.pts)
        if isinstance(self.ret.kv, _KeysOf):
            roots |= set(self.ret.kv.objs)
        for oid, _ns, _row in self.rows_fp:
            if oid not in templates:
                roots.add(oid)
        for (oid, _ns, _k), _v in self.slots_fp:
            if oid not in templates:
                roots.add(oid)
        for oid, _kv in self.elems_fp:
            if oid not in templates:
                roots.add(oid)
        reach: Set[int] = set()
        work = list(roots)

        def push_value(v: Any) -> None:
            if isinstance(v, Value):
                work.extend(v.pts)
                if isinstance(v.kv, _KeysOf):
                    work.extend(v.kv.objs)
                elif isinstance(v.kv, Param):
                    work.append(v.kv.ph)

        while work:
            o = work.pop()
            if o in reach:
                continue
            reach.add(o)
            for _nk, v in slots.for_obj(o):
                push_value(v)
            kv = elems.get(o)
            if kv is not None:
                push_value(Value(frozenset(), kv))
            ob = objects.get(o)
            if ob is None:
                continue
            if ob.derived_from is not None:
                work.append(ob.derived_from)
            payload = ob.payload
            if ob.kind == "function" and payload is not None and hasattr(payload, "closure"):
                work.extend(payload.closure.values())
            elif ob.kind == "ph" and isinstance(payload, PH) and payload.parent is not None:
                work.append(payload.parent)
            elif isinstance(payload, Value):
                push_value(payload)
            elif isinstance(payload, tuple):
                for x in payload:
                    if isinstance(x, Value):
                        push_value(x)
                    elif isinstance(x, int) and not isinstance(x, bool):
                        work.append(x)
                    elif isinstance(x, tuple):
                        work.extend(y for y in x if isinstance(y, int) and not isinstance(y, bool))
            elif isinstance(payload, int) and not isinstance(payload, bool):
                work.append(payload)
        dead = {t for t in templates if t not in reach}
        if not dead:
            return
        self.templates = templates - dead
        self.rows_fp = [(o, ns, r) for o, ns, r in self.rows_fp if o not in dead]
        self.slots_fp = [((o, ns, k), v) for (o, ns, k), v in self.slots_fp if o not in dead]
        self.elems_fp = [(o, kv) for o, kv in self.elems_fp if o not in dead]


class SummaryMixin:
    """Mixed into Analyzer.  Relies on Analyzer's interpreter methods."""

    # ------------------------------------------------------------ placeholders
    def new_placeholder(self, kind: str, func: int, name: str = "", parent: Optional[int] = None,
                        ns: str = "item", key: Any = None, cls: Optional[int] = None, state: Optional[State] = None,
                        tag: str = "") -> int:
        if kind == "path":
            site = f"ph:path:{func}:{parent}:{ns}:{key!r}{tag}"
        else:
            site = f"ph:{kind}:{func}:{name}"
        oid = self._alloc("ph", site, f"sum{func}", True, f"ρ{name or key!r}", payload=PH(kind, func, name, parent, ns, key, cls))
        self.objects[oid].payload = PH(kind, func, name, parent, ns, key, cls)
        if kind == "path":
            self.objects[oid].depth = (self.objects[parent].depth + 1) if parent is not None else 1
            self.objects[oid].iter_elem = key == "*"
        return oid

    def narrowed_placeholder(self, parent: int, cls: int, state: State) -> int:
        """`isinstance(x, C)` on placeholder x: the placeholder standing for x when it is a C.
        It resolves to the C-instances among x's actual objects."""
        func = self.sum_stack[-1]
        site = f"ph:narrow:{func}:{parent}:{cls}"
        oid = self._alloc("ph", site, f"sum{func}", True, f"{self.objects[parent].label}:{self.objects[cls].label}",
                          payload=PH("narrow", func, parent=parent, cls=cls))
        self.objects[oid].payload = PH("narrow", func, parent=parent, cls=cls)
        self.objects[oid].depth = self.objects[parent].depth
        if (oid, "item") not in state.rows:
            state.set_row(oid, "item", Row.entry(rho(oid, "item")))
            state.set_row(oid, "attr", Row.entry(rho(oid, "attr")))
        return oid

    def path_placeholder(self, parent: int, ns: str, key: Any, state: State, cls: Optional[int] = None,
                         tag: str = "") -> int:
        """Placeholder for the (unknown) content of an entry object's slot, inside a summary."""
        func = self.sum_stack[-1]
        if cls is None and ns == "cell":
            cls = self.cell_classes.get(parent)
        oid = self.new_placeholder("path", func, parent=parent, ns=ns, key=key, state=state, cls=cls, tag=tag)
        if (oid, "item") not in state.rows:
            state.set_row(oid, "item", Row.entry(rho(oid, "item")))
            state.set_row(oid, "attr", Row.entry(rho(oid, "attr")))
        return oid

    def is_ph(self, oid: int) -> bool:
        return self.objects[oid].kind == "ph"

    def record_event(self, kind: str, ph: int) -> None:
        """The symbolic analysis just lost information about entry object `ph` (a placeholder,
        or a pre-existing object whose row is symbolic here)."""
        if self.sum_stack and (self.objects[ph].kind == "ph" or self.is_entry_object(ph)):
            self.sum_events[-1][(kind, ph)] = Event(kind, ph)

    def _defer(self, sr: "SymReq") -> None:
        """Record a deferred access; the same access seen again (loop iterations, nested
        instantiations) is merged by joining the rows it was made against."""
        # the context kept inside a summary is bounded (§9.5): the caller's concrete chain is
        # prepended at instantiation, so verdicts still name the call path into the function
        if len(sr.rel_chain) > MAX_REL_CHAIN:
            sr.rel_chain = sr.rel_chain[:MAX_REL_CHAIN]
        key = (sr.node, sr.key_kv, sr.oid, sr.ns, sr.guarded, sr.rel_chain, sr.kind, sr.cls)
        reqs = self.sum_reqs[-1]
        prev = reqs.get(key)
        if prev is None:
            reqs[key] = sr
        elif prev.row != sr.row:
            prev.row = prev.row.join(sr.row)

    def record_symreq(self, node: Any, key_kv: Any, row: Row, oid: int, ns: str, guarded: bool, kind: str) -> None:
        mod = self.objects[self.frame.module].payload
        import os
        fname = "" if self.frame.module == self.main_module else os.path.basename(mod.path or mod.name)
        try:
            text = ast.unparse(node)
        except Exception:
            text = ""
        guard = None
        ob = self.objects[oid]
        if ob.kind == "ph" and ob.payload.cls is not None:
            guard = ob.payload.cls       # declared / narrowed class of the placeholder
        self._defer(SymReq(node, key_kv, row, oid, ns, guarded, tuple(self.call_chain), kind, fname, text, guard))

    # ------------------------------------------------------------- summarise
    def summarise(self, foid: int, state: State) -> Optional[Summary]:
        """Compute (or fetch) the summary of function object foid.  None while it is being
        computed (recursion): the caller then behaves as a ⊥ summary."""
        fo = self.objects[foid]
        fi = fo.payload
        fnode = fi.node
        # closure cells holding code (sibling closures, classes) are stable facts the summary
        # is specialised on; cells holding data stay symbolic (resolved per call)
        code_cells: Dict[int, Value] = {}
        typed_cells: Dict[int, int] = {}
        # the closure's own cells, plus (transitively) the cells of the sibling closures they
        # hold — a nested closure summarised from inside a sibling's summary finds what it calls
        cells: List[int] = list(fi.closure.values())
        seen_cells: set = set()
        i = 0
        while i < len(cells):
            cid = cells[i]
            i += 1
            if cid in seen_cells:
                continue
            seen_cells.add(cid)
            for o in state.slot(cid, "cell", "v").pts:
                ob = self.objects[o]
                if ob.kind == "function" and ob.payload is not None:
                    cells.extend(ob.payload.closure.values())
        for cid in dict.fromkeys(cells):
            cv = state.slot(cid, "cell", "v")
            if cv.pts and all(self.objects[o].kind in CODE_KINDS for o in cv.pts):
                code_cells[cid] = cv
            elif cv.pts:
                # a data cell holding instances of one class: the summary knows the class
                # (methods resolve), not the objects
                classes = {self.objects[o].payload for o in cv.pts
                           if self.objects[o].kind == "instance" and isinstance(self.objects[o].payload, int)}
                if len(classes) == 1 and all(self.objects[o].kind == "instance" for o in cv.pts):
                    typed_cells[cid] = next(iter(classes))
        skey = (foid, tuple(sorted((cid, tuple(sorted(v.pts))) for cid, v in code_cells.items())),
                tuple(sorted(typed_cells.items())))
        done = self.summary_table.get(skey)
        if done is not None:
            return done
        if foid in self.sum_stack:
            self.sum_hit.add(foid)
            return self.sum_partial.get(foid)
        # save interpreter context
        saved = (self.frames, self.call_chain, self.loop_stack, self.guards, self.may_raise, self.cond_depth,
                 self.exh_log, self.active, self.active_by_func, self.rec_entry, self.rec_widened, self.rec_hit)
        self.frames = list(self.frames[:1]) if self.frames else []
        self.call_chain = []
        self.loop_stack = []
        self.guards = []
        self.may_raise = []
        self.cond_depth = 0
        self.exh_log = None
        self.active = {}
        self.active_by_func = {}
        self.rec_entry = {}
        self.rec_widened = set()
        self.rec_hit = set()
        self.sum_stack.append(foid)
        self.sum_reqs.append({})
        self.sum_events.append({})
        self.sum_obj_mark.append(len(self.objects) + 1)
        verdict_mark = len(self.verdicts)
        notes_mark = len(self.notes)
        summary: Optional[Summary] = None
        try:
            from .interp import Frame  # local import to avoid a cycle at module load
            ctx = f"sum{foid}"
            for rnd in range(MAX_SUMMARY_ROUNDS):
                del self.verdicts[verdict_mark:]
                for n_ in self.notes[notes_mark:]:
                    self._notes_set.discard(n_)
                del self.notes[notes_mark:]
                self._verdict_keys = {k for k in self._verdict_keys if not (len(k) > 6 and False)}  # keys of rel verdicts are re-added on emit
                self.sum_reqs[-1] = {}
                self.sum_events[-1] = {}
                obj_mark = self.sum_obj_mark[-1]
                st = State()
                st.symbolic = True
                # module bindings (functions, classes, constants) and the contents of
                # module-level literal containers are entry facts assumed stable
                for moid in self.modules.values():
                    for k, v in state.obj_slots(moid, "attr"):
                        st.slots[(moid, "attr", k)] = v
                        for x in v.pts:
                            xo = self.objects[x]
                            if xo.kind in ("list", "tuple", "set"):
                                for (ns2, k2), v2 in state.slots.for_obj(x):
                                    st.slots[(x, ns2, k2)] = v2
                                e = state.elems.get(x)
                                if e is not None:
                                    st.elems[x] = e
                            elif xo.kind == "dict" and self.module_done:
                                # module-level tables are entry facts once the module has run
                                # (while it runs they may still be being filled: symbolic)
                                for ns2 in ("item", "attr"):
                                    r = state.rows.get((x, ns2))
                                    if r is not None:
                                        st.set_row(x, ns2, r)
                                for (ns2, k2), v2 in state.slots.for_obj(x):
                                    st.slots[(x, ns2, k2)] = v2
                                e = state.elems.get(x)
                                if e is not None:
                                    st.elems[x] = e
                for cid, cv in code_cells.items():
                    st.slots[(cid, "cell", "v")] = cv
                self.cell_classes.update(typed_cells)
                fr = Frame("function", fi.scope, fi.module, ctx, True, func=fo, label=fi.qualname)
                fr.symbolic = True
                self.frames.append(fr)
                ph_by_name: Dict[str, int] = {}
                a = fnode.args
                positional = [p for p in list(a.posonlyargs) + list(a.args)]
                self_ph = None
                if fi.cls is not None and not fi.is_static and positional:
                    pcls = self.objects[fi.cls].id if fi.cls in self.objects else None
                    self_ph = self.new_placeholder("self", foid, positional[0].arg, cls=fi.cls, state=st)
                    ph_by_name[positional[0].arg] = self_ph
                    positional = positional[1:]
                args: List[Value] = []
                for p in positional:
                    cls = self.annotation_class(p.annotation, fi, state) if p.annotation is not None else None
                    ph = self.new_placeholder("param", foid, p.arg, cls=cls, state=st)
                    ph_by_name[p.arg] = ph
                    args.append(Value(frozenset({ph}), Param(ph)))
                kwargs: Dict[str, Value] = {}
                for p in a.kwonlyargs:
                    cls = self.annotation_class(p.annotation, fi, state) if p.annotation is not None else None
                    ph = self.new_placeholder("param", foid, p.arg, cls=cls, state=st)
                    ph_by_name[p.arg] = ph
                    kwargs[p.arg] = Value(frozenset({ph}), Param(ph))
                for ph in ph_by_name.values():
                    st.set_row(ph, "item", Row.entry(rho(ph, "item")))
                    st.set_row(ph, "attr", Row.entry(rho(ph, "attr")))
                # cells of this function (for nested closures) are allocated by the normal binding code
                for name in fi.scope.cells:
                    cid = self._alloc("cell", f"cell:{fi.qualname}.{name}", ctx, True, f"cell {name}")
                    fr.cells[name] = cid
                fr.cells.update(fi.closure)
                st.env = {}
                self_v = Value(frozenset({self_ph}), Param(self_ph)) if self_ph is not None else None
                fake = ast.Call(func=ast.Name(id=fi.name, ctx=ast.Load()), args=[], keywords=[])
                fake.lineno = getattr(fnode, "lineno", 0)
                fake.col_offset = 0
                self._bind_params(fi, self_v, args, kwargs, [], [], st, fake)
                if a.vararg is not None:
                    ph = self.new_placeholder("vararg", foid, a.vararg.arg, state=st)
                    ph_by_name[a.vararg.arg] = ph
                    self.store_name(a.vararg.arg, Value(frozenset({ph}), Param(ph)), st)
                if a.kwarg is not None:
                    ph = self.new_placeholder("kwarg", foid, a.kwarg.arg, state=st)
                    ph_by_name[a.kwarg.arg] = ph
                    st.set_row(ph, "item", Row.entry(rho(ph, "item")))
                    self.store_name(a.kwarg.arg, Value(frozenset({ph}), Param(ph)), st)
                base = st.copy()
                # run the body
                if isinstance(fnode, ast.Lambda):
                    v = self.eval(fnode.body, st)
                    st.ret = v
                    st.record_exit()
                    st.reachable = False
                else:
                    self.exec_block(fnode.body, st)
                ret = st.ret if st.ret is not None else Value()
                if st.reachable:
                    ret = ret.join(self.NONE)
                if st.exits is not None:
                    ex = st.exits
                    st.exits = None
                    st = st.join(ex) if st.reachable else ex
                    st.exits = None
                st.reachable = True
                gen = fi.__dict__.get("generator")
                if gen is not None and not isinstance(fnode, ast.Lambda) and _is_generator(fnode):
                    ret = Value.obj(gen)
                self.frames.pop()
                templates = {oid for oid in range(obj_mark, len(self.objects) + 1) if oid in self.objects
                             and self.objects[oid].kind not in ("ph", "method", "bound", "external", "super", "view")}
                rel_full = tuple(self.verdicts[verdict_mark:])
                seen_rel: set = set()
                rel_list = []
                for v in rel_full:
                    self._verdict_keys.discard((v.file, v.line, v.col, _key_str(v.key), v.tier, v.oid, v.chain))
                    if len(v.chain) > MAX_REL_CHAIN:
                        v = _replace(v, chain=v.chain[:MAX_REL_CHAIN])
                    dk = (v.file, v.line, v.col, _key_str(v.key), v.tier, v.oid, v.chain)
                    if dk not in seen_rel:
                        seen_rel.add(dk)
                        rel_list.append(v)
                rel = tuple(rel_list)
                del self.verdicts[verdict_mark:]
                # notes made while analysing the body belong to the summary: they are reported
                # when (and where) the summary is actually used
                body_notes = self.notes[notes_mark:]
                for n_ in body_notes:
                    self._notes_set.discard(n_)
                del self.notes[notes_mark:]
                summary = Summary(foid, ph_by_name, self_ph, st, ret, list(self.sum_reqs[-1].values()), rel, body_notes,
                                  templates, list(self.sum_events[-1].values()), rounds=rnd + 1)
                summary.compute_footprint(self.objects, base)
                if foid in self.sum_hit:
                    prev = self.sum_partial.get(foid)
                    if prev is not None and rnd >= 1:
                        # widen: later rounds accumulate (the sequence becomes monotone)
                        st = st.join(prev.exit) if not prev.exit.same(st) else st
                        ret = ret.join(prev.ret)
                        summary.exit = st
                        summary.ret = ret
                        summary.compute_footprint(self.objects, base)
                    summary.partial = True
                    self.sum_partial[foid] = summary
                    if prev is not None and prev.ret == ret and prev.exit.same(st) and len(prev.symreqs) == len(summary.symreqs):
                        summary.partial = False
                        break
                    if _DEBUG and rnd >= 2:
                        import sys as _sys
                        why = "ret" if prev is None or prev.ret != ret else ("exit" if not prev.exit.same(st) else f"symreqs {len(prev.symreqs)}->{len(summary.symreqs)}")
                        print(f"[rowan] round {rnd + 1} for {fi.qualname}: not converged ({why}) stack={[self.objects[f].payload.qualname for f in self.sum_stack][-3:]}", file=_sys.stderr, flush=True)
                    continue
                break
        finally:
            self.sum_stack.pop()
            self.sum_reqs.pop()
            self.sum_events.pop()
            self.sum_obj_mark.pop()
            self.sum_hit.discard(foid)
            self.sum_partial.pop(foid, None)
            (self.frames, self.call_chain, self.loop_stack, self.guards, self.may_raise, self.cond_depth,
             self.exh_log, self.active, self.active_by_func, self.rec_entry, self.rec_widened, self.rec_hit) = saved
        if summary is not None:
            summary.partial = False
            self.summary_table[skey] = summary
        return summary

    def annotation_class(self, ann: Any, fi: Any, state: State) -> Optional[int]:
        node = ann
        while isinstance(node, ast.Subscript):
            node = node.slice
            if isinstance(node, ast.Tuple) and node.elts:
                node = node.elts[0]
        if isinstance(node, ast.BinOp) and isinstance(node.op, ast.BitOr):
            node = node.left
        if isinstance(node, ast.Constant) and isinstance(node.value, str):
            try:
                node = ast.parse(node.value, mode="eval").body
            except SyntaxError:
                return None
        if not isinstance(node, (ast.Name, ast.Attribute)):
            return None
        from .interp import Frame
        mi = self.objects[fi.module].payload
        self.frames.append(Frame("module", mi.scopes[mi.tree], fi.module, "", True, label="<ann>"))
        try:
            v = self.eval(node, state)
        except Exception:
            return None
        finally:
            self.frames.pop()
        for oid in v.pts:
            if self.objects[oid].kind == "class":
                return oid
        return None

    # ------------------------------------------------------- fallback decision
    def binding_of(self, summary: Summary, ph: int, bind: Dict[int, Value], state: State, memo: Dict[int, Value]) -> Value:
        """Actual value a placeholder stands for at this call (parameters and paths)."""
        if ph in memo:
            return memo[ph]
        memo[ph] = Value()
        if ph in bind:
            v = bind[ph]
        else:
            ob = self.objects[ph]
            if ob.kind != "ph":
                v = Value.obj(ph)          # a pre-existing object: the call site always knows it
            elif ob.payload.kind == "narrow":
                pv = self.binding_of(summary, ob.payload.parent, bind, state, memo)
                v = Value(frozenset(o for o in pv.pts if self._may_be_instance(o, ob.payload.cls)), pv.kv)
            elif ob.kind == "ph" and ob.payload.kind == "path":
                p: PH = ob.payload
                if p.parent is None:
                    parents: FrozenSet[int] = frozenset()
                elif self.objects[p.parent].kind == "ph":
                    parents = self.binding_of(summary, p.parent, bind, state, memo).pts
                elif p.parent in summary.templates:
                    parents = frozenset()
                else:
                    parents = frozenset({p.parent})
                v = Value()
                for par in parents:
                    if par in summary.templates:
                        continue
                    if p.key == "*":
                        items, elem, pair, _ = self.iterate(Value.obj(par), ast.Name(id="x", ctx=ast.Load()), state)
                        v = v.join(elem[0]).join(elem[1]) if pair else v.join(elem)
                    elif isinstance(p.key, tuple):
                        v = v.join(state.all_slots(par, p.ns))
                    else:
                        v = v.join(state.slot(par, p.ns, p.key))
            else:
                v = Value()
        memo[ph] = v
        return v

    def should_tabulate(self, summary: Summary, bind: Dict[int, Value], state: State) -> bool:
        ev = self.tabulation_reason(summary, bind, state)
        if ev is None:
            return False
        # bounded per function and kind of event; beyond the budget the summary is used (a
        # callback stays an unknown call; a dispatch applies each class's method only to
        # receivers of that class — SymReq.cls)
        key = (summary.foid, ev.kind if ev.kind in ("call", "dispatch") else "lit")
        n = self.tab_count.get(key, 0)
        if n >= (MAX_CALL_TABULATIONS if ev.kind in ("call", "dispatch") else MAX_TABULATIONS):
            return False
        self.tab_count[key] = n + 1
        if _DEBUG:
            _debug_tabulate(self, summary, ev)
        return True

    def tabulation_reason(self, summary: Summary, bind: Dict[int, Value], state: State) -> Optional[Event]:
        """The event whose concrete information this call site has (→ analyse the body for
        this call site), or None when the symbolic summary is as precise."""
        if summary.partial:
            return None
        memo: Dict[int, Value] = {}
        for ev in summary.events:
            v = self.binding_of(summary, ev.ph, bind, state, memo)
            if v.empty and ev.kind == "call":
                # a call through `x.m` where the caller's x has no slot m (a frame's method, a
                # reader's method...): the call is about x itself — unless x is an instance whose
                # class does not define m either (concrete analysis would not know more)
                ph = ev.ph
                while v.empty and self.objects[ph].kind == "ph" and self.objects[ph].payload.kind in ("path", "narrow") \
                        and self.objects[ph].payload.parent is not None:
                    key = self.objects[ph].payload.key
                    ph = self.objects[ph].payload.parent
                    pv = self.binding_of(summary, ph, bind, state, memo)
                    useful = frozenset(o for o in pv.pts if self.objects[o].kind != "ph" and (
                        self.objects[o].kind != "instance" or (isinstance(key, str) and self._class_resolves(o, key))))
                    v = Value(useful, pv.kv) if useful else Value()
            if v.empty:
                continue
            concrete = [o for o in v.pts if self.objects[o].kind != "ph"]
            if ev.kind == "call":
                # the symbolic body could only treat the call as unknown; any actual object
                # (a function, a frame, a reader, ...) lets the call site do better
                if concrete:
                    return ev
            elif ev.kind == "dispatch":
                if any(self.objects[o].kind in ("function", "bound", "class", "method", "partial", "instance", "external") for o in concrete):
                    return ev
            elif ev.kind == "iter":
                for o in concrete:
                    ob = self.objects[o]
                    if ob.kind in ("list", "tuple", "set") and ob.literal_len is not None:
                        return ev
                    if ob.kind == "dict" and self._literal_dict_items(o, state) is not None:
                        return ev
                    if ob.kind in ("view", "zip", "pair"):
                        return ev
                if not v.pts and isinstance(v.kv, (Lit, Lits)):
                    return ev
            elif ev.kind in ("narrow", "truth"):
                if isinstance(v.kv, (Lit, Lits)) or concrete:
                    return ev
            elif ev.kind in ("isinstance", "concrete", "guarded"):
                if concrete:
                    return ev
        return None

    # --------------------------------------------------------------- instantiate
    def instantiate(self, summary: Summary, bind: Dict[int, Value], state: State, node: Any,
                    call_ctx: str, ctx_single: bool) -> Value:
        if _DEBUG:
            return _debug_instantiate(self, summary, lambda: self._instantiate(summary, bind, state, node, call_ctx, ctx_single))
        return self._instantiate(summary, bind, state, node, call_ctx, ctx_single)

    def _instantiate(self, summary: Summary, bind: Dict[int, Value], state: State, node: Any,
                     call_ctx: str, ctx_single: bool) -> Value:
        pre = state.copy()
        chain = tuple(self.call_chain)
        if summary.partial and chain:
            # a recursive call inside the function's own summary: its deferred accesses are
            # the function's own, re-seen one level down; keep them at the function's level so
            # the rounds converge instead of nesting the recursive site round after round
            chain = chain[:-1]
        clones: Dict[int, int] = {}
        resolved: Dict[int, FrozenSet[int]] = {}
        kv_cache: Dict[int, Any] = {}
        fr_single = self.frame.single and self.frame.loop_depth == 0 and ctx_single
        in_summary = bool(self.sum_stack)

        def kv_of(ph: int) -> Any:
            if ph in kv_cache:
                return kv_cache[ph]
            v = bind.get(ph)
            if v is not None:
                kv = v.kv
            else:
                ob = self.objects[ph]
                if ob.kind == "ph" and ob.payload.kind == "path":
                    kv = self.path_value(ob.payload, resolve, kv_of, pre, state).kv
                elif ob.kind == "ph" and ob.payload.kind == "narrow":
                    kv = kv_of(ob.payload.parent)
                else:
                    kv = DATA
            kv_cache[ph] = kv
            return kv

        def resolve(oid: int) -> FrozenSet[int]:
            if oid in resolved:
                return resolved[oid]
            resolved[oid] = frozenset()        # cycle guard
            ob = self.objects.get(oid)
            if ob is None:
                out: FrozenSet[int] = frozenset()
            elif oid in summary.templates:
                out = frozenset({clone(oid)})
            elif ob.kind == "ph":
                if oid in bind:
                    out = bind[oid].pts
                elif ob.payload.kind == "path":
                    out = self.path_value(ob.payload, resolve, kv_of, pre, state).pts
                elif ob.payload.kind == "narrow":
                    out = frozenset(o for o in resolve(ob.payload.parent) if self._may_be_instance(o, ob.payload.cls))
                else:
                    out = frozenset()
            else:
                out = frozenset({oid})
            resolved[oid] = out
            return out

        def remap_value(v: Value) -> Value:
            pts: Set[int] = set()
            for o in v.pts:
                pts |= resolve(o)
            kv = v.kv
            if isinstance(kv, Param):
                kv = kv_of(kv.ph)
            elif isinstance(kv, KeysOf):
                objs: Set[int] = set()
                for o in kv.objs:
                    objs |= resolve(o)
                kv = KeysOf(frozenset(objs), kv.ns, kv.exclude, kv.chain, -1) if objs else DATA
            return Value(frozenset(pts), kv)

        erow_cache: Dict[int, Row] = {}
        conc_cache: Dict[Row, Row] = {}

        def entry_row_of(r: int) -> Row:
            hit = erow_cache.get(r)
            if hit is not None:
                return hit
            oid, ns = unrho(r)
            targets = resolve(oid)
            if not targets:
                out = Row()
            else:
                out = join_rows((pre.row(t, ns) if (t, ns) in pre.rows else state.row(t, ns)) for t in targets)
            erow_cache[r] = out
            return out

        def erow_of(a: int, ns_: str) -> Row:
            return pre.row(a, ns_) if (a, ns_) in pre.rows else state.row(a, ns_)

        def mentions(row: Row, ph: int) -> bool:
            refs = row.refs
            return (ph * 2) in refs or (ph * 2 + 1) in refs

        def concretise(row: Row, focus: Optional[Tuple[int, Any]] = None) -> Row:
            """`focus=(ph, a)`: the row as it is for target `a` of placeholder `ph` — ph's own
            entry rows are a's (not the join over all of ph's targets); `a` may also be a pair
            of rows (item, attr) standing for a group of targets."""
            if focus is not None and not mentions(row, focus[0]):
                focus = None          # the focus only matters when the row mentions that placeholder
            ck = (row, focus)
            hit = conc_cache.get(ck)
            if hit is None:
                erow = entry_row_of
                if focus is not None:
                    ph, a = focus
                    r_item, r_attr = rho(ph, "item"), rho(ph, "attr")
                    if isinstance(a, tuple):
                        a_item, a_attr = a

                        def erow(r: int) -> Row:   # noqa: E306
                            if r == r_item:
                                return a_item
                            if r == r_attr:
                                return a_attr
                            return entry_row_of(r)
                    else:
                        def erow(r: int) -> Row:   # noqa: E306
                            if r == r_item or r == r_attr:
                                return erow_of(a, "item" if r == r_item else "attr")
                            return entry_row_of(r)
                if row.is_entry:
                    hit = erow(next(iter(row.terms))[0])      # fast path: the untouched entry row
                else:
                    hit = row.concretise(erow, kv_of)
                if hit.symbolic and not in_summary:
                    hit = hit.concretise(erow, kv_of)
                conc_cache[ck] = hit
            return hit

        new_clones: Set[int] = set()

        def clone(t: int) -> int:
            if t in clones:
                return clones[t]
            tobj = self.objects[t]
            payload = tobj.payload
            existed = (tobj.kind, tobj.site, call_ctx) in self.obj_index
            nid = self.alloc(tobj.kind, tobj.site, state, label=tobj.label, payload=None,
                             ns="attr" if tobj.kind in ("instance", "namespace") else "item",
                             single=fr_single and tobj.single, ctx=call_ctx)
            if not existed:
                new_clones.add(nid)
            clones[t] = nid
            nobj = self.objects[nid]
            nobj.literal_len = tobj.literal_len
            nobj.iter_elem = tobj.iter_elem
            nobj.depth = tobj.depth
            if tobj.derived_from is not None:
                dd = resolve(tobj.derived_from)
                nobj.derived_from = next(iter(dd)) if len(dd) == 1 else None
            if tobj.kind == "function":
                from .interp import FuncInfo
                fi = payload
                nfi = FuncInfo(fi.node, fi.name, fi.qualname, fi.module, fi.scope,
                               {n: next(iter(resolve(c))) if resolve(c) else c for n, c in fi.closure.items()},
                               dict(fi.defaults), fi.cls, fi.is_property, fi.is_static, fi.is_classmethod)
                nobj.payload = nfi
            elif tobj.kind == "bound":
                f, sv = payload
                fr_ = resolve(f)
                nobj.payload = (next(iter(fr_)) if fr_ else f, remap_value(sv))
            elif tobj.kind in ("method", "view", "zip", "enumerate", "pair", "partial", "groupby", "locidx"):
                nobj.payload = _remap_payload(payload, remap_value, resolve)
            else:
                nobj.payload = payload
            return nid

        # 1. clones for all templates (rows/slots set below)
        for t in sorted(summary.templates):
            clone(t)
        # 2. rows
        targets_count: Dict[Tuple[int, str], int] = {}
        plan = []
        for oid, ns, expr in summary.rows_fp:
            tg = resolve(oid)
            plan.append((oid, ns, expr, tg))
            for a in tg:
                targets_count[(a, ns)] = targets_count.get((a, ns), 0) + 1
        written: Set[Tuple[int, str]] = set()
        for oid, ns, expr, tg in plan:
            if not tg:
                continue
            is_template = oid in summary.templates
            many = len(tg) > 1
            # the exit row of target a is the body's effect applied to a's own entry row; with
            # many targets, objects of one allocation site share the join of their entry rows
            per_target: Dict[int, Row] = {}
            if not many or not mentions(expr, oid):
                c0 = concretise(expr, None)
                for a in tg:
                    per_target[a] = c0
            elif len(tg) <= MAX_SYMREQ_TARGETS:
                for a in tg:
                    per_target[a] = concretise(expr, (oid, a))
            else:
                by_site: Dict[Tuple[str, str], List[int]] = {}
                for a in tg:
                    ob = self.objects[a]
                    by_site.setdefault((ob.kind, ob.site), []).append(a)
                for _sk, members in by_site.items():
                    if len(members) == 1:
                        per_target[members[0]] = concretise(expr, (oid, members[0]))
                    else:
                        rows_pair = (join_rows(erow_of(a, "item") for a in members), join_rows(erow_of(a, "attr") for a in members))
                        c0 = concretise(expr, (oid, rows_pair))
                        for a in members:
                            per_target[a] = c0
            for a in tg:
                concrete = per_target[a]
                # a template's row is exact for every object this callee allocates at the site
                # (new clone, or a reused one whose row does not depend on the entry rows)
                strong = targets_count[(a, ns)] == 1 and (a in new_clones or (is_template and (self.strongable(a) or not expr.symbolic))
                                                          or (not many and not is_template and (self.strongable(a) or a in self.fresh_init)))
                if strong:
                    state.set_row(a, ns, concrete)
                elif (a, ns) in written:
                    # several callee objects map onto a: their rows combine
                    state.set_row(a, ns, state.row(a, ns).join(concrete))
                elif (a, ns) in pre.rows:
                    state.set_row(a, ns, pre.row(a, ns).join(concrete))
                else:
                    # an object that did not exist before the call: nothing to join with
                    state.set_row(a, ns, concrete)
                written.add((a, ns))
        # rows are final from here on: entry rows read through `state` (objects created by this
        # instantiation) may be cached
        erow_cache.clear()
        conc_cache.clear()
        # 3. slots
        for (oid, ns, k), v in summary.slots_fp:
            tg = resolve(oid)
            if not tg:
                continue
            nv = remap_value(v)
            is_template = oid in summary.templates
            keys: List[Any]
            if isinstance(k, tuple) and len(k) == 2 and k[0] == "@":
                kv = kv_of(k[1])
                lits = kv_literals(kv)
                keys = sorted(lits, key=str) if lits else ["*"]
            else:
                keys = [k]
            for a in tg:
                strong = is_template or (len(tg) == 1 and self.strongable(a) and targets_count.get((a, ns), 1) == 1 and len(keys) == 1)
                for kk in keys:
                    state.set_slot(a, ns, kk, nv, strong)
        # 4. elems
        for oid, kv in summary.elems_fp:
            for a in resolve(oid):
                nk = remap_value(Value(frozenset(), kv)).kv
                if a in clones.values() or oid in summary.templates:
                    state.elems[a] = nk
                else:
                    from .domain import kv_join
                    cur = state.elems.get(a)
                    state.elems[a] = nk if cur is None else kv_join(cur, nk)
        # 5. requirements on concrete sources created in the callee
        for src, d in summary.exit.reqs.items():
            for k, info in d.items():
                state.require(src, k, next(iter(info.sites)) if info.sites else f"L{getattr(node, 'lineno', 0)}",
                              info.alternatives, info.caveats, info.alt_keys)
                if not info.always:
                    tgt = state.reqs.writable(src)
                    cur = tgt[k]
                    from .domain import ReqInfo
                    tgt[k] = ReqInfo(False, cur.sites, cur.alternatives, cur.caveats, cur.alt_keys)
        # 6. symbolic requirements → verdicts / requirements now
        failed: List[Tuple[int, str, Any]] = []
        for sr in summary.symreqs:
            tg = resolve(sr.oid)
            if not tg:
                continue          # the placeholder stands for no record here (e.g. None): nothing to access
            kv = sr.key_kv
            if isinstance(kv, Param):
                kv = kv_of(kv.ph)
            elif isinstance(kv, KeysOf):
                kv = remap_value(Value(frozenset(), kv)).kv
            if sr.cls is not None:
                tg = frozenset(a for a in tg if self._may_be_instance(a, sr.cls))
            # only record-like targets have key rows (a list, a function... indexed through a
            # placeholder is not a missing-key question)
            tg = frozenset(a for a in tg if self.objects[a].kind in RECORD_KINDS or self.objects[a].kind == "frame")
            if sr.ns == "attr" and isinstance(sr.key_kv, Lit):
                # attributes resolved by the class (methods, properties, class attributes)
                tg = frozenset(a for a in tg if not self._class_resolves(a, sr.key_kv.key))
            elif sr.ns == "item":
                tg = frozenset(a for a in tg if not self._indexes_without_getitem(a))
            if not tg:
                continue
            many = len(tg) > 1
            sr_chain = chain_extend(chain, sr.rel_chain)
            # one verdict per distinct row among the targets (the report combines them, §13);
            # targets with the same entry row get the same row — concretise once per entry row
            groups: Dict[Row, List[int]] = {}
            if not many:
                groups[concretise(sr.row, None)] = [next(iter(tg))]
            elif sr.row.is_entry and sr.row.refs == frozenset({rho(sr.oid, sr.ns)}):
                # the untouched entry row of the accessed object itself: its row is each
                # target's own row
                for a in sorted(tg):
                    r0 = erow_of(a, sr.ns)
                    if r0.symbolic and not in_summary:
                        r0 = concretise(r0, None)
                    groups.setdefault(r0, []).append(a)
            elif not mentions(sr.row, sr.oid):
                groups[concretise(sr.row, None)] = sorted(tg)
            elif len(tg) <= MAX_SYMREQ_TARGETS:
                for a in sorted(tg):
                    groups.setdefault(concretise(sr.row, (sr.oid, a)), []).append(a)
            else:
                # many targets: objects of one allocation site are reported together, against
                # the join of their entry rows
                by_site: Dict[Tuple[str, str], List[int]] = {}
                for a in sorted(tg):
                    ob = self.objects[a]
                    by_site.setdefault((ob.kind, ob.site), []).append(a)
                for _sk, members in by_site.items():
                    if len(members) == 1:
                        groups.setdefault(concretise(sr.row, (sr.oid, members[0])), []).append(members[0])
                    else:
                        rows_pair = (join_rows(erow_of(a, "item") for a in members), join_rows(erow_of(a, "attr") for a in members))
                        groups.setdefault(concretise(sr.row, (sr.oid, rows_pair)), []).extend(members)
            for row_c, members in groups.items():
                tiers = self.emit_access(sr.node, kv, row_c, members[0], sr.ns, sr.guarded, sr_chain, sr.file, state, sr.text, sr.cls)
                for k, tier in tiers:
                    if tier == MISSING:
                        for a in members:
                            failed.append((a, sr.ns, k))
        # an access that definitely fails ends the call: its fall-through narrowing never happened
        for a, ns, k in failed:
            if k in pre.row(a, ns).present:
                continue
            row = state.row(a, ns)
            if k in row.present:
                from dataclasses import replace as _replace
                state.set_row(a, ns, _replace(row, present=row.present - {k}, definite=row.definite - {k}, likely=row.likely - {k}))
        # 7. verdicts decided inside the body
        for v in summary.rel_verdicts:
            full = chain_extend(chain, v.chain)
            dk = (v.file, v.line, v.col, _key_str(v.key), v.tier, v.oid, full)
            if dk not in self._verdict_keys:
                self._verdict_keys.add(dk)
                from .interp import Verdict
                self.verdicts.append(Verdict(v.line, v.col, v.key, v.tier, v.obj, full, v.note, v.providers, v.ns, v.text, v.oid, v.file))
        for n in summary.notes:
            self.note(n)
        ret = remap_value(summary.ret)
        if not in_summary:
            fi = self.objects[summary.foid].payload
            self.summaries.setdefault((fi.qualname, call_ctx), (ret, {o: state.row(o, "item") for o in ret.pts if self.objects[o].kind in RECORD_KINDS or self.objects[o].kind == "frame"}))
        return ret

    def path_value(self, ph: PH, resolve, kv_of, pre: State, state: State) -> Value:
        """Actual value of an entry object's slot that a path placeholder stands for."""
        parents = resolve(ph.parent) if ph.parent is not None else frozenset()
        acc = Value()
        for p in parents:
            if ph.key == "*":
                # element of an iterable
                items, elem, pair, _ = self.iterate(Value.obj(p), ast.Name(id="x", ctx=ast.Load()), state)
                if pair:
                    acc = acc.join(elem[0]).join(elem[1])
                else:
                    acc = acc.join(elem)
                continue
            if isinstance(ph.key, tuple) and len(ph.key) == 2 and ph.key[0] == "@":
                kv = kv_of(ph.key[1])
                lits = kv_literals(kv)
                if lits:
                    for k in lits:
                        acc = acc.join(pre.slot(p, ph.ns, k))
                    acc = acc.join(pre.slot(p, ph.ns, "*"))
                else:
                    acc = acc.join(pre.all_slots(p, ph.ns))
                if acc.empty:
                    row = pre.row(p, ph.ns)
                    if row.open:
                        acc = acc.join(self._path_placeholder(p, ph.ns, "*", row, state))
                continue
            v = pre.slot(p, ph.ns, ph.key)
            star = pre.slot(p, ph.ns, "*")
            if star.pts:
                v = v.join(Value(star.pts, DATA))
            if v.empty:
                row = pre.row(p, ph.ns)
                if ph.ns == "cell":
                    v = Value()
                elif row.open or self.objects[p].kind == "ph":
                    v = self._path_placeholder(p, ph.ns, ph.key, row, state)
            po = self.objects[p]
            if ph.ns == "attr" and po.kind == "instance" and isinstance(po.payload, int) and isinstance(ph.key, str) \
                    and self.field_is_any(po.payload, ph.key):
                v = v.join(self.dynamic_value(f"{po.label}.{ph.key}", state))
            acc = acc.join(v)
        if ph.cls is not None and acc.pts:
            # the placeholder stands for an instance of its class (annotation / isinstance guard):
            # instances of unrelated classes are not what it denotes
            kept = frozenset(o for o in acc.pts if self._may_be_instance(o, ph.cls))
            if kept != acc.pts:
                acc = Value(kept, acc.kv)
        return acc

    # ---------------------------------------------------------------- access
    def _class_resolves(self, oid: int, name: str) -> bool:
        ob = self.objects[oid]
        if ob.kind != "instance" or not isinstance(ob.payload, int):
            return False
        return self.class_lookup(ob.payload, name) is not None

    def _indexes_without_getitem(self, oid: int) -> bool:
        """An instance of a plain class (no __getitem__, not a TypedDict): indexing it is not a
        key question."""
        from .interp import ClassInfo
        ob = self.objects[oid]
        if ob.kind != "instance" or not isinstance(ob.payload, int) or ob.payload not in self.objects:
            return False
        ci = self.objects[ob.payload].payload
        if not isinstance(ci, ClassInfo) or ci.typed_dict or ci.named_tuple or ci.open_base:
            return False
        return self.class_lookup(ob.payload, "__getitem__") is None

    def _may_be_instance(self, oid: int, cls: int) -> bool:
        ob = self.objects[oid]
        if ob.kind == "instance":
            return isinstance(ob.payload, int) and self._is_subclass(ob.payload, cls)
        return True       # placeholders / unknowns: cannot exclude

    def emit_access(self, node: Any, kv: Any, row: Row, oid: int, ns: str, guarded: bool,
                    chain: Tuple[int, ...], file: str, state: State, text: str = "",
                    guard_cls: Optional[int] = None) -> List[Tuple[Any, str]]:
        """Classify an access against a (concrete or symbolic) row and record its consequences.
        Returns the (key, tier) pairs decided."""
        out: List[Tuple[Any, str]] = []
        if row.symbolic:
            # still symbolic: we are instantiating inside another summary → defer again
            if self.sum_stack:
                self._defer(SymReq(node, kv, row, oid, ns, guarded, chain, "load", file, text, guard_cls))
            return out
        site = f"L{getattr(node, 'lineno', 0)}"
        saved_chain = self.call_chain
        self.call_chain = list(chain)
        try:
            if isinstance(kv, (Lit, Lits)):
                for k in sorted(kv_literals(kv), key=str):
                    cls = classify(row, k)
                    out.append((k, cls.tier))
                    if guarded:
                        continue
                    self.verdict(node, k, cls, oid, ns, file_override=file, text_override=text)
                    if cls.tier == INPUT:
                        srcs = [s for s, _ in cls.providers]
                        for s, terms in cls.providers:
                            for t in terms:
                                state.require(s, t, site, alternatives=frozenset(x for x in srcs if x != s),
                                              caveats=row.mutators, alt_keys=terms)
            elif isinstance(kv, KeysOf):
                pass
            else:
                if not guarded and row.closed and not row.is_map and row.present and self.objects[oid].kind in ("dict", "frame"):
                    self.verdict(node, f"<data {kv}>", Classification(DATAKEY, note="data-derived key into a closed record"), oid, ns, file_override=file, text_override=text)
        finally:
            self.call_chain = saved_chain
        return out


def _remap_payload(payload: Any, remap_value, resolve):
    if isinstance(payload, Value):
        return remap_value(payload)
    if isinstance(payload, tuple):
        out = []
        for x in payload:
            if isinstance(x, Value):
                out.append(remap_value(x))
            elif isinstance(x, int) and not isinstance(x, bool):
                r = resolve(x)
                out.append(next(iter(r)) if len(r) == 1 else x)
            elif isinstance(x, tuple) and all(isinstance(y, int) for y in x):
                out.append(tuple(next(iter(resolve(y))) if len(resolve(y)) == 1 else y for y in x))
            else:
                out.append(x)
        return tuple(out)
    return payload


def _is_generator(fn: ast.AST) -> bool:
    for n in ast.walk(fn):
        if isinstance(n, (ast.Yield, ast.YieldFrom)):
            return True
    return False


def _key_str(k: Any) -> str:
    from .domain import key_str
    return key_str(k)


_DEBUG = __import__("os").environ.get("ROWAN_DEBUG", "") not in ("", "0")
_inst_stats: Dict[int, list] = {}
_inst_total = [0]


def _debug_instantiate(an, summary, fn):
    import sys as _sys, time as _time
    t0 = _time.perf_counter()
    try:
        return fn()
    finally:
        dt = _time.perf_counter() - t0
        st = _inst_stats.setdefault(summary.foid, [0, 0.0, 0, 0, 0, 0])
        st[0] += 1
        st[1] += dt
        st[2] = max(st[2], len(summary.rows_fp))
        st[3] = max(st[3], len(summary.symreqs))
        st[4] = max(st[4], len(summary.templates))
        st[5] = max(st[5], sum(len(r.sources) + len(r.terms) + len(r.dterms) + len(r.lterms) + len(r.pstores) for _o, _n, r in summary.rows_fp))
        _inst_total[0] += 1
        if dt > 0.5:
            print(f"[rowan] slow instantiation {an.objects[summary.foid].payload.qualname} dt={dt:.1f}s rows={len(summary.rows_fp)} "
                  f"symreqs={len(summary.symreqs)} templates={len(summary.templates)} "
                  f"sum_stack={[an.objects[f].payload.qualname for f in an.sum_stack][-4:]}", file=_sys.stderr, flush=True)
        if _inst_total[0] % 1000 == 0:
            top = sorted(_inst_stats.items(), key=lambda kv: -kv[1][1])[:8]
            print(f"[rowan] instantiations={_inst_total[0]} summaries={len(an.summary_table)} objs={len(an.objects)} verdicts={len(an.verdicts)} "
                  f"sum_stack={[an.objects[f].payload.qualname for f in an.sum_stack][-3:]}", file=_sys.stderr)
            for f, (n, t, rows, sreq, tmpl, sz) in top:
                print(f"      {an.objects[f].payload.qualname:40s} n={n:5d} t={t:6.1f}s rows={rows:4d} symreqs={sreq:4d} templates={tmpl:3d} rowsize={sz}", file=_sys.stderr)
            _sys.stderr.flush()




_tab_stats: Dict[Tuple[int, str], int] = {}
_tab_total = [0]


def _debug_tabulate(an, summary, ev) -> None:
    import sys as _sys
    k = (summary.foid, ev.kind)
    _tab_stats[k] = _tab_stats.get(k, 0) + 1
    if _tab_stats[k] in (1, 50):
        ob = an.objects[ev.ph]
        lab = ob.label
        par = ob.payload.parent if ob.kind == "ph" else None
        print(f"[rowan] tabulate {an.objects[summary.foid].payload.qualname} on {ev.kind} ph={lab} parent={an.objects[par].label if par else None} "
              f"parent_cls={an.objects[par].payload.cls if par and an.objects[par].kind == 'ph' else None}", file=_sys.stderr)
    _tab_total[0] += 1
    if _tab_total[0] % 200 == 0:
        top = sorted(_tab_stats.items(), key=lambda kv: -kv[1])[:10]
        print(f"[rowan] tabulations={_tab_total[0]} sum_stack={[an.objects[f].payload.qualname for f in an.sum_stack][-4:]}", file=_sys.stderr)
        for (f, kind), n in top:
            print(f"      {an.objects[f].payload.qualname:45s} {kind:10s} n={n}", file=_sys.stderr)
        _sys.stderr.flush()
