"""The data model, checked exhaustively against CPython (AGENTS.md, Calls: Builtins run the dunders
Python calls for them, `builtin_dunder`; Mixed values; library code given the program's
instances, `library_dunders`): which dunders an operation runs, in which order, with which
fallbacks, and what it raises.

- a class `KN` (its `__init__` sets `v`) defining one of the sets of dunders `CLASSES` (all of
  them; comparisons with `__hash__`; `__len__` / `__getitem__` alone; `__eq__` alone, which makes
  it unhashable; `__gt__` alone; none; the attribute hooks), each logging its name into `EN`;
- one operation (`OPS`) on instances `a` (v=1) and `b` (v=2): operators and their reflected and
  in-place forms, comparisons, truth, `len` / `int` / `float` / `abs` / `hash` / `repr` / `str`
  / formatting, containers of them sorted, compared, searched, hashed into a dict or set,
  `sum` / `max` / `min`, iteration, unpacking, `reversed`, `in`, subscripts, calls, `with`,
  attributes read / set / deleted, `copy.copy`;
- run in `pN` itself or in a helper given `a` and `b` (`WHERE`), in a `try` that catches what it
  raises.

Observation points: the log's length and its text after the operation, and the result. Each
batch runs under the default limits and `budget=0` (the helper skipped).

`--pairs`: one of `FIRST` (an operation that changes `a` / `b` or runs their dunders) then any
operation, each in its own `try`, on the same instances; observed after the second.

    uv run --no-project python tests/dunder_protocol.py [--pairs] [--rowan N | --all] [--seed S] [-v]
"""
from __future__ import annotations

import itertools

from protocol_lib import Prog, cli

NAMES = ["O"]

OTHER = "(o.v if isinstance(o, K{N}) else o)"
DUNDERS = {
    "__eq__": ["def __eq__(self, o):", "    E{N}.append('eq')", f"    return self.v == {OTHER}"],
    "__hash__": ["def __hash__(self):", "    E{N}.append('hash')", "    return self.v"],
    "__lt__": ["def __lt__(self, o):", "    E{N}.append('lt')", f"    return self.v < {OTHER}"],
    "__gt__": ["def __gt__(self, o):", "    E{N}.append('gt')", f"    return self.v > {OTHER}"],
    "__repr__": ["def __repr__(self):", "    E{N}.append('repr')", "    return 'K'"],
    "__str__": ["def __str__(self):", "    E{N}.append('str')", "    return 'k'"],
    "__format__": ["def __format__(self, spec):", "    E{N}.append('format')", "    return 'f'"],
    "__add__": ["def __add__(self, o):", "    E{N}.append('add')", f"    return self.v + {OTHER}"],
    "__radd__": ["def __radd__(self, o):", "    E{N}.append('radd')", f"    return {OTHER} + self.v"],
    "__iadd__": ["def __iadd__(self, o):", "    E{N}.append('iadd')", f"    self.v = self.v + {OTHER}", "    return self"],
    "__neg__": ["def __neg__(self):", "    E{N}.append('neg')", "    return -self.v"],
    "__abs__": ["def __abs__(self):", "    E{N}.append('abs')", "    return self.v"],
    "__bool__": ["def __bool__(self):", "    E{N}.append('bool')", "    return self.v > 1"],
    "__len__": ["def __len__(self):", "    E{N}.append('len')", "    return self.v"],
    "__int__": ["def __int__(self):", "    E{N}.append('int')", "    return self.v"],
    "__float__": ["def __float__(self):", "    E{N}.append('float')", "    return 1.5"],
    "__index__": ["def __index__(self):", "    E{N}.append('index')", "    return self.v"],
    "__iter__": ["def __iter__(self):", "    E{N}.append('iter')", "    return iter([self.v, self.v])"],
    "__reversed__": ["def __reversed__(self):", "    E{N}.append('reversed')", "    return iter([2, 1])"],
    "__getitem__": ["def __getitem__(self, k):", "    E{N}.append('getitem')", "    if isinstance(k, int) and k >= self.v:", "        raise IndexError(k)", "    return self.v"],
    "__setitem__": ["def __setitem__(self, k, x):", "    E{N}.append('setitem')"],
    "__delitem__": ["def __delitem__(self, k):", "    E{N}.append('delitem')"],
    "__contains__": ["def __contains__(self, x):", "    E{N}.append('contains')", "    return x == self.v"],
    "__call__": ["def __call__(self, *a):", "    E{N}.append('call')", "    return self.v"],
    "__enter__": ["def __enter__(self):", "    E{N}.append('enter')", "    return self.v"],
    "__exit__": ["def __exit__(self, *e):", "    E{N}.append('exit')", "    return False"],
    "__getattr__": ["def __getattr__(self, n):", "    E{N}.append('getattr')", "    if n.startswith('__'):", "        raise AttributeError(n)", "    return 0"],
    "__setattr__": ["def __setattr__(self, n, x):", "    E{N}.append('setattr')", "    object.__setattr__(self, n, x)"],
    "__delattr__": ["def __delattr__(self, n):", "    E{N}.append('delattr')", "    object.__delattr__(self, n)"],
    "__copy__": ["def __copy__(self):", "    E{N}.append('copy')", "    return K{N}(self.v)"],
}

