"""Library models (SPECIFICATION.md §11).

A model is `f(call) -> Value`; it may mutate `call.state`.  Models are keyed by
qualified name (`json.load`, `dict.get`, `DataFrame.merge`, `builtins.len`) with
prefix fallbacks (`Series.*`, `str.*`, `DataFrame.*`).
"""
from __future__ import annotations

import ast
from typing import Any, Callable, Dict, FrozenSet, List, Optional

from .domain import (DATA, INPUT, MAP, MISSING, PROVEN, Classification, Data, KeysOf, Kill, Lit, Lits,
                     RenameFn, RenameMap, Row, Source, Value, classify, join_rows, join_values,
                     kv_join, kv_literals, kv_map)
from .state import RECORD_KINDS

DF_METHODS = {
    "merge", "join", "rename", "drop", "assign", "copy", "head", "tail", "sample", "sort_values",
    "sort_index", "drop_duplicates", "fillna", "dropna", "astype", "reset_index", "set_index", "groupby",
    "apply", "applymap", "map", "pipe", "iterrows", "itertuples", "to_dict", "to_csv", "to_parquet",
    "to_excel", "to_json", "to_sql", "to_numpy", "to_records", "to_string", "to_html", "to_markdown",
    "query", "eval", "where", "mask", "pivot", "pivot_table", "melt", "stack", "unstack", "transpose",
    "explode", "describe", "corr", "cov", "agg", "aggregate", "filter", "select_dtypes", "insert", "pop",
    "nlargest", "nsmallest", "isna", "isnull", "notna", "notnull", "round", "abs", "clip", "replace",
    "shift", "diff", "cumsum", "cumprod", "cummax", "cummin", "rank", "pct_change", "rolling", "expanding",
    "ewm", "resample", "mean", "sum", "min", "max", "count", "median", "std", "var", "nunique", "idxmax",
    "idxmin", "any", "all", "value_counts", "memory_usage", "info", "equals", "eq", "ne", "lt", "le", "gt",
    "ge", "add", "sub", "mul", "div", "truediv", "floordiv", "mod", "pow", "dot", "squeeze", "transform",
    "reindex", "reindex_like", "set_axis", "add_prefix", "add_suffix", "align", "combine_first", "update",
    "interpolate", "bfill", "ffill", "pad", "backfill", "convert_dtypes", "infer_objects", "iloc", "loc",
    "at", "iat", "xs", "get", "items", "keys", "lookup", "first", "last", "nth", "take", "truncate",
    "between_time", "at_time", "asfreq", "tz_localize", "tz_convert", "plot", "hist", "boxplot", "style",
    "compare", "duplicated", "isin", "swaplevel", "droplevel", "reorder_levels", "sort", "sem", "skew",
    "kurt", "prod", "quantile", "mode", "cumcount", "ngroup", "head", "corrwith", "to_period", "to_timestamp",
    "assign", "columns", "index", "shape", "values", "dtypes", "T", "empty", "size", "ndim", "axes",
}
DF_ATTRS = {"columns", "index", "shape", "values", "dtypes", "T", "empty", "size", "ndim", "axes", "style",
            "plot", "iloc", "loc", "at", "iat", "str", "dt", "cat", "attrs", "flags"}

FRAME_PRODUCERS = {
    "pandas.read_csv": "read_csv", "pandas.read_excel": "read_excel", "pandas.read_parquet": "read_parquet",
    "pandas.read_sql": "read_sql", "pandas.read_sql_query": "read_sql", "pandas.read_sql_table": "read_sql",
    "pandas.read_json": "read_json", "pandas.read_table": "read_table", "pandas.read_feather": "read_feather",
    "pandas.read_pickle": "read_pickle", "pandas.read_html": "read_html", "pandas.read_fwf": "read_fwf",
    "pandas.read_stata": "read_stata", "pandas.read_hdf": "read_hdf", "pandas.read_orc": "read_orc",
    "pandas.read_xml": "read_xml", "pandas.read_clipboard": "read_clipboard", "pandas.read_gbq": "read_gbq",
}
OPEN_FRAME_OPS = {"pivot", "pivot_table", "stack", "unstack", "transpose", "T", "describe", "corr", "cov",
                  "agg", "aggregate", "explode_unknown", "get_dummies", "crosstab", "json_normalize", "melt_unknown"}
COPY_FRAME_OPS = {"copy", "head", "tail", "sample", "sort_values", "sort_index", "drop_duplicates", "fillna",
                  "dropna", "astype", "query", "where", "mask", "round", "abs", "clip", "replace", "shift",
                  "diff", "cumsum", "cumprod", "cummax", "cummin", "rank", "pct_change", "isna", "isnull",
                  "notna", "notnull", "convert_dtypes", "infer_objects", "interpolate", "bfill", "ffill", "pad",
                  "backfill", "eq", "ne", "lt", "le", "gt", "ge", "add", "sub", "mul", "div", "truediv",
                  "floordiv", "mod", "pow", "applymap", "map", "take", "truncate", "between_time", "at_time",
                  "asfreq", "tz_localize", "tz_convert", "first", "last", "isin", "duplicated_frame", "combine_first",
                  "align", "reindex_like", "sort", "squeeze_frame", "compare_frame", "nlargest", "nsmallest",
                  "to_period", "to_timestamp", "swaplevel", "droplevel", "reorder_levels"}
SERIES_RESULT_OPS = {"mean", "sum", "min", "max", "count", "median", "std", "var", "nunique", "idxmax", "idxmin",
                     "any", "all", "value_counts", "memory_usage", "sem", "skew", "kurt", "prod", "quantile",
                     "mode", "squeeze", "duplicated", "dot", "corrwith", "size", "cumcount", "ngroup"}
NOEFFECT_OPS = {"to_csv", "to_parquet", "to_excel", "to_json", "to_sql", "to_numpy", "to_records", "to_string",
                "to_html", "to_markdown", "info", "equals", "plot", "hist", "boxplot", "to_pickle", "to_feather",
                "to_clipboard", "to_latex", "to_stata", "to_hdf", "to_xml", "to_orc"}

ALLOWLIST_PREFIXES = ("logging.", "print", "warnings.", "time.", "datetime.", "re.", "math.", "random.", "os.",
                      "shutil.", "glob.", "subprocess.", "io.", "tempfile.", "csv.writer", "csv.reader", "sys.",
                      "traceback.", "inspect.", "importlib.", "platform.", "socket.", "ssl.", "signal.", "threading.",
                      "multiprocessing.", "concurrent.", "asyncio.", "queue.", "heapq.", "bisect.", "array.", "zlib.",
                      "gzip.", "bz2.", "lzma.", "zipfile.", "tarfile.", "hmac.", "secrets.", "binascii.", "codecs.",
                      "unicodedata.", "locale.", "gettext.", "calendar.", "difflib.", "pprint.pp", "textwrap.",
                      "typing.", "pathlib.", "sys.", "uuid.", "hashlib.", "base64.", "textwrap.", "string.",
                      "itertools.count", "numpy.", "np.", "pd.api", "pytest.", "unittest.", "abc.", "enum.",
                      "decimal.", "fractions.", "statistics.", "operator.", "struct.", "builtins.NotImplementedError")

STR_FNS = {"upper", "lower", "strip", "title", "lstrip", "rstrip", "capitalize", "casefold", "swapcase"}


