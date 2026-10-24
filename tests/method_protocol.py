"""Methods, properties, `__call__` and `__init__` as the callee, checked exhaustively against
CPython (AGENTS.md, Calls: the budget and skipped calls, what `effects.rs` lets a skipped method
change through its receiver, `receiver_effects` / `must_stores`, result summaries; class
attributes by name; properties' setters and getters):

- a class `BN` whose callee (`KINDS`: a method `f(self, a)`, a property `f` whose getter has the
  effect, `__call__`, `__init__`, a classmethod `f(cls, a)`) has one effect (`EFFECTS`: an
  attribute of the instance stored, built on, deleted or set by `setattr`, a list or dict it
  holds changed or rebound, `+=` on its list, an attribute only some calls make, the class's
  attribute by name, by `type(self)` or `self.__class__`, the instance shadowing it, a property's
  setter, another method of its own, an attribute of an attribute, its own `__add__`, a global,
  the list it is given, a raise; plain or under `if c1:`; a method's two of `PAIRED`) and a
  result (`RESULTS`);
- called by `pN` in one of its kind's call shapes (`CALLS`: on the instance, through the class,
  by `getattr`, through a bound method held in a name, twice, through `super()` in a subclass
  that overrides it, on an instance of a subclass that does not, on two instances in a loop);
- then the caller observes the instance's attributes, its list's and dict's lengths, the class
  attribute, the attribute of its attribute, `G`, the argument's length and the result.

Each batch runs under every limit set (`LIMITS`): the default, `budget=0` and `budget=6`.

    uv run --no-project python tests/method_protocol.py [--rowan N | --all] [--seed S] [--limits L] [-v]
"""
from __future__ import annotations

import itertools

from protocol_lib import Prog, cli

NAMES = ["G", "O"]

# effects on the receiver `self` (`cls` in a classmethod: those marked `cls`), its argument `a`
EFFECTS = {
    "attr": ["self.v = 2"],
    "build": ["self.v = self.v * 10 + 3"],
    "del": ["del self.v"],
    "setattr": ["setattr(self, 'v', 4)"],
    "append": ["self.items.append(1)"],
    "rebind": ["self.items = [5, 5]"],
    "iadd": ["self.items += [6]"],
    "dict": ["self.d['k'] = 1"],
    "maybe_new": ["if len(a) > 1:", "    self.z = 1"],
    "cls_name": ["B{N}.count = B{N}.count + 1"],
    "cls_type": ["type(self).count = type(self).count + 2"],
    "cls_dunder": ["self.__class__.count += 3"],
    "shadow": ["self.count = 9"],
    "prop": ["self.p = 5"],
    "own": ["self.h()"],
    "deep": ["self.child.v = 6"],
    "dunder": ["self + 1"],
    "glob": ["G = G * 10 + 1"],
    "arg": ["a.append(len(a))"],
    "boom": ["boom()"],
}
# the effects a classmethod's `cls` has (the rest: an instance's)
CLS_EFFECTS = ["attr", "build", "setattr", "cls_name", "cls_type", "glob", "arg", "boom", "maybe_new"]
CONDS = ["plain", "c1"]
RESULTS = {
    "none": [],
    "self": ["return self"],
    "attr": ["return self.v"],
    "items": ["return self.items"],
    "new": ["return B{N}()"],
    "len": ["return len(a)"],
}

CALLS = {
    "method": {
        "plain": ["r = ob.f(xs)"],
        "cls": ["r = B{N}.f(ob, xs)"],
        "getattr": ["r = getattr(ob, 'f')(xs)"],
        "held": ["m = ob.f", "r = m(xs)"],
        "twice": ["r = ob.f(xs)", "r = ob.f(xs)"],
        "super": ["ob = S{N}()", "r = ob.f(xs)"],
        "inherit": ["ob = T{N}()", "r = ob.f(xs)"],
        "loop": ["for x in (ob, B{N}()):", "    r = x.f(xs)"],
    },
    "prop": {
        "plain": ["r = ob.f"],
        "getattr": ["r = getattr(ob, 'f')"],
        "twice": ["r = ob.f", "r = ob.f"],
        "inherit": ["ob = T{N}()", "r = ob.f"],
    },
    "call": {
        "plain": ["r = ob(xs)"],
        "twice": ["r = ob(xs)", "r = ob(xs)"],
        "inherit": ["ob = T{N}()", "r = ob(xs)"],
    },
    "init": {
        "plain": ["ob = B{N}(xs)", "r = None"],
        "twice": ["ob = B{N}(xs)", "ob = B{N}(xs)", "r = None"],
        "inherit": ["ob = T{N}(xs)", "r = None"],
    },
    "classmethod": {
        "plain": ["r = ob.f(xs)"],
        "cls": ["r = B{N}.f(xs)"],
        "twice": ["r = B{N}.f(xs)", "r = ob.f(xs)"],
        "inherit": ["ob = T{N}()", "r = ob.f(xs)"],
    },
}