CLASSES = {
    "full": [d for d in DUNDERS if d not in ("__getattr__", "__setattr__", "__delattr__")],
    "cmp": ["__eq__", "__hash__", "__lt__"],
    "seq": ["__len__", "__getitem__"],
    "eqonly": ["__eq__"],
    "gt": ["__gt__"],
    "plain": [],
    "attr": ["__getattr__", "__setattr__", "__delattr__", "__eq__", "__hash__"],
}

# the operation: lines binding `r` (an int, a bool, a str, or anything: observed by its type)
OPS = {
    "eq": ["r = a == b"],
    "ne": ["r = a != b"],
    "eq_lit": ["r = 1 == a"],
    "lt": ["r = a < b"],
    "gt": ["r = a > b"],
    "le": ["r = a <= b"],
    "lt_lit": ["r = 1 < a"],
    "add": ["r = a + b"],
    "radd": ["r = 1 + a"],
    "iadd": ["x = a", "x += b", "r = x is a"],
    "neg": ["r = -a"],
    "abs": ["r = abs(a)"],
    "truth": ["r = 1 if a else 0"],
    "not": ["r = not a"],
    "bool": ["r = bool(b)"],
    "len": ["r = len(a)"],
    "int": ["r = int(a)"],
    "float": ["r = float(a)"],
    "index": ["r = [5, 6, 7][a]"],
    "hash": ["r = hash(a)"],
    "repr": ["r = repr(a)"],
    "str": ["r = str(a)"],
    "fstr": ["r = f'{a}'"],
    "fmt_spec": ["r = format(a, '')"],
    "percent": ["r = '%s' % (a,)"],
    "list_repr": ["r = str([a])"],
    "sorted": ["r = len(sorted([b, a]))"],
    "sort": ["xs = [b, a]", "xs.sort()", "r = xs[0] is a"],
    "max": ["r = max(a, b) is b"],
    "min": ["r = min([a, b]) is a"],
    "sum": ["r = sum([a, b])"],
    "in_list": ["r = a in [b, 1]"],
    "in_self": ["r = 1 in a"],
    "count": ["r = [b, a].count(a)"],
    "index_of": ["r = [b, a].index(a)"],
    "list_eq": ["r = [a] == [b]"],
    "tuple_lt": ["r = (a, 1) < (b, 2)"],
    "dict_key": ["d = {a: 1}", "r = d.get(b, 0)"],
    "set": ["r = len({a, b})"],
    "in_set": ["r = a in {b}"],
    "iter": ["r = len(list(a))"],
    "for": ["r = 0", "for x in a:", "    r = r + 1"],
    "unpack": ["x, y = a", "r = x"],
    "reversed": ["r = len(list(reversed(a)))"],
    "getitem": ["r = a[0]"],
    "setitem": ["a[0] = 1", "r = 0"],
    "delitem": ["del a[0]", "r = 0"],
    "call": ["r = a(1)"],
    "with": ["with a as x:", "    r = x"],
    "getattr": ["r = a.missing"],
    "getattr_fn": ["r = getattr(a, 'missing', 9)"],
    "hasattr": ["r = hasattr(a, 'missing')"],
    "setattr": ["a.w = 3", "r = a.w"],
    "delattr": ["a.w = 3", "del a.w", "r = 0"],
    "copy": ["r = copy.copy(a) is a"],
}

WHERE = ["here", "helper"]


def program(cls, op, where, first=None) -> Prog:
    lines = ["import copy", "E{N} = []", "", "class K{N}:", "    def __init__(self, v):", "        object.__setattr__(self, 'v', v)"]
    for d in CLASSES[cls]:
        lines += ["    " + ln for ln in DUNDERS[d]]
    body = ["try:", *("    " + ln for ln in OPS[op]), "except Exception:", "    r = -1"]
    if first:
        body = ["try:", *("    " + ln for ln in OPS[first]), "except Exception:", "    r = -1", *body]
    obs = ["n = len(E{N})", "O = n", "t = ','.join(E{N})", "O = t", "t = r if isinstance(r, (bool, int, str)) else 'obj'", "O = t"]
    lines += ["", "def h{N}(a, b):", "    global O", "    r = 0", *("    " + ln for ln in body + obs), "    return r", ""]
    lines += ["def p{N}():", "    global O", "    a = K{N}(1)", "    b = K{N}(2)", "    E{N}.clear()"]
    if where == "here":
        lines += ["    r = 0", *("    " + ln for ln in body + obs)]
    else:
        lines += ["    r = h{N}(a, b)", *("    " + ln for ln in obs)]
    return Prog(lines, end=["O = O"], desc=f"{cls} {first + '+' if first else ''}{op} {where}")


def programs():
    for cls, op, where in itertools.product(CLASSES, OPS, WHERE):
        yield program(cls, op, where)


# the first operation of a pair: those that change the instances or the log's dunders' state
FIRST = ["iadd", "setattr", "delattr", "sort", "setitem", "delitem", "with", "copy", "getattr", "hasattr", "eq", "hash",
         "dict_key", "bool", "iter"]


def pair_programs():
    for cls, first, op, where in itertools.product(CLASSES, FIRST, OPS, WHERE):
        yield program(cls, op, where, first)


LIMITS = (None, "budget=0")

if __name__ == "__main__":
    cli(programs, NAMES, limit_sets=LIMITS, pairs=pair_programs)