class Registry:
    def __init__(self) -> None:
        self.table: Dict[str, Callable[["Call"], Value]] = {}
        self._install()

    def register(self, *names: str) -> Callable:
        def deco(f: Callable) -> Callable:
            for n in names:
                self.table[n] = f
            return f
        return deco

    def lookup(self, qual: str) -> Optional[Callable[["Call"], Value]]:
        if qual in self.table:
            return self.table[qual]
        if qual.startswith(("pandas.DataFrame.", "pd.DataFrame.")):
            q = "DataFrame." + qual.split(".")[-1]
            if q in self.table:
                return self.table[q]
        for prefix in ("Series.", "str.", "DataFrame.", "list.", "set.", "tuple.", "dict.", "view.", "groupby.",
                       "zip.", "enumerate.", "unknown.", "class.", "pair.", "parser.", "partial.", "response."):
            if qual.startswith(prefix) and prefix + "*" in self.table:
                return self.table[prefix + "*"]
        for prefix in ALLOWLIST_PREFIXES:
            if qual.startswith(prefix) or qual.startswith("builtins." + prefix):
                return _noeffect
        if qual.startswith("builtins."):
            return _noeffect
        return None

    # ------------------------------------------------------------------ install
    def _install(self) -> None:
        r = self.register

        # -------------------------------------------------------- builtins
        @r("builtins.dict")
        def _dict(c: "Call") -> Value:
            an, st = c.an, c.state
            oid = an.alloc("dict", c.node, st, label=f"dict@L{c.line}")
            if c.args:
                src = c.args[0]
                recs = an.record_objects(src)
                if recs:
                    an.copy_into(oid, "item", src, st, strong=True)
                else:
                    for x in src.pts:
                        o = an.objects[x]
                        if o.kind == "zip":
                            ks, vs = o.payload
                            lits = an.lit_keys(ks, st)
                            vel = an.elem_of(vs, c.node, st)
                            if lits is not None:
                                row = st.row(oid, "item")
                                for k in lits:
                                    row = row.store(k, True)
                                    st.set_slot(oid, "item", k, Value(vel.pts, DATA), False)
                                st.set_row(oid, "item", row)
                            else:
                                kel = an.elem_of(ks, c.node, st)
                                an.store_key(oid, "item", kel.kv, vel, False, c.node, st)
                        elif o.kind in ("list", "tuple", "set", "view"):
                            items, elem, pair, _ = an.iterate(Value.obj(x), c.node, st)
                            if pair:
                                an.store_key(oid, "item", elem[0].kv, elem[1], False, c.node, st)
                            elif items is not None:
                                for it in items:
                                    parts = an.unpack(it, 2, st, c.node)
                                    an.store_key(oid, "item", parts[0].kv, parts[1], True, c.node, st)
                            else:
                                st.set_row(oid, "item", st.row(oid, "item").data_store(Source(f"dict@L{c.line}", "data")))
                        else:
                            st.set_row(oid, "item", st.row(oid, "item").data_store(Source(f"dict@L{c.line}", "data")))
                    if not src.pts:
                        st.set_row(oid, "item", st.row(oid, "item").data_store(Source(f"dict@L{c.line}", "data")))
            for k, v in c.kwargs.items():
                an.store_key(oid, "item", Lit(k), v, True, c.node, st)
            for sk in c.star_kwargs:
                an.copy_into(oid, "item", sk, st, strong=True)
            return Value.obj(oid)

        @r("builtins.dict.fromkeys")
        def _fromkeys(c: "Call") -> Value:
            an, st = c.an, c.state
            oid = an.alloc("dict", c.node, st, label=f"dict.fromkeys@L{c.line}")
            ks = an.lit_keys(c.args[0], st) if c.args else None
            val = c.args[1] if len(c.args) > 1 else an.NONE
            if ks is not None:
                row = st.row(oid, "item")
                for k in ks:
                    row = row.store(k, True)
                    st.set_slot(oid, "item", k, val, True)
                st.set_row(oid, "item", row)
            elif c.args:
                el = an.elem_of(c.args[0], c.node, st)
                an.store_key(oid, "item", el.kv, val, False, c.node, st)
            return Value.obj(oid)

        @r("builtins.list", "builtins.tuple", "builtins.set", "builtins.sorted", "builtins.reversed",
           "builtins.frozenset", "builtins.iter")
        def _list(c: "Call") -> Value:
            an, st = c.an, c.state
            kind = {"builtins.tuple": "tuple", "builtins.set": "set", "builtins.frozenset": "set"}.get(c.qual, "list")
            oid = an.alloc(kind, c.node, st, label=f"{c.qual.split('.')[-1]}@L{c.line}")
            an.objects[oid].literal_len = None
            if c.args:
                items, elem, pair, _ = an.iterate(c.args[0], c.node, st)
                if pair:
                    pid = an._alloc("pair", f"pair:{c.site}", an.frame.ctx, True, "pair", payload=elem)
                    st.set_slot(oid, "elem", "*", Value.obj(pid), True)
                    st.elems[oid] = DATA
                else:
                    st.set_slot(oid, "elem", "*", Value(elem.pts, DATA), True)
                    st.elems[oid] = elem.kv
                    if items is not None and an.objects[oid].single and len(items) <= 64:
                        an.objects[oid].literal_len = len(items)
                        for i, it in enumerate(items):
                            st.set_slot(oid, "pos", i, it, True)
            else:
                st.elems[oid] = Lits(frozenset())
                an.objects[oid].literal_len = 0
            return Value.obj(oid)

        @r("builtins.zip")
        def _zip(c: "Call") -> Value:
            an = c.an
            a = c.args[0] if c.args else Value()
            b = c.args[1] if len(c.args) > 1 else Value()
            oid = an.alloc("zip", c.node, c.state, label=f"zip@L{c.line}", payload=(a, b))
            an.objects[oid].payload = (a, b)
            return Value.obj(oid)

        @r("builtins.enumerate")
        def _enumerate(c: "Call") -> Value:
            an = c.an
            oid = an.alloc("enumerate", c.node, c.state, label=f"enumerate@L{c.line}", payload=c.args[0] if c.args else Value())
            an.objects[oid].payload = c.args[0] if c.args else Value()
            return Value.obj(oid)

        @r("builtins.next")
        def _next(c: "Call") -> Value:
            an = c.an
            if not c.args:
                return Value.data("next")
            items, elem, pair, _ = an.iterate(c.args[0], c.node, c.state)
            if pair:
                pid = an._alloc("pair", f"pair:{c.site}", an.frame.ctx, True, "pair", payload=elem)
                return Value.obj(pid)
            if items:
                return items[0]
            return elem

        @r("builtins.getattr")
        def _getattr(c: "Call") -> Value:
            an = c.an
            if len(c.args) >= 2 and isinstance(c.args[1].kv, Lit):
                return an.load_attr(c.args[0], c.args[1].kv.key, c.node, c.state, report=len(c.args) == 2)
            if c.args:
                return join_values(an.all_values(o, "attr", c.state) for o in c.args[0].pts)
            return Value.data("getattr")

        @r("builtins.setattr")
        def _setattr(c: "Call") -> Value:
            an = c.an
            if len(c.args) >= 3:
                if isinstance(c.args[1].kv, Lit):
                    an.store_attr(c.args[0], c.args[1].kv.key, c.args[2], c.node, c.state)
                else:
                    for o in an.record_objects(c.args[0]):
                        an.store_key(o, "attr", c.args[1].kv, c.args[2], False, c.node, c.state)
            return an.NONE

        @r("builtins.vars")
        def _vars(c: "Call") -> Value:
            an, st = c.an, c.state
            if not c.args:
                return Value.data("vars")
            oid = an.alloc("dict", c.node, st, label=f"vars@L{c.line}")
            for x in an.record_objects(c.args[0]):
                st.set_row(oid, "item", st.row(oid, "item").copy_from(st.row(x, "attr"), strong=True))
                for k, v in st.obj_slots(x, "attr"):
                    st.set_slot(oid, "item", k, v, False)
            return Value.obj(oid)

        @r("builtins.map", "builtins.filter")
        def _map(c: "Call") -> Value:
            an, st = c.an, c.state
            oid = an.alloc("list", c.node, st, label=f"map@L{c.line}")
            an.objects[oid].literal_len = None
            if len(c.args) >= 2:
                _, elem, pair, _ = an.iterate(c.args[1], c.node, st)
                if pair:
                    elem = Value()
                if c.qual.endswith("filter"):
                    res = elem
                else:
                    res = an.call_value(c.args[0], c.node, st, override_args=[elem])
                st.set_slot(oid, "elem", "*", Value(res.pts, DATA), True)
                st.elems[oid] = res.kv
            return Value.obj(oid)

        @r("builtins.str")
        def _str(c: "Call") -> Value:
            if c.args:
                return Value(frozenset(), kv_map(c.args[0].kv, RenameFn("str")))
            return Value.lit("")

        @r("builtins.int", "builtins.float", "builtins.bool", "builtins.len", "builtins.print", "builtins.repr",
           "builtins.isinstance", "builtins.type", "builtins.id", "builtins.hash", "builtins.abs", "builtins.round",
           "builtins.min", "builtins.max", "builtins.sum", "builtins.any", "builtins.all", "builtins.format",
           "builtins.input", "builtins.callable", "builtins.issubclass", "builtins.hex", "builtins.chr",
           "builtins.ord", "builtins.range", "builtins.open", "builtins.hasattr", "builtins.divmod", "builtins.pow",
           "builtins.bytes", "builtins.object")
        def _data(c: "Call") -> Value:
            return Value.data(f"L{c.line}")

        @r("builtins.super")
        def _super(c: "Call") -> Value:
            an = c.an
            fr = an.frame
            if fr.func is None or fr.func.payload.cls is None:
                return Value.data("super")
            self_v = c.state.env.get("self") or Value()
            for name, cid in fr.cells.items():
                if name == "self":
                    self_v = c.state.slot(cid, "cell", "v")
            if c.args:
                self_v = c.args[1] if len(c.args) > 1 else self_v
            oid = an._alloc("super", f"super:{fr.func.id}:{c.site}", fr.ctx, True, "super()", payload=(fr.func.payload.cls, self_v))
            an.objects[oid].payload = (fr.func.payload.cls, self_v)
            return Value.obj(oid)

        @r("builtins.str.upper", "builtins.str.lower", "builtins.str.strip", "builtins.str.title",
           "builtins.str.capitalize", "builtins.str.casefold", "builtins.str.lstrip", "builtins.str.rstrip")
        def _str_fn(c: "Call") -> Value:
            fn = c.qual.split(".")[-1]
            if c.args:
                return Value(frozenset(), kv_map(c.args[0].kv, RenameFn(fn if fn in ("upper", "lower", "strip", "title") else "strip")))
            return Value.data("str")

        # ------------------------------------------------------------ str.*
        @r("str.*")
        def _str_method(c: "Call") -> Value:
            name = c.qual.split(".")[-1]
            kv = c.recv.kv if c.recv is not None else DATA
            if name in ("upper", "lower", "strip", "title"):
                return Value(frozenset(), kv_map(kv, RenameFn(name)))
            if name in ("lstrip", "rstrip", "capitalize", "casefold", "swapcase"):
                return Value(frozenset(), kv_map(kv, RenameFn("strip")) if name in ("lstrip", "rstrip") else Value.data("str").kv)
            if name == "replace" and len(c.args) >= 2 and isinstance(c.args[0].kv, Lit) and isinstance(c.args[1].kv, Lit):
                return Value(frozenset(), kv_map(kv, RenameFn("replace", (c.args[0].kv.key, c.args[1].kv.key))))
            if name == "format" and isinstance(kv, Lit) and isinstance(kv.key, str) and kv.key.count("{}") == 1 and len(c.args) == 1:
                pre, post = kv.key.split("{}")
                out = c.args[0].kv
                if isinstance(out, Lit) and not isinstance(out.key, str):
                    out = Lit(str(out.key))
                if pre:
                    out = kv_map(out, RenameFn("prefix", pre))
                if post:
                    out = kv_map(out, RenameFn("suffix", post))
                return Value(frozenset(), out)
            if name == "join":
                return Value.data(f"L{c.line}")
            if name in ("split", "splitlines", "partition", "rsplit"):
                an = c.an
                oid = an.alloc("list", c.node, c.state, label=f"split@L{c.line}")
                an.objects[oid].literal_len = None
                c.state.set_slot(oid, "elem", "*", Value(), True)
                c.state.elems[oid] = DATA
                return Value.obj(oid)
            return Value.data(f"L{c.line}")

        # ----------------------------------------------------------- dict.*
        def _dict_recv(c: "Call") -> List[int]:
            return [o for o in (c.recv.pts if c.recv else ()) if c.an.objects[o].kind in RECORD_KINDS]

        @r("dict.get")
        def _get(c: "Call") -> Value:
            an, st = c.an, c.state
            if not c.args:
                return Value.data("get")
            key = c.args[0]
            default = c.args[1] if len(c.args) > 1 else an.NONE
            out = an.load_item(c.recv, "item", key, c.node, st, report=False)
            return out.join(default)

        @r("dict.setdefault")
        def _setdefault(c: "Call") -> Value:
            an, st = c.an, c.state
            if not c.args:
                return Value.data("setdefault")
            key = c.args[0]
            default = c.args[1] if len(c.args) > 1 else an.NONE
            cur = an.load_item(c.recv, "item", key, c.node, st, report=False)
            an.store_item(c.recv, "item", key, cur.join(default) if not cur.empty else default, c.node, st)
            return an.load_item(c.recv, "item", key, c.node, st, report=False)

        @r("dict.update")
        def _update(c: "Call") -> Value:
            an, st = c.an, c.state
            recs = _dict_recv(c)
            strong = len(recs) == 1 and an.objects[recs[0]].single
            for oid in recs:
                if c.args:
                    src = c.args[0]
                    if an.record_objects(src):
                        an.copy_into(oid, "item", src, st, strong=strong)
                    elif src.pts:
                        items, elem, pair, _ = an.iterate(src, c.node, st)
                        if pair:
                            an.store_key(oid, "item", elem[0].kv, elem[1], False, c.node, st)
                        elif items is not None:
                            for it in items:
                                parts = an.unpack(it, 2, st, c.node)
                                an.store_key(oid, "item", parts[0].kv, parts[1], strong, c.node, st)
                        else:
                            st.set_row(oid, "item", st.row(oid, "item").data_store(Source(f"update@L{c.line}", "data")))
                    else:
                        st.set_row(oid, "item", st.row(oid, "item").data_store(Source(f"update@L{c.line}", "data")))
                for k, v in c.kwargs.items():
                    an.store_key(oid, "item", Lit(k), v, strong, c.node, st)
                for sk in c.star_kwargs:
                    an.copy_into(oid, "item", sk, st, strong=strong)
            return an.NONE

        @r("dict.pop")
        def _pop(c: "Call") -> Value:
            an, st = c.an, c.state
            if not c.args:
                return Value.data("pop")
            key = c.args[0]
            has_default = len(c.args) > 1
            out = an.load_item(c.recv, "item", key, c.node, st, report=not has_default)
            an.delete_item(c.recv, "item", key, c.node, st, report=False)
            if has_default:
                out = out.join(c.args[1])
            return out

        @r("dict.popitem")
        def _popitem(c: "Call") -> Value:
            an, st = c.an, c.state
            for oid in _dict_recv(c):
                row = st.row(oid, "item")
                st.set_row(oid, "item", Row(present=row.present, sources=row.sources, mutators=row.mutators, top=row.top))
            return Value.data("popitem")

        @r("dict.clear")
        def _clear(c: "Call") -> Value:
            an, st = c.an, c.state
            for oid in _dict_recv(c):
                strong = an.objects[oid].single and len(_dict_recv(c)) == 1
                if strong:
                    st.set_row(oid, "item", Row())
                    st.slots.drop_obj(oid, "item")
                else:
                    row = st.row(oid, "item")
                    st.set_row(oid, "item", Row(present=row.present, sources=row.sources, mutators=row.mutators))
            return an.NONE

        @r("dict.keys", "dict.values", "dict.items")
        def _views(c: "Call") -> Value:
            an = c.an
            kind = c.qual.split(".")[-1]
            oid = an._alloc("view", f"view:{kind}:{','.join(map(str, sorted(c.recv.pts)))}", "", True, f"{kind} view", payload=(kind, c.recv))
            an.objects[oid].payload = (kind, c.recv)
            return Value.obj(oid)

        @r("dict.copy")
        def _dcopy(c: "Call") -> Value:
            an, st = c.an, c.state
            outs = [Value.obj(an.copy_object(o, c.node, st, deep=False)) for o in _dict_recv(c)]
            return join_values(outs) if outs else Value.data("copy")

        @r("dict.*")
        def _dict_unknown(c: "Call") -> Value:
            an = c.an
            return an.unknown_call(c.qual, c.recv, c.args, c.kwargs, c.star_args, c.star_kwargs, c.state, c.node)

        # ----------------------------------------------------------- list.*
        @r("list.append", "set.add", "list.insert", "list.extend", "set.update")
        def _append(c: "Call") -> Value:
            an, st = c.an, c.state
            name = c.qual.split(".")[-1]
            for oid in (c.recv.pts if c.recv else ()):
                o = an.objects[oid]
                if o.kind not in ("list", "set", "tuple"):
                    continue
                if name in ("extend", "update") and c.args:
                    _, elem, pair, _ = an.iterate(c.args[0], c.node, st)
                    if pair:
                        elem = Value()
                else:
                    elem = c.args[-1] if c.args else Value()
                st.set_slot(oid, "elem", "*", Value(elem.pts, DATA), False)
                st.elems[oid] = kv_join(st.elems.get(oid, elem.kv), elem.kv) if oid in st.elems else elem.kv
                if (o.literal_len is not None and o.single and name == "append" and an.frame.loop_depth == 0
                        and an.cond_depth == 0 and o.literal_len < 16):
                    st.set_slot(oid, "pos", o.literal_len, elem, True)
                    o.literal_len += 1
                else:
                    if o.literal_len is not None:
                        st.slots.drop_obj(oid, "pos")
                    o.literal_len = None
            return an.NONE

        @r("list.pop", "set.pop")
        def _lpop(c: "Call") -> Value:
            an, st = c.an, c.state
            outs = []
            for oid in (c.recv.pts if c.recv else ()):
                o = an.objects[oid]
                outs.append(an.elem_of(Value.obj(oid), c.node, st))
                o.literal_len = None
            return join_values(outs) if outs else Value.data("pop")

        @r("list.*", "set.*", "tuple.*", "zip.*", "enumerate.*", "pair.*")
        def _list_other(c: "Call") -> Value:
            an = c.an
            name = c.qual.split(".")[-1]
            if name in ("copy",):
                outs = [Value.obj(an.copy_object(o, c.node, c.state, deep=False)) for o in c.recv.pts]
                return join_values(outs)
            if name in ("remove", "discard", "clear", "sort", "reverse"):
                for oid in c.recv.pts:
                    an.objects[oid].literal_len = None
                return an.NONE
            return Value.data(f"L{c.line}")

        @r("view.*")
        def _view(c: "Call") -> Value:
            an, st = c.an, c.state
            name = c.qual.split(".")[-1]
            if name in ("tolist", "to_list", "copy") and c.recv is not None:
                oid = an.alloc("list", c.node, st, label=f"{name}@L{c.line}")
                an.objects[oid].literal_len = None
                _, elem, pair, _ = an.iterate(c.recv, c.node, st)
                if pair:
                    elem = Value()
                st.set_slot(oid, "elem", "*", Value(elem.pts, DATA), True)
                st.elems[oid] = elem.kv
                return Value.obj(oid)
            return Value.data(f"L{c.line}")

        # ---------------------------------------------------------- json/copy
        @r("json.load", "json.loads", "yaml.safe_load", "yaml.load", "yaml.full_load", "tomllib.load",
           "tomllib.loads", "toml.load", "toml.loads", "pickle.load", "pickle.loads",
           "orjson.loads", "ujson.loads", "simplejson.loads", "simplejson.load")
        def _json_load(c: "Call") -> Value:
            an, st = c.an, c.state
            tag = c.qual.split(".")[0] + "." + c.qual.split(".")[-1]
            src = Source(f"{tag}@L{c.line}", "external")
            oid = an.alloc("unknown", c.node, st, label=f"{tag}@L{c.line}", row=Row(sources=frozenset({(src, ())})))
            st.set_row(oid, "attr", Row(sources=frozenset({(src, ())})))
            return Value.obj(oid)

        @r("requests.get", "requests.post", "requests.put", "requests.patch", "requests.delete", "requests.request",
           "requests.Session.get", "requests.Session.post", "httpx.get", "httpx.post", "httpx.Client.get",
           "httpx.Client.post", "urllib.request.urlopen")
        def _http(c: "Call") -> Value:
            an = c.an
            tag = c.qual.split(".")[0] + "." + c.qual.split(".")[-1]
            oid = an.alloc("response", c.node, c.state, label=f"{tag}@L{c.line}", payload=f"{tag}@L{c.line}")
            an.objects[oid].payload = f"{tag}@L{c.line}"
            return Value.obj(oid)

        @r("response.*")
        def _response_method(c: "Call") -> Value:
            an, st = c.an, c.state
            name = c.qual.split(".")[-1]
            if name in ("json", "read"):
                outs = []
                for x in (c.recv.pts if c.recv else ()):
                    tag = an.objects[x].payload
                    src = Source(str(tag), "external")
                    oid = an.alloc("unknown", c.node, st, label=f"{tag}.json()", row=Row(sources=frozenset({(src, ())})))
                    st.set_row(oid, "attr", Row(sources=frozenset({(src, ())})))
                    outs.append(Value.obj(oid))
                return join_values(outs) if outs else Value.data("json")
            return Value.data(f"L{c.line}")

        @r("json.dump", "json.dumps", "yaml.dump", "yaml.safe_dump", "pickle.dump", "pickle.dumps",
           "pprint.pprint", "pprint.pformat", "orjson.dumps")
        def _json_dump(c: "Call") -> Value:
            return Value.data(f"L{c.line}")

        @r("copy.copy", "copy.deepcopy")
        def _copy(c: "Call") -> Value:
            an, st = c.an, c.state
            if not c.args:
                return Value.data("copy")
            deep = c.qual.endswith("deepcopy")
            outs = []
            for o in c.args[0].pts:
                if an.objects[o].kind in ("dict", "list", "tuple", "set", "instance", "frame"):
                    outs.append(Value.obj(an.copy_object(o, c.node, st, deep=deep)))
                else:
                    outs.append(Value.obj(o))
            return join_values(outs) if outs else c.args[0]

        @r("dataclasses.asdict", "attr.asdict", "attrs.asdict")
        def _asdict(c: "Call") -> Value:
            return _vars(c)

        @r("dataclasses.replace")
        def _dc_replace(c: "Call") -> Value:
            an, st = c.an, c.state
            if not c.args:
                return Value.data("replace")
            outs = []
            for o in c.args[0].pts:
                if an.objects[o].kind == "instance":
                    nid = an.copy_object(o, c.node, st, deep=False)
                    for k, v in c.kwargs.items():
                        an.store_key(nid, "attr", Lit(k), v, True, c.node, st)
                    outs.append(Value.obj(nid))
            return join_values(outs) if outs else c.args[0]

        @r("dataclasses.field", "typing.cast", "typing.TypeVar", "typing.NewType")
        def _passthrough(c: "Call") -> Value:
            if c.qual == "typing.cast" and len(c.args) >= 2:
                return c.args[1]
            return Value.data(f"L{c.line}")

        @r("collections.defaultdict", "collections.Counter", "collections.OrderedDict", "collections.ChainMap")
        def _defaultdict(c: "Call") -> Value:
            an, st = c.an, c.state
            name = c.qual.split(".")[-1]
            oid = an.alloc("dict", c.node, st, label=f"{name}@L{c.line}")
            if name in ("defaultdict", "Counter"):
                st.set_row(oid, "item", st.row(oid, "item").data_store(Source(f"{name}@L{c.line}", "data")))
                if name == "defaultdict" and c.args:
                    dflt = an.call_value(c.args[0], c.node, st) if c.args[0].pts else Value.data("default")
                    st.set_slot(oid, "item", "*", Value(dflt.pts, DATA), False)
                src_arg = c.args[1] if name == "defaultdict" and len(c.args) > 1 else (c.args[0] if name == "Counter" and c.args else None)
            else:
                src_arg = c.args[0] if c.args else None
            if src_arg is not None:
                if an.record_objects(src_arg):
                    an.copy_into(oid, "item", src_arg, st, strong=True)
                elif name == "Counter":
                    pass
                else:
                    st.set_row(oid, "item", st.row(oid, "item").data_store(Source(f"{name}@L{c.line}", "data")))
            for k, v in c.kwargs.items():
                an.store_key(oid, "item", Lit(k), v, True, c.node, st)
            return Value.obj(oid)

        @r("csv.DictReader")
        def _dictreader(c: "Call") -> Value:
            an, st = c.an, c.state
            fields = an.lit_keys(c.kw("fieldnames"), st) if c.kw("fieldnames") is not None else None
            if fields is not None:
                row = Row(present=fields, definite=fields, likely=fields)
            else:
                row = Row(sources=frozenset({(Source(f"csv.DictReader@L{c.line}", "external"), ())}))
            rid = an.alloc("dict", c.node, st, label=f"DictReader row@L{c.line}", row=row, single=False)
            an.objects[rid].iter_elem = True
            vid = an._alloc("view", f"view:elems:{c.site}", an.frame.ctx, True, "DictReader", payload=("elems", Value.obj(rid)))
            an.objects[vid].payload = ("elems", Value.obj(rid))
            return Value.obj(vid)

        @r("csv.DictWriter")
        def _dictwriter(c: "Call") -> Value:
            return Value.data(f"L{c.line}")

        @r("argparse.ArgumentParser")
        def _argparser(c: "Call") -> Value:
            an, st = c.an, c.state
            oid = an.alloc("parser", c.node, st, label=f"ArgumentParser@L{c.line}")
            st.set_row(oid, "attr", Row())
            return Value.obj(oid)

        @r("parser.*")
        def _parser_method(c: "Call") -> Value:
            an, st = c.an, c.state
            name = c.qual.split(".")[-1]
            parsers = [o for o in c.recv.pts if an.objects[o].kind == "parser"]
            if name == "add_argument":
                dest = None
                dv = c.kw("dest")
                if dv is not None and isinstance(dv.kv, Lit):
                    dest = dv.kv.key
                else:
                    for a in c.args:
                        if isinstance(a.kv, Lit) and isinstance(a.kv.key, str):
                            s = a.kv.key
                            if s.startswith("--"):
                                dest = s[2:].replace("-", "_")
                                break
                            if not s.startswith("-"):
                                dest = s.replace("-", "_")
                                break
                            if dest is None:
                                dest = s.lstrip("-").replace("-", "_")
                for p in parsers:
                    if dest is not None:
                        st.set_row(p, "attr", st.row(p, "attr").store(dest, an.objects[p].single))
                        st.set_slot(p, "attr", dest, Value.data(f"arg {dest}"), True)
                    else:
                        st.set_row(p, "attr", st.row(p, "attr").data_store(Source(f"add_argument@L{c.line}", "data")))
                return Value.data("action")
            if name in ("parse_args", "parse_known_args", "parse_intermixed_args"):
                nid = an.alloc("namespace", c.node, st, label=f"args@L{c.line}", ns="attr")
                for p in parsers:
                    st.set_row(nid, "attr", st.row(nid, "attr").copy_from(st.row(p, "attr"), strong=True))
                    for k, v in st.obj_slots(p, "attr"):
                        st.set_slot(nid, "attr", k, v, False)
                if name == "parse_known_args":
                    tid = an.alloc("tuple", c.node, st, label="parse_known_args result")
                    an.objects[tid].literal_len = 2
                    st.set_slot(tid, "pos", 0, Value.obj(nid), True)
                    st.set_slot(tid, "pos", 1, Value.data("rest"), True)
                    return Value.obj(tid)
                return Value.obj(nid)
            if name in ("add_subparsers", "add_argument_group", "add_mutually_exclusive_group"):
                return c.recv
            return Value.data(f"L{c.line}")

        @r("functools.partial")
        def _partial(c: "Call") -> Value:
            an = c.an
            if not c.args:
                return Value.data("partial")
            oid = an.alloc("partial", c.node, c.state, label=f"partial@L{c.line}", payload=(c.args[0], c.args[1:], dict(c.kwargs)))
            an.objects[oid].payload = (c.args[0], c.args[1:], dict(c.kwargs))
            return Value.obj(oid)

        @r("functools.reduce")
        def _reduce(c: "Call") -> Value:
            return Value.data(f"L{c.line}")

        @r("itertools.chain", "itertools.chain.from_iterable")
        def _chain(c: "Call") -> Value:
            an, st = c.an, c.state
            oid = an.alloc("list", c.node, st, label=f"chain@L{c.line}")
            an.objects[oid].literal_len = None
            acc = Value()
            kv: Any = None
            for a in c.args:
                _, elem, pair, _ = an.iterate(a, c.node, st)
                if pair:
                    continue
                acc = acc.join(Value(elem.pts, DATA))
                kv = elem.kv if kv is None else kv_join(kv, elem.kv)
            st.set_slot(oid, "elem", "*", acc, True)
            st.elems[oid] = kv if kv is not None else DATA
            return Value.obj(oid)

        # ------------------------------------------------------------ pandas
        for q in FRAME_PRODUCERS:
            r(q)(_read_frame)
        r("pandas.DataFrame")(_pd_dataframe)
        r("pandas.DataFrame.from_records", "pandas.DataFrame.from_dict")(_pd_dataframe)
        r("pandas.merge")(lambda c: _merge(c, c.args[0] if c.args else Value(), c.args[1] if len(c.args) > 1 else c.kw("right") or Value(), 1))
        r("pandas.concat")(_concat)
        r("pandas.json_normalize", "pandas.get_dummies", "pandas.crosstab", "pandas.pivot_table", "pandas.wide_to_long",
          "pandas.read_html_table")(_open_frame_call)
        r("pandas.Series", "pandas.to_datetime", "pandas.to_numeric", "pandas.isna", "pandas.notna", "pandas.isnull",
          "pandas.notnull", "pandas.cut", "pandas.qcut", "pandas.date_range", "pandas.Timestamp", "pandas.Timedelta",
          "pandas.to_timedelta", "pandas.factorize", "pandas.unique", "pandas.array", "pandas.Categorical",
          "pandas.Index", "pandas.MultiIndex", "pandas.NamedAgg", "pandas.Grouper", "pandas.set_option",
          "pandas.option_context", "pandas.testing.assert_frame_equal", "pandas.ExcelWriter", "pandas.eval")
        def _series_call(c: "Call") -> Value:
            return Value(frozenset({c.an.SERIES}), DATA)

        r("Series.*")(_series_call)

        r("DataFrame.merge")(lambda c: _merge(c, c.recv, c.args[0] if c.args else c.kw("right") or Value(), 0))
        r("DataFrame.join")(_join)
        r("DataFrame.rename")(_rename)
        r("DataFrame.add_prefix", "DataFrame.add_suffix")(_add_prefix)
        r("DataFrame.drop")(_drop)
        r("DataFrame.assign")(_assign)
        r("DataFrame.reset_index")(_reset_index)
        r("DataFrame.set_index")(_set_index)
        r("DataFrame.groupby")(_groupby)
        r("DataFrame.apply")(_apply)
        r("DataFrame.pipe")(_pipe)
        r("DataFrame.iterrows")(_iterrows)
        r("DataFrame.itertuples")(_itertuples)
        r("DataFrame.to_dict")(_to_dict)
        r("DataFrame.insert")(_insert)
        r("DataFrame.pop")(_df_pop)
        r("DataFrame.melt")(_melt)
        r("DataFrame.explode")(_explode)
        r("DataFrame.filter")(_filter)
        r("DataFrame.reindex")(_reindex)
        r("DataFrame.select_dtypes")(_select_dtypes)
        r("DataFrame.sort_values", "DataFrame.nlargest", "DataFrame.nsmallest")(_sort_values)
        r("DataFrame.transform")(_transform)
        r("DataFrame.update")(_df_update)
        r("DataFrame.set_axis")(_set_axis)
        r("DataFrame.*")(_df_generic)
        r("groupby.*")(_groupby_method)
        r("partial.*")(_partial_call)
        r("unknown.*")(lambda c: c.an.unknown_call(c.qual, c.recv, c.args, c.kwargs, c.star_args, c.star_kwargs, c.state, c.node))
        r("class.*")(lambda c: c.an.unknown_call(c.qual, c.recv, c.args, c.kwargs, c.star_args, c.star_kwargs, c.state, c.node))

        @r("logging.getLogger", "logging.getLogger.getChild", "structlog.get_logger", "loguru.logger.bind")
        def _get_logger(c: "Call") -> Value:
            an = c.an
            return Value.obj(an._alloc("external", "ext:logging.Logger", "", True, "logging.Logger", payload="logging.Logger"))

        @r("os.getenv", "os.environ.get")
        def _getenv(c: "Call") -> Value:
            an, st = c.an, c.state
            env = an.models.external_record(an, "os.environ", st)
            return _get(Call(an, "dict.get", Value.obj(env), c.args, c.kwargs, c.star_args, c.star_kwargs, st, c.node, c.site, c.line))

    # ------------------------------------------------------------- externals that are records
    def external_record(self, an, qual: str, state) -> Optional[int]:
        if qual in ("os.environ", "os.environb"):
            oid = an._alloc("dict", f"ext-record:{qual}", "", True, qual)
            if (oid, "item") not in state.rows:
                state.set_row(oid, "item", Row(sources=frozenset({(Source(qual, "external"), ())})))
            return oid
        return None

    # ------------------------------------------------------------- frame hooks used by the interpreter
    def frame_getattr(self, an, oid: int, name: str, node, state, guarded: bool) -> Value:
        if name in ("loc", "iloc", "at", "iat"):
            lid = an._alloc("locidx", f"loc:{name}:{oid}", "", True, f"{name} of {an.objects[oid].label}", payload=(name, oid))
            an.objects[lid].payload = (name, oid)
            return Value.obj(lid)
        if name == "columns":
            vid = an._alloc("view", f"view:keys:{oid}", "", True, "columns", payload=("keys", Value.obj(oid)))
            an.objects[vid].payload = ("keys", Value.obj(oid))
            return Value.obj(vid)
        if name in ("str", "dt", "cat"):
            return Value(frozenset({an.SERIES}), DATA)
        if name in DF_ATTRS or name in DF_METHODS:
            if name in DF_ATTRS and name not in ("columns",):
                if name in ("index", "values", "T", "dtypes", "style", "plot"):
                    return Value.data(name) if name != "T" else _open_frame_from(an, oid, node, state, "T")
                return Value.data(name)
            return an.method_value(f"DataFrame.{name}", Value.obj(oid))
        # attribute-style column access
        return self.frame_getitem(an, oid, Value.lit(name), node, state, guarded)

    def frame_setattr(self, an, oid: int, name: str, v: Value, node, state, strong: bool) -> None:
        if name == "columns":
            row = state.row(oid, "item")
            ks = an.lit_keys(v, state)
            if ks is not None:
                state.set_row(oid, "item", row.project(ks))
                return
            kv = v.kv if not v.pts else None
            for x in v.pts:
                if an.objects[x].kind == "list":
                    e = state.elems.get(x, DATA)
                    kv = e if kv is None else kv_join(kv, e)
                elif an.objects[x].kind == "view":
                    e = KeysOf(frozenset(an.objects[x].payload[1].pts), "item")
                    kv = e if kv is None else kv_join(kv, e)
            if kv is None:
                kv = DATA
            if isinstance(kv, KeysOf) and kv.objs == frozenset({oid}) and not kv.exclude:
                for step in kv.chain:
                    row = row.rename(step)
                state.set_row(oid, "item", row)
                return
            state.set_row(oid, "item", Row(sources=frozenset({(Source(f"columns=@L{node.lineno}", "call"), ())})))
            an.note(f"df.columns assigned from an unknown value at L{node.lineno}")
            return
        if name in DF_ATTRS:
            return
        self.frame_setitem(an, oid, Value.lit(name), v, node, state, strong)

    def frame_getitem(self, an, oid: int, key: Value, node, state, guarded: bool) -> Value:
        row = state.row(oid, "item")
        kv = key.kv
        site = an.access_site(node)
        if isinstance(kv, Lit) and not key.pts:
            cls = classify(row, kv.key)
            if not guarded:
                an.verdict(node, kv.key, cls, oid, "item")
                if cls.tier == INPUT:
                    srcs = [s for s, _ in cls.providers]
                    for s, terms in cls.providers:
                        for t in terms:
                            state.require(s, t, site, alternatives=frozenset(x for x in srcs if x != s),
                                          caveats=row.mutators, alt_keys=terms)
                if cls.tier != MISSING and an.objects[oid].single:
                    state.set_row(oid, "item", row.narrow_in(kv.key, True))
            return Value(frozenset({an.SERIES}), DATA)
        ks = an.lit_keys(key, state) if key.pts else (kv_literals(kv) if isinstance(kv, Lits) else None)
        if ks is not None and (key.pts or isinstance(kv, Lits)):
            # projection df[["a", "b"]]
            for k in sorted(ks, key=str):
                self.frame_getitem(an, oid, Value.lit(k), node, state, guarded)
            row = state.row(oid, "item")
            nid = an.alloc("frame", node, state, label=f"frame@L{node.lineno}", row=row.project(ks))
            return Value.obj(nid)
        if an.SERIES in key.pts or any(an.objects[x].kind == "frame" for x in key.pts):
            nid = an.alloc("frame", node, state, label=f"frame@L{node.lineno}", row=row)
            _copy_meta(an, oid, nid, state)
            return Value.obj(nid)
        if isinstance(kv, KeysOf):
            nid = an.alloc("frame", node, state, label=f"frame@L{node.lineno}", row=Row().copy_from(row, strong=False))
            return Value.obj(nid)
        if key.pts and any(an.objects[x].kind in ("list", "view") for x in key.pts):
            # unknown column list
            nid = an.alloc("frame", node, state, label=f"frame@L{node.lineno}",
                           row=Row(present=row.present, sources=row.sources | frozenset({(Source(f"columns@L{node.lineno}", "call"), ())}), mutators=row.mutators))
            return Value.obj(nid)
        return Value(frozenset({an.SERIES}), DATA)

    def frame_setitem(self, an, oid: int, key: Value, v: Value, node, state, strong: bool) -> None:
        row = state.row(oid, "item")
        kv = key.kv
        ks = an.lit_keys(key, state) if key.pts else None
        if isinstance(kv, Lit) and not key.pts:
            state.set_row(oid, "item", row.store(kv.key, strong))
            return
        if ks is not None or isinstance(kv, Lits):
            ks = ks if ks is not None else kv_literals(kv)
            r = row
            for k in ks:
                r = r.store(k, strong)
            state.set_row(oid, "item", r)
            return
        if isinstance(kv, KeysOf):
            srcrow = join_rows(state.row(x, kv.ns) for x in kv.objs)
            state.set_row(oid, "item", row.copy_from(srcrow, strong=False, kill=kv.exclude, chain=kv.chain))
            return
        if an.SERIES in key.pts:
            return  # df[mask] = value: row assignment, columns unchanged
        state.set_row(oid, "item", row.data_store(Source(f"store@L{node.lineno}", "data")))

    def frame_delitem(self, an, oid: int, key: Value, node, state, strong: bool, report: bool) -> None:
        row = state.row(oid, "item")
        for k in sorted(kv_literals(key.kv), key=str):
            if report:
                cls = classify(row, k)
                an.verdict(node, k, cls, oid, "item")
                if cls.tier == INPUT:
                    for s, terms in cls.providers:
                        for t in terms:
                            state.require(s, t, an.access_site(node))
            row = row.delete(k, strong and isinstance(key.kv, Lit))
        state.set_row(oid, "item", row)

    def loc_get(self, an, lo, key: Value, node, state) -> Value:
        name, foid = lo.payload
        row = state.row(foid, "item")
        parts = None
        for x in key.pts:
            o = an.objects[x]
            if o.kind == "tuple" and o.literal_len == 2:
                parts = (state.slot(x, "pos", 0), state.slot(x, "pos", 1))
        if parts is None:
            if name == "iloc":
                nid = an.alloc("frame", node, state, label=f"frame@L{node.lineno}", row=row)
                return Value.obj(nid)
            nid = an.alloc("frame", node, state, label=f"frame@L{node.lineno}", row=row)
            return Value.obj(nid)
        _, cols = parts
        if name in ("iloc", "iat"):
            return Value(frozenset({an.SERIES}), DATA)
        if isinstance(cols.kv, Lit) and not cols.pts:
            return self.frame_getitem(an, foid, cols, node, state, an.is_guarded("item"))
        ks = an.lit_keys(cols, state)
        if ks is not None:
            return self.frame_getitem(an, foid, Value(cols.pts, Lits(ks)), node, state, an.is_guarded("item"))
        if isinstance(cols.kv, Data) and not cols.pts and cols.kv.site.startswith("slice"):
            nid = an.alloc("frame", node, state, label=f"frame@L{node.lineno}", row=row)
            return Value.obj(nid)
        return Value(frozenset({an.SERIES}), DATA)

    def loc_set(self, an, lo, key: Value, v: Value, node, state) -> None:
        name, foid = lo.payload
        for x in key.pts:
            o = an.objects[x]
            if o.kind == "tuple" and o.literal_len == 2:
                cols = state.slot(x, "pos", 1)
                if name in ("iloc", "iat"):
                    return
                self.frame_setitem(an, foid, cols, v, node, state, an.objects[foid].single)
                return


