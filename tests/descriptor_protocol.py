"""Descriptors, checked exhaustively against CPython (AGENTS.md, Calls: properties are data
descriptors, `Heap::prop_names`; Soundness: descriptors beyond properties):
- a class `AN` whose attribute `v` is one of `SHAPES`: a data descriptor (`__get__` / `__set__` /
  `__delete__`, also without `__delete__`, also naming itself by `__set_name__` and keeping the
  value in the instance's `__dict__`), a non-data descriptor (`__get__` only; beside an instance
  attribute of the same name its `__init__` sets), a data descriptor beside such an instance
  attribute, a descriptor returning itself on the class, a property made by `property(...)`, a
  `functools.cached_property` counting its runs, a classmethod, a staticmethod, a plain
  function, a `__slots__` member; with a subclass `BN` that adds nothing, sets `v = 2`, or
  defines its own non-data descriptor;
- a receiver (`RECV`): `AN()`, `BN()`, one or the other by `c1`;
- one operation (`OPS`): reads (`o.v`, twice, `AN.v`, `type(o).v`, `getattr` with a default,
  `hasattr`, `o.__dict__`'s `v`), calls (`o.v()`), a store then a read (also by `setattr`,
  also `o.__dict__['v'] = 7` then a read), `del` then a read, a store on the class then a read;
- in a `try` catching AttributeError and TypeError; and the same operation in `qN(o: AN)` for
  any call, called with each receiver.
Observation points: the result, then how many times a counting `__get__` / getter ran.

Each batch runs under every limit set: the default and `budget=0`.

    uv run --no-project python tests/descriptor_protocol.py [--rowan N | --all] [--seed S] [--limits L] [-v]
"""
from __future__ import annotations

import itertools

from protocol_lib import Prog, cli

NAMES = ["O", "OK"]

# the descriptor classes every program defines
DESCR = [
    "class Data{N}:",
    "    def __get__(self, obj, owner=None):",
    "        global OK",
    "        OK = OK + 1",
    "        if obj is None:",
    "            return 40",
    "        return obj.__dict__.get('_v', 1) + 10",
    "    def __set__(self, obj, value):",
    "        obj.__dict__['_v'] = value * 2",
    "    def __delete__(self, obj):",
    "        obj.__dict__.pop('_v', None)",
    "",
    "class NoDel{N}:",
    "    def __get__(self, obj, owner=None):",
    "        return 41 if obj is None else obj.__dict__.get('_v', 3)",
    "    def __set__(self, obj, value):",
    "        obj.__dict__['_v'] = value + 1",
    "",
    "class Named{N}:",
    "    def __set_name__(self, owner, name):",
    "        self.key = '_' + name",
    "    def __get__(self, obj, owner=None):",
    "        if obj is None:",
    "            return self",
    "        return getattr(obj, self.key, 6)",
    "    def __set__(self, obj, value):",
    "        setattr(obj, self.key, value * 3)",
    "",
    "class NonData{N}:",
    "    def __get__(self, obj, owner=None):",
    "        global OK",
    "        OK = OK + 1",
    "        return 50 if obj is None else 51",
    "",
    "class Other{N}:",
    "    def __get__(self, obj, owner=None):",
    "        return 60",
    "",
]

# name: the class body of A{N} (after `class A{N}:`)
SHAPES = {
    "data": ["v = Data{N}()"],
    "nodel": ["v = NoDel{N}()"],
    "named": ["v = Named{N}()"],
    "nondata": ["v = NonData{N}()"],
    "nondata_shadow": ["v = NonData{N}()", "def __init__(self):", "    self.v = 3"],
    "data_dict": ["v = Data{N}()", "def __init__(self):", "    self.__dict__['v'] = 3"],
    "prop_call": ["def _g(self):", "    return 70", "def _s(self, x):", "    self.__dict__['_p'] = x", "v = property(_g, _s)"],
    "cached": ["@functools.cached_property", "def v(self):", "    global OK", "    OK = OK + 1", "    return 80"],
    "classmethod": ["@classmethod", "def v(cls):", "    return 90 if cls is A{N} else 91"],
    "staticmethod": ["@staticmethod", "def v():", "    return 95"],
    "function": ["def v(self):", "    return 99"],
    "slots": ["__slots__ = ('v',)"],
}

SUBS = {
    "plain": ["class B{N}(A{N}):", "    pass"],
    "over": ["class B{N}(A{N}):", "    v = 2"],
    "own": ["class B{N}(A{N}):", "    v = Other{N}()"],
}

RECV = {"A": "A{N}()", "B": "B{N}()", "either": "A{N}() if c1 else B{N}()"}

OPS = {
    "read": ["O = o.v"],
    "read2": ["o.v", "O = o.v"],
    "cls": ["O = A{N}.v"],
    "type": ["O = type(o).v"],
    "getattr": ["O = getattr(o, 'v', -1)"],
    "hasattr": ["O = hasattr(o, 'v')"],
    "dict": ["O = o.__dict__.get('v', -1)"],
    "call": ["O = o.v()"],
    "store": ["o.v = 5", "O = o.v"],
    "setattr": ["setattr(o, 'v', 5)", "O = o.v"],
    "dictstore": ["o.__dict__['v'] = 7", "O = o.v"],
    "del": ["del o.v", "O = o.v"],
    "delonly": ["del o.v", "O = 0"],
    "clsstore": ["A{N}.v = 9", "O = o.v"],
}


def body(op) -> list:
    """The operation in a try, its result then observed (an observation line must not raise)."""
    return ["try:", *("    " + ln.replace("O = ", "r = ") for ln in OPS[op]), "except AttributeError:", "    r = 'attr'",
            "except TypeError:", "    r = 'type'", "O = r", "O = OK"]


def program(shape, sub, recv, op) -> Prog:
    ind = lambda ls: ["    " + ln for ln in ls]
    lines = ["import functools", "", *DESCR, "class A{N}:", *ind(SHAPES[shape]), "", *SUBS[sub], "",
             "def p{N}():", "    global O, OK", f"    o = {RECV[recv]}", *ind(body(op)), "",
             "def q{N}(o: A{N}):", "    global O, OK", *ind(body(op)), ""]
    calls = [RECV["A"], RECV["B"]] if recv == "either" else [RECV[recv]]
    end = [f"run(lambda: q{{N}}({c}))" for c in calls]
    return Prog(lines, end=end + ["O = O"], desc=f"{shape} {sub} {recv} {op}")


def programs():
    for shape, sub, recv, op in itertools.product(SHAPES, SUBS, RECV, OPS):
        yield program(shape, sub, recv, op)


if __name__ == "__main__":
    cli(programs, NAMES, quick=3000, limit_sets=(None, "budget=0"))
