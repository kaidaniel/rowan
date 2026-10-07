"""Abstract interpreter over the Python AST (SPECIFICATION.md §5–§10).

Phase-0 simplifications versus the spec:
  * no separate IR / SSA: the interpreter walks the AST directly with a
    flow-sensitive state (env + heap), joining at merge points;
  * summaries are *tabulated* per call site (each call analyses the callee
    body against the actual abstract arguments) instead of symbolic RowExprs;
    recursion is handled by iterating per (function, call site) to a fixpoint;
  * heap identity: allocation site × context, context = innermost call site,
    or the receiver object for methods (§6.1).
"""
from __future__ import annotations

import ast
import os
import sys
from dataclasses import dataclass, field, replace
from typing import Any, Callable, Dict, FrozenSet, List, Optional, Sequence, Tuple

from . import scopes as scopemod
from .domain import (BOT, Bot, DATA, chain_extend, INPUT, SYM, Param, ReqInfo, key_str, LIKELY, LIKELY_MISSING, MAP, MISSING, POSSIBLY, PROVEN, UNKNOWN,
                     DATAKEY, Classification, Data, KeysOf, Kill, Lit, Lits, RenameFn, RenameMap,
                     Row, Source, Value, classify, join_rows, join_values, kv_exclude, kv_join,
                     kv_literals, kv_map, kv_restrict)
from .state import CONTAINER_KINDS, RECORD_KINDS, Obj, State
from .summary import PH, SummaryMixin, rho

UNROLL_MAX = 16
LOOP_MAX_ITER = 40
REC_MAX_ITER = 8
PATH_DEPTH = 3
MAX_ACTIVE = 2          # nested activations of one function analysed separately before cutting
MAX_MUTATED = 24        # objects an unknown call is assumed to touch (closest first)
MAX_CLONES = 8          # clones of one allocation site before collapsing to a summary object
DEBUG = os.environ.get("ROWAN_DEBUG", "") not in ("", "0")
import time as _time
DEBUG_T0 = _time.perf_counter()


SAFE_METHODS = {"dict.get", "dict.keys", "dict.values", "dict.items", "dict.setdefault", "dict.copy", "list.append",
                "str.upper", "str.lower", "str.strip", "str.title", "str.format", "str.join"}
STR_METHOD_NAMES = {"strip", "lower", "upper", "title", "split", "replace", "startswith", "endswith", "format",
                    "join", "lstrip", "rstrip", "encode", "decode", "isdigit", "isalpha", "zfill", "casefold",
                    "capitalize", "partition", "rsplit", "splitlines", "find", "count", "ljust", "rjust", "center"}
SCALAR_METHOD_NAMES = STR_METHOD_NAMES | {"append", "extend", "sort", "index", "insert", "remove", "add",
                                          "isoformat", "strftime", "timestamp", "date", "time", "total_seconds",
                                          "to_bytes", "bit_length", "as_integer_ratio", "is_integer", "hex",
                                          "conjugate", "real", "imag", "tolist", "item", "copy", "clear", "reverse"}
NOEFFECT_METHOD_NAMES = {"info", "debug", "warning", "warn", "error", "critical", "exception", "log", "write",
                         "flush", "close", "print", "assert_called", "assert_called_with"}
DICT_METHOD_NAMES = {"get", "items", "keys", "values", "update", "pop", "setdefault", "copy", "popitem", "clear"}
ITEM_GUARDS = {"KeyError", "LookupError", "Exception", "BaseException", "IndexError", "*"}
ATTR_GUARDS = {"AttributeError", "Exception", "BaseException", "*"}

KNOWN_IDENTITY_DECORATORS = {
    "functools.lru_cache", "functools.cache", "functools.wraps", "contextlib.contextmanager",
    "abc.abstractmethod", "typing.overload", "typing.final", "builtins.staticmethod",
    "builtins.classmethod", "pytest.fixture", "asyncio.coroutine", "dataclasses.dataclass",
    "typing.no_type_check", "functools.total_ordering", "typing.runtime_checkable",
}


# ---------------------------------------------------------------------------
# records produced for the reporter
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class Verdict:
    line: int
    col: int
    key: Any
    tier: str
    obj: str
    chain: Tuple[int, ...]          # call sites (lines) active when the access ran
    note: str = ""
    providers: Tuple[str, ...] = ()
    ns: str = "item"
    text: str = ""                   # source text of the access
    oid: int = 0
    file: str = ""                   # module the access is in (empty = main file)


@dataclass
class FuncInfo:
    node: Any
    name: str
    qualname: str
    module: int
    scope: scopemod.Scope
    closure: Dict[str, int] = field(default_factory=dict)      # free name -> cell oid
    defaults: Dict[str, Value] = field(default_factory=dict)
    cls: Optional[int] = None                                    # defining class oid
    is_property: bool = False
    is_static: bool = False
    is_classmethod: bool = False


@dataclass
class ClassInfo:
    name: str
    node: Any
    bases: Tuple[int, ...]
    module: int
    open_base: bool = False          # inherits from something we do not see
    dataclass_fields: Optional[List[Tuple[str, Optional[ast.expr]]]] = None
    typed_dict: bool = False
    named_tuple: bool = False
    field_classes: Optional[Dict[str, FrozenSet[int]]] = None   # attr → classes assigned to self.attr (lazy)


@dataclass
class ModuleInfo:
    name: str
    path: Optional[str]
    tree: Any
    scopes: Dict[Any, scopemod.Scope]


@dataclass
class Frame:
    kind: str                       # module | function | class
    scope: scopemod.Scope
    module: int
    ctx: str
    single: bool
    func: Optional[Obj] = None
    cells: Dict[str, int] = field(default_factory=dict)
    class_oid: Optional[int] = None
    overlays: List[Dict[str, Value]] = field(default_factory=list)
    loop_depth: int = 0
    label: str = "<module>"
    symbolic: bool = False


@dataclass
class LoopCtx:
    breaks: Optional[State] = None
    continues: Optional[State] = None


@dataclass
class Call:
    """Everything a model needs about a call."""
    an: "Analyzer"
    qual: str
    recv: Optional[Value]
    args: List[Value]
    kwargs: Dict[str, Value]
    star_args: List[Value]
    star_kwargs: List[Value]
    state: State
    node: Any
    site: str
    line: int

    def arg(self, i: int, name: Optional[str] = None) -> Optional[Value]:
        if i < len(self.args):
            return self.args[i]
        if name is not None and name in self.kwargs:
            return self.kwargs[name]
        return None

    def kw(self, name: str) -> Optional[Value]:
        return self.kwargs.get(name)

    def arg_node(self, i: int, name: Optional[str] = None) -> Optional[ast.expr]:
        node = self.node
        if not isinstance(node, ast.Call):
            return None
        if i < len(node.args) and not isinstance(node.args[i], ast.Starred):
            return node.args[i]
        if name is not None:
            for kw in node.keywords:
                if kw.arg == name:
                    return kw.value
        return None


class _Unbound(Exception):
    pass


def _chain_text(node: ast.expr) -> Optional[str]:
    """`a.b.c` → "a.b.c" for pure name/attribute chains, else None."""
    parts = []
    while isinstance(node, ast.Attribute):
        parts.append(node.attr)
        node = node.value
    if not isinstance(node, ast.Name):
        return None
    parts.append(node.id)
    return ".".join(reversed(parts))


# ---------------------------------------------------------------------------
# the analyser
# ---------------------------------------------------------------------------