# ----------------------------------------------------------------------- pandas helpers

def _mark_view(an, vrow: int, frames: List[int]) -> None:
    an.objects[vrow].iter_elem = True
    if len(frames) == 1:
        an.objects[vrow].derived_from = frames[0]


def _noeffect(c: "Call") -> Value:
    return Value.data(f"L{c.line}")


def _frames(an, v: Optional[Value]) -> List[int]:
    if v is None:
        return []
    return [o for o in v.pts if an.objects[o].kind == "frame"]


def _copy_meta(an, src: int, dst: int, state) -> None:
    m = state.slots.get((src, "meta", "index"))
    if m is not None:
        state.set_slot(dst, "meta", "index", m, True)


def _new_frame(an, c: "Call", row: Row, label: Optional[str] = None, src: Optional[int] = None) -> Value:
    nid = an.alloc("frame", c.node, c.state, label=label or f"frame@L{c.line}", row=row)
    if src is not None:
        _copy_meta(an, src, nid, c.state)
    return Value.obj(nid)


def _recv_row(c: "Call") -> Row:
    fs = _frames(c.an, c.recv)
    return join_rows(c.state.row(f, "item") for f in fs) if fs else Row(top=True)


def _open_frame_from(an, oid: int, node, state, op: str) -> Value:
    row = state.row(oid, "item")
    nid = an.alloc("frame", node, state, label=f"frame@L{node.lineno}",
                   row=Row(sources=frozenset({(Source(f"{op}@L{node.lineno}", "call"), ())}), mutators=row.mutators))
    return Value.obj(nid)


