"""Classes and inheritance, checked exhaustively against CPython (AGENTS.md, Calls: a method on
any instance, class attributes by name, `class_resolves`, `__getattr__`, `new_instance`;
Soundness: `isinstance`; `classes.rs`, `attrs.rs`):
- a base `AN` (a class attribute `v`, an `__init__` setting `a`, a method `f`) and a subclass
  `BN` of one of the shapes (`SHAPES`): nothing added, `v` or `f` overridden, `super().f()`,
  an `__init__` calling `super().__init__()` or not or storing on a path, `__getattr__`,
  `__init_subclass__`, `__slots__`, a shared class-level list, a classmethod constructor, a
  property overridden by a class attribute, a property with a setter, a staticmethod, a mixin
  before the base, a per-class counter through `type(self)`, an instance attribute shadowing
  the class's then deleted, `__init__` calling an overridden method that reads what the
  subclass sets later, the instance shadowing `v` in `f`, a `__new__` storing before
  `__init__`, a property in the subclass over the base's class attribute;
- a receiver (`RECV`): `AN()`, `BN()`, one or the other by `c1`;
- one operation (`OPS`): attribute reads (`v`, `a`, `b`, `tag`, through `type(o)`, `getattr`
  with a default, `hasattr`, `__dict__`, `__name__`, `vars`), `f()` once or three times, a store
  then a read (on the instance, by `setattr`, on the class; then through `type(o)`), `del` then
  a read (on the instance, on the class), `isinstance`, the classmethod, the unbound method,
  two-argument `super`, a method only one shape has, two constructions then the counter;
- in a `try` catching AttributeError and TypeError; and the same operation in `qN(o: AN)` for
  any call, called with each receiver.
Observation points: the result (`O = r` after the `try`), then the classes' own `v` (by
`getattr` with a default: an observation line must not raise).
    uv run --no-project python tests/class_protocol.py [--rowan N | --all] [--seed S] [--limits L] [-v]
"""
from __future__ import annotations

import itertools

from protocol_lib import Prog, cli

NAMES = ["O", "OA", "OB"]

BASE = ["class A{N}:", "    v = 1", "    def __init__(self):", "        self.a = 1", "    def f(self):", "        return self.v"]

# name: (the base's lines, the subclass's lines)
SHAPES = {
    "plain": (BASE, ["class B{N}(A{N}):", "    pass"]),
    "over_v": (BASE, ["class B{N}(A{N}):", "    v = 2"]),
    "over_f": (BASE, ["class B{N}(A{N}):", "    def f(self):", "        return self.v + 10"]),
    "super_f": (BASE, ["class B{N}(A{N}):", "    def f(self):", "        return super().f() + 10"]),
    "init_super": (BASE, ["class B{N}(A{N}):", "    def __init__(self):", "        super().__init__()", "        self.b = 2"]),
    "init_nosuper": (BASE, ["class B{N}(A{N}):", "    def __init__(self):", "        self.b = 2"]),
    "init_path": (BASE, ["class B{N}(A{N}):", "    def __init__(self):", "        super().__init__()", "        if c1:", "            self.b = 2"]),
    "getattr": ([*BASE, "    def __getattr__(self, name):", "        return 9"], ["class B{N}(A{N}):", "    pass"]),
    "init_subclass": ([*BASE, "    def __init_subclass__(cls, **kw):", "        cls.tag = 7"], ["class B{N}(A{N}):", "    pass"]),
    "slots": (["class A{N}:", "    __slots__ = ('a',)", "    v = 1", "    def __init__(self):", "        self.a = 1", "    def f(self):", "        return self.v"],
              ["class B{N}(A{N}):", "    __slots__ = ('b',)"]),
    "shared": ([*BASE, "    items = []", "    def g(self):", "        self.items.append(1)", "        return len(self.items)"],
               ["class B{N}(A{N}):", "    def f(self):", "        return self.g() * 10"]),
    "classmethod": ([*BASE, "    @classmethod", "    def make(cls):", "        return cls()"], ["class B{N}(A{N}):", "    v = 2"]),
    "prop": (["class A{N}:", "    def __init__(self):", "        self.a = 1", "    @property", "    def v(self):", "        return self.a + 100", "    def f(self):", "        return self.v"],
             ["class B{N}(A{N}):", "    v = 2"]),
    "setter": (["class A{N}:", "    def __init__(self):", "        self.a = 1", "    @property", "    def v(self):", "        return self.a", "    @v.setter", "    def v(self, x):", "        self.a = x * 2", "    def f(self):", "        return self.v"],
               ["class B{N}(A{N}):", "    pass"]),
    "static": ([*BASE, "    @staticmethod", "    def s():", "        return 5"], ["class B{N}(A{N}):", "    def f(self):", "        return self.s() + self.v"]),
    "mixin": (BASE, ["class X{N}:", "    v = 3", "    def f(self):", "        return self.v * 100", "class B{N}(X{N}, A{N}):", "    pass"]),
    "counter": (["class A{N}:", "    v = 1", "    count = 0", "    def __init__(self):", "        self.a = 1", "        type(self).count += 1", "    def f(self):", "        return self.count"],
                ["class B{N}(A{N}):", "    pass"]),
    "shadow_del": (["class A{N}:", "    v = 1", "    def __init__(self):", "        self.a = 1", "        self.v = 5", "    def f(self):", "        del self.v", "        return self.v"],
                   ["class B{N}(A{N}):", "    v = 2"]),
    "init_calls_f": (["class A{N}:", "    v = 1", "    def __init__(self):", "        self.a = self.f()", "    def f(self):", "        return self.v"],
                     ["class B{N}(A{N}):", "    def __init__(self):", "        super().__init__()", "        self.b = 2", "    def f(self):", "        return self.b"]),
    "self_shadow": (["class A{N}:", "    v = 1", "    def __init__(self):", "        self.a = 1", "    def f(self):", "        self.v = self.v + 1", "        return self.v"],
                    ["class B{N}(A{N}):", "    pass"]),
    "new": (["class A{N}:", "    v = 1", "    def __new__(cls):", "        inst = super().__new__(cls)", "        inst.n = 3", "        return inst", "    def __init__(self):", "        self.a = self.n", "    def f(self):", "        return self.a"],
            ["class B{N}(A{N}):", "    def __init__(self):", "        self.a = 4"]),
    "sub_prop": (BASE, ["class B{N}(A{N}):", "    @property", "    def v(self):", "        return self.a + 10"]),
}