class Analyzer(SummaryMixin):
    def __init__(self, source: str, filename: str = "<module>", search_paths: Sequence[str] = (),
                 on_stmt: Optional[Callable[[ast.stmt, State, "Analyzer"], None]] = None,
                 analyse_entry_points: bool = True, follow_imports: bool = True,
                 use_summaries: Optional[bool] = None) -> None:
        from .models import Registry   # local import: models import this module's types
        self.filename = filename
        self.use_summaries = (os.environ.get("ROWAN_SUMMARIES", "1") != "0") if use_summaries is None else use_summaries
        self.summary_table: Dict[Any, Any] = {}
        self.tab_count: Dict[int, int] = {}
        self.module_done: bool = False
        self.fresh_init: set = set()      # instances whose constructor is running (§6.1: fresh objects)
        self.cell_classes: Dict[int, int] = {}   # closure cells known to hold instances of one class
        self.sum_stack: List[int] = []
        self.sum_reqs: List[dict] = []
        self.sum_events: List[dict] = []
        self.sum_hit: set = set()
        self.sum_partial: Dict[int, Any] = {}
        self.sum_obj_mark: List[int] = []
        self.model_call: int = 0
        self.search_paths = list(search_paths) if follow_imports else []
        self.analyse_entry_points = analyse_entry_points
        self.source_lines = source.splitlines()
        self.objects: Dict[int, Obj] = {}
        self.obj_index: Dict[Tuple[Any, ...], int] = {}
        self.verdicts: List[Verdict] = []
        self._verdict_keys: set = set()
        self.notes: List[str] = []
        self._notes_set: set = set()
        self.summaries: Dict[Tuple[str, str], Tuple[Value, Dict[int, Row]]] = {}
        self.models = Registry()
        self.frames: List[Frame] = []
        self.active: Dict[Tuple[int, str], bool] = {}
        self.active_by_func: Dict[int, List[Tuple[int, str]]] = {}
        self.rec_table: Dict[Tuple[int, str], Tuple[Value, State, Any]] = {}
        self.rec_hit: set = set()
        self.rec_entry: Dict[Tuple[int, str], Tuple[Optional[Value], List[Value], Dict[str, Value]]] = {}
        self.rec_widened: set = set()
        self.loop_stack: List[LoopCtx] = []
        self.guards: List[set] = []
        self.may_raise: List[bool] = []
        self.cond_depth: int = 0
        self.call_chain: List[int] = []
        self.on_stmt = on_stmt
        self.analysed_funcs: set = set()
        self.modules: Dict[str, int] = {}
        self.exh_log: Optional[list] = None
        self.final_state: Optional[State] = None
        self.user_classes: List[int] = []
        self._cls_slots: Dict[Tuple[int, str, str], Value] = {}   # class attributes (flow-insensitive)
        self.singletons: Dict[str, int] = {}
        self.site_clones: Dict[str, int] = {}
        self.call_cache: Dict[Any, Any] = {}
        self.entry_instances: Dict[int, int] = {}
        self._fresh_in_progress: set = set()
        tree = ast.parse(source, filename)
        self.tree = tree
        self.main_module = self._make_module("__main__", filename, tree)
        self.SERIES = self.alloc_global("series", "builtin:Series", label="Series")
        self.NONE = Value.lit(None)

    # ------------------------------------------------------------------ setup
    def _make_module(self, name: str, path: Optional[str], tree: ast.Module) -> int:
        oid = self.alloc_global("module", f"module:{name}", label=f"module {name}",
                                payload=ModuleInfo(name, path, tree, scopemod.analyse(tree)))
        self.modules[name] = oid
        return oid

    @property
    def frame(self) -> Frame:
        return self.frames[-1]

    # ------------------------------------------------------------ allocation
    def alloc_global(self, kind: str, site: str, label: str = "", payload: Any = None) -> int:
        return self._alloc(kind, site, "", True, label or site, payload)

    def _alloc(self, kind: str, site: str, ctx: str, single: bool, label: str, payload: Any = None) -> int:
        key = (kind, site, ctx)
        if key in self.obj_index:
            oid = self.obj_index[key]
            self.objects[oid].single = self.objects[oid].single and single
            if payload is not None:
                self.objects[oid].payload = payload
            return oid
        n = self.site_clones.get(site, 0)
        if n >= MAX_CLONES and ctx != "":
            # too many clones of one allocation site: collapse onto the context-free summary object
            return self._alloc(kind, site, "", False, label, payload)
        self.site_clones[site] = n + 1
        oid = len(self.objects) + 1
        self.objects[oid] = Obj(oid, kind, site, ctx, single, label, payload)
        self.obj_index[key] = oid
        return oid

    def site_of(self, node: ast.AST) -> str:
        return f"L{getattr(node, 'lineno', 0)}:{getattr(node, 'col_offset', 0)}-{getattr(node, 'end_col_offset', 0)}"

    def alloc(self, kind: str, node_or_site: Any, state: State, label: str = "", payload: Any = None,
              row: Optional[Row] = None, ns: str = "item", single: Optional[bool] = None,
              ctx: Optional[str] = None) -> int:
        site = node_or_site if isinstance(node_or_site, str) else self.site_of(node_or_site)
        fr = self.frame
        if ctx is None:
            ctx = fr.ctx
        if single is None:
            single = fr.single and fr.loop_depth == 0
        oid = self._alloc(kind, site, ctx, single, label or f"{kind}@{site}", payload)
        obj = self.objects[oid]
        if kind in RECORD_KINDS or kind == "frame":
            fresh = row if row is not None else Row()
            if (oid, ns) in state.rows and not obj.single:
                state.set_row(oid, ns, state.row(oid, ns).join(fresh))
            else:
                state.set_row(oid, ns, fresh)
                if obj.single:
                    # strong re-allocation: drop stale slots of this object
                    state.slots.drop_obj(oid)
        elif kind in CONTAINER_KINDS:
            if obj.single:
                state.slots.drop_obj(oid)
                state.elems.pop(oid, None)
        return oid

    def unknown_source(self, site: str, kind: str, state: State, label: str = "", ctx: str = "",
                       ns: str = "item", path: Tuple[Any, ...] = ()) -> int:
        src = Source(site, kind, path)
        oid = self._alloc("unknown", f"{kind}:{src}", ctx, True, label or str(src))
        for n in ("item", "attr"):
            if (oid, n) not in state.rows:
                state.set_row(oid, n, Row(sources=frozenset({(src, ())})))
        return oid

    # ------------------------------------------------------------ reporting
    def verdict(self, node: ast.AST, key: Any, cls: Classification, oid: int, ns: str = "item",
                file_override: Optional[str] = None, text_override: Optional[str] = None) -> None:
        if cls.tier in (MAP, SYM):
            return
        line = getattr(node, "lineno", 0)
        col = getattr(node, "col_offset", 0)
        text = text_override or ""
        if not text:
            try:
                text = ast.unparse(node)
            except Exception:
                pass
        if file_override is not None:
            fname = file_override
        else:
            mod = self.objects[self.frame.module].payload
            fname = "" if self.frame.module == self.main_module else os.path.basename(mod.path or mod.name)
        chain = tuple(self.call_chain)
        dk = (fname, line, col, key_str(key), cls.tier, oid, chain)
        if dk in self._verdict_keys:
            return
        self._verdict_keys.add(dk)
        self.verdicts.append(Verdict(line, col, key, cls.tier, self.objects[oid].label,
                                     chain, cls.note,
                                     tuple(f"{s}" for s, _ in cls.providers), ns, text, oid, fname))

    def note(self, msg: str) -> None:
        if msg not in self._notes_set:
            self._notes_set.add(msg)
            self.notes.append(msg)

    def access_site(self, node: ast.AST) -> str:
        return f"L{getattr(node, 'lineno', 0)}"

    # ------------------------------------------------------------- entry
    def run(self) -> State:
        state = State()
        self._run_module(self.main_module, state)
        self.module_done = True
        if self.analyse_entry_points:
            self._analyse_uncalled(state)
        self.final_state = state
        return state

    def _run_module(self, moid: int, state: State) -> None:
        info: ModuleInfo = self.objects[moid].payload
        fr = Frame("module", info.scopes[info.tree], moid, "", True, label=f"<module {info.name}>")
        self.frames.append(fr)
        try:
            self.exec_block(info.tree.body, state)
        finally:
            self.frames.pop()
        state.reachable = True
        state.exits = None

    def _analyse_uncalled(self, state: State) -> None:
        """Entry-point rule (§6.4): functions never called get analysed with open parameters."""
        for oid in sorted(list(self.objects)):
            obj = self.objects[oid]
            if obj.kind != "function" or oid in self.analysed_funcs:
                continue
            fi: FuncInfo = obj.payload
            if fi.closure:
                continue   # inner closures are reached through their factory
            if fi.module != self.main_module:
                continue   # imported modules: only what the main module reaches
            node = fi.node
            if isinstance(node, ast.Lambda):
                continue
            if DEBUG:
                import time as _t
                print(f"[rowan] entry {fi.qualname} L{node.lineno} t={_t.perf_counter()-DEBUG_T0:.1f}s "
                      f"objs={len(self.objects)} summaries={len(self.summary_table)} verdicts={len(self.verdicts)}",
                      file=sys.stderr, flush=True)
            st = state.copy()
            st.ret = None
            args: List[Value] = []
            self.frames.append(Frame("module", self.objects[fi.module].payload.scopes[self.objects[fi.module].payload.tree],
                                     fi.module, f"entry:{fi.qualname}", True, label="<entry>"))
            try:
                params = scopemod._all_args(node.args)
                self_val = None
                if fi.cls is not None and not fi.is_static:
                    # one representative instance per class, shared by all its method entry points
                    inst = self.entry_instances.get(fi.cls)
                    if inst is None:
                        inst = self._fresh_instance(fi.cls, state, f"entry:{self.objects[fi.cls].payload.name}")
                        self.entry_instances[fi.cls] = inst
                        st = state.copy()
                        st.ret = None
                    self_val = Value.obj(inst)
                    params = params[1:]
                for a in params:
                    if a is node.args.vararg or a is node.args.kwarg:
                        continue
                    if a.arg in fi.defaults:
                        args.append(fi.defaults[a.arg])
                        continue
                    v = self._param_from_annotation(a.annotation, fi, st) if a.annotation is not None else None
                    if v is None:
                        poid = self.unknown_source(f"param {fi.qualname}.{a.arg}", "param", st,
                                                   label=f"param {a.arg}")
                        v = Value.obj(poid)
                    args.append(v)
                fake = ast.Call(func=ast.Name(id=fi.name, ctx=ast.Load()), args=[], keywords=[])
                fake.lineno = node.lineno
                fake.col_offset = 0
                self.call_user(oid, self_val, args, {}, [], [], st, fake, entry=True)
            finally:
                self.frames.pop()
            # keep schema requirements discovered in entry points
            for src, d in st.reqs.items():
                tgt = state.reqs.writable(src)
                for k, info in d.items():
                    tgt[k] = tgt[k].join(info) if k in tgt else info

    def _param_from_annotation(self, ann: ast.expr, fi: FuncInfo, state: State) -> Optional[Value]:
        """A declared row for an annotated entry-point parameter (§8.5)."""
        node = ann
        while isinstance(node, ast.Subscript):          # Optional[X], list[X] → X (approximation)
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
        try:
            v = self.eval(node, state)
        except Exception:
            return None
        for oid in v.pts:
            o = self.objects[oid]
            if o.kind == "class":
                ci: ClassInfo = o.payload
                if ci.dataclass_fields is not None:
                    fields = frozenset(f for f, _ in ci.dataclass_fields)
                    ns = "item" if ci.typed_dict else "attr"
                    inst = self.alloc("instance", f"param:{fi.qualname}:{ci.name}", state, label=f"{ci.name} param",
                                      payload=oid, ns=ns, row=Row(present=fields, definite=fields, likely=fields),
                                      ctx=f"entry:{fi.qualname}", single=True)
                    for f in fields:
                        state.set_slot(inst, ns, f, Value.data(f"field {f}"), True)
                    return Value.obj(inst)
                return Value.obj(self._fresh_instance(oid, state, f"entry:{fi.qualname}"))
            if o.kind == "external" and str(o.payload).endswith("DataFrame"):
                fid = self.alloc("frame", f"param:{fi.qualname}", state, label=f"frame param",
                                 row=Row(sources=frozenset({(Source(f"param {fi.qualname}", "param"), ())})),
                                 ctx=f"entry:{fi.qualname}", single=True)
                return Value.obj(fid)
        return None

    def _fresh_instance(self, cls_oid: int, state: State, ctx: str) -> int:
        ci: ClassInfo = self.objects[cls_oid].payload
        if cls_oid in self._fresh_in_progress:
            # self-referential annotation (a Node whose __init__ takes a Node): open instance
            return self.unknown_source(f"instance {ci.name}", "class", state, label=f"{ci.name} (recursive)", ns="attr")
        self._fresh_in_progress.add(cls_oid)
        try:
            return self._fresh_instance_inner(cls_oid, ci, state, ctx)
        finally:
            self._fresh_in_progress.discard(cls_oid)

    def _fresh_instance_inner(self, cls_oid: int, ci: ClassInfo, state: State, ctx: str) -> int:
        has_init = self.class_lookup(cls_oid, "__init__") is not None
        has_fields = any(isinstance(self.objects[c].payload, ClassInfo) and self.objects[c].payload.dataclass_fields is not None
                         for c in self.mro(cls_oid))
        # a class that never initialises its own attributes (a mixin, a protocol-like base) is
        # only ever used through subclasses: its entry instance is open
        open_inst = ci.open_base or not (has_init or has_fields)
        inst = self.alloc("instance", f"instance:{ci.name}:{ctx}", state, label=f"{ci.name}()", payload=cls_oid,
                          ns="attr", ctx=ctx, single=True,
                          row=Row(sources=frozenset({(Source(f"instance {ci.name}", "class"), ())})) if open_inst else Row())
        # dataclass / TypedDict / NamedTuple fields (own and inherited) are present on a fresh instance
        seen = set()
        order = [cls_oid]
        while order:
            c = order.pop(0)
            if c in seen:
                continue
            seen.add(c)
            cinfo: ClassInfo = self.objects[c].payload
            if cinfo.dataclass_fields is not None:
                ns = "item" if cinfo.typed_dict else "attr"
                for f, _ in cinfo.dataclass_fields:
                    state.set_row(inst, ns, state.row(inst, ns).store(f, True))
                    state.set_slot(inst, ns, f, Value.data(f"field {f}"), True)
            order.extend(cinfo.bases)
        init_cls = self.class_lookup(cls_oid, "__init__")
        if init_cls is not None:
            for init in self._cls_slots.get((init_cls, "attr", "__init__"), Value()).pts:
                if self.objects[init].kind != "function":
                    continue
                fi: FuncInfo = self.objects[init].payload
                params = scopemod._all_args(fi.node.args)[1:]
                args = []
                for a in params:
                    if a is fi.node.args.vararg or a is fi.node.args.kwarg:
                        continue
                    if a.arg in fi.defaults:
                        args.append(fi.defaults[a.arg])
                        continue
                    v = self._param_from_annotation(a.annotation, fi, state) if a.annotation is not None else None
                    args.append(v if v is not None else Value.obj(
                        self.unknown_source(f"param {fi.qualname}.{a.arg}", "param", state, label=f"param {a.arg}")))
                fake = ast.Call(func=ast.Name(id="__init__", ctx=ast.Load()), args=[], keywords=[])
                fake.lineno = fi.node.lineno
                fake.col_offset = 0
                self.call_user(init, Value.obj(inst), args, {}, [], [], state, fake, entry=True)
        return inst

    # ---------------------------------------------------------- statements
    def exec_block(self, stmts: Sequence[ast.stmt], state: State) -> None:
        for s in stmts:
            if not state.reachable:
                return
            self.exec_stmt(s, state)
            if self.on_stmt is not None:
                self.on_stmt(s, state, self)

    def exec_stmt(self, node: ast.stmt, state: State) -> None:
        m = getattr(self, "stmt_" + type(node).__name__, None)
        if m is None:
            self.note(f"unsupported statement {type(node).__name__} at L{node.lineno}")
            return
        m(node, state)

    def stmt_Expr(self, node: ast.Expr, state: State) -> None:
        self.eval(node.value, state)

    def stmt_Pass(self, node, state) -> None:
        pass

    stmt_Global = stmt_Pass
    stmt_Nonlocal = stmt_Pass
    stmt_TypeAlias = stmt_Pass

    def stmt_Assign(self, node: ast.Assign, state: State) -> None:
        v = self.eval(node.value, state)
        for t in node.targets:
            self.bind_target(t, v, state)

    def stmt_AnnAssign(self, node: ast.AnnAssign, state: State) -> None:
        if node.value is not None:
            v = self.eval(node.value, state)
            self.bind_target(node.target, v, state)

    def stmt_AugAssign(self, node: ast.AugAssign, state: State) -> None:
        cur = self.eval(node.target, state)
        rhs = self.eval(node.value, state)
        res = self.binop(node.op, cur, rhs, node, state)
        self.bind_target(node.target, res, state)

    def stmt_Return(self, node: ast.Return, state: State) -> None:
        v = self.eval(node.value, state) if node.value is not None else self.NONE
        state.ret = v if state.ret is None else state.ret.join(v)
        state.record_exit()
        state.reachable = False

    def stmt_Raise(self, node: ast.Raise, state: State) -> None:
        if node.exc is not None:
            self.eval(node.exc, state)
        state.reachable = False

    def stmt_Assert(self, node: ast.Assert, state: State) -> None:
        self.eval(node.test, state)
        self.narrow(node.test, True, state)

    def stmt_Delete(self, node: ast.Delete, state: State) -> None:
        for t in node.targets:
            if isinstance(t, ast.Subscript):
                obj = self.eval(t.value, state)
                key = self.eval_key(t.slice, state)
                self.delete_item(obj, "item", key, t, state)
            elif isinstance(t, ast.Attribute):
                obj = self.eval(t.value, state)
                self.delete_item(obj, "attr", Value.lit(t.attr), t, state)
            elif isinstance(t, ast.Name):
                state.env.pop(t.id, None)

    def stmt_Import(self, node: ast.Import, state: State) -> None:
        for a in node.names:
            top = a.name.split(".")[0]
            if a.asname:
                self.store_name(a.asname, self.import_module(a.name, node, state), state)
            else:
                self.store_name(top, self.import_module(top, node, state), state)

    def stmt_ImportFrom(self, node: ast.ImportFrom, state: State) -> None:
        modname = node.module or ""
        mod = self.import_module(modname, node, state) if modname else Value()
        for a in node.names:
            if a.name == "*":
                continue
            v = self.load_attr(mod, a.name, node, state, report=False) if mod.pts else Value()
            self.store_name(a.asname or a.name, v, state)

    def import_module(self, name: str, node: ast.AST, state: State) -> Value:
        if name in self.modules:
            return Value.obj(self.modules[name])
        # local module file?
        for base in self.search_paths:
            p = os.path.join(base, *name.split(".")) + ".py"
            if os.path.exists(p):
                real = os.path.realpath(p)
                for moid in self.modules.values():
                    mi: ModuleInfo = self.objects[moid].payload
                    if mi.path and os.path.realpath(mi.path) == real:
                        self.modules[name] = moid       # same file under another name (e.g. the main module)
                        return Value.obj(moid)
                with open(p, encoding="utf-8") as f:
                    src = f.read()
                tree = ast.parse(src, p)
                moid = self._make_module(name, p, tree)
                self._run_module(moid, state)
                return Value.obj(moid)
        oid = self._alloc("external", f"ext:{name}", "", True, name, payload=name)
        return Value.obj(oid)

    def stmt_If(self, node: ast.If, state: State) -> None:
        if _is_main_guard(node.test):
            self.exec_block(node.body, state)       # the script's own run: unconditional
            return
        self.eval(node.test, state)
        then_s = state.copy()
        else_s = state.copy()
        self.narrow(node.test, True, then_s)
        self.narrow(node.test, False, else_s)
        self.cond_depth += 1
        if then_s.reachable:
            self.exec_block(node.body, then_s)
        if else_s.reachable:
            self.exec_block(node.orelse, else_s)
        self.cond_depth -= 1
        state.set_to(then_s.join(else_s))

    def stmt_While(self, node: ast.While, state: State) -> None:
        pre = state.copy()
        head = state.copy()
        exit_acc: Optional[State] = None
        self.frame.loop_depth += 1
        self.cond_depth += 1
        for _ in range(LOOP_MAX_ITER):
            body_s = head.copy()
            self.eval(node.test, body_s)
            self.narrow(node.test, True, body_s)
            ctx = LoopCtx()
            self.loop_stack.append(ctx)
            if body_s.reachable:
                self.exec_block(node.body, body_s)
            self.loop_stack.pop()
            if ctx.breaks is not None:
                exit_acc = ctx.breaks if exit_acc is None else exit_acc.join(ctx.breaks)
            back = body_s if ctx.continues is None else body_s.join(ctx.continues)
            new_head = pre.join(back)
            if new_head.same(head):
                break
            head = new_head
        self.cond_depth -= 1
        self.frame.loop_depth -= 1
        exit_s = head.copy()
        self.narrow(node.test, False, exit_s)
        if node.orelse and exit_s.reachable:
            self.exec_block(node.orelse, exit_s)
        if exit_acc is not None:
            exit_s = exit_s.join(exit_acc)
        state.set_to(exit_s)

    def stmt_For(self, node: ast.For, state: State) -> None:
        itv = self.eval(node.iter, state)
        items, elem, pair, exhaustive = self.iterate(itv, node.iter, state)
        has_jump = _contains_jump(node.body)
        if items is not None and len(items) <= UNROLL_MAX and not has_jump:
            for it in items:
                if not state.reachable:
                    break
                self.bind_iter_target(node.target, it, state, pair=False)
                self.exec_block(node.body, state)
            if node.orelse and state.reachable:
                self.exec_block(node.orelse, state)
            return
        pre = state.copy()
        # dry run for exhaustive key iteration (§8.3)
        exh_ops: list = []
        key_part = elem[0] if pair else elem
        if exhaustive and not has_jump and isinstance(key_part.kv, KeysOf):
            saved_log = self.exh_log
            self.exh_log = exh_ops
            saved_verdicts = len(self.verdicts)
            dry = state.copy()
            self.frame.loop_depth += 1
            self.cond_depth += 1
            kk = key_part.kv
            ekv = KeysOf(kk.objs, kk.ns, kk.exclude, kk.chain, self.cond_depth)
            dry_elem = (Value(key_part.pts, ekv), elem[1]) if pair else Value(key_part.pts, ekv)
            self.bind_iter_target(node.target, dry_elem, dry, pair=pair)
            self.exec_block(node.body, dry)
            self.cond_depth -= 1
            self.frame.loop_depth -= 1
            for v in self.verdicts[saved_verdicts:]:
                self._verdict_keys.discard((v.file, v.line, v.col, key_str(v.key), v.tier, v.oid, v.chain))
            del self.verdicts[saved_verdicts:]
            self.exh_log = saved_log
        head = state.copy()
        exit_acc: Optional[State] = None
        self.frame.loop_depth += 1
        self.cond_depth += 1
        for _ in range(LOOP_MAX_ITER):
            body_s = head.copy()
            ctx = LoopCtx()
            self.loop_stack.append(ctx)
            self.bind_iter_target(node.target, elem, body_s, pair=pair)
            self.exec_block(node.body, body_s)
            self.loop_stack.pop()
            if ctx.breaks is not None:
                exit_acc = ctx.breaks if exit_acc is None else exit_acc.join(ctx.breaks)
            back = body_s if ctx.continues is None else body_s.join(ctx.continues)
            new_head = pre.join(back)
            if new_head.same(head):
                break
            head = new_head
        self.cond_depth -= 1
        self.frame.loop_depth -= 1
        exit_s = head.copy()
        self.promote_elem_reqs(exit_s, back, elem if not pair else elem[1], state)
        self.promote_elem_narrowing(exit_s, back, elem if not pair else elem[1])
        for (oid, ns, srcrow, kill, chain) in exh_ops:
            exit_s.set_row(oid, ns, exit_s.row(oid, ns).copy_from(srcrow, strong=True, kill=kill, chain=chain))
        if node.orelse and exit_s.reachable:
            self.exec_block(node.orelse, exit_s)
        if exit_acc is not None:
            exit_s = exit_s.join(exit_acc)
        state.set_to(exit_s)

    stmt_AsyncFor = stmt_For

    def stmt_Break(self, node, state: State) -> None:
        if self.loop_stack:
            ctx = self.loop_stack[-1]
            ctx.breaks = state.copy() if ctx.breaks is None else ctx.breaks.join(state)
        state.reachable = False

    def stmt_Continue(self, node, state: State) -> None:
        if self.loop_stack:
            ctx = self.loop_stack[-1]
            ctx.continues = state.copy() if ctx.continues is None else ctx.continues.join(state)
        state.reachable = False

    def stmt_Try(self, node: ast.Try, state: State) -> None:
        caught: set = set()
        for h in node.handlers:
            if h.type is None:
                caught.add("*")
            else:
                for n in (h.type.elts if isinstance(h.type, ast.Tuple) else [h.type]):
                    caught.add(_dotted_last(n))
        pre = state.copy()
        self.guards.append(caught)
        self.may_raise.append(False)
        self.cond_depth += 1
        self.exec_block(node.body, state)
        self.guards.pop()
        body_may_raise = self.may_raise.pop() or not state.reachable
        body_end = state.copy()
        handler_entry = pre.join(body_end) if node.handlers else None
        if not body_may_raise and node.handlers:
            handler_entry = State.bottom()   # nothing in the body can raise: handlers are dead
        results: List[State] = []
        if node.orelse and body_end.reachable:
            self.exec_block(node.orelse, body_end)
        results.append(body_end)
        for h in node.handlers:
            hs = handler_entry.copy()
            if not hs.reachable:
                continue
            if h.name:
                self.store_name(h.name, Value.data(f"exc@L{h.lineno}"), hs)
            self.exec_block(h.body, hs)
            results.append(hs)
        self.cond_depth -= 1
        out = results[0]
        for r in results[1:]:
            out = out.join(r)
        if node.finalbody:
            if not out.reachable:
                out = pre.copy()
            self.exec_block(node.finalbody, out)
        state.set_to(out)

    stmt_TryStar = stmt_Try

    def stmt_With(self, node: ast.With, state: State) -> None:
        for item in node.items:
            v = self.eval(item.context_expr, state)
            entered = self.call_method_if_defined(v, "__enter__", item.context_expr, state)
            if entered is None:
                entered = v if any(self.objects[o].kind in RECORD_KINDS for o in v.pts) else Value.data(f"with@L{node.lineno}")
            if item.optional_vars is not None:
                self.bind_target(item.optional_vars, entered, state)
        self.exec_block(node.body, state)

    stmt_AsyncWith = stmt_With

    def stmt_FunctionDef(self, node: ast.FunctionDef, state: State) -> None:
        v = self.make_function(node, state)
        v = self.apply_decorators(node, v, state)
        self.store_name(node.name, v, state)

    stmt_AsyncFunctionDef = stmt_FunctionDef

    def stmt_ClassDef(self, node: ast.ClassDef, state: State) -> None:
        fr = self.frame
        bases: List[int] = []
        open_base = False
        typed_dict = named_tuple = False
        for b in node.bases:
            bv = self.eval(b, state)
            for oid in bv.pts:
                o = self.objects[oid]
                if o.kind == "class":
                    bases.append(oid)
                elif o.kind == "external" and str(o.payload).endswith(("TypedDict",)):
                    typed_dict = True
                elif o.kind == "external" and str(o.payload).endswith(("NamedTuple",)):
                    named_tuple = True
                elif o.kind == "external" and str(o.payload) in ("builtins.object", "object", "abc.ABC", "typing.Protocol", "typing.Generic"):
                    pass
                else:
                    open_base = True
            if not bv.pts:
                open_base = True
        info = ClassInfo(node.name, node, tuple(bases), fr.module, open_base, typed_dict=typed_dict, named_tuple=named_tuple)
        coid = self.alloc("class", node, state, label=f"class {node.name}", payload=info, ns="attr")
        self.user_classes.append(coid)
        is_dataclass = False
        for d in node.decorator_list:
            q = self.decorator_qualname(d, state)
            if q and (q.endswith("dataclass") or q.endswith("attrs.define") or q.endswith("attr.s")):
                is_dataclass = True
        if is_dataclass or typed_dict or named_tuple:
            fields: List[Tuple[str, Optional[ast.expr]]] = []
            for s in node.body:
                if isinstance(s, ast.AnnAssign) and isinstance(s.target, ast.Name):
                    fields.append((s.target.id, s.value))
            info.dataclass_fields = fields
        # class body
        mscope = self.objects[fr.module].payload.scopes
        cscope = mscope.get(node) or scopemod.Scope(node, "class", None)
        cf = Frame("class", cscope, fr.module, fr.ctx, fr.single, class_oid=coid, cells=dict(fr.cells),
                   label=f"class {node.name}")
        cf.overlays = []
        self.frames.append(cf)
        try:
            for s in node.body:
                if isinstance(s, ast.AnnAssign) and s.value is None:
                    continue
                self.exec_stmt(s, state)
        finally:
            self.frames.pop()
        self.store_name(node.name, Value.obj(coid), state)

    def stmt_Match(self, node: ast.Match, state: State) -> None:
        self.eval(node.subject, state)
        outs: List[State] = []
        self.cond_depth += 1
        for case in node.cases:
            cs = state.copy()
            self.exec_block(case.body, cs)
            outs.append(cs)
        self.cond_depth -= 1
        outs.append(state.copy())
        out = outs[0]
        for o in outs[1:]:
            out = out.join(o)
        state.set_to(out)

    # ----------------------------------------------------------- functions
    def make_function(self, node: Any, state: State, name: Optional[str] = None) -> Value:
        fr = self.frame
        mod_info: ModuleInfo = self.objects[fr.module].payload
        scope = mod_info.scopes.get(node)
        if scope is None:
            scope = scopemod.Scope(node, "function", None)
        fname = name or getattr(node, "name", "<lambda>")
        qual = fname if fr.kind == "module" else f"{fr.label}.{fname}"
        closure = {n: fr.cells[n] for n in scope.free if n in fr.cells}
        defaults: Dict[str, Value] = {}
        args = node.args
        pos = list(args.posonlyargs) + list(args.args)
        for a, d in zip(pos[len(pos) - len(args.defaults):], args.defaults):
            defaults[a.arg] = self.eval(d, state)
        for a, d in zip(args.kwonlyargs, args.kw_defaults):
            if d is not None:
                defaults[a.arg] = self.eval(d, state)
        info = FuncInfo(node, fname, qual, fr.module, scope, closure, defaults,
                        cls=fr.class_oid if fr.kind == "class" else None)
        oid = self.alloc("function", node, state, label=f"def {qual}", payload=info)
        self.objects[oid].payload = info
        return Value.obj(oid)

    def decorator_qualname(self, d: ast.expr, state: State) -> Optional[str]:
        target = d.func if isinstance(d, ast.Call) else d
        try:
            v = self.eval(target, state)
        except Exception:
            return None
        for oid in v.pts:
            o = self.objects[oid]
            if o.kind == "external":
                return str(o.payload)
            if o.kind == "function":
                return "user:" + o.payload.qualname
            if o.kind == "class":
                return "class:" + o.payload.name
        return None

    def apply_decorators(self, node: Any, fv: Value, state: State) -> Value:
        for d in reversed(node.decorator_list):
            q = self.decorator_qualname(d, state) or ""
            fobj = self.objects[next(iter(fv.pts))] if fv.pts else None
            if q.endswith("property") or q == "builtins.property":
                if fobj is not None and fobj.kind == "function":
                    fobj.payload.is_property = True
                continue
            if q.endswith("staticmethod"):
                if fobj is not None and fobj.kind == "function":
                    fobj.payload.is_static = True
                continue
            if q.endswith("classmethod"):
                if fobj is not None and fobj.kind == "function":
                    fobj.payload.is_classmethod = True
                continue
            if q in KNOWN_IDENTITY_DECORATORS or q.endswith((".setter", ".getter", ".deleter", ".register")):
                continue
            if q.startswith("user:"):
                dv = self.eval(d.func if isinstance(d, ast.Call) else d, state)
                if isinstance(d, ast.Call):
                    dv = self.call_value(dv, d, state)   # decorator factory
                fv = self.call_value(dv, node, state, override_args=[fv])
                continue
            self.note(f"unknown decorator {q or ast.unparse(d)} at L{node.lineno}: treated as identity")
        return fv

    def call_method_if_defined(self, v: Value, name: str, node: ast.AST, state: State) -> Optional[Value]:
        outs = []
        for oid in v.pts:
            o = self.objects[oid]
            if o.kind == "instance":
                m = self.class_lookup(o.payload, name)
                if m is not None and self.objects[m].kind == "function":
                    outs.append(self.call_user(m, Value.obj(oid), [], {}, [], [], state, node))
        return join_values(outs) if outs else None

    # ------------------------------------------------------------- names
    def load_name(self, name: str, node: ast.AST, state: State) -> Value:
        fr = self.frame
        for ov in reversed(fr.overlays):
            if name in ov:
                return ov[name]
        if fr.kind == "function":
            if name in fr.cells:
                v = state.slot(fr.cells[name], "cell", "v")
                if v.empty and self.sum_stack and self.is_entry_object(fr.cells[name]):
                    v = self._path_placeholder(fr.cells[name], "cell", "v", Row(), state)
                return v
            if fr.scope.is_local(name):
                if name in state.env:
                    return state.env[name]
                return Value.data(f"unbound {name}")
        if fr.kind == "class":
            v = state.slots.get((fr.class_oid, "attr", name))
            if v is not None:
                return v
            if name in fr.cells:
                return state.slot(fr.cells[name], "cell", "v")
        v = state.slots.get((fr.module, "attr", name))
        if v is not None:
            return v
        if name in BUILTIN_NAMES:
            return Value.obj(self._alloc("external", f"ext:builtins.{name}", "", True, name, payload=f"builtins.{name}"))
        oid = self.unknown_source(f"name {name}", "name", state, label=f"?{name}")
        return Value.obj(oid)

    def store_name(self, name: str, v: Value, state: State) -> None:
        if state.narrowed:
            for k in [k for k in state.narrowed if k == name or k.startswith(name + ".")]:
                del state.narrowed[k]
        fr = self.frame
        for ov in reversed(fr.overlays):
            if name in ov:
                ov[name] = v
                return
        if fr.kind == "function":
            if name in fr.cells:
                state.set_slot(fr.cells[name], "cell", "v", v, strong=self.objects[fr.cells[name]].single)
                return
            if name in fr.scope.globals_decl:
                state.set_slot(fr.module, "attr", name, v, strong=True)
                return
            state.env[name] = v
            return
        if fr.kind == "class":
            state.set_slot(fr.class_oid, "attr", name, v, strong=True)
            state.set_row(fr.class_oid, "attr", state.row(fr.class_oid, "attr").store(name, True))
            key = (fr.class_oid, "attr", name)
            self._cls_slots[key] = self._cls_slots[key].join(v) if key in self._cls_slots else v
            return
        strong = fr.loop_depth == 0
        state.set_slot(fr.module, "attr", name, v, strong=strong)
        state.set_row(fr.module, "attr", state.row(fr.module, "attr").store(name, strong))

    # ------------------------------------------------------------ targets
    def bind_target(self, t: ast.expr, v: Value, state: State) -> None:
        if isinstance(t, ast.Name):
            self.store_name(t.id, v, state)
        elif isinstance(t, ast.Attribute):
            obj = self.eval(t.value, state)
            self.store_attr(obj, t.attr, v, t, state)
        elif isinstance(t, ast.Subscript):
            obj = self.eval(t.value, state)
            key = self.eval_key(t.slice, state)
            self.store_item(obj, "item", key, v, t, state)
        elif isinstance(t, (ast.Tuple, ast.List)):
            parts = self.unpack(v, len(t.elts), state, t)
            for sub, pv in zip(t.elts, parts):
                if isinstance(sub, ast.Starred):
                    self.bind_target(sub.value, pv, state)
                else:
                    self.bind_target(sub, pv, state)
        elif isinstance(t, ast.Starred):
            self.bind_target(t.value, v, state)

    def unpack(self, v: Value, n: int, state: State, node: ast.AST) -> List[Value]:
        parts = [Value() for _ in range(n)]
        any_pos = False
        for oid in v.pts:
            o = self.objects[oid]
            if o.kind in ("tuple", "list") and o.literal_len is not None:
                any_pos = True
                for i in range(n):
                    parts[i] = parts[i].join(state.slot(oid, "pos", i))
            elif o.kind == "pair":
                any_pos = True
                k, val = o.payload
                if n >= 1:
                    parts[0] = parts[0].join(k)
                if n >= 2:
                    parts[1] = parts[1].join(val)
            else:
                e = self.elem_of(Value.obj(oid), node, state)
                for i in range(n):
                    parts[i] = parts[i].join(e)
        if not v.pts:
            parts = [Value.data(f"unpack@L{getattr(node, 'lineno', 0)}") for _ in range(n)]
        return parts

    def bind_iter_target(self, target: ast.expr, elem: Any, state: State, pair: bool) -> None:
        if pair:
            k, val = elem
            if isinstance(target, (ast.Tuple, ast.List)) and len(target.elts) == 2:
                self.bind_target(target.elts[0], k, state)
                self.bind_target(target.elts[1], val, state)
            else:
                pid = self._alloc("pair", f"pair:{id(target)}", "", True, "pair", payload=(k, val))
                self.bind_target(target, Value.obj(pid), state)
        else:
            self.bind_target(target, elem, state)

    # ------------------------------------------------------------ iteration
    def iterate(self, v: Value, node: ast.AST, state: State):
        """Returns (items | None, elem, pair, exhaustive).

        items: list of Values to unroll; elem: abstract element (Value, or a
        (key Value, value Value) pair when pair=True); exhaustive: iteration
        covers every key of the KeysOf in elem.
        """
        items: Optional[List[Any]] = []
        elems: List[Value] = []
        pairs: List[Tuple[Value, Value]] = []
        pair = False
        exhaustive = True
        for oid in v.pts:
            o = self.objects[oid]
            if o.kind in ("list", "tuple", "set"):
                if o.literal_len is not None and items is not None:
                    for i in range(o.literal_len):
                        items.append(state.slot(oid, "pos", i))
                else:
                    items = None
                elems.append(self.elem_of(Value.obj(oid), node, state))
            elif o.kind in ("dict", "frame"):
                lit = self._literal_dict_items(oid, state)
                if lit is not None and items is not None:
                    items.extend(Value.lit(k) for k, _ in lit)
                else:
                    items = None
                elems.append(Value(frozenset(), KeysOf(frozenset({oid}), "item")))
            elif o.kind == "view":
                kind, recv = o.payload
                if kind in ("rows", "elems"):
                    items = None
                    exhaustive = False
                    if kind == "rows":
                        pair = True
                        pairs.append((Value.data("index"), recv))
                    else:
                        elems.append(recv)
                    continue
                kv = KeysOf(frozenset(recv.pts), "item")
                vals = join_values(self.all_values(r, "item", state) for r in recv.pts)
                lit = self._literal_dict_items(next(iter(recv.pts)), state) if len(recv.pts) == 1 else None
                if lit is not None and items is not None:
                    for k, val in lit:
                        if kind == "keys":
                            items.append(Value.lit(k))
                        elif kind == "values":
                            items.append(val)
                        else:
                            pid = self._alloc("pair", f"pair:{oid}:{k!r}", "", True, "pair", payload=(Value.lit(k), val))
                            self.objects[pid].payload = (Value.lit(k), val)
                            items.append(Value.obj(pid))
                else:
                    items = None
                if kind == "keys":
                    elems.append(Value(frozenset(), kv))
                elif kind == "values":
                    elems.append(vals)
                elif kind == "items":
                    pair = True
                    pairs.append((Value(frozenset(), kv), vals))
            elif o.kind == "zip":
                ks, vs = o.payload
                kitems, kelem, _, _ = self.iterate(ks, node, state)
                vitems, velem, _, _ = self.iterate(vs, node, state)
                if kitems is not None and vitems is not None and items is not None and len(kitems) == len(vitems):
                    for k, val in zip(kitems, vitems):
                        pid = self._alloc("pair", f"pair:{self.site_of(node)}:{len(items)}", self.frame.ctx, True, "pair", payload=(k, val))
                        items.append(Value.obj(pid))
                else:
                    items = None
                pair = True
                pairs.append((kelem, velem))
            elif o.kind == "enumerate":
                inner = o.payload
                _, ielem, ipair, _ = self.iterate(inner, node, state)
                items = None
                pair = True
                pairs.append((Value.data("index"), ielem if not ipair else Value()))
            elif o.kind == "ph":
                items = None
                exhaustive = False
                self.record_event("iter", oid)
                if o.depth >= PATH_DEPTH:
                    elems.append(Value.data("deep"))
                    continue
                eid = self.path_placeholder(oid, "item", "*", state)
                elems.append(Value(frozenset({eid}), Param(eid)))
            elif o.kind == "unknown":
                items = None
                exhaustive = False
                if o.depth >= PATH_DEPTH:
                    elems.append(Value.data("deep"))
                    continue
                srcs = state.row(oid, "item").sources
                eid = self._alloc("unknown", f"elem:{oid}", "", False, f"elem of {o.label}")
                self.objects[eid].iter_elem = True
                self.objects[eid].depth = o.depth + 1
                if (eid, "item") not in state.rows:
                    r = Row(sources=frozenset((s.extend("*"), ()) for s, _ in srcs), mutators=state.row(oid, "item").mutators)
                    state.set_row(eid, "item", r)
                    state.set_row(eid, "attr", r)
                elems.append(Value.obj(eid))
            elif o.kind == "series":
                items = None
                exhaustive = False
                elems.append(Value.data(f"L{getattr(node, 'lineno', 0)}"))
            else:
                items = None
                exhaustive = False
                elems.append(Value.data(f"L{getattr(node, 'lineno', 0)}"))
        if not v.pts:
            items = None
            exhaustive = False
            if isinstance(v.kv, (Lit, Lits)):
                ks = kv_literals(v.kv)
                items = [Value.lit(k) for k in sorted(ks, key=str)] if len(ks) <= UNROLL_MAX else None
                elems.append(Value(frozenset(), Lits(ks) if len(ks) != 1 else Lit(next(iter(ks)))))
            else:
                elems.append(Value.data(f"L{getattr(node, 'lineno', 0)}"))
        if items is not None and not items:
            items = None
        if pair:
            kacc = join_values(p[0] for p in pairs)
            vacc = join_values(p[1] for p in pairs)
            for e in elems:          # mixing pairs and plain elements: degrade
                kacc = kacc.join(e)
            return items, (kacc, vacc), True, exhaustive and len(pairs) == 1
        elem = join_values(elems)
        return items, elem, False, exhaustive and len(elems) == 1 and isinstance(elem.kv, KeysOf)

    def promote_elem_reqs(self, exit_s: State, body_s: State, elem: Value, state: State) -> None:
        """Requirements on the loop's own elements are per element, not per program run:
        keep them 'always' even though the loop may run zero times."""
        if not body_s.reachable:
            return
        srcs: set = set()
        for oid in elem.pts:
            for ns in ("item", "attr"):
                for src, _ in state.row(oid, ns).sources:
                    if not src.is_data:
                        srcs.add(src)
        for src in srcs:
            for k, info in (body_s.reqs.inner(src) or {}).items():
                cur_d = exit_s.reqs.inner(src)
                if info.always and cur_d is not None and k in cur_d:
                    cur = cur_d[k]
                    if not cur.always:
                        exit_s.reqs.writable(src)[k] = ReqInfo(True, cur.sites, cur.alternatives, cur.caveats, cur.alt_keys)

    def promote_elem_narrowing(self, exit_s: State, body_s: State, elem: Value) -> None:
        """Keys proven on a loop element that is a view of a record (rows of a frame) are
        proven on the record itself, even though the loop may run zero times."""
        if not body_s.reachable:
            return
        for oid in elem.pts:
            src = getattr(self.objects[oid], "derived_from", None)
            if src is None or not self.objects[src].single:
                continue
            for ns in ("item", "attr"):
                proven = body_s.row(oid, ns).definite & body_s.row(src, ns).definite
                row = exit_s.row(src, ns)
                for k in proven - row.definite:
                    row = row.narrow_in(k, True)
                exit_s.set_row(src, ns, row)

    def _literal_dict_items(self, oid: int, state: State):
        """[(key, value)] for a closed dict whose keys are all definite literals (unrollable)."""
        if self.objects[oid].kind == "ph":
            self.record_event("iter", oid)
            return None
        row = state.row(oid, "item")
        if row.open or row.present != row.definite or len(row.present) > UNROLL_MAX or not row.present:
            return None
        if (oid, "item", "*") in state.slots:
            return None
        return [(k, state.slot(oid, "item", k)) for k in sorted(row.present, key=str)]

    def elem_of(self, v: Value, node: ast.AST, state: State) -> Value:
        acc = Value()
        for oid in v.pts:
            o = self.objects[oid]
            if o.kind in ("list", "tuple", "set"):
                e = state.slot(oid, "elem", "*")
                kv = state.elems.get(oid, DATA)
                acc = acc.join(Value(e.pts, kv))
            elif o.kind in ("dict", "frame"):
                acc = acc.join(Value(frozenset(), KeysOf(frozenset({oid}), "item")))
            else:
                acc = acc.join(Value.data(f"L{getattr(node, 'lineno', 0)}"))
        if not v.pts:
            acc = Value.data(f"L{getattr(node, 'lineno', 0)}")
        return acc

    def all_values(self, oid: int, ns: str, state: State) -> Value:
        return state.all_slots(oid, ns)

    # ----------------------------------------------------------- expressions
    def eval(self, node: ast.expr, state: State) -> Value:
        m = getattr(self, "expr_" + type(node).__name__, None)
        if m is None:
            self.note(f"unsupported expression {type(node).__name__} at L{getattr(node, 'lineno', 0)}")
            return Value.data(f"L{getattr(node, 'lineno', 0)}")
        return m(node, state)

    def eval_key(self, node: ast.expr, state: State) -> Value:
        if isinstance(node, ast.Tuple):
            parts = [self.eval(e, state) for e in node.elts]
            if all(isinstance(p.kv, Lit) for p in parts):
                return Value.lit(tuple(p.kv.key for p in parts))
            tid = self.alloc("tuple", node, state, label="tuple")
            self.objects[tid].literal_len = len(parts)
            for i, p in enumerate(parts):
                state.set_slot(tid, "pos", i, p, True)
            return Value.obj(tid)
        if isinstance(node, ast.Slice):
            return Value.data(f"slice@L{node.lineno}")
        return self.eval(node, state)

    def expr_Constant(self, node: ast.Constant, state: State) -> Value:
        return Value.lit(node.value)

    def expr_Name(self, node: ast.Name, state: State) -> Value:
        return self.load_name(node.id, node, state)

    def expr_Attribute(self, node: ast.Attribute, state: State) -> Value:
        if state.narrowed:
            key = _chain_text(node)
            if key is not None:
                hit = state.narrowed.get(key)
                if hit is not None:
                    return hit
        obj = self.eval(node.value, state)
        return self.load_attr(obj, node.attr, node, state)

    def expr_Subscript(self, node: ast.Subscript, state: State) -> Value:
        obj = self.eval(node.value, state)
        key = self.eval_key(node.slice, state)
        return self.load_item(obj, "item", key, node, state)

    def expr_Call(self, node: ast.Call, state: State) -> Value:
        fv = self.eval(node.func, state)
        if self.may_raise and not all(self.objects[o].kind == "method" and self.objects[o].payload[0] in SAFE_METHODS for o in fv.pts):
            self.may_raise[-1] = True
        return self.call_value(fv, node, state)

    def expr_Dict(self, node: ast.Dict, state: State) -> Value:
        oid = self.alloc("dict", node, state, label=f"dict@L{node.lineno}")
        for k, v in zip(node.keys, node.values):
            val = self.eval(v, state)
            if k is None:      # **spread
                self.copy_into(oid, "item", val, state, strong=True)
            else:
                kv = self.eval_key(k, state)
                self.store_key(oid, "item", kv.kv, val, True, node, state)
        return Value.obj(oid)

    def _container(self, node: ast.expr, kind: str, elts: List[ast.expr], state: State) -> Value:
        oid = self.alloc(kind, node, state, label=f"{kind}@L{node.lineno}")
        obj = self.objects[oid]
        vals: List[Value] = []
        literal = True
        for e in elts:
            if isinstance(e, ast.Starred):
                literal = False
                inner = self.eval(e.value, state)
                _, el, _, _ = self.iterate(inner, e, state)
                vals.append(el if not isinstance(el, tuple) else Value())
            else:
                vals.append(self.eval(e, state))
        acc = Value()
        kvacc: Any = None
        for v in vals:
            acc = acc.join(Value(v.pts, DATA))
            kvacc = v.kv if kvacc is None else kv_join(kvacc, v.kv)
        state.set_slot(oid, "elem", "*", Value(acc.pts, DATA), obj.single)
        if vals:
            state.elems[oid] = kvacc if obj.single or oid not in state.elems else kv_join(state.elems[oid], kvacc)
        else:
            state.elems[oid] = Lits(frozenset())
        if literal and obj.single:
            obj.literal_len = len(vals)
            for i, v in enumerate(vals):
                state.set_slot(oid, "pos", i, v, True)
        else:
            obj.literal_len = None
        return Value.obj(oid)

    def expr_List(self, node: ast.List, state: State) -> Value:
        return self._container(node, "list", node.elts, state)

    def expr_Tuple(self, node: ast.Tuple, state: State) -> Value:
        return self._container(node, "tuple", node.elts, state)

    def expr_Set(self, node: ast.Set, state: State) -> Value:
        return self._container(node, "set", node.elts, state)

    def expr_Lambda(self, node: ast.Lambda, state: State) -> Value:
        return self.make_function(node, state, name=f"lambda@L{node.lineno}")

    def expr_NamedExpr(self, node: ast.NamedExpr, state: State) -> Value:
        v = self.eval(node.value, state)
        self.bind_target(node.target, v, state)
        return v

    def expr_IfExp(self, node: ast.IfExp, state: State) -> Value:
        self.eval(node.test, state)
        a = state.copy()
        b = state.copy()
        self.narrow(node.test, True, a)
        self.narrow(node.test, False, b)
        va = self.eval(node.body, a) if a.reachable else Value()
        vb = self.eval(node.orelse, b) if b.reachable else Value()
        state.set_to(a.join(b))
        return va.join(vb)

    def expr_BoolOp(self, node: ast.BoolOp, state: State) -> Value:
        """`a and b and c`: operands are evaluated sequentially, each under the narrowing
        implied by the previous ones; the state afterwards is the join of the early-exit
        state (after the first operand) and the fall-through state."""
        acc = self.eval(node.values[0], state)
        is_and = isinstance(node.op, ast.And)
        pre = state.copy()
        s = state.copy()
        self.cond_depth += 1
        prev = node.values[0]
        for v in node.values[1:]:
            truth = self.truthiness(acc, s)
            if (not is_and and truth is True) or (is_and and truth is False):
                break          # short-circuit: v never evaluated
            self.narrow(prev, is_and, s)
            if not s.reachable:
                break
            rhs = self.eval(v, s)
            if (not is_and and truth is False) or (is_and and truth is True):
                acc = rhs      # the left operand is never the result
            else:
                acc = acc.join(rhs)
            prev = v
        self.cond_depth -= 1
        if s.reachable:
            state.set_to(pre.join(s))
        else:
            state.set_to(pre)
        return acc

    def truthiness(self, v: Value, state: State) -> Optional[bool]:
        """True/False when a value is definitely truthy/falsy, else None."""
        if isinstance(v.kv, Lit) and not v.pts:
            return bool(v.kv.key)
        if isinstance(v.kv, Param):
            self.record_event("truth", v.kv.ph)
            return None
        if not isinstance(v.kv, Bot) or not v.pts:
            return None
        verdicts = set()
        for oid in v.pts:
            o = self.objects[oid]
            if o.kind == "ph":
                self.record_event("truth", oid)
                return None
            if o.kind == "dict":
                row = state.row(oid, "item")
                if row.definite:
                    verdicts.add(True)
                elif row.closed and not row.present:
                    verdicts.add(False)
                else:
                    return None
            elif o.kind in ("list", "tuple", "set"):
                if o.literal_len is None:
                    return None
                verdicts.add(o.literal_len > 0)
            elif o.kind in ("instance", "function", "bound", "class", "module", "frame", "namespace"):
                verdicts.add(True) if o.kind != "frame" else None
                if o.kind == "frame":
                    return None
            else:
                return None
        return next(iter(verdicts)) if len(verdicts) == 1 else None

    def expr_UnaryOp(self, node: ast.UnaryOp, state: State) -> Value:
        v = self.eval(node.operand, state)
        if isinstance(node.op, ast.Not):
            return Value.data("bool")
        if isinstance(node.op, ast.USub) and isinstance(v.kv, Lit) and isinstance(v.kv.key, (int, float)):
            return Value.lit(-v.kv.key)
        return Value(v.pts & {self.SERIES}, DATA)

    def expr_BinOp(self, node: ast.BinOp, state: State) -> Value:
        a = self.eval(node.left, state)
        b = self.eval(node.right, state)
        return self.binop(node.op, a, b, node, state)

    def binop(self, op: ast.operator, a: Value, b: Value, node: ast.AST, state: State) -> Value:
        if isinstance(op, ast.BitOr):
            recs = [o for o in a.pts | b.pts if self.objects[o].kind == "dict"]
            if recs:
                oid = self.alloc("dict", node, state, label=f"dict@L{node.lineno}")
                self.copy_into(oid, "item", a, state, strong=True)
                self.copy_into(oid, "item", b, state, strong=True)
                return Value.obj(oid)
        if isinstance(op, ast.Add):
            if isinstance(a.kv, Lit) and isinstance(b.kv, Lit) and isinstance(a.kv.key, str) and isinstance(b.kv.key, str):
                return Value.lit(a.kv.key + b.kv.key)
            if isinstance(a.kv, Lit) and isinstance(a.kv.key, str) and isinstance(b.kv, (Lits, KeysOf)):
                return Value(frozenset(), kv_map(b.kv, RenameFn("prefix", a.kv.key)))
            if isinstance(b.kv, Lit) and isinstance(b.kv.key, str) and isinstance(a.kv, (Lits, KeysOf)):
                return Value(frozenset(), kv_map(a.kv, RenameFn("suffix", b.kv.key)))
            if isinstance(a.kv, Lit) and isinstance(b.kv, Lit) and isinstance(a.kv.key, (int, float)) and isinstance(b.kv.key, (int, float)):
                return Value.lit(a.kv.key + b.kv.key)
            lists = [o for o in a.pts | b.pts if self.objects[o].kind == "list"]
            if lists:
                oid = self.alloc("list", node, state, label=f"list@L{node.lineno}")
                acc = Value()
                kv: Any = None
                for o in lists:
                    acc = acc.join(state.slot(o, "elem", "*"))
                    kv = state.elems.get(o, DATA) if kv is None else kv_join(kv, state.elems.get(o, DATA))
                state.set_slot(oid, "elem", "*", acc, True)
                state.elems[oid] = kv if kv is not None else DATA
                return Value.obj(oid)
        if isinstance(op, ast.Sub) and isinstance(a.kv, Lit) and isinstance(b.kv, Lit) and isinstance(a.kv.key, (int, float)) and isinstance(b.kv.key, (int, float)):
            return Value.lit(a.kv.key - b.kv.key)
        if isinstance(op, ast.Mod) and isinstance(a.kv, Lit) and isinstance(a.kv.key, str) and a.kv.key.count("%s") == 1 and "%" not in a.kv.key.replace("%s", ""):
            pre, post = a.kv.key.split("%s")
            kv = b.kv
            if pre:
                kv = kv_map(kv, RenameFn("prefix", pre))
            if post:
                kv = kv_map(kv, RenameFn("suffix", post))
            return Value(frozenset(), kv)
        if self.SERIES in a.pts or self.SERIES in b.pts:
            return Value(frozenset({self.SERIES}), DATA)
        frames = [o for o in a.pts | b.pts if self.objects[o].kind == "frame"]
        if frames:
            return Value(frozenset(frames), DATA)
        return Value.data(f"L{getattr(node, 'lineno', 0)}")

    def expr_Compare(self, node: ast.Compare, state: State) -> Value:
        a = self.eval(node.left, state)
        for c in node.comparators:
            b = self.eval(c, state)
            if self.SERIES in a.pts or self.SERIES in b.pts:
                return Value(frozenset({self.SERIES}), DATA)
            if any(self.objects[o].kind == "frame" for o in a.pts | b.pts):
                return Value(frozenset(o for o in a.pts | b.pts if self.objects[o].kind == "frame"), DATA)
        return Value.data("bool")

    def expr_JoinedStr(self, node: ast.JoinedStr, state: State) -> Value:
        parts: List[Any] = []
        for v in node.values:
            if isinstance(v, ast.Constant):
                parts.append(str(v.value))
            elif isinstance(v, ast.FormattedValue):
                if v.format_spec is not None or v.conversion != -1:
                    parts.append(None)
                else:
                    parts.append(self.eval(v.value, state))
            else:
                parts.append(None)
        holes = [p for p in parts if not isinstance(p, str)]
        if not holes:
            return Value.lit("".join(parts))  # type: ignore[arg-type]
        if len(holes) == 1 and holes[0] is not None:
            i = parts.index(holes[0])
            pre = "".join(p for p in parts[:i] if isinstance(p, str))
            post = "".join(p for p in parts[i + 1:] if isinstance(p, str))
            kv = holes[0].kv
            if isinstance(kv, Lit) and not isinstance(kv.key, str):
                kv = Lit(str(kv.key))
            if pre:
                kv = kv_map(kv, RenameFn("prefix", pre))
            if post:
                kv = kv_map(kv, RenameFn("suffix", post))
            return Value(frozenset(), kv)
        return Value.data(f"fstring@L{node.lineno}")

    def expr_Starred(self, node: ast.Starred, state: State) -> Value:
        return self.eval(node.value, state)

    def expr_Await(self, node: ast.Await, state: State) -> Value:
        return self.eval(node.value, state)

    def expr_Yield(self, node: ast.Yield, state: State) -> Value:
        if node.value is not None:
            v = self.eval(node.value, state)
            fr = self.frame
            if fr.func is not None:
                gid = self._alloc("list", f"gen:{fr.func.id}", fr.ctx, False, f"generator {fr.label}")
                state.set_slot(gid, "elem", "*", v, False)
                state.elems[gid] = kv_join(state.elems.get(gid, v.kv), v.kv)
                fr.func.payload.__dict__.setdefault("generator", gid)
        return Value.data("yield")

    expr_YieldFrom = expr_Yield

    def expr_Slice(self, node: ast.Slice, state: State) -> Value:
        return Value.data("slice")

    # comprehensions ---------------------------------------------------------
    def _comp(self, node: Any, state: State, body: Callable[[State], None]) -> None:
        fr = self.frame
        fr.overlays.append({})
        try:
            self._comp_gen(node.generators, 0, state, body)
        finally:
            fr.overlays.pop()

    def _comp_gen(self, gens: List[ast.comprehension], i: int, state: State, body: Callable[[State], None]) -> None:
        if i == len(gens):
            body(state)
            return
        g = gens[i]
        itv = self.eval(g.iter, state)
        items, elem, pair, exhaustive = self.iterate(itv, g.iter, state)
        fr = self.frame

        def run_one(it: Any, st: State, exh_depth: int) -> None:
            self._comp_bind(g.target, it, st, pair and exh_depth >= 0)
            st2 = st
            for cond in g.ifs:
                self.eval(cond, st2)
                self.narrow(cond, True, st2)
                if not st2.reachable:
                    return
            self._comp_gen(gens, i + 1, st2, body)

        understood = all(self._cond_is_exclusion(c, g.target) for c in g.ifs)
        if items is not None and len(items) <= UNROLL_MAX:
            if not understood:
                self.cond_depth += 1
            for it in items:
                s2 = state.copy()
                fr.overlays.append({})
                run_one(it, s2, -1)
                fr.overlays.pop()
                if s2.reachable:
                    if understood:
                        state.set_to(s2)          # the iteration definitely ran
                    else:
                        state.set_to(state.join(s2))   # the filter may have skipped it
            if not understood:
                self.cond_depth -= 1
            return
        depth = self.cond_depth
        if exhaustive and understood and not pair and isinstance(elem.kv, KeysOf):
            elem = Value(elem.pts, KeysOf(elem.kv.objs, elem.kv.ns, elem.kv.exclude, elem.kv.chain, depth))
        elif exhaustive and understood and pair and isinstance(elem[0].kv, KeysOf):
            k0 = elem[0]
            elem = (Value(k0.pts, KeysOf(k0.kv.objs, k0.kv.ns, k0.kv.exclude, k0.kv.chain, depth)), elem[1])
        s2 = state.copy()
        fr.overlays.append({})
        if not understood:
            self.cond_depth += 1
        run_one(elem, s2, depth)
        if not understood:
            self.cond_depth -= 1
        fr.overlays.pop()
        if exhaustive and understood and s2.reachable:
            # iteration over every key of a record: copy semantics already account for emptiness
            state.set_to(s2)
        else:
            # zero iterations are possible: join with the incoming state
            joined = state.join(s2)
            self.promote_elem_reqs(joined, s2, elem if not pair else elem[1], state)
            state.set_to(joined)

    def _comp_bind(self, target: ast.expr, it: Any, state: State, pair: bool) -> None:
        ov = self.frame.overlays[-1]
        names = [n.id for n in ast.walk(target) if isinstance(n, ast.Name)]
        for n in names:
            ov.setdefault(n, Value())
        self.bind_iter_target(target, it, state, pair)

    def _cond_is_exclusion(self, cond: ast.expr, target: ast.expr) -> bool:
        """`k != "x"`, `k not in (...)`, `k in (...)`, and conjunctions of those, on the key target."""
        tnames = {n.id for n in ast.walk(target) if isinstance(n, ast.Name)}
        if isinstance(cond, ast.BoolOp) and isinstance(cond.op, ast.And):
            return all(self._cond_is_exclusion(c, target) for c in cond.values)
        if isinstance(cond, ast.UnaryOp) and isinstance(cond.op, ast.Not):
            return self._cond_is_exclusion(cond.operand, target)
        if isinstance(cond, ast.Compare) and len(cond.ops) == 1 and isinstance(cond.left, ast.Name) and cond.left.id in tnames:
            op = cond.ops[0]
            rhs = cond.comparators[0]
            lit = isinstance(rhs, ast.Constant) or (isinstance(rhs, (ast.Tuple, ast.List, ast.Set)) and all(isinstance(e, ast.Constant) for e in rhs.elts))
            if lit and isinstance(op, (ast.NotEq, ast.Eq, ast.In, ast.NotIn)):
                return True
            if isinstance(op, (ast.NotIn, ast.In)) and isinstance(rhs, ast.Name):
                return True
        return False

    def expr_DictComp(self, node: ast.DictComp, state: State) -> Value:
        oid = self.alloc("dict", node, state, label=f"dictcomp@L{node.lineno}")
        base_depth = self.cond_depth

        def body(st: State) -> None:
            k = self.eval_key(node.key, st)
            v = self.eval(node.value, st)
            strong = self.objects[oid].single and self.cond_depth == base_depth
            self.store_key(oid, "item", k.kv, v, strong, node, st)
        self._comp(node, state, body)
        return Value.obj(oid)

    def _seq_comp(self, node: Any, kind: str, state: State) -> Value:
        oid = self.alloc(kind, node, state, label=f"{kind}comp@L{node.lineno}")
        self.objects[oid].literal_len = None

        def body(st: State) -> None:
            v = self.eval(node.elt, st)
            st.set_slot(oid, "elem", "*", Value(v.pts, DATA), False)
            st.elems[oid] = kv_join(st.elems.get(oid, v.kv), v.kv)
        state.elems.setdefault(oid, BOT)
        self._comp(node, state, body)
        return Value.obj(oid)

    def expr_ListComp(self, node: ast.ListComp, state: State) -> Value:
        return self._seq_comp(node, "list", state)

    def expr_SetComp(self, node: ast.SetComp, state: State) -> Value:
        return self._seq_comp(node, "set", state)

    def expr_GeneratorExp(self, node: ast.GeneratorExp, state: State) -> Value:
        return self._seq_comp(node, "list", state)

    # ------------------------------------------------------------- narrowing
    def narrow(self, test: ast.expr, truth: bool, state: State) -> None:
        if not state.reachable:
            return
        if isinstance(test, ast.UnaryOp) and isinstance(test.op, ast.Not):
            self.narrow(test.operand, not truth, state)
            return
        if isinstance(test, ast.BoolOp):
            if (isinstance(test.op, ast.And) and truth) or (isinstance(test.op, ast.Or) and not truth):
                for v in test.values:
                    self.narrow(v, truth, state)
            return
        if isinstance(test, ast.NamedExpr):
            self.narrow(test.target, truth, state)
            return
        if isinstance(test, ast.Compare) and len(test.ops) == 1:
            op = test.ops[0]
            left, right = test.left, test.comparators[0]
            if isinstance(op, (ast.In, ast.NotIn)):
                member = truth if isinstance(op, ast.In) else not truth
                self._narrow_membership(left, right, member, state)
                return
            if isinstance(op, (ast.Eq, ast.NotEq)) and isinstance(left, ast.Name):
                eq = truth if isinstance(op, ast.Eq) else not truth
                rv = self.eval(right, state)
                ks = kv_literals(rv.kv)
                if ks:
                    self._narrow_kv(left.id, ks, eq, state)
                return
            if isinstance(op, (ast.Is, ast.IsNot)) and isinstance(right, ast.Constant) and right.value is None:
                is_none = truth if isinstance(op, ast.Is) else not truth
                if not is_none:
                    self._narrow_get_truthy(left, state)
                return
            if isinstance(op, (ast.LtE, ast.Lt)) and isinstance(right, ast.Call) and isinstance(right.func, ast.Attribute) and right.func.attr == "keys":
                lv = self.eval(left, state)
                ks = self.lit_keys(lv, state)
                if ks is not None and truth:
                    rec = self.eval(right.func.value, state)
                    for k in ks:
                        self._narrow_record(rec, "item", k, True, state)
                return
            return
        if isinstance(test, ast.Call):
            if isinstance(test.func, ast.Name) and test.func.id == "isinstance" and len(test.args) == 2:
                if isinstance(test.args[0], ast.Name):
                    self._narrow_isinstance(test.args[0].id, test.args[1], truth, state)
                elif isinstance(test.args[0], ast.Attribute):
                    self._narrow_isinstance(test.args[0], test.args[1], truth, state)
                return
            if isinstance(test.func, ast.Name) and test.func.id == "hasattr" and len(test.args) == 2:
                kv = self.eval(test.args[1], state)
                if isinstance(kv.kv, Lit):
                    rec = self.eval(test.args[0], state)
                    self._narrow_record(rec, "attr", kv.kv.key, truth, state)
                return
            if isinstance(test.func, ast.Name) and test.func.id == "all" and len(test.args) == 1 and isinstance(test.args[0], ast.GeneratorExp):
                g = test.args[0]
                if truth and len(g.generators) == 1 and isinstance(g.elt, ast.Compare) and isinstance(g.elt.ops[0], ast.In):
                    ks = self.lit_keys(self.eval(g.generators[0].iter, state), state)
                    if ks is not None and isinstance(g.elt.left, ast.Name):
                        rec = self.eval(g.elt.comparators[0], state)
                        for k in ks:
                            self._narrow_record(rec, "item", k, True, state)
                return
            if isinstance(test.func, ast.Attribute) and test.func.attr == "issubset" and len(test.args) == 1 and truth:
                ks = self.lit_keys(self.eval(test.func.value, state), state)
                if ks is not None:
                    rec = self.eval(test.args[0], state)
                    for k in ks:
                        self._narrow_record(rec, "item", k, True, state)
                return
            if isinstance(test.func, ast.Attribute) and test.func.attr == "get" and truth:
                self._narrow_get_truthy(test, state)
                return

    def _narrow_isinstance(self, target: Any, cls_expr: ast.expr, truth: bool, state: State) -> None:
        """`isinstance(x, C)`: keep (truth) or drop (not truth) the objects of x that can be instances
        of C.  `target` is a variable name or an attribute chain (`self.ret.kv`): the latter narrows
        the attribute's slot when the base is one object."""
        if isinstance(target, str):
            fake = ast.Name(id=target, ctx=ast.Load())
            cur = self.load_name(target, fake, state)
            store = lambda v: self.store_name(target, v, state)   # noqa: E731
        else:
            base = self.eval(target.value, state)
            recs = [o for o in base.pts if self.objects[o].kind in ("instance", "ph", "namespace")]
            if len(recs) != 1 or len(base.pts) != 1 or not self.strongable(recs[0]):
                # several possible bases: remember the narrowed value of the chain on this path
                key = _chain_text(target)
                if key is None or not truth:
                    return
                cur = self.eval(target, state)
                if not cur.pts:
                    return
                store = lambda v: state.narrowed.__setitem__(key, v)   # noqa: E731
                self._narrow_isinstance_value(cur, cls_expr, truth, state, store)
                return
            bo = recs[0]
            cur = state.slot(bo, "attr", target.attr)
            if self.objects[bo].kind == "ph" and truth and self.sum_stack:
                # symbolic: on this path the attribute holds an instance of C — a narrowed
                # placeholder for the slot's content records that
                cv0 = self.eval(cls_expr, state)
                cl = [c for c in cv0.pts if self.objects[c].kind == "class"]
                if len(cl) == 1 and len(cv0.pts) == 1:
                    pid = self.path_placeholder(bo, "attr", target.attr, state, cls=cl[0], tag=f":is{cl[0]}")
                    state.set_slot(bo, "attr", target.attr, Value(frozenset({pid}), Param(pid)), True)
                return
            store = lambda v: state.set_slot(bo, "attr", target.attr, v, True)   # noqa: E731
        if not cur.pts:
            return
        self._narrow_isinstance_value(cur, cls_expr, truth, state, store)

    def _narrow_isinstance_value(self, cur: Value, cls_expr: ast.expr, truth: bool, state: State, store: Callable[[Value], None]) -> None:
        cv = self.eval(cls_expr, state)
        classes: List[int] = []
        kinds: set = set()
        narrowed: Optional[List[Tuple[int, int]]] = [] if self.sum_stack else None
        for c in cv.pts:
            o = self.objects[c]
            if o.kind == "class":
                classes.append(c)
            elif o.kind == "external":
                q = str(o.payload).split(".")[-1]
                kinds |= {"dict": {"dict", "unknown"}, "list": {"list"}, "tuple": {"tuple", "pair"}, "set": {"set"},
                          "frozenset": {"set"}, "str": set(), "int": set(), "float": set(), "bool": set(),
                          "bytes": set(), "DataFrame": {"frame"}, "Series": {"series"}, "Mapping": {"dict", "unknown"},
                          "MutableMapping": {"dict", "unknown"}, "Sequence": {"list", "tuple"},
                          "Iterable": {"list", "tuple", "set", "dict", "unknown", "view"}}.get(q, {"*"})
            elif o.kind in ("tuple", "list"):
                for i in range(o.literal_len or 0):
                    sub = state.slot(c, "pos", i)
                    cv2 = Value(sub.pts, BOT)
                    for c2 in cv2.pts:
                        o2 = self.objects[c2]
                        if o2.kind == "class":
                            classes.append(c2)
                        elif o2.kind == "external":
                            q = str(o2.payload).split(".")[-1]
                            kinds |= {"dict": {"dict", "unknown"}, "list": {"list"}, "tuple": {"tuple", "pair"},
                                      "set": {"set"}, "DataFrame": {"frame"}}.get(q, {"*"} if q[:1].isupper() else set())
            else:
                return
        if "*" in kinds:
            return
        def matches(oid: int) -> Optional[bool]:
            o = self.objects[oid]
            if o.kind == "instance":
                if o.payload is None:
                    return None
                return any(self._is_subclass(o.payload, c) for c in classes) or (o.payload in classes)
            if o.kind == "unknown":
                return None          # could be anything
            if o.kind == "ph":
                self.record_event("isinstance", oid)
                if o.payload.cls is not None and classes:
                    return any(self._is_subclass(o.payload.cls, c) for c in classes)
                if truth and len(classes) == 1 and not kinds and self.sum_stack and narrowed is not None:
                    # the placeholder when it is a C: a narrowed placeholder stands in for it
                    narrowed.append((oid, classes[0]))
                return None
            return o.kind in kinds
        keep = []
        for oid in cur.pts:
            m = matches(oid)
            if m is None or m == truth:
                keep.append(oid)
        if narrowed:
            repl = {ph: self.narrowed_placeholder(ph, c, state) for ph, c in narrowed}
            keep = [repl.get(o, o) for o in keep]
            kv = cur.kv
            if isinstance(kv, Param) and kv.ph in repl:
                kv = Param(repl[kv.ph])
            store(Value(frozenset(keep), kv))
            return
        new_pts = frozenset(keep)
        if new_pts == cur.pts:
            return
        if not new_pts and (truth or isinstance(cur.kv, Bot)):
            # x definitely is not an instance of C on this path: infeasible when x has no scalar part
            if isinstance(cur.kv, Bot):
                state.reachable = False
                return
        store(Value(new_pts, cur.kv))

    def _is_subclass(self, cls: int, base: int) -> bool:
        seen = set()
        order = [cls]
        while order:
            c = order.pop(0)
            if c == base:
                return True
            if c in seen:
                continue
            seen.add(c)
            ci: ClassInfo = self.objects[c].payload
            order.extend(ci.bases)
        return False

    def _narrow_get_truthy(self, expr: ast.expr, state: State) -> None:
        if isinstance(expr, ast.Call) and isinstance(expr.func, ast.Attribute) and expr.func.attr == "get" and expr.args:
            kv = self.eval(expr.args[0], state)
            if isinstance(kv.kv, Lit):
                rec = self.eval(expr.func.value, state)
                self._narrow_record(rec, "item", kv.kv.key, True, state)

    def _narrow_membership(self, left: ast.expr, right: ast.expr, member: bool, state: State) -> None:
        # `"k" in d`, `"k" in d.keys()`, `"k" in df.columns`
        rec_expr = right
        if isinstance(right, ast.Call) and isinstance(right.func, ast.Attribute) and right.func.attr == "keys":
            rec_expr = right.func.value
        elif isinstance(right, ast.Attribute) and right.attr in ("columns", "index"):
            rec_expr = right.value
        rv = self.eval(rec_expr, state)
        lv = self.eval(left, state)
        # user classes defining `__contains__` as `return <x> in <y>`: narrow <y> instead
        insts = [o for o in rv.pts if self.objects[o].kind == "instance"]
        if insts and len(rv.pts) == 1:
            o = self.objects[insts[0]]
            m = self.class_lookup(o.payload, "__contains__")
            if m is not None:
                for f in self._cls_slots.get((m, "attr", "__contains__"), Value()).pts:
                    fo = self.objects[f]
                    if fo.kind != "function":
                        continue
                    body = fo.payload.node.body
                    if (len(body) == 1 and isinstance(body[0], ast.Return) and isinstance(body[0].value, ast.Compare)
                            and len(body[0].value.ops) == 1 and isinstance(body[0].value.ops[0], ast.In)):
                        cmp = body[0].value
                        self.in_method_frame(f, Value.obj(insts[0]), [lv], state,
                                             lambda: self._narrow_membership(cmp.left, cmp.comparators[0], member, state))
                return
        recs = [o for o in rv.pts if self.objects[o].kind in RECORD_KINDS or self.objects[o].kind == "frame"]
        if recs:
            if isinstance(lv.kv, Lit):
                self._narrow_record(rv, "item", lv.kv.key, member, state)
            elif isinstance(lv.kv, Lits) and not member:
                for k in lv.kv.keys:
                    pass  # "one of them is absent": no information
            elif isinstance(lv.kv, Lits) and member:
                # `k in d` with k one of several literals: k is one the record may have
                rows = [state.row(o, "item") for o in recs]
                if all(r.closed for r in rows):
                    may: set = set()
                    for r in rows:
                        may |= r.present
                    self._narrow_key_expr(left, lv, may, state)
                else:
                    for o, r in zip(recs, rows):
                        if r.symbolic:
                            self.record_event("narrow", o)
            elif isinstance(left, ast.Name) and isinstance(lv.kv, KeysOf):
                pass
            return
        # `k in ("a", "b")`: narrow the key variable
        if isinstance(left, ast.Name):
            ks = self.lit_keys(rv, state)
            if ks is not None:
                self._narrow_kv(left.id, ks, member, state)

    def _narrow_key_expr(self, left: ast.expr, lv: Value, may: set, state: State) -> None:
        """Narrow the variable a key expression is built from (`prefix + "*" in d`) to the
        alternatives whose key the record may have."""
        if isinstance(left, ast.Name):
            self._narrow_kv(left.id, frozenset(k for k in lv.kv.keys if k in may), True, state)
            return
        if any(isinstance(n, ast.Call) for n in ast.walk(left)):
            return
        names = {n.id for n in ast.walk(left) if isinstance(n, ast.Name)}
        cands = [n for n in sorted(names) if isinstance(self.load_name(n, ast.Name(id=n, ctx=ast.Load()), state).kv, Lits)]
        if len(cands) != 1:
            return
        name = cands[0]
        cur = self.load_name(name, ast.Name(id=name, ctx=ast.Load()), state)
        if len(cur.kv.keys) > 64:
            return
        keep = set()
        fr = self.frame
        for c in sorted(cur.kv.keys, key=str):
            fr.overlays.append({name: Value(cur.pts, Lit(c))})
            try:
                v = self.eval(left, state)
            finally:
                fr.overlays.pop()
            if isinstance(v.kv, Lit):
                if v.kv.key in may:
                    keep.add(c)
            elif isinstance(v.kv, Lits):
                if v.kv.keys & may:
                    keep.add(c)
            else:
                return
        self._narrow_kv(name, frozenset(keep), True, state)

    def _narrow_kv(self, name: str, keys: FrozenSet[Any], restrict: bool, state: State) -> None:
        if not keys:
            return
        fake = ast.Name(id=name, ctx=ast.Load())
        cur = self.load_name(name, fake, state)
        if isinstance(cur.kv, Param):
            self.record_event("narrow", cur.kv.ph)
            return
        new = kv_restrict(cur.kv, keys) if restrict else kv_exclude(cur.kv, keys)
        if new is None:
            if isinstance(cur.kv, (Lit, Lits)):
                state.reachable = False
            return
        if new != cur.kv:
            self.store_name(name, Value(cur.pts, new), state)

    def _narrow_record(self, rec: Value, ns: str, k: Any, member: bool, state: State) -> None:
        targets = [o for o in rec.pts if self.objects[o].kind in RECORD_KINDS or self.objects[o].kind == "frame"]
        single = len(targets) == 1
        for oid in targets:
            row = state.row(oid, ns)
            if member:
                strong = single and self.strongable(oid)
                state.set_row(oid, ns, row.narrow_in(k, strong))
                if strong:
                    self.propagate_narrowing(oid, ns, k, state)
            elif single:
                if k in row.definite:
                    state.reachable = False
                state.set_row(oid, ns, row.narrow_out(k))

    def strongable(self, oid: int) -> bool:
        o = self.objects[oid]
        return o.single or getattr(o, "iter_elem", False)

    def propagate_narrowing(self, oid: int, ns: str, k: Any, state: State) -> None:
        """A key proven on a view of a record (a row of a frame, an element of a reader)
        is proven on the record it was derived from."""
        src = getattr(self.objects[oid], "derived_from", None)
        seen = set()
        while src is not None and src not in seen:
            seen.add(src)
            so = self.objects[src]
            if so.single and (src, ns) in state.rows:
                state.set_row(src, ns, state.row(src, ns).narrow_in(k, True))
            src = getattr(so, "derived_from", None)

    # ------------------------------------------------------ record access
    def is_guarded(self, ns: str) -> bool:
        want = ITEM_GUARDS if ns == "item" else ATTR_GUARDS
        return any(g & want for g in self.guards)

    def record_objects(self, v: Value) -> List[int]:
        return [o for o in v.pts if self.objects[o].kind in RECORD_KINDS or self.objects[o].kind == "frame"]

    def load_item(self, obj: Value, ns: str, key: Value, node: ast.AST, state: State,
                  report: bool = True) -> Value:
        acc = Value()
        guarded = self.is_guarded(ns) or not report
        single_target = len(obj.pts) == 1
        for oid in sorted(obj.pts):
            o = self.objects[oid]
            if o.kind == "locidx":
                acc = acc.join(self.models.loc_get(self, o, key, node, state))
                continue
            if o.kind == "groupby":
                from .models import _groupby_select
                acc = acc.join(_groupby_select(self, oid, key, node, state))
                continue
            if o.kind == "instance" and ns == "item":
                m = self.class_lookup(o.payload, "__getitem__")
                if m is not None:
                    for f in self._cls_slots.get((m, "attr", "__getitem__"), Value()).pts:
                        if self.objects[f].kind == "function":
                            acc = acc.join(self.call_user(f, Value.obj(oid), [key], {}, [], [], state, node))
                    continue
                ci = self.objects[o.payload].payload if o.payload is not None and o.payload in self.objects else None
                if isinstance(ci, ClassInfo) and not ci.typed_dict and not ci.named_tuple and not ci.open_base:
                    # indexing an instance of a class without __getitem__ is a type error, not a
                    # key question: nothing to report here
                    acc = acc.join(Value.data(f"L{getattr(node, 'lineno', 0)}"))
                    continue
            if o.kind in ("view", "pair", "zip", "enumerate"):
                acc = acc.join(Value.data(f"L{getattr(node, 'lineno', 0)}"))
                continue
            if o.kind in ("list", "tuple", "set"):
                if isinstance(key.kv, Lit) and isinstance(key.kv.key, int) and o.literal_len is not None:
                    idx = key.kv.key
                    if 0 <= idx < o.literal_len:
                        acc = acc.join(state.slot(oid, "pos", idx))
                    elif -o.literal_len <= idx < 0:
                        acc = acc.join(state.slot(oid, "pos", o.literal_len + idx))
                    else:
                        acc = acc.join(state.slot(oid, "elem", "*"))
                else:
                    e = state.slot(oid, "elem", "*")
                    acc = acc.join(Value(e.pts, state.elems.get(oid, DATA) if False else DATA))
                continue
            if o.kind == "series":
                acc = acc.join(Value(frozenset({self.SERIES}), DATA))
                continue
            if o.kind == "frame" and ns == "item":
                acc = acc.join(self.models.frame_getitem(self, oid, key, node, state, guarded))
                continue
            if o.kind in RECORD_KINDS:
                acc = acc.join(self._load_record(oid, ns, key, node, state, guarded, single_target))
                continue
            if o.kind in ("function", "bound", "class", "external", "method", "cell", "zip", "view", "pair", "enumerate"):
                acc = acc.join(Value.data(f"L{getattr(node, 'lineno', 0)}"))
                continue
            acc = acc.join(Value.data(f"L{getattr(node, 'lineno', 0)}"))
        if not obj.pts:
            acc = Value.data(f"L{getattr(node, 'lineno', 0)}")
        return acc

    def _load_record(self, oid: int, ns: str, key: Value, node: ast.AST, state: State,
                     guarded: bool, single_target: bool) -> Value:
        o = self.objects[oid]
        row = state.row(oid, ns)
        kv = key.kv
        site = self.access_site(node)
        if isinstance(kv, Param):
            # key is the unknown value of a placeholder: decided at instantiation
            if not guarded:
                self.record_symreq(node, kv, row, oid, ns, guarded, "load")
            elif self.may_raise:
                self.may_raise[-1] = True
            v = self.all_values(oid, ns, state)
            v = v.join(self._path_placeholder(oid, ns, ("@", kv.ph), row, state))
            return Value(v.pts, Param(next(iter(v.pts))) if len(v.pts) == 1 and self.is_ph(next(iter(v.pts))) else DATA)
        if isinstance(kv, (Lit, Lits)):
            acc = Value()
            ks = kv_literals(kv)
            for k in sorted(ks, key=str):
                cls = classify(row, k)
                if cls.tier == SYM:
                    if guarded:
                        if self.may_raise:
                            self.may_raise[-1] = True
                        self.record_event("guarded", oid)
                    else:
                        self.record_symreq(node, Lit(k), row, oid, ns, guarded, "load")
                    if isinstance(kv, Lit) and single_target and self.strongable(oid):
                        row = row.narrow_in(k, True)
                        state.set_row(oid, ns, row)
                elif guarded and cls.tier != PROVEN and self.may_raise:
                    self.may_raise[-1] = True
                if not guarded and cls.tier != SYM:
                    self.verdict(node, k, cls, oid, ns)
                    if cls.tier == INPUT:
                        srcs = [s for s, _ in cls.providers]
                        for s, terms in cls.providers:
                            for t in terms:
                                state.require(s, t, site, alternatives=frozenset(x for x in srcs if x != s),
                                              caveats=row.mutators, alt_keys=terms)
                    if cls.tier != MISSING and isinstance(kv, Lit) and single_target and self.strongable(oid):
                        row = row.narrow_in(k, True)
                        state.set_row(oid, ns, row)
                        self.propagate_narrowing(oid, ns, k, state)
                v = state.slot(oid, ns, k)
                star = state.slot(oid, ns, "*")
                v = v.join(Value(star.pts, DATA)) if star.pts else v
                if not v.pts and isinstance(v.kv, (Data, Bot)) and (row.open or cls.tier == MAP):
                    v = v.join(self._path_placeholder(oid, ns, k, row, state))
                    if isinstance(kv, Lit):
                        state.set_slot(oid, ns, k, v, strong=False)
                acc = acc.join(v)
            return acc
        if isinstance(kv, KeysOf):
            srcrow = join_rows(state.row(x, kv.ns) for x in kv.objs)
            if not guarded and srcrow.closed and row.closed:
                missing = {lens_apply_chain(kv.chain, k) for k in srcrow.present - kv.exclude} - row.present
                missing.discard(None)
                if missing:
                    for k in sorted(missing, key=str):
                        self.verdict(node, k, Classification(POSSIBLY, note="key from another record's key set"), oid, ns)
            return self.all_values(oid, ns, state).join(Value(frozenset(), DATA))
        # Data key
        if not guarded and row.symbolic:
            self.record_symreq(node, kv, row, oid, ns, guarded, "load")
        elif not guarded and row.closed and not row.is_map and row.present and o.kind in ("dict", "frame"):
            self.verdict(node, f"<data {kv}>", Classification(DATAKEY, note="data-derived key into a closed record"), oid, ns)
        v = self.all_values(oid, ns, state)
        if not v.pts and row.open:
            v = v.join(self._path_placeholder(oid, ns, "*", row, state))
        return Value(v.pts, DATA)

    def _path_placeholder(self, oid: int, ns: str, k: Any, row: Row, state: State) -> Value:
        o = self.objects[oid]
        if o.depth >= PATH_DEPTH:
            return Value.data("deep")
        if self.sum_stack and self.is_entry_object(oid):
            pid = self.path_placeholder(oid, ns, k, state)
            return Value(frozenset({pid}), Param(pid))
        label = f"{o.label}[{k!r}]"
        pid = self._alloc("unknown", f"path:{oid}:{ns}:{k!r}", "", True, label)
        self.objects[pid].depth = o.depth + 1
        if (pid, "item") not in state.rows:
            srcs = frozenset((s.extend(k), ()) for s, _ in row.sources if not s.is_data)
            r = Row(sources=srcs, mutators=row.mutators)
            if not srcs and not row.mutators:
                r = Row(top=True)
            state.set_row(pid, "item", r)
            state.set_row(pid, "attr", r)
        return Value.obj(pid)

    def store_item(self, obj: Value, ns: str, key: Value, v: Value, node: ast.AST, state: State) -> None:
        targets = [o for o in obj.pts if self.objects[o].kind not in ("series", "external", "function", "bound", "class", "method")]
        for oid in targets:
            o = self.objects[oid]
            strong = len(targets) == 1 and self.strongable(oid)
            if o.kind == "locidx":
                self.models.loc_set(self, o, key, v, node, state)
                continue
            if o.kind in ("list", "tuple", "set"):
                state.set_slot(oid, "elem", "*", Value(v.pts, DATA), False)
                if isinstance(key.kv, Lit) and isinstance(key.kv.key, int) and o.literal_len is not None and 0 <= key.kv.key < o.literal_len:
                    state.set_slot(oid, "pos", key.kv.key, v, strong)
                continue
            if o.kind == "frame":
                self.models.frame_setitem(self, oid, key, v, node, state, strong)
                continue
            if o.kind == "instance" and ns == "item":
                m = self.class_lookup(o.payload, "__setitem__")
                if m is not None:
                    for f in self._cls_slots.get((m, "attr", "__setitem__"), Value()).pts:
                        if self.objects[f].kind == "function":
                            self.call_user(f, Value.obj(oid), [key, v], {}, [], [], state, node)
                    continue
            self.store_key(oid, ns, key.kv, v, strong, node, state)

    def store_key(self, oid: int, ns: str, kv: Any, v: Value, strong: bool, node: ast.AST, state: State) -> None:
        row = state.row(oid, ns)
        if isinstance(kv, Param):
            state.set_row(oid, ns, row.param_store(kv.ph, strong))
            state.set_slot(oid, ns, ("@", kv.ph), v, strong)
            return
        if isinstance(kv, Lit):
            state.set_row(oid, ns, row.store(kv.key, strong))
            state.set_slot(oid, ns, kv.key, v, strong)
        elif isinstance(kv, Lits):
            state.set_row(oid, ns, row.store_many(kv.keys))
            for k in kv.keys:
                state.set_slot(oid, ns, k, v, False)
        elif isinstance(kv, KeysOf):
            srcrow = join_rows(state.row(x, kv.ns) for x in kv.objs)
            exhaustive = strong and kv.exhaustive_depth == self.cond_depth
            if self.exh_log is not None and exhaustive:
                self.exh_log.append((oid, ns, srcrow, kv.exclude, kv.chain))
            state.set_row(oid, ns, row.copy_from(srcrow, strong=exhaustive, kill=kv.exclude, chain=kv.chain))
            for x in kv.objs:
                for k in state.slot_keys(x, kv.ns):
                    if k in kv.exclude:
                        continue
                    k2 = lens_apply_chain(kv.chain, k)
                    if k2 is not None:
                        state.set_slot(oid, ns, k2, v, False)
            state.set_slot(oid, ns, "*", Value(v.pts, DATA), False)
        else:
            site = f"store@L{getattr(node, 'lineno', 0)}"
            state.set_row(oid, ns, row.data_store(Source(site, "data")))
            state.set_slot(oid, ns, "*", Value(v.pts, DATA), False)

    def copy_into(self, oid: int, ns: str, src: Value, state: State, strong: bool, kill: FrozenSet[Any] = frozenset()) -> None:
        """oid[ns] ∪= keys of every record in src (dict spread / update / dict(x))."""
        recs = self.record_objects(src)
        if not recs:
            if src.pts:
                # e.g. list of pairs: unknown keys
                state.set_row(oid, ns, state.row(oid, ns).data_store(Source(f"copy@{len(state.rows)}", "data")))
            return
        srcrow = join_rows(state.row(x, "item" if self.objects[x].kind != "instance" else "attr") for x in recs)
        state.set_row(oid, ns, state.row(oid, ns).copy_from(srcrow, strong=strong, kill=kill))
        for x in recs:
            xns = "item" if self.objects[x].kind != "instance" else "attr"
            for k, val in state.obj_slots(x, xns):
                if k not in kill:
                    state.set_slot(oid, ns, k, val, strong=False)

    def delete_item(self, obj: Value, ns: str, key: Value, node: ast.AST, state: State, report: bool = True) -> None:
        targets = self.record_objects(obj)
        for oid in targets:
            o = self.objects[oid]
            strong = len(targets) == 1 and o.single
            if o.kind == "frame" and ns == "item":
                self.models.frame_delitem(self, oid, key, node, state, strong, report)
                continue
            if o.kind == "instance" and ns == "item":
                m = self.class_lookup(o.payload, "__delitem__")
                if m is not None:
                    for f in self._cls_slots.get((m, "attr", "__delitem__"), Value()).pts:
                        if self.objects[f].kind == "function":
                            self.call_user(f, Value.obj(oid), [key], {}, [], [], state, node)
                    continue
            row = state.row(oid, ns)
            for k in sorted(kv_literals(key.kv), key=str):
                if report and not self.is_guarded(ns):
                    cls = classify(row, k)
                    self.verdict(node, k, cls, oid, ns)
                    if cls.tier == INPUT:
                        for s, terms in cls.providers:
                            for t in terms:
                                state.require(s, t, self.access_site(node))
                row = row.delete(k, strong and isinstance(key.kv, Lit))
                state.slots.pop((oid, ns, k), None)
            state.set_row(oid, ns, row)

    # ---------------------------------------------------------- attributes
    def load_attr(self, obj: Value, name: str, node: ast.AST, state: State, report: bool = True) -> Value:
        acc = Value()
        guarded = self.is_guarded("attr") or not report
        for oid in sorted(obj.pts):
            o = self.objects[oid]
            k = o.kind
            if k == "module":
                v = state.slots.get((oid, "attr", name))
                if v is None:
                    if not guarded:
                        self.verdict(node, name, Classification(MISSING, note="module has no such attribute"), oid, "attr")
                    v = Value.data(f"L{node.lineno}")
                acc = acc.join(v)
            elif k == "external":
                q = f"{o.payload}.{name}"
                rec = self.models.external_record(self, q, state)
                if rec is not None:
                    acc = acc.join(Value.obj(rec))
                else:
                    acc = acc.join(Value.obj(self._alloc("external", f"ext:{q}", "", True, q, payload=q)))
            elif k in ("parser", "groupby", "partial"):
                acc = acc.join(self.method_value(f"{k}.{name}", Value.obj(oid)))
            elif k == "namespace":
                acc = acc.join(self.load_instance_attr(oid, name, node, state, guarded, len(obj.pts) == 1))
            elif k == "class":
                m = self.class_lookup(oid, name)
                if m is not None:
                    v = state.slots.get((m if False else oid, "attr", name))
                    acc = acc.join(self.class_attr_value(oid, name, state))
                else:
                    acc = acc.join(Value.obj(self._alloc("method", f"classattr:{oid}.{name}", "", True, f"{o.label}.{name}", payload=(f"class.{name}", Value.obj(oid)))))
            elif k == "instance":
                acc = acc.join(self.load_instance_attr(oid, name, node, state, guarded, len(obj.pts) == 1))
            elif k == "dict":
                acc = acc.join(self.method_value(f"dict.{name}", Value.obj(oid)))
            elif k in ("list", "tuple", "set"):
                acc = acc.join(self.method_value(f"{k}.{name}", Value.obj(oid)))
            elif k == "frame":
                acc = acc.join(self.models.frame_getattr(self, oid, name, node, state, guarded))
            elif k == "series":
                acc = acc.join(Value.obj(self._alloc("external", f"ext:Series.{name}", "", True, f"Series.{name}", payload=f"Series.{name}")))
            elif k == "unknown":
                acc = acc.join(self.load_unknown_attr(oid, name, node, state, guarded))
            elif k == "ph":
                acc = acc.join(self.load_ph_attr(oid, name, node, state, guarded, len(obj.pts) == 1))
            elif k in ("function", "bound"):
                acc = acc.join(Value.data(f"L{node.lineno}"))
            elif k == "view" and name == "str" and o.payload[0] == "keys":
                acc = acc.join(Value(frozenset(), KeysOf(frozenset(o.payload[1].pts), "item")))
            elif k == "zip" or k == "view" or k == "enumerate":
                acc = acc.join(self.method_value(f"{k}.{name}", Value.obj(oid)))
            elif k == "response":
                acc = acc.join(self.method_value(f"response.{name}", Value.obj(oid)))
            elif k == "locidx":
                acc = acc.join(Value.data("loc"))
            elif k == "super":
                cls, self_v = o.payload
                ci: ClassInfo = self.objects[cls].payload
                found = None
                for b in ci.bases:
                    found = self.class_lookup(b, name)
                    if found is not None:
                        break
                if found is not None:
                    for f in self._cls_slots.get((found, "attr", name), Value()).pts:
                        if self.objects[f].kind == "function":
                            acc = acc.join(Value.obj(self._alloc("bound", f"bound:{f}:{','.join(map(str, sorted(self_v.pts)))}", "", True, f"{self.objects[f].label} via super", payload=(f, self_v))))
                else:
                    acc = acc.join(Value.data("super"))
            else:
                acc = acc.join(Value.data(f"L{node.lineno}"))
        if not obj.pts:
            if isinstance(obj.kv, (Lit, Lits, KeysOf)):
                return self.method_value(f"str.{name}", obj)
            return self.method_value(f"unknown.{name}", obj)
        return acc

    def method_value(self, qual: str, recv: Value) -> Value:
        key = f"method:{qual}:{','.join(map(str, sorted(recv.pts)))}:{recv.kv}"
        return Value.obj(self._alloc("method", key, "", True, qual, payload=(qual, recv)))

    def mro(self, cls: int) -> List[int]:
        """Breadth-first linearisation of the user classes above `cls` (good enough here)."""
        seen: set = set()
        order = [cls]
        out: List[int] = []
        while order:
            c = order.pop(0)
            if c in seen or c not in self.objects:
                continue
            seen.add(c)
            out.append(c)
            ci = self.objects[c].payload
            if isinstance(ci, ClassInfo):
                order.extend(ci.bases)
        return out

    def class_lookup(self, cls: int, name: str) -> Optional[int]:
        """Return the class oid (in MRO) that defines `name`, or None."""
        seen = set()
        order = [cls]
        while order:
            c = order.pop(0)
            if c in seen:
                continue
            seen.add(c)
            if (c, "attr", name) in self._cls_slots:
                return c
            ci: ClassInfo = self.objects[c].payload
            order.extend(ci.bases)
        return None

    def class_attr_value(self, cls: int, name: str, state: State) -> Value:
        c = self.class_lookup(cls, name)
        if c is None:
            return Value()
        return self._cls_slots.get((c, "attr", name), Value())

    def load_instance_attr(self, oid: int, name: str, node: ast.AST, state: State, guarded: bool, single_target: bool) -> Value:
        o = self.objects[oid]
        v = state.slots.get((oid, "attr", name))
        row = state.row(oid, "attr")
        if v is not None and (name in row.present):
            cls = classify(row, name)
            if not guarded:
                self.verdict(node, name, cls, oid, "attr")
            if o.payload is not None and self.field_is_any(o.payload, name):
                v = v.join(self.dynamic_value(f"{o.label}.{name}", state))
            return v
        c = self.class_lookup(o.payload, name) if o.payload is not None else None
        if c is not None:
            cv = self._cls_slots.get((c, "attr", name), Value())
            outs = []
            for f in cv.pts:
                fo = self.objects[f]
                if fo.kind == "function":
                    fi: FuncInfo = fo.payload
                    if fi.is_property:
                        outs.append(self.call_user(f, Value.obj(oid), [], {}, [], [], state, node))
                    elif fi.is_static:
                        outs.append(Value.obj(f))
                    elif fi.is_classmethod:
                        outs.append(Value.obj(self._alloc("bound", f"bound:{f}:cls{o.payload}", "", True, f"{fo.label} bound", payload=(f, Value.obj(o.payload)))))
                    else:
                        outs.append(Value.obj(self._alloc("bound", f"bound:{f}:{oid}", "", True, f"{fo.label} bound to {o.label}", payload=(f, Value.obj(oid)))))
                else:
                    outs.append(Value.obj(f))
            if not cv.pts:
                outs.append(cv)
            return join_values(outs)
        cls = classify(row, name)
        if cls.tier == SYM:
            if not guarded:
                self.record_symreq(node, Lit(name), row, oid, "attr", guarded, "attr")
            if single_target and self.strongable(oid):
                state.set_row(oid, "attr", row.narrow_in(name, True))
            return self._path_placeholder(oid, "attr", name, row, state)
        if not guarded:
            self.verdict(node, name, cls, oid, "attr")
            if cls.tier == INPUT:
                for s, terms in cls.providers:
                    for t in terms:
                        state.require(s, t, self.access_site(node), caveats=row.mutators)
            if cls.tier != MISSING and single_target and self.strongable(oid):
                state.set_row(oid, "attr", row.narrow_in(name, True))
                self.propagate_narrowing(oid, "attr", name, state)
        if row.open:
            return self._path_placeholder(oid, "attr", name, row, state)
        return Value.data(f"L{node.lineno}")

    def is_entry_object(self, oid: int) -> bool:
        """Inside a summary: was this object around before the function started (or does it
        stand for something the caller passes)?"""
        return bool(self.sum_stack) and (oid < self.sum_obj_mark[-1] or self.objects[oid].kind == "ph")

    def load_ph_attr(self, oid: int, name: str, node: ast.AST, state: State, guarded: bool, single_target: bool) -> Value:
        ph: PH = self.objects[oid].payload
        v = state.slots.get((oid, "attr", name))
        if v is not None:
            return v
        # declared class (annotation / self): resolve methods and class attributes
        if ph.cls is not None:
            c = self.class_lookup(ph.cls, name)
            if c is not None:
                cv = self._cls_slots.get((c, "attr", name), Value())
                outs = []
                for f in cv.pts:
                    fo = self.objects[f]
                    if fo.kind == "function":
                        fi: FuncInfo = fo.payload
                        if fi.is_property:
                            outs.append(self.call_user(f, Value.obj(oid), [], {}, [], [], state, node))
                        elif fi.is_static:
                            outs.append(Value.obj(f))
                        elif fi.is_classmethod:
                            outs.append(Value.obj(self._alloc("bound", f"bound:{f}:cls{ph.cls}", "", True, f"{fo.label} bound", payload=(f, Value.obj(ph.cls)))))
                        else:
                            outs.append(Value.obj(self._alloc("bound", f"bound:{f}:{oid}", "", True, f"{fo.label} bound to {self.objects[oid].label}", payload=(f, Value.obj(oid)))))
                    else:
                        outs.append(Value.obj(f))
                if not cv.pts:
                    outs.append(cv)
                # subclasses overriding the attribute make the lookup depend on the actual class
                if any((sc, "attr", name) in self._cls_slots for sc in self.user_classes if sc != c and self._is_subclass(sc, ph.cls)):
                    self.record_event("dispatch", oid)
                return join_values(outs)
            # attribute defined only in subclasses of the declared class
            if any((sc, "attr", name) in self._cls_slots for sc in self.user_classes if sc != ph.cls and self._is_subclass(sc, ph.cls)):
                self.record_event("dispatch", oid)
        if name in DICT_METHOD_NAMES and ph.cls is None:
            return self.method_value(f"dict.{name}", Value.obj(oid))
        if name in SCALAR_METHOD_NAMES and ph.cls is None:
            return self.method_value(f"str.{name}" if name in STR_METHOD_NAMES else f"unknown.{name}", Value(frozenset(), Param(oid)))
        if ph.cls is None:
            cands = [c for c in self.user_classes if (c, "attr", name) in self._cls_slots]
            if cands:
                outs = []
                for c in cands:
                    for f in self._cls_slots[(c, "attr", name)].pts:
                        fo = self.objects[f]
                        if fo.kind == "function":
                            outs.append(Value.obj(self._alloc("bound", f"bound:{f}:{oid}", "", True, f"{fo.label} bound to {self.objects[oid].label}", payload=(f, Value.obj(oid)))))
                if outs:
                    if len(cands) > 1:
                        self.record_event("dispatch", oid)
                    return join_values(outs)
        row = state.row(oid, "attr")
        if not guarded:
            self.record_symreq(node, Lit(name), row, oid, "attr", guarded, "attr")
        if single_target:
            state.set_row(oid, "attr", row.narrow_in(name, True))
        v = self._path_placeholder(oid, "attr", name, row, state)
        if ph.cls is not None and len(v.pts) == 1 and self.is_ph(next(iter(v.pts))):
            fc = self.field_class(ph.cls, name, state)
            if fc is not None:
                self.objects[next(iter(v.pts))].payload.cls = fc
        if ph.cls is not None and self.field_is_any(ph.cls, name):
            v = v.join(self.dynamic_value(f"{self.objects[oid].label}.{name}", state))
        return v

    def dynamic_value(self, label: str, state: State) -> Value:
        """A value declared dynamically typed (`Any`): accesses on it are not checkable (§8.5)."""
        oid = self._alloc("unknown", f"any:{label}", "", True, f"Any {label}")
        if (oid, "item") not in state.rows:
            state.set_row(oid, "item", Row(top=True))
            state.set_row(oid, "attr", Row(top=True))
        return Value.obj(oid)

    def field_is_any(self, cls: int, name: str) -> bool:
        """Is the attribute declared `Any` (or `object`) in the class body or its bases?"""
        for c in self.mro(cls):
            ci = self.objects[c].payload
            if not isinstance(ci, ClassInfo):
                continue
            anns = ci.__dict__.get("_any_fields")
            if anns is None:
                anns = set()
                for stmt in ci.node.body:
                    if isinstance(stmt, ast.AnnAssign) and isinstance(stmt.target, ast.Name):
                        a = stmt.annotation
                        txt = ast.unparse(a) if not (isinstance(a, ast.Constant) and isinstance(a.value, str)) else a.value
                        txt = txt.strip()
                        if txt in ("Any", "typing.Any", "object", "t.Any"):
                            anns.add(stmt.target.id)
                ci.__dict__["_any_fields"] = anns
            if name in anns:
                return True
        return False

    def field_class(self, cls: int, name: str, state: State) -> Optional[int]:
        """The one user class an attribute of instances of `cls` is known to hold, from the
        class body: `self.name = C(...)` in any method, `name: C` annotations, dataclass
        fields (§8.5).  None when unknown or ambiguous.  Base classes are consulted too."""
        for c in self.mro(cls):
            ci = self.objects[c].payload
            if not isinstance(ci, ClassInfo):
                continue
            if ci.field_classes is None:
                ci.field_classes = self._infer_field_classes(ci, state)
            found = ci.field_classes.get(name)
            if found:
                return next(iter(found)) if len(found) == 1 else None
        return None

    def _infer_field_classes(self, ci: ClassInfo, state: State) -> Dict[str, FrozenSet[int]]:
        out: Dict[str, set] = {}

        def class_of(expr: Optional[ast.expr]) -> Optional[int]:
            if expr is None:
                return None
            # strip Optional[X] / "X" / X | None
            e = expr
            while isinstance(e, ast.Subscript):
                e = e.slice
                if isinstance(e, ast.Tuple) and e.elts:
                    e = e.elts[0]
            if isinstance(e, ast.BinOp) and isinstance(e.op, ast.BitOr):
                e = e.left
            if isinstance(e, ast.Constant) and isinstance(e.value, str):
                try:
                    e = ast.parse(e.value, mode="eval").body
                except SyntaxError:
                    return None
            if isinstance(e, ast.Call):
                e = e.func
            if not isinstance(e, (ast.Name, ast.Attribute)):
                return None
            v = state.slot(ci.module, "attr", e.id) if isinstance(e, ast.Name) else self._static_attr_value(e, ci.module, state)
            for o in v.pts:
                if self.objects[o].kind == "class":
                    return o
            return None

        def note(name: str, expr: Optional[ast.expr]) -> None:
            c = class_of(expr)
            if c is not None:
                out.setdefault(name, set()).add(c)

        for stmt in ci.node.body:
            if isinstance(stmt, ast.AnnAssign) and isinstance(stmt.target, ast.Name):
                note(stmt.target.id, stmt.annotation)
            elif isinstance(stmt, (ast.FunctionDef, ast.AsyncFunctionDef)) and stmt.args.args:
                me = stmt.args.args[0].arg
                for sub in ast.walk(stmt):
                    if isinstance(sub, ast.Assign):
                        for t in sub.targets:
                            if isinstance(t, ast.Attribute) and isinstance(t.value, ast.Name) and t.value.id == me:
                                note(t.attr, sub.value)
                    elif isinstance(sub, ast.AnnAssign):
                        t = sub.target
                        if isinstance(t, ast.Attribute) and isinstance(t.value, ast.Name) and t.value.id == me:
                            note(t.attr, sub.annotation)
                            note(t.attr, sub.value)
        return {k: frozenset(v) for k, v in out.items()}

    def _static_attr_value(self, e: ast.Attribute, module: int, state: State) -> Value:
        if isinstance(e.value, ast.Name):
            base = state.slot(module, "attr", e.value.id)
            acc = Value()
            for o in base.pts:
                if self.objects[o].kind == "module":
                    acc = acc.join(state.slot(o, "attr", e.attr))
            return acc
        return Value()

    def load_unknown_attr(self, oid: int, name: str, node: ast.AST, state: State, guarded: bool) -> Value:
        # an unknown record used like a dict is a dict (optimistic, §10)
        if name in DICT_METHOD_NAMES:
            return self.method_value(f"dict.{name}", Value.obj(oid))
        if name in SCALAR_METHOD_NAMES:
            return self.method_value(f"str.{name}" if name in STR_METHOD_NAMES else f"unknown.{name}", Value.data(f"L{node.lineno}"))
        # class-hierarchy dispatch by method name (§9.6 fallback for unknown receivers)
        cands = [c for c in self.user_classes if (c, "attr", name) in self._cls_slots]
        if cands:
            outs = []
            for c in cands:
                for f in self._cls_slots[(c, "attr", name)].pts:
                    fo = self.objects[f]
                    if fo.kind == "function":
                        outs.append(Value.obj(self._alloc("bound", f"bound:{f}:{oid}", "", True, f"{fo.label} bound to {self.objects[oid].label}", payload=(f, Value.obj(oid)))))
            if outs:
                return join_values(outs)
        row = state.row(oid, "attr")
        v = state.slots.get((oid, "attr", name))
        if v is not None:
            return v
        cls = classify(row, name)
        if not guarded and cls.tier != MAP:
            self.verdict(node, name, cls, oid, "attr")
            if cls.tier == INPUT:
                for s, terms in cls.providers:
                    for t in terms:
                        state.require(s, t, self.access_site(node), caveats=row.mutators)
        ph = self._path_placeholder(oid, "attr", name, row, state)
        if ph.pts:
            return ph       # calling the placeholder is an unknown call (call_one)
        return self.method_value(f"unknown.{name}", Value.obj(oid))

    def store_attr(self, obj: Value, name: str, v: Value, node: ast.AST, state: State) -> None:
        if state.narrowed:
            for k in [k for k in state.narrowed if ("." + name) in k]:
                del state.narrowed[k]
        targets = [o for o in obj.pts if self.objects[o].kind in ("instance", "module", "class", "unknown", "frame", "namespace", "ph")]
        for oid in targets:
            o = self.objects[oid]
            strong = len(targets) == 1 and o.single
            if o.kind == "frame":
                self.models.frame_setattr(self, oid, name, v, node, state, strong)
                continue
            state.set_row(oid, "attr", state.row(oid, "attr").store(name, strong or (len(targets) == 1 and oid in self.fresh_init)))
            state.set_slot(oid, "attr", name, v, strong)
            if o.kind == "class":
                self._cls_slots[(oid, "attr", name)] = v

    # ---------------------------------------------------------------- calls
    def call_value(self, fv: Value, node: ast.AST, state: State, override_args: Optional[List[Value]] = None) -> Value:
        if override_args is not None:
            # a model invoking a callback (probe arguments, virtual rows): analyse concretely
            self.model_call += 1
            try:
                return self._call_value(fv, node, state, override_args)
            finally:
                self.model_call -= 1
        return self._call_value(fv, node, state, None)

    def _call_value(self, fv: Value, node: ast.AST, state: State, override_args: Optional[List[Value]] = None) -> Value:
        args: List[Value] = []
        kwargs: Dict[str, Value] = {}
        star_args: List[Value] = []
        star_kwargs: List[Value] = []
        if override_args is not None:
            args = list(override_args)
        elif isinstance(node, ast.Call):
            for a in node.args:
                if isinstance(a, ast.Starred):
                    star_args.append(self.eval(a.value, state))
                else:
                    args.append(self.eval(a, state))
            for kw in node.keywords:
                if kw.arg is None:
                    star_kwargs.append(self.eval(kw.value, state))
                else:
                    kwargs[kw.arg] = self.eval(kw.value, state)
        site = f"L{getattr(node, 'lineno', 0)}"
        line = getattr(node, "lineno", 0)
        if not fv.pts:
            return self.unknown_call("?", None, args, kwargs, star_args, star_kwargs, state, node)
        targets = sorted(fv.pts)
        if len(targets) == 1:
            return self.call_one(targets[0], args, kwargs, star_args, star_kwargs, state, node)
        pre = state.copy()
        outs: List[Value] = []
        states: List[State] = []
        for oid in targets:
            st = pre.copy()
            outs.append(self.call_one(oid, args, kwargs, star_args, star_kwargs, st, node))
            states.append(st)
        out = states[0]
        for s in states[1:]:
            out = out.join(s)
        state.set_to(out)
        return join_values(outs)

    def call_one(self, oid: int, args: List[Value], kwargs: Dict[str, Value], star_args: List[Value],
                 star_kwargs: List[Value], state: State, node: ast.AST) -> Value:
        o = self.objects[oid]
        if o.kind == "function":
            fi: FuncInfo = o.payload
            if fi.cls is not None and not fi.is_static and args and not fi.is_classmethod:
                # unbound method called with explicit self: Base.method(self, ...)
                return self.call_user(oid, args[0], args[1:], kwargs, star_args, star_kwargs, state, node)
            return self.call_user(oid, None, args, kwargs, star_args, star_kwargs, state, node)
        if o.kind == "bound":
            f, self_v = o.payload
            return self.call_user(f, self_v, args, kwargs, star_args, star_kwargs, state, node)
        if o.kind == "class":
            return self.construct(oid, args, kwargs, star_args, star_kwargs, state, node)
        if o.kind == "external":
            qual = str(o.payload)
            call = Call(self, qual, None, args, kwargs, star_args, star_kwargs, state, node, f"L{node.lineno}", node.lineno)
            m = self.models.lookup(qual)
            if m is not None:
                return m(call)
            return self.unknown_call(qual, None, args, kwargs, star_args, star_kwargs, state, node)
        if o.kind == "method":
            qual, recv = o.payload
            call = Call(self, qual, recv, args, kwargs, star_args, star_kwargs, state, node, f"L{node.lineno}", node.lineno)
            m = self.models.lookup(qual)
            if m is not None:
                return m(call)
            if qual.startswith("class."):
                return self.unknown_call(qual, recv, args, kwargs, star_args, star_kwargs, state, node)
            return self.unknown_call(qual, recv, args, kwargs, star_args, star_kwargs, state, node)
        if o.kind == "unknown":
            return self.unknown_call(f"call of {o.label}", Value.obj(oid), args, kwargs, star_args, star_kwargs, state, node)
        if o.kind == "ph":
            self.record_event("call", oid)
            return self.unknown_call(f"call of {o.label}", Value.obj(oid), args, kwargs, star_args, star_kwargs, state, node)
        if o.kind == "instance":
            m = self.class_lookup(o.payload, "__call__")
            if m is not None:
                f = self._cls_slots.get((m, "attr", "__call__"), Value())
                return join_values(self.call_user(x, Value.obj(oid), args, kwargs, star_args, star_kwargs, state, node)
                                   for x in f.pts if self.objects[x].kind == "function") if f.pts else Value()
            return self.unknown_call(f"call of {o.label}", Value.obj(oid), args, kwargs, star_args, star_kwargs, state, node)
        return self.unknown_call(f"call of {o.label}", None, args, kwargs, star_args, star_kwargs, state, node)

    def unknown_call(self, qual: str, recv: Optional[Value], args: List[Value], kwargs: Dict[str, Value],
                     star_args: List[Value], star_kwargs: List[Value], state: State, node: ast.AST) -> Value:
        site = f"{qual}@L{getattr(node, 'lineno', 0)}"
        name = qual.split(".")[-1]
        if name in NOEFFECT_METHOD_NAMES and (recv is None or not self.record_objects(recv)):
            return Value.data(f"L{getattr(node, 'lineno', 0)}")
        if state.narrowed:
            state.narrowed = {}
        vals = list(args) + list(kwargs.values()) + list(star_args) + list(star_kwargs)
        if recv is not None:
            vals.append(recv)
        touched = self.reachable_records(vals, state, depth=1)
        for oid, ns in touched[:MAX_MUTATED]:
            state.set_row(oid, ns, state.row(oid, ns).mutate(site))
        arg_recs = [(oid, ns) for oid, ns in touched if any(oid in v.pts for v in vals)]
        rid = self.alloc("unknown", f"ret:{site}", state, label=f"result of {site}", ns="item", single=True)
        if arg_recs:
            row = join_rows(state.row(oid, ns) for oid, ns in arg_recs)
            row = replace(row, definite=frozenset(), absent=frozenset(), mutators=row.mutators | {site},
                          dterms=frozenset(), lterms=row.lterms | row.dterms,
                          pstores=frozenset((r, False, l) for r, st_, l in row.pstores))
            state.set_row(rid, "item", row)
            state.set_row(rid, "attr", Row(mutators=frozenset({site})))
            for oid, ns in arg_recs:
                for k, v in state.obj_slots(oid, ns):
                    state.set_slot(rid, "item", k, v, strong=False)
        else:
            src = Source(site, "call")
            state.set_row(rid, "item", Row(sources=frozenset({(src, ())})))
            state.set_row(rid, "attr", Row(sources=frozenset({(src, ())})))
        self.note(f"unknown call {site}")
        return Value.obj(rid)

    def reachable_records(self, vals: List[Value], state: State, depth: int = 2) -> List[Tuple[int, str]]:
        out: List[Tuple[int, str]] = []
        seen: set = set()
        work = [(oid, 0) for v in vals for oid in v.pts]
        while work:
            oid, d = work.pop()
            if oid in seen:
                continue
            seen.add(oid)
            o = self.objects[oid]
            if o.kind in ("dict", "frame", "unknown", "ph"):
                out.append((oid, "item"))
            if o.kind in ("instance", "unknown", "ph"):
                out.append((oid, "attr"))
            if d < depth:
                for _nk, v in state.slots.for_obj(oid):
                    work.extend((x, d + 1) for x in v.pts)
        return out

    def construct(self, cls: int, args: List[Value], kwargs: Dict[str, Value], star_args: List[Value],
                  star_kwargs: List[Value], state: State, node: ast.AST) -> Value:
        ci: ClassInfo = self.objects[cls].payload
        row = Row(sources=frozenset({(Source(f"instance {ci.name}@L{node.lineno}", "class"), ())})) if ci.open_base else Row()
        inst = self.alloc("instance", node, state, label=f"{ci.name}@L{node.lineno}", payload=cls, ns="attr", row=row)
        ns = "item" if ci.typed_dict else "attr"
        if ci.typed_dict:
            state.set_row(inst, "item", Row())
        if ci.dataclass_fields is not None:
            fields = ci.dataclass_fields
            for i, (fname, default) in enumerate(fields):
                v: Optional[Value] = None
                if i < len(args):
                    v = args[i]
                elif fname in kwargs:
                    v = kwargs[fname]
                elif default is not None:
                    v = self.eval(default, state) if not (isinstance(default, ast.Call) and _dotted_last(default.func) == "field") else Value.data("field")
                elif star_kwargs:
                    v = join_values(state.slot(x, "item", fname) for sk in star_kwargs for x in sk.pts)
                if v is None and not star_kwargs:
                    if not ci.typed_dict:
                        self.verdict(node, fname, Classification(MISSING, note="dataclass field not supplied"), inst, ns)
                    continue
                state.set_row(inst, ns, state.row(inst, ns).store(fname, True))
                state.set_slot(inst, ns, fname, v or Value.data("field"), True)
            if ci.typed_dict:
                for sk in star_kwargs:
                    self.copy_into(inst, "item", sk, state, strong=True)
            return Value.obj(inst)
        init = self.class_lookup(cls, "__init__")
        if init is not None:
            fv = self._cls_slots.get((init, "attr", "__init__"), Value())
            # while the constructor runs, the new object has no other references: an attribute
            # it sets is set on every object of this allocation site (keys strong, values weak)
            self.fresh_init.add(inst)
            try:
                for f in fv.pts:
                    if self.objects[f].kind == "function":
                        self.call_user(f, Value.obj(inst), args, kwargs, star_args, star_kwargs, state, node)
            finally:
                self.fresh_init.discard(inst)
        return Value.obj(inst)

    def call_user(self, foid: int, self_v: Optional[Value], args: List[Value], kwargs: Dict[str, Value],
                  star_args: List[Value], star_kwargs: List[Value], state: State, node: ast.AST,
                  entry: bool = False) -> Value:
        if state.narrowed:
            state.narrowed = {}
        fo = self.objects[foid]
        fi: FuncInfo = fo.payload
        fnode = fi.node
        site = f"L{getattr(node, 'lineno', 0)}"
        self.analysed_funcs.add(foid)
        # --- symbolic summaries (§7.3/§9): analyse the body once, instantiate per call ---
        if self.use_summaries and not self.model_call:
            summ = self.summarise(foid, state)
            if summ is None:
                # recursion through the symbolic path: ⊥ summary (no effects, no result)
                return Value()
            # context for allocations cloned from the summary (§6.1)
            if self_v is not None and len(self_v.pts) == 1 and self.objects[next(iter(self_v.pts))].kind == "instance":
                recv = self.objects[next(iter(self_v.pts))]
                if not recv.single:
                    call_ctx = site
                elif recv.ctx.startswith("o") and recv.ctx[1:].isdigit():
                    call_ctx = recv.ctx
                elif self.site_clones.get(recv.site, 0) >= MAX_CLONES:
                    call_ctx = ""
                else:
                    call_ctx = f"o{recv.id}"
                ctx_single = recv.single
            else:
                call_ctx = site
                ctx_single = True
            bound = self.compute_bindings(fi, self_v, args, kwargs, star_args, star_kwargs, state, node)
            bind: Dict[int, Value] = {}
            for name, ph in summ.ph_by_name.items():
                if name in bound:
                    bv = bound[name]
                    pcls = self.objects[ph].payload.cls
                    if pcls is not None and bv.pts:
                        # declared parameter class (§8.5): instances of unrelated classes are
                        # not what the parameter denotes
                        kept = frozenset(o for o in bv.pts if self._may_be_instance(o, pcls))
                        if kept != bv.pts:
                            bv = Value(kept, bv.kv)
                    bind[ph] = bv
            if summ.ph_self is not None and self_v is not None:
                bind[summ.ph_self] = self_v
            if not self.should_tabulate(summ, bind, state):
                saved_chain = self.call_chain
                self.call_chain = list(chain_extend(tuple(saved_chain), (getattr(node, "lineno", 0),)))
                try:
                    return self.instantiate(summ, bind, state, node, call_ctx, ctx_single)
                finally:
                    self.call_chain = saved_chain
        # --- per-call-site (tabulated) analysis ---
        # recursion is tabulated per (function, call site), but at most MAX_ACTIVE nested
        # activations of one function are kept apart: with k recursive call sites in one
        # body an unbounded per-site cut would nest k^k analyses
        key = (foid, site if not entry else f"entry:{site}")
        act = self.active_by_func.get(foid, [])
        if key not in self.active and len(act) >= MAX_ACTIVE:
            key = act[-1]
        if key in self.active:
            self.rec_hit.add(key)
            # widen the entry arguments with the ones of this recursive call (§9.7)
            ent = self.rec_entry.get(key)
            if ent is not None:
                e_self, e_args, e_kwargs = ent
                n_self = e_self.join(self_v) if (e_self is not None and self_v is not None) else e_self
                n_args = [a.join(b) if i < len(args) else a for i, (a, b) in enumerate(zip(e_args, args))] + list(e_args[len(args):])
                if len(args) > len(e_args):
                    n_args += list(args[len(e_args):])
                n_kwargs = dict(e_kwargs)
                for k, v in kwargs.items():
                    n_kwargs[k] = n_kwargs[k].join(v) if k in n_kwargs else v
                n_args = [_widen_value(v) for v in n_args]
                n_kwargs = {k: _widen_value(v) for k, v in n_kwargs.items()}
                if (n_self, n_args, n_kwargs) != (e_self, e_args, e_kwargs):
                    self.rec_entry[key] = (n_self, n_args, n_kwargs)
                    self.rec_widened.add(key)
            prev = self.rec_table.get(key)
            if prev is not None:
                ret, exit_state, fp = prev
                state.join_heap(exit_state, fp)
                return ret
            return Value()
        # summary cache: same callee, same call site, same observable input → replay the effects
        sig = self.call_signature(foid, site, entry, self_v, args, kwargs, star_args, star_kwargs, state)
        hit = self.call_cache.get(sig) if sig is not None else None
        if hit is not None:
            ret, fp_state, fp, verdicts = hit
            rk, sk, ek = fp
            for k in rk:
                if k in fp_state.rows:
                    state.rows[k] = fp_state.rows[k]
            for k in sk:
                if k in fp_state.slots:
                    state.slots[k] = fp_state.slots[k]
            for k in ek:
                if k in fp_state.elems:
                    state.elems[k] = fp_state.elems[k]
            for src, d in fp_state.reqs.items():
                for k, info in d.items():
                    state.require(src, k, next(iter(info.sites)) if info.sites else site, info.alternatives, info.caveats, info.alt_keys)
                    if not info.always:
                        tgt = state.reqs.writable(src)
                        cur = tgt[k]
                        tgt[k] = ReqInfo(False, cur.sites, cur.alternatives, cur.caveats, cur.alt_keys)
            chain = chain_extend(tuple(self.call_chain), (getattr(node, "lineno", 0),))
            for v in verdicts:
                full = chain_extend(chain, v.chain)
                dk = (v.file, v.line, v.col, key_str(v.key), v.tier, v.oid, full)
                if dk not in self._verdict_keys:
                    self._verdict_keys.add(dk)
                    self.verdicts.append(Verdict(v.line, v.col, v.key, v.tier, v.obj, full, v.note, v.providers, v.ns, v.text, v.oid, v.file))
            self.summaries.setdefault((fi.qualname, site), (ret, {o: state.row(o, "item") for o in ret.pts if self.objects[o].kind in RECORD_KINDS or self.objects[o].kind == "frame"}))
            return ret
        # context for allocations inside the callee (§6.1): 1-object sensitivity for methods,
        # bounded to depth 1 (a receiver that was itself allocated under a receiver context
        # passes that context on instead of opening a new one)
        if self_v is not None and len(self_v.pts) == 1 and self.objects[next(iter(self_v.pts))].kind == "instance":
            recv = self.objects[next(iter(self_v.pts))]
            if not recv.single:
                ctx = site            # a summary receiver (allocated in a loop) is no basis for cloning
            elif recv.ctx.startswith("o") and recv.ctx[1:].isdigit():
                ctx = recv.ctx
            elif self.site_clones.get(recv.site, 0) >= MAX_CLONES:
                ctx = ""
            else:
                ctx = f"o{recv.id}"
            ctx_single = recv.single
        else:
            ctx = site
            ctx_single = True
        fr_single = self.frame.single and self.frame.loop_depth == 0 and ctx_single
        new_fr = Frame("function", fi.scope, fi.module, ctx, fr_single, func=fo, label=fi.qualname)
        # cells
        for name in fi.scope.cells:
            cid = self._alloc("cell", f"cell:{fi.qualname}.{name}", ctx, fr_single, f"cell {name}")
            new_fr.cells[name] = cid
        new_fr.cells.update(fi.closure)
        entry_env: Dict[str, Value] = {}
        saved_env, saved_ret = state.env, state.ret
        state.env = entry_env
        state.ret = None
        self.frames.append(new_fr)
        saved_chain_list = self.call_chain
        self.call_chain = list(chain_extend(tuple(saved_chain_list), (getattr(node, "lineno", 0),)))
        self.active[key] = True
        self.active_by_func.setdefault(foid, []).append(key)
        self.rec_entry[key] = (self_v, list(args), dict(kwargs))
        verdict_mark = len(self.verdicts)
        chain_len = len(self.call_chain)
        try:
            pre_bind = state.copy()
            self._bind_params(fi, self_v, args, kwargs, star_args, star_kwargs, state, node)
            entry_state = state.copy()
            ret = Value()
            for _ in range(REC_MAX_ITER):
                st = entry_state.copy()
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
                # function exit = join of the fall-through state and every return point
                if st.exits is not None:
                    ex = st.exits
                    st.exits = None
                    st = st.join(ex) if st.reachable else ex
                    st.exits = None
                st.ret = ret
                st.reachable = True
                if key in self.rec_widened:
                    # a recursive call used different arguments: restart with the widened entry
                    self.rec_widened.discard(key)
                    w_self, w_args, w_kwargs = self.rec_entry[key]
                    self.rec_table[key] = (ret, st.copy(), st.footprint(entry_state))
                    state.set_to(pre_bind.copy())
                    self._bind_params(fi, w_self, w_args, w_kwargs, star_args, star_kwargs, state, node)
                    entry_state = state.copy()
                    continue
                if key in self.rec_hit:
                    prev = self.rec_table.get(key)
                    if prev is not None and prev[0] == ret and prev[1].same(st):
                        break
                    self.rec_table[key] = (ret, st.copy(), st.footprint(entry_state))
                    continue
                break
            if sig is not None and key not in self.rec_hit:
                # requirements added by the callee only (relative to entry)
                rel = State()
                for src, d in st.reqs.items():
                    base = pre_bind.reqs.inner(src) or {}
                    for k, info in d.items():
                        if base.get(k) != info:
                            rel.reqs.writable(src)[k] = info
                fp = st.footprint(pre_bind)
                rk, sk, ek = fp
                fp_state = State()
                for k in rk:
                    if k in st.rows:
                        fp_state.rows[k] = st.rows[k]
                for k in sk:
                    if k in st.slots:
                        fp_state.slots[k] = st.slots[k]
                for k in ek:
                    if k in st.elems:
                        fp_state.elems[k] = st.elems[k]
                fp_state.reqs = rel.reqs
                cur_chain = tuple(self.call_chain)
                rel_verdicts = tuple(Verdict(v.line, v.col, v.key, v.tier, v.obj,
                                             v.chain[chain_len:] if v.chain[:chain_len] == cur_chain else v.chain,
                                             v.note, v.providers, v.ns, v.text, v.oid, v.file)
                                     for v in self.verdicts[verdict_mark:])
                self.call_cache[sig] = (ret, fp_state, fp, rel_verdicts)
            state.set_to(st)
            gen = fi.__dict__.get("generator")
            if gen is not None and isinstance(fnode, (ast.FunctionDef, ast.AsyncFunctionDef)) and _is_generator(fnode):
                ret = Value.obj(gen)
            self.summaries[(fi.qualname, ctx)] = (ret, {o: state.row(o, "item") for o in ret.pts if self.objects[o].kind in RECORD_KINDS or self.objects[o].kind == "frame"})
        finally:
            self.active.pop(key, None)
            lst = self.active_by_func.get(foid)
            if lst:
                lst.pop()
            self.rec_hit.discard(key)
            self.rec_entry.pop(key, None)
            self.rec_widened.discard(key)
            self.call_chain = saved_chain_list
            self.frames.pop()
            state.env, state.ret = saved_env, saved_ret
        return ret

    def in_method_frame(self, foid: int, self_v: Value, args: List[Value], state: State, fn: Callable[[], None]) -> None:
        fo = self.objects[foid]
        fi: FuncInfo = fo.payload
        new_fr = Frame("function", fi.scope, fi.module, self.frame.ctx, False, func=fo, label=fi.qualname)
        new_fr.cells.update(fi.closure)
        saved_env = state.env
        state.env = {}
        self.frames.append(new_fr)
        try:
            fake = ast.Call(func=ast.Name(id=fi.name, ctx=ast.Load()), args=[], keywords=[])
            fake.lineno = fi.node.lineno
            fake.col_offset = 0
            self._bind_params(fi, self_v, args, {}, [], [], state, fake)
            fn()
        finally:
            self.frames.pop()
            state.env = saved_env

    def call_signature(self, foid: int, site: str, entry: bool, self_v: Optional[Value], args: List[Value],
                       kwargs: Dict[str, Value], star_args: List[Value], star_kwargs: List[Value], state: State):
        """Hashable description of everything a callee can observe, or None if too large to bother."""
        fi: FuncInfo = self.objects[foid].payload
        roots: List[int] = []
        vals: List[Value] = list(args) + list(kwargs.values()) + list(star_args) + list(star_kwargs)
        if self_v is not None:
            vals.append(self_v)
        for cid in fi.closure.values():
            vals.append(state.slot(cid, "cell", "v"))
        for v in vals:
            roots.extend(v.pts)
        roots.append(fi.module)
        seen: List[int] = []
        seen_set: set = set()
        work = [(o, 0) for o in roots]
        while work:
            oid, d = work.pop()
            if oid in seen_set:
                continue
            if len(seen) > 400:
                return None
            seen_set.add(oid)
            seen.append(oid)
            if d >= 3:
                continue
            for _nk, v in state.slots.for_obj(oid):
                for x in v.pts:
                    if x not in seen_set:
                        work.append((x, d + 1))
        heap = []
        for oid in sorted(seen):
            r_item = state.rows.get((oid, "item"))
            r_attr = state.rows.get((oid, "attr"))
            slots = tuple(sorted(((nk, v) for nk, v in state.slots.for_obj(oid)), key=lambda kv: repr(kv[0])))
            heap.append((oid, r_item, r_attr, slots, state.elems.get(oid), self.objects[oid].single))
        guards = tuple(frozenset(g) for g in self.guards)
        return (foid, site, entry, self_v, tuple(args), tuple(sorted(kwargs.items())), tuple(star_args),
                tuple(star_kwargs), tuple(heap), guards, self.frame.single and self.frame.loop_depth == 0,
                self.frame.ctx)

    def _bind_params(self, fi: FuncInfo, self_v: Optional[Value], args: List[Value], kwargs: Dict[str, Value],
                     star_args: List[Value], star_kwargs: List[Value], state: State, node: ast.AST) -> None:
        for name, v in self.compute_bindings(fi, self_v, args, kwargs, star_args, star_kwargs, state, node).items():
            self.store_name(name, v, state)

    def compute_bindings(self, fi: FuncInfo, self_v: Optional[Value], args: List[Value], kwargs: Dict[str, Value],
                         star_args: List[Value], star_kwargs: List[Value], state: State, node: ast.AST) -> Dict[str, Value]:
        """Parameter name -> argument value (allocating the *args tuple / **kwargs dict in `state`)."""
        a = fi.node.args
        positional = [p.arg for p in list(a.posonlyargs) + list(a.args)]
        kwonly = [p.arg for p in a.kwonlyargs]
        args = list(args)
        if self_v is not None and positional:
            args = [self_v] + args
        if star_args:
            extra = join_values(self.elem_of(sa, node, state) for sa in star_args)
        bound: Dict[str, Value] = {}
        for i, name in enumerate(positional):
            if i < len(args):
                bound[name] = args[i]
            elif star_args:
                bound[name] = extra
        rest_pos = args[len(positional):]
        for name in positional + kwonly:
            if name in kwargs:
                bound[name] = kwargs[name]
        used_kw = set(bound) & set(kwargs)
        for name in positional + kwonly:
            if name not in bound:
                if name in fi.defaults:
                    bound[name] = fi.defaults[name]
                elif star_kwargs:
                    vals = [state.slot(x, "item", name) for sk in star_kwargs for x in sk.pts]
                    bound[name] = join_values(vals) if vals else Value.data("kw")
                    if not any(name in state.row(x, "item").present for sk in star_kwargs for x in sk.pts):
                        # might come from an open **mapping
                        bound[name] = bound[name].join(Value.data("kw"))
                else:
                    bound[name] = Value.data(f"missing arg {name}")
        if a.vararg is not None:
            tid = self.alloc("tuple", f"varargs:{fi.qualname}", state, label=f"*{a.vararg.arg}")
            acc = join_values(rest_pos) if rest_pos else Value()
            if star_args:
                acc = acc.join(extra)
            state.set_slot(tid, "elem", "*", Value(acc.pts, DATA), True)
            state.elems[tid] = acc.kv if rest_pos else DATA
            self.objects[tid].literal_len = len(rest_pos) if not star_args else None
            for i, v in enumerate(rest_pos):
                state.set_slot(tid, "pos", i, v, True)
            bound[a.vararg.arg] = Value.obj(tid)
        if a.kwarg is not None:
            did = self.alloc("dict", f"kwargs:{fi.qualname}", state, label=f"**{a.kwarg.arg}")
            row = Row()
            for name, v in kwargs.items():
                if name in used_kw:
                    continue
                row = row.store(name, True)
                state.set_slot(did, "item", name, v, True)
            state.set_row(did, "item", row)
            for sk in star_kwargs:
                self.copy_into(did, "item", sk, state, strong=True, kill=frozenset(positional + kwonly))
            bound[a.kwarg.arg] = Value.obj(did)
        return bound

    # --------------------------------------------------------- utilities
    def lit_keys(self, v: Value, state: State) -> Optional[FrozenSet[Any]]:
        """Finite literal key set denoted by a value (string/list/tuple/set of literals), or None."""
        if isinstance(v.kv, (Lit, Lits)) and not v.pts:
            return kv_literals(v.kv)
        out: set = set()
        for oid in v.pts:
            o = self.objects[oid]
            if o.kind == "ph":
                self.record_event("concrete", oid)
                return None
            if o.kind in ("list", "tuple", "set"):
                kv = state.elems.get(oid)
                if isinstance(kv, (Lit, Lits)):
                    out |= kv_literals(kv)
                else:
                    return None
            elif o.kind == "dict":
                row = state.row(oid, "item")
                if row.closed and row.present == row.definite:
                    out |= row.present
                else:
                    return None
            else:
                return None
        return frozenset(out) if v.pts else None

    def literal_mapping(self, v: Value, state: State) -> Optional[Dict[Any, Any]]:
        out: Dict[Any, Any] = {}
        for oid in v.pts:
            o = self.objects[oid]
            if o.kind == "ph":
                self.record_event("concrete", oid)
                return None
            if o.kind != "dict":
                return None
            row = state.row(oid, "item")
            if row.open or row.present != row.definite:
                return None
            for k in row.present:
                sv = state.slot(oid, "item", k)
                if isinstance(sv.kv, Lit):
                    out[k] = sv.kv.key
                else:
                    return None
        return out if v.pts else None

    def copy_object(self, oid: int, node: ast.AST, state: State, deep: bool, _seen: Optional[Dict[int, int]] = None) -> int:
        o = self.objects[oid]
        _seen = _seen if _seen is not None else {}
        if oid in _seen:
            return _seen[oid]
        kind = o.kind if o.kind in ("dict", "list", "tuple", "set", "instance", "frame") else "dict"
        tag = "deep" if deep else "copy"
        nid = self.alloc(kind, f"{tag}:{self.site_of(node)}:{kind}", state, label=f"{tag} of {o.label}@L{node.lineno}",
                         payload=o.payload, ns="attr" if kind == "instance" else "item")
        _seen[oid] = nid
        self.objects[nid].literal_len = o.literal_len
        for ns in ("item", "attr"):
            if (oid, ns) in state.rows:
                prev = state.rows.get((nid, ns))
                state.set_row(nid, ns, state.row(oid, ns) if prev is None or self.objects[nid].single else prev.join(state.row(oid, ns)))
        if oid in state.elems:
            state.elems[nid] = state.elems[oid]
        for (ns, k), v in list(state.slots.for_obj(oid)):
            if deep and len(_seen) < 64:
                newp = frozenset(self.copy_object(x, node, state, True, _seen) if self.objects[x].kind in ("dict", "list", "tuple", "set", "instance") else x for x in v.pts)
                v = Value(newp, v.kv)
            state.set_slot(nid, ns, k, v, True)
        return nid