def _read_frame(c: "Call") -> Value:
    an, st = c.an, c.state
    tag = FRAME_PRODUCERS.get(c.qual, c.qual.split(".")[-1])
    usecols = c.kw("usecols")
    names = c.kw("names")
    ks = None
    if usecols is not None:
        ks = an.lit_keys(usecols, st)
    if names is not None:
        ks = an.lit_keys(names, st)
    if ks is not None:
        row = Row(present=ks, definite=ks, likely=ks)
    else:
        row = Row(sources=frozenset({(Source(f"{tag}@L{c.line}", "external"), ())}))
    return _new_frame(an, c, row, label=f"{tag}@L{c.line}")


def _pd_dataframe(c: "Call") -> Value:
    an, st = c.an, c.state
    data = c.arg(0, "data")
    cols = c.kw("columns")
    if cols is not None:
        ks = an.lit_keys(cols, st)
        if ks is not None:
            return _new_frame(an, c, Row(present=ks, definite=ks, likely=ks))
    if data is None:
        return _new_frame(an, c, Row())
    recs = an.record_objects(data)
    if recs:
        row = Row()
        for x in recs:
            row = row.copy_from(st.row(x, "item"), strong=True)
        return _new_frame(an, c, row)
    for x in data.pts:
        o = an.objects[x]
        if o.kind in ("list", "tuple"):
            items, elem, pair, _ = an.iterate(data, c.node, st)
            if items is not None:
                rows = []
                for it in items:
                    rs = an.record_objects(it)
                    rows.append(join_rows(st.row(y, "item") for y in rs) if rs else Row(top=True))
                row = Row()
                for rr in rows:
                    row = row.copy_from(rr, strong=True)   # NaN-filled union
                return _new_frame(an, c, row)
            rs = an.record_objects(elem) if not pair else []
            if rs:
                row = Row()
                for y in rs:
                    row = row.copy_from(st.row(y, "item"), strong=True)
                return _new_frame(an, c, row)
    return _new_frame(an, c, Row(sources=frozenset({(Source(f"DataFrame@L{c.line}", "call"), ())})))