# pairs of these for a method (every single effect alone, for every kind)
PAIRED = ["attr", "build", "del", "append", "rebind", "cls_type", "prop", "own", "dunder", "glob", "arg", "boom"]


def callee(kind, effect, cond, result) -> list:
    body = [ln for e in effect.split("+") for ln in EFFECTS[e]]
    if cond == "c1":
        body = ["if c1:", *("    " + ln for ln in body)]
    ret = list(RESULTS[result])
    if kind == "prop":
        return ["@property", "def f(self):", "    global G", "    a = self.items", *("    " + ln for ln in body + ret)]
    if kind == "call":
        return ["def __call__(self, a):", "    global G", *("    " + ln for ln in body + ret)]
    if kind == "classmethod":
        body = [ln.replace("self", "cls") for ln in body]
        ret = [ln.replace("self", "cls") for ln in ret]
        return ["@classmethod", "def f(cls, a):", "    global G", *("    " + ln for ln in body + ret)]
    return ["def f(self, a):", "    global G", *("    " + ln for ln in body + ret)]


def program(kind, effect, cond, result, call) -> Prog:
    init = ["def __init__(self, a=None):", "    global G", "    self.v = 1", "    self.items = []", "    self.d = {}", "    self.child = C{N}()"]
    if kind == "init":
        init += ["    if a is not None:", *("    " + ln for ln in callee("method", effect, cond, "none")[2:])]
    lines = [
        "class C{N}:",
        "    v = 0",
        "",
        "class B{N}:",
        "    count = 0",
        "    v = 0",
        "    items = []",
        *("    " + ln for ln in init),
        "    @property",
        "    def p(self):",
        "        return self.v",
        "    @p.setter",
        "    def p(self, x):",
        "        self.v = x",
        "    def h(self):",
        "        self.w = 4",
        "    def __add__(self, x):",
        "        self.v = 7",
        "        return x",
        *("    " + ln for ln in (callee(kind, effect, cond, result) if kind != "init" else [])),
        "",
        "class S{N}(B{N}):",
        "    def f(self, a):",
        "        r = super().f(a)",
        "        self.s = 1",
        "        return r",
        "",
        "class T{N}(B{N}):",
        "    pass",
        "",
        "def p{N}():",
        "    global G, O",
        "    xs = [0]",
        "    ob = B{N}()",
        *("    " + ln for ln in CALLS[kind][call]),
        "    n = r.v if isinstance(r, B{N}) else r if isinstance(r, int) else len(r) if isinstance(r, list) else -2",
        "    O = n",
        "    n = getattr(ob, 'v', -1)",
        "    O = n",
        "    n = len(ob.items)",
        "    O = n",
        "    n = len(ob.d)",
        "    O = n",
        "    n = ob.count",
        "    O = n",
        "    n = B{N}.count",
        "    O = n",
        "    n = getattr(ob, 'z', -1)",
        "    O = n",
        "    n = getattr(ob, 'w', -1)",
        "    O = n",
        "    n = ob.child.v",
        "    O = n",
        "    n = len(xs)",
        "    O = n",
        "    O = G",
    ]
    return Prog(lines, end=["O = G"], desc=f"{kind} {effect} {cond} {result} {call}")


def programs():
    for kind, calls in CALLS.items():
        effects = CLS_EFFECTS if kind == "classmethod" else list(EFFECTS)
        if kind == "method":
            effects += [f"{a}+{b}" for a, b in itertools.product(PAIRED, repeat=2) if a != "boom"]
        results = ["none"] if kind == "init" else list(RESULTS)
        for effect, cond, result, call in itertools.product(effects, CONDS, results, calls):
            yield program(kind, effect, cond, result, call)


LIMITS = (None, "budget=0", "budget=6")

if __name__ == "__main__":
    cli(programs, NAMES, limit_sets=LIMITS)