def lens_apply_chain(chain, k):
    from .domain import lens_apply
    return lens_apply(chain, k)


def _widen_value(v: Value) -> Value:
    """Scalar arguments that vary across recursive calls become Data (keeps the fixpoint finite)."""
    if isinstance(v.kv, Lits) and len(v.kv.keys) > 2:
        return Value(v.pts, Data("recursion"))
    return v


def _contains_jump(body: Sequence[ast.stmt]) -> bool:
    """True if the loop body has a break/continue targeting this loop (prevents unrolling).
    `return`/`raise` are fine: they just make the rest of the unrolled path unreachable."""
    def visit(n: ast.AST) -> bool:
        if isinstance(n, (ast.Break, ast.Continue)):
            return True
        if isinstance(n, (ast.For, ast.While, ast.AsyncFor, ast.FunctionDef, ast.AsyncFunctionDef, ast.Lambda, ast.ClassDef)):
            # a nested loop owns its own break/continue; but its `else` and the loop header belong to us
            if isinstance(n, (ast.For, ast.While, ast.AsyncFor)):
                return any(visit(c) for c in n.orelse)
            return False
        return any(visit(c) for c in ast.iter_child_nodes(n))
    return any(visit(s) for s in body)


def _unused_contains_jump(body: Sequence[ast.stmt]) -> bool:
    for s in body:
        for n in ast.walk(s):
            if isinstance(n, (ast.Break, ast.Continue, ast.Return, ast.Raise)):
                return True
    return False