def _concat(c: "Call") -> Value:
    an, st = c.an, c.state
    objs = c.arg(0, "objs")
    if objs is None:
        return _open_frame_call(c)
    axis = c.kw("axis")
    join = c.kw("join")
    inner = join is not None and isinstance(join.kv, Lit) and join.kv.key == "inner"
    frames: List[int] = []
    items, elem, pair, _ = an.iterate(objs, c.node, st)
    if items is not None:
        for it in items:
            frames.extend(_frames(an, it))
    elif not pair:
        frames.extend(_frames(an, elem))
    if not frames:
        return _open_frame_call(c)
    rows = [st.row(f, "item") for f in frames]
    if inner and (axis is None or (isinstance(axis.kv, Lit) and axis.kv.key in (0, "index"))):
        row = rows[0]
        for r in rows[1:]:
            row = Row(present=row.present & r.present, definite=row.definite & r.definite, likely=row.likely & r.likely,
                      sources=row.sources | r.sources, mutators=row.mutators | r.mutators, top=row.top or r.top)
        if items is None:
            row = Row(present=row.present, sources=row.sources, mutators=row.mutators, top=row.top)
        return _new_frame(an, c, row)
    row = Row()
    for r in rows:
        row = row.copy_from(r, strong=True)
    if items is None:
        # unknown number of frames of one shape: all of them share the summary row
        pass
    return _new_frame(an, c, row)