RECV = {"A": "A{N}()", "B": "B{N}()", "either": "A{N}() if c1 else B{N}()"}

OPS = {
    "v": ["O = o.v"],
    "a": ["O = o.a"],
    "b": ["O = o.b"],
    "tag": ["O = o.tag"],
    "typev": ["O = type(o).v"],
    "getattr": ["O = getattr(o, 'b', -1)"],
    "hasattr": ["O = hasattr(o, 'b')"],
    "dict": ["O = o.__dict__.get('a', -1)"],
    "name": ["O = type(o).__name__"],
    "f": ["O = o.f()"],
    "f3": ["o.f()", "o.f()", "O = o.f()"],
    "store": ["o.v = 5", "O = o.v"],
    "del": ["del o.v", "O = o.v"],
    "isinst": ["O = isinstance(o, B{N})"],
    "make": ["O = o.make().v"],
    "unbound": ["O = A{N}.f(o)"],
    "twice": ["A{N}()", "B{N}()", "O = o.count"],
    "clsstore": ["A{N}.v = 9", "O = o.v"],
    "storetype": ["o.v = 5", "O = type(o).v"],
    "setattr": ["setattr(o, 'v', 5)", "O = o.v"],
    "delcls": ["del A{N}.v", "O = o.v"],
    "super2": ["O = super(B{N}, o).f()"],
    "vars": ["O = len(vars(o))"],
    "hasv": ["O = hasattr(o, 'v')"],
    "g": ["O = o.g()"],
}


def body(op) -> list:
    """The operation in a try, its result then observed (an observation line must not raise:
    the tracer would read the name's old value there)."""
    return ["try:", *("    " + ln.replace("O = ", "r = ") for ln in OPS[op]), "except AttributeError:", "    r = 'attr'",
            "except TypeError:", "    r = 'type'", "O = r"]


def program(shape, recv, op) -> Prog:
    base, sub = SHAPES[shape]
    lines = [*base, *sub, "", "def p{N}():", "    global O, OA, OB", f"    o = {RECV[recv]}", *("    " + ln for ln in body(op)),
             "    OA = getattr(A{N}, 'v', -1)", "    OB = getattr(B{N}, 'v', -1)", "",
             "def q{N}(o: A{N}):", "    global O, OA, OB", *("    " + ln for ln in body(op)), "    OA = getattr(A{N}, 'v', -1)", "    OB = getattr(B{N}, 'v', -1)", ""]
    calls = [RECV["A"], RECV["B"]] if recv == "either" else [RECV[recv]]
    end = [f"run(lambda: q{{N}}({c}))" for c in calls]
    return Prog(lines, end=end + ["O = O"], desc=f"{shape} {recv} {op}")


def programs():
    for shape, recv, op in itertools.product(SHAPES, RECV, OPS):
        yield program(shape, recv, op)


if __name__ == "__main__":
    cli(programs, NAMES, limit_sets=(None, "budget=0"))