def _is_main_guard(test: ast.expr) -> bool:
    return (isinstance(test, ast.Compare) and isinstance(test.left, ast.Name) and test.left.id == "__name__"
            and len(test.ops) == 1 and isinstance(test.ops[0], ast.Eq)
            and isinstance(test.comparators[0], ast.Constant) and test.comparators[0].value == "__main__")


def _is_generator(fn: ast.AST) -> bool:
    for n in ast.walk(fn):
        if isinstance(n, (ast.Yield, ast.YieldFrom)):
            return True
    return False


def _dotted_last(n: ast.expr) -> str:
    if isinstance(n, ast.Name):
        return n.id
    if isinstance(n, ast.Attribute):
        return n.attr
    if isinstance(n, ast.Call):
        return _dotted_last(n.func)
    return ast.unparse(n)


BUILTIN_NAMES = set(dir(__builtins__)) if isinstance(__builtins__, dict) is False else set(__builtins__.keys())  # type: ignore[union-attr]
BUILTIN_NAMES |= {"dict", "list", "set", "tuple", "len", "print", "str", "int", "float", "bool", "range",
                  "enumerate", "zip", "sorted", "min", "max", "sum", "any", "all", "isinstance", "type",
                  "open", "getattr", "setattr", "hasattr", "repr", "iter", "next", "abs", "round", "map",
                  "filter", "reversed", "id", "hash", "object", "Exception", "KeyError", "ValueError",
                  "TypeError", "AttributeError", "IndexError", "LookupError", "RuntimeError", "property",
                  "staticmethod", "classmethod", "super", "vars", "format", "frozenset", "bytes", "input",
                  "NotImplementedError", "StopIteration", "BaseException", "callable", "chr", "ord", "divmod",
                  "pow", "slice", "issubclass", "memoryview", "bytearray", "complex", "globals", "locals",
                  "exec", "eval", "compile", "delattr", "help", "hex", "oct", "bin", "ascii", "breakpoint",
                  "FileNotFoundError", "OSError", "IOError", "ZeroDivisionError", "AssertionError",
                  "NameError", "ImportError", "ModuleNotFoundError", "PermissionError", "TimeoutError",
                  "ConnectionError", "StopAsyncIteration", "GeneratorExit", "KeyboardInterrupt", "SystemExit",
                  "Warning", "UserWarning", "DeprecationWarning", "__name__", "__file__", "__doc__", "NotImplemented",
                  "Ellipsis", "None", "True", "False", "anext", "aiter"}