def _open_frame_call(c: "Call") -> Value:
    tag = c.qual.split(".")[-1]
    return _new_frame(c.an, c, Row(sources=frozenset({(Source(f"{tag}@L{c.line}", "call"), ())})))


def _require_cols(c: "Call", frames: List[int], keys: FrozenSet[Any], node=None) -> None:
    for f in frames:
        for k in sorted(keys, key=str):
            c.an.models.frame_getitem(c.an, f, Value.lit(k), node or c.node, c.state, c.an.is_guarded("item"))


def _merge(c: "Call", left: Value, right: Value, _mode: int) -> Value:
    an, st = c.an, c.state
    lf, rf = _frames(an, left), _frames(an, right)
    on = c.kw("on")
    if on is None and len(c.args) > (1 if _mode == 0 else 2):
        on = c.args[1 if _mode == 0 else 2]
    left_on, right_on = c.kw("left_on"), c.kw("right_on")
    on_keys = an.lit_keys(on, st) if on is not None else None
    lkeys = an.lit_keys(left_on, st) if left_on is not None else None
    rkeys = an.lit_keys(right_on, st) if right_on is not None else None
    if on_keys is not None:
        _require_cols(c, lf, on_keys)
        _require_cols(c, rf, on_keys)
    if lkeys is not None:
        _require_cols(c, lf, lkeys)
    if rkeys is not None:
        _require_cols(c, rf, rkeys)
    lrow = join_rows(st.row(f, "item") for f in lf) if lf else Row(top=True)
    rrow = join_rows(st.row(f, "item") for f in rf) if rf else Row(top=True)
    suffixes = ("_x", "_y")
    sv = c.kw("suffixes")
    if sv is not None:
        for x in sv.pts:
            o = an.objects[x]
            if o.kind in ("tuple", "list") and o.literal_len == 2:
                a, b = st.slot(x, "pos", 0), st.slot(x, "pos", 1)
                if isinstance(a.kv, Lit) and isinstance(b.kv, Lit):
                    suffixes = (a.kv.key or "", b.kv.key or "")
    keys_both = set(on_keys or ())
    if on_keys is None and lkeys is None and rkeys is None:
        keys_both = set(lrow.present & rrow.present) if lrow.closed and rrow.closed else set()
    row = Row().copy_from(lrow, strong=True).copy_from(rrow, strong=True)
    overlap = (lrow.present & rrow.present) - keys_both
    for col in sorted(overlap, key=str):
        both_def = col in lrow.definite and col in rrow.definite
        if isinstance(col, str):
            row = row.store(f"{col}{suffixes[0]}", both_def) if suffixes[0] else row
            row = row.store(f"{col}{suffixes[1]}", both_def) if suffixes[1] else row
        if both_def:
            row = row.delete(col, True)
        else:
            row = Row(present=row.present, definite=row.definite - {col}, likely=row.likely - {col},
                      absent=row.absent, absent_likely=row.absent_likely, sources=row.sources,
                      mutators=row.mutators, top=row.top)
    if on_keys is None and (lrow.open or rrow.open) and lkeys is None and rkeys is None:
        an.note(f"merge@L{c.line}: join keys unknown (no on=)")
    return _new_frame(an, c, row)


def _join(c: "Call") -> Value:
    an, st = c.an, c.state
    other = c.args[0] if c.args else c.kw("other") or Value()
    lf, rf = _frames(an, c.recv), _frames(an, other)
    on = c.kw("on")
    on_keys = an.lit_keys(on, st) if on is not None else None
    if on_keys is not None:
        _require_cols(c, lf, on_keys)
    lrow = join_rows(st.row(f, "item") for f in lf) if lf else Row(top=True)
    rrow = join_rows(st.row(f, "item") for f in rf) if rf else Row(top=True)
    ls, rs = c.kw("lsuffix"), c.kw("rsuffix")
    lsuf = ls.kv.key if ls is not None and isinstance(ls.kv, Lit) else ""
    rsuf = rs.kv.key if rs is not None and isinstance(rs.kv, Lit) else ""
    row = Row().copy_from(lrow, strong=True).copy_from(rrow, strong=True)
    overlap = (lrow.present & rrow.present) - set(on_keys or ())
    for col in sorted(overlap, key=str):
        both_def = col in lrow.definite and col in rrow.definite
        if not lsuf and not rsuf:
            if both_def:
                an.verdict(c.node, col, Classification(MISSING, note="join: overlapping column without suffix raises ValueError"), lf[0] if lf else rf[0], "item")
            continue
        if lsuf:
            row = row.store(f"{col}{lsuf}", both_def)
        if rsuf:
            row = row.store(f"{col}{rsuf}", both_def)
        if both_def and (lsuf and rsuf):
            row = row.delete(col, True)
        elif both_def and (lsuf or rsuf):
            pass  # one side keeps the bare name
    return _new_frame(an, c, row)


def _rename(c: "Call") -> Value:
    an, st = c.an, c.state
    cols = c.kw("columns")
    mapper = c.kw("mapper") if c.kw("mapper") is not None else (c.args[0] if c.args else None)
    axis = c.kw("axis")
    if cols is None and mapper is not None:
        if axis is not None and isinstance(axis.kv, Lit) and axis.kv.key in (1, "columns"):
            cols = mapper
        else:
            # index rename: columns unchanged
            fs = _frames(an, c.recv)
            return _new_frame(an, c, _recv_row(c), src=fs[0] if fs else None)
    row = _recv_row(c)
    fs = _frames(an, c.recv)
    if cols is None:
        return _new_frame(an, c, row, src=fs[0] if fs else None)
    step = _rename_step(an, cols, c, fs)
    if step is None:
        an.note(f"rename@L{c.line}: unknown column mapping")
        return _new_frame(an, c, Row(present=row.present, sources=row.sources | frozenset({(Source(f"rename@L{c.line}", "call"), ())}), mutators=row.mutators))
    out = row
    for s in step:
        out = out.rename(s)
    return _new_frame(an, c, out, src=fs[0] if fs else None)


def _rename_step(an, cols: Value, c: "Call", frames: List[int]):
    st = c.state
    m = an.literal_mapping(cols, st)
    if m is not None:
        return [RenameMap(tuple(sorted(m.items(), key=lambda kv: str(kv[0]))))]
    # callable: str.upper / lambda c: c.lower()
    for x in cols.pts:
        o = an.objects[x]
        if o.kind == "external":
            fn = str(o.payload).split(".")[-1]
            if fn in ("upper", "lower", "strip", "title"):
                return [RenameFn(fn)]
        if o.kind in ("function", "bound"):
            probe = Value(frozenset(), KeysOf(frozenset(frames), "item"))
            res = an.call_value(Value.obj(x), c.node, st, override_args=[probe])
            if isinstance(res.kv, KeysOf) and res.kv.objs == frozenset(frames) and not res.kv.exclude:
                return list(res.kv.chain)
    return None


def _add_prefix(c: "Call") -> Value:
    an = c.an
    fn = "prefix" if c.qual.endswith("prefix") else "suffix"
    arg = c.arg(0, fn)
    row = _recv_row(c)
    if arg is not None and isinstance(arg.kv, Lit):
        return _new_frame(an, c, row.rename(RenameFn(fn, arg.kv.key)))
    return _new_frame(an, c, Row(present=row.present, sources=row.sources | frozenset({(Source(f"add_{fn}@L{c.line}", "call"), ())}), mutators=row.mutators))


def _drop(c: "Call") -> Value:
    an, st = c.an, c.state
    fs = _frames(an, c.recv)
    cols = c.kw("columns")
    axis = c.kw("axis")
    labels = c.arg(0, "labels")
    is_cols = cols is not None or (axis is not None and isinstance(axis.kv, Lit) and axis.kv.key in (1, "columns"))
    errors = c.kw("errors")
    ignore = errors is not None and isinstance(errors.kv, Lit) and errors.kv.key == "ignore"
    row = _recv_row(c)
    if not is_cols:
        return _new_frame(an, c, row, src=fs[0] if fs else None)
    target = cols if cols is not None else labels
    ks = an.lit_keys(target, st) if target is not None else None
    if ks is None:
        kv = target.kv if target is not None else DATA
        if isinstance(kv, KeysOf):
            # drop(columns=[c for c in df if ...]) → unknown subset removed: present only
            return _new_frame(an, c, Row(present=row.present, likely=frozenset(), sources=row.sources, mutators=row.mutators))
        an.note(f"drop@L{c.line}: unknown columns")
        return _new_frame(an, c, Row(present=row.present, sources=row.sources, mutators=row.mutators))
    if not ignore:
        _require_cols(c, fs, ks)
    row = _recv_row(c)
    for k in ks:
        row = row.delete(k, True)
    return _new_frame(an, c, row, src=fs[0] if fs else None)


def _assign(c: "Call") -> Value:
    an = c.an
    row = _recv_row(c)
    for k in c.kwargs:
        row = row.store(k, True)
    fs = _frames(an, c.recv)
    return _new_frame(an, c, row, src=fs[0] if fs else None)


def _reset_index(c: "Call") -> Value:
    an, st = c.an, c.state
    fs = _frames(an, c.recv)
    row = _recv_row(c)
    drop = c.kw("drop")
    if drop is not None and isinstance(drop.kv, Lit) and drop.kv.key is True:
        return _new_frame(an, c, row)
    meta = join_values(st.slot(f, "meta", "index") for f in fs) if fs else Value()
    ks = kv_literals(meta.kv)
    if ks:
        for k in ks:
            row = row.store(k, isinstance(meta.kv, Lit))
    else:
        row = row.store("index", False)
        if row.closed:
            row = Row(present=row.present, definite=row.definite, likely=row.likely, absent=row.absent,
                      absent_likely=row.absent_likely, sources=row.sources | frozenset({(Source(f"reset_index@L{c.line}", "call"), ())}),
                      mutators=row.mutators)
    return _new_frame(an, c, row)


def _set_index(c: "Call") -> Value:
    an, st = c.an, c.state
    fs = _frames(an, c.recv)
    keys = c.arg(0, "keys")
    ks = an.lit_keys(keys, st) if keys is not None else None
    row = _recv_row(c)
    drop = c.kw("drop")
    keep = drop is not None and isinstance(drop.kv, Lit) and drop.kv.key is False
    if ks is None:
        return _new_frame(an, c, Row(present=row.present, sources=row.sources, mutators=row.mutators))
    _require_cols(c, fs, ks)
    row = _recv_row(c)
    if not keep:
        for k in ks:
            row = row.delete(k, True)
    v = _new_frame(an, c, row)
    nid = next(iter(v.pts))
    st.set_slot(nid, "meta", "index", Value(frozenset(), Lits(ks) if len(ks) > 1 else Lit(next(iter(ks)))), True)
    return v


def _groupby(c: "Call") -> Value:
    an, st = c.an, c.state
    fs = _frames(an, c.recv)
    by = c.arg(0, "by")
    ks = an.lit_keys(by, st) if by is not None else None
    if ks is not None:
        _require_cols(c, fs, ks)
    as_index = c.kw("as_index")
    ai = not (as_index is not None and isinstance(as_index.kv, Lit) and as_index.kv.key is False)
    gid = an.alloc("groupby", c.node, st, label=f"groupby@L{c.line}", payload=(tuple(fs), ks, ai, None))
    an.objects[gid].payload = (tuple(fs), ks, ai, None)
    return Value.obj(gid)


def _groupby_select(an, gid: int, key: Value, node, state) -> Value:
    fs, ks, ai, sel = an.objects[gid].payload
    cols = an.lit_keys(key, state) if key.pts else kv_literals(key.kv)
    if cols:
        _require_cols(Call(an, "groupby.select", None, [], {}, [], [], state, node, f"L{node.lineno}", node.lineno), list(fs), frozenset(cols))
    nid = an.alloc("groupby", node, state, label=f"groupby@L{node.lineno}", payload=(fs, ks, ai, frozenset(cols) if cols else None))
    an.objects[nid].payload = (fs, ks, ai, (frozenset(cols), isinstance(key.kv, Lit) and not key.pts) if cols else None)
    return Value.obj(nid)


def _groupby_method(c: "Call") -> Value:
    an, st = c.an, c.state
    name = c.qual.split(".")[-1]
    gids = [o for o in c.recv.pts if an.objects[o].kind == "groupby"]
    if not gids:
        return Value.data(f"L{c.line}")
    fs, ks, ai, sel = an.objects[gids[0]].payload
    base = join_rows(st.row(f, "item") for f in fs) if fs else Row(top=True)
    keys = ks or frozenset()

    def with_index(row: Row) -> Value:
        v = _new_frame(an, c, row)
        nid = next(iter(v.pts))
        if keys:
            st.set_slot(nid, "meta", "index", Value(frozenset(), Lits(keys) if len(keys) > 1 else Lit(next(iter(keys)))), True)
        return v

    if name in ("agg", "aggregate"):
        if c.kwargs and all(True for _ in c.kwargs):
            named = {}
            for k, v in c.kwargs.items():
                for x in v.pts:
                    o = an.objects[x]
                    if o.kind == "tuple" and o.literal_len == 2:
                        col = st.slot(x, "pos", 0)
                        if isinstance(col.kv, Lit):
                            named[k] = col.kv.key
            if named or c.kwargs:
                _require_cols(c, list(fs), frozenset(named.values()))
                row = Row()
                for k in c.kwargs:
                    row = row.store(k, True)
                if not ai:
                    for k in keys:
                        row = row.store(k, True)
                return with_index(row)
        if c.args:
            spec = c.args[0]
            m = an.literal_mapping(spec, st)
            recs = an.record_objects(spec)
            if recs:
                row = Row()
                for x in recs:
                    for k in st.row(x, "item").present:
                        row = row.store(k, k in st.row(x, "item").definite)
                _require_cols(c, list(fs), row.present)
                if not ai:
                    for k in keys:
                        row = row.store(k, True)
                return with_index(row)
        name = "sum"  # agg(f): same shape as a reduction
    if name in ("sum", "mean", "min", "max", "count", "median", "std", "var", "first", "last", "nunique",
                "prod", "sem", "skew", "any", "all", "quantile", "size", "idxmax", "idxmin"):
        if name == "size":
            return Value(frozenset({an.SERIES}), DATA)
        if sel is not None:
            cols, single = sel
            if single:
                return Value(frozenset({an.SERIES}), DATA)
            row = Row(present=cols, definite=cols, likely=cols)
        else:
            row = base
            for k in keys:
                row = row.delete(k, True)
            if base.closed:
                row = Row(present=row.present, definite=row.definite, likely=row.likely)
        if not ai:
            for k in keys:
                row = row.store(k, True)
        return with_index(row)
    if name in ("apply", "pipe"):
        fn = c.args[0] if c.args else None
        if fn is not None and fn.pts:
            vid = an.alloc("frame", c.node, st, label=f"group@L{c.line}", row=base)
            res = an.call_value(fn, c.node, st, override_args=[Value.obj(vid)] + list(c.args[1:]))
            if _frames(an, res):
                return res
            if an.record_objects(res):
                row = join_rows(st.row(x, "item") for x in an.record_objects(res))
                return with_index(row)
        return Value(frozenset({an.SERIES}), DATA)
    if name in ("transform", "rank", "cumsum", "cumcount", "shift", "diff", "pct_change", "ngroup", "fillna", "ffill", "bfill"):
        if sel is not None and sel[1]:
            return Value(frozenset({an.SERIES}), DATA)
        row = base if sel is None else Row(present=sel[0], definite=sel[0], likely=sel[0])
        return _new_frame(an, c, row)
    if name in ("head", "tail", "nth", "sample", "filter", "get_group", "take"):
        return _new_frame(an, c, base)
    if name == "reset_index":
        return Value.data("gb.reset_index")
    if name == "describe":
        return _new_frame(an, c, Row(sources=frozenset({(Source(f"describe@L{c.line}", "call"), ())})))
    if name in ("__getitem__",):
        return _groupby_select(an, gids[0], c.args[0], c.node, st)
    return Value(frozenset({an.SERIES}), DATA)


def _apply(c: "Call") -> Value:
    an, st = c.an, c.state
    fs = _frames(an, c.recv)
    fn = c.arg(0, "func")
    axis = c.kw("axis")
    rowwise = axis is not None and isinstance(axis.kv, Lit) and axis.kv.key in (1, "columns")
    if fn is None or not fn.pts:
        return Value(frozenset({an.SERIES}), DATA)
    if rowwise:
        base = _recv_row(c)
        vrow = an.alloc("dict", c.node, st, label=f"row of {an.objects[fs[0]].label if fs else 'frame'}", row=base, single=False)
        _mark_view(an, vrow, fs)
        for k in base.present:
            st.set_slot(vrow, "item", k, Value.data(f"cell {k}"), False)
        extra = list(c.args[1:])
        res = an.call_value(fn, c.node, st, override_args=[Value.obj(vrow)] + extra)
        rt = c.kw("result_type")
        if rt is not None and isinstance(rt.kv, Lit) and rt.kv.key == "expand" and an.record_objects(res):
            row = join_rows(st.row(x, "item") for x in an.record_objects(res))
            return _new_frame(an, c, row)
        if _frames(an, res) or an.record_objects(res):
            # returning a Series/dict per row expands into columns
            recs = an.record_objects(res)
            if recs:
                row = join_rows(st.row(x, "item") for x in recs)
                return _new_frame(an, c, row)
        return Value(frozenset({an.SERIES}), DATA)
    # column-wise apply: callback receives a Series
    an.call_value(fn, c.node, st, override_args=[Value(frozenset({an.SERIES}), DATA)] + list(c.args[1:]))
    return _new_frame(an, c, _recv_row(c))


def _pipe(c: "Call") -> Value:
    an, st = c.an, c.state
    fn = c.args[0] if c.args else None
    if fn is None:
        return c.recv
    return an.call_value(fn, c.node, st, override_args=[c.recv] + list(c.args[1:]))


def _iterrows(c: "Call") -> Value:
    an, st = c.an, c.state
    base = _recv_row(c)
    vrow = an.alloc("dict", c.node, st, label=f"row@L{c.line}", row=base, single=False)
    _mark_view(an, vrow, _frames(an, c.recv))
    for k in base.present:
        st.set_slot(vrow, "item", k, Value.data(f"cell {k}"), False)
    vid = an._alloc("view", f"view:rows:{c.site}", an.frame.ctx, True, "iterrows", payload=("rows", Value.obj(vrow)))
    an.objects[vid].payload = ("rows", Value.obj(vrow))
    return Value.obj(vid)


def _itertuples(c: "Call") -> Value:
    an, st = c.an, c.state
    base = _recv_row(c)
    vrow = an.alloc("namespace", c.node, st, label=f"row@L{c.line}", row=base, ns="attr", single=False)
    _mark_view(an, vrow, _frames(an, c.recv))
    for k in base.present:
        if isinstance(k, str):
            st.set_slot(vrow, "attr", k, Value.data(f"cell {k}"), False)
    st.set_row(vrow, "attr", st.row(vrow, "attr").store("Index", True))
    vid = an._alloc("view", f"view:elems:{c.site}", an.frame.ctx, True, "itertuples", payload=("elems", Value.obj(vrow)))
    an.objects[vid].payload = ("elems", Value.obj(vrow))
    return Value.obj(vid)


def _to_dict(c: "Call") -> Value:
    an, st = c.an, c.state
    base = _recv_row(c)
    orient = c.arg(0, "orient")
    o = orient.kv.key if orient is not None and isinstance(orient.kv, Lit) else "dict"
    if o == "records":
        vrow = an.alloc("dict", c.node, st, label=f"record@L{c.line}", row=base, single=False)
        _mark_view(an, vrow, _frames(an, c.recv))
        for k in base.present:
            st.set_slot(vrow, "item", k, Value.data(f"cell {k}"), False)
        lid = an.alloc("list", c.node, st, label=f"records@L{c.line}")
        an.objects[lid].literal_len = None
        st.set_slot(lid, "elem", "*", Value.obj(vrow), True)
        st.elems[lid] = DATA
        return Value.obj(lid)
    if o in ("dict", "list", "series", "split", "tight", "index"):
        oid = an.alloc("dict", c.node, st, label=f"to_dict@L{c.line}", row=base if o in ("dict", "list", "series") else Row(sources=frozenset({(Source(f"to_dict@L{c.line}", "call"), ())})))
        return Value.obj(oid)
    return Value.data("to_dict")


def _insert(c: "Call") -> Value:
    an, st = c.an, c.state
    col = c.arg(1, "column")
    for f in _frames(an, c.recv):
        if col is not None:
            an.models.frame_setitem(an, f, col, c.arg(2, "value") or Value(), c.node, st, an.objects[f].single)
    return an.NONE


def _df_pop(c: "Call") -> Value:
    an, st = c.an, c.state
    col = c.arg(0, "item")
    for f in _frames(an, c.recv):
        if col is not None:
            an.models.frame_delitem(an, f, col, c.node, st, an.objects[f].single, True)
    return Value(frozenset({an.SERIES}), DATA)


def _melt(c: "Call") -> Value:
    an, st = c.an, c.state
    fs = _frames(an, c.recv)
    idv = c.kw("id_vars") or c.arg(0)
    var_name = c.kw("var_name")
    value_name = c.kw("value_name")
    ids = an.lit_keys(idv, st) if idv is not None else frozenset()
    if ids is None:
        return _open_frame_call(Call(an, "melt", None, [], {}, [], [], st, c.node, c.site, c.line))
    _require_cols(c, fs, ids)
    vn = var_name.kv.key if var_name is not None and isinstance(var_name.kv, Lit) else "variable"
    vln = value_name.kv.key if value_name is not None and isinstance(value_name.kv, Lit) else "value"
    ks = frozenset(ids) | {vn, vln}
    return _new_frame(an, c, Row(present=ks, definite=ks, likely=ks))


def _explode(c: "Call") -> Value:
    an, st = c.an, c.state
    fs = _frames(an, c.recv)
    col = c.arg(0, "column")
    ks = an.lit_keys(col, st) if col is not None else None
    if ks is not None:
        _require_cols(c, fs, ks)
    return _new_frame(an, c, _recv_row(c))


def _filter(c: "Call") -> Value:
    an, st = c.an, c.state
    items = c.kw("items") or c.arg(0)
    row = _recv_row(c)
    if items is not None:
        ks = an.lit_keys(items, st)
        if ks is not None:
            return _new_frame(an, c, row.project(ks & row.present if row.closed else ks))
    return _new_frame(an, c, Row(present=row.present, sources=row.sources | frozenset({(Source(f"filter@L{c.line}", "call"), ())}), mutators=row.mutators))


def _reindex(c: "Call") -> Value:
    an, st = c.an, c.state
    cols = c.kw("columns")
    row = _recv_row(c)
    if cols is not None:
        ks = an.lit_keys(cols, st)
        if ks is not None:
            return _new_frame(an, c, row.project(ks))
    return _new_frame(an, c, row)


def _select_dtypes(c: "Call") -> Value:
    row = _recv_row(c)
    return _new_frame(c.an, c, Row(present=row.present, sources=row.sources, mutators=row.mutators))


def _sort_values(c: "Call") -> Value:
    an, st = c.an, c.state
    fs = _frames(an, c.recv)
    by = c.kw("by") if c.kw("by") is not None else (c.arg(1, "columns") if c.qual.endswith(("nlargest", "nsmallest")) else c.arg(0))
    ks = an.lit_keys(by, st) if by is not None else None
    if ks is not None:
        _require_cols(c, fs, ks)
    return _new_frame(an, c, _recv_row(c), src=fs[0] if fs else None)


def _transform(c: "Call") -> Value:
    return _new_frame(c.an, c, _recv_row(c))


def _df_update(c: "Call") -> Value:
    an, st = c.an, c.state
    other = c.args[0] if c.args else None
    if other is not None:
        for f in _frames(an, c.recv):
            for g in _frames(an, other):
                st.set_row(f, "item", st.row(f, "item").copy_from(st.row(g, "item"), strong=False))
    return an.NONE


def _set_axis(c: "Call") -> Value:
    an, st = c.an, c.state
    axis = c.kw("axis")
    labels = c.arg(0, "labels")
    row = _recv_row(c)
    if axis is not None and isinstance(axis.kv, Lit) and axis.kv.key in (1, "columns") and labels is not None:
        ks = an.lit_keys(labels, st)
        if ks is not None:
            return _new_frame(an, c, row.project(ks))
        return _new_frame(an, c, Row(sources=frozenset({(Source(f"set_axis@L{c.line}", "call"), ())})))
    return _new_frame(an, c, row)


def _df_generic(c: "Call") -> Value:
    an, st = c.an, c.state
    name = c.qual.split(".")[-1]
    fs = _frames(an, c.recv)
    row = _recv_row(c)
    if name in NOEFFECT_OPS:
        return Value.data(f"L{c.line}")
    if name in SERIES_RESULT_OPS:
        return Value(frozenset({an.SERIES}), DATA)
    if name in COPY_FRAME_OPS:
        return _new_frame(an, c, row, src=fs[0] if fs else None)
    if name in OPEN_FRAME_OPS or name in ("pivot", "pivot_table", "stack", "unstack", "transpose", "describe",
                                           "corr", "cov", "agg", "aggregate"):
        return _open_frame_from(an, fs[0], c.node, st, name) if fs else Value.data(name)
    if name in ("get",):
        if c.args and isinstance(c.args[0].kv, Lit):
            return an.models.frame_getitem(an, fs[0], c.args[0], c.node, st, True) if fs else Value()
        return Value(frozenset({an.SERIES}), DATA)
    if name in ("items", "keys"):
        vid = an._alloc("view", f"view:{name}:{','.join(map(str, fs))}", "", True, name, payload=("items" if name == "items" else "keys", c.recv))
        an.objects[vid].payload = ("items" if name == "items" else "keys", c.recv)
        return Value.obj(vid)
    if name in ("rolling", "expanding", "ewm", "resample", "squeeze", "xs"):
        return _new_frame(an, c, row)
    an.note(f"unmodelled DataFrame.{name} at L{c.line}: result treated as an open frame derived from the receiver")
    return _new_frame(an, c, Row(present=row.present, likely=row.definite,
                                 sources=row.sources | frozenset({(Source(f"DataFrame.{name}@L{c.line}", "call"), ())}),
                                 mutators=row.mutators))


def _partial_call(c: "Call") -> Value:
    an = c.an
    outs = []
    for x in (c.recv.pts if c.recv else ()):
        o = an.objects[x]
        if o.kind != "partial":
            continue
        fn, bound_args, bound_kw = o.payload
        kw = dict(bound_kw)
        kw.update(c.kwargs)
        node = c.node
        fake_args = list(bound_args) + list(c.args)
        # build a call with override args; kwargs via a temporary Call
        for f in fn.pts:
            outs.append(an.call_one(f, fake_args, kw, c.star_args, c.star_kwargs, c.state, node))
    return join_values(outs) if outs else Value.data("partial")


# the Call dataclass lives in interp; import lazily for type checkers
from .interp import Call  # noqa: E402
