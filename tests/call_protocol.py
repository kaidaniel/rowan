"""Calls and skipped calls, checked exhaustively against CPython (AGENTS.md, Calls: the budget,
skipped calls and what `effects.rs` lets them change, result summaries, unknown calls):

- a callee `fN(a, d, o, acc=[])` whose body is one effect, or two of `PAIRED` (`EFFECTS`: a
  global, the list, dict or instance it is given, a default, a module-level list or name it gives
  its argument away to, a helper, a method (also by `getattr`), a class attribute, a closure, a
  generator, an operator / `str` / iteration / `with` / property running the instance's dunders,
  a raise; plain or under `if c1:`) and a result (`RESULTS`: none, a literal, an argument, a new
  list, a global, an attribute, an item, a closure);
- called by `pN` in one of the call shapes (`CALLS`: positional, keywords, `*args`, `**kw`, twice,
  through a name, in a loop, a wrapper, a `try`, a method taking it, `functools.partial`, a
  lambda, a dict of functions, a comprehension);
- then the caller uses what the call may have given away (the result, the keeper, the global
  name, a stored callback) and observes the lengths, the attribute, `G`, the class attribute, a
  module dict and the result.

Each batch runs under every limit set (`LIMITS`): the default, `budget=0` (calls skipped: what
large programs fall back to) and `budget=6` (the first runs, later ones skipped). Rowan's claims
must cover CPython's runs in all of them.

    uv run --no-project python tests/call_protocol.py [--rowan N | --all] [--seed S] [--limits L] [-v]
"""
from __future__ import annotations

import itertools

from protocol_lib import Prog, cli

NAMES = ["E", "G", "O", "A"]

EFFECTS = {
    "g": ["G = G * 10 + 1"],
    "app": ["a.append(1)"],
    "pop": ["a.pop()"],
    "clear": ["a.clear()"],
    "dset": ["d['k'] = 1"],
    "ddel": ["del d['x']"],
    "dpop": ["d.pop('x', None)"],
    "attr": ["o.v = 2"],
    "delattr": ["del o.v"],
    "keep": ["K{N}.append(a)"],
    "give": ["A = a"],
    "acc": ["acc.append(1)", "G = G * 10 + len(acc)"],
    "helper": ["h{N}(a, o)"],
    "method": ["o.m(a)"],
    "boom": ["boom()"],
    "rebind": ["a = [5, 5]", "a.append(1)"],
    "setattr": ["setattr(o, 'v', 3)"],
    "getattr": ["getattr(o, 'm')(a)"],
    "gdict": ["GD{N}['k'] = len(a)"],
    "cls": ["B{N}.count = B{N}.count + 1"],
    "typecls": ["type(o).count = type(o).count + 2"],
    "store": ["d['a'] = a"],
    "nested": ["d['l'] = []", "d['l'].append(a)"],
    "cb": ["o.cb = lambda: a.append(3)"],
    "caught": ["try:", "    boom()", "except KeyError:", "    G = G * 10 + 2"],
    "loop": ["for x in list(a):", "    d[x] = x"],
    "rec": ["if len(a) < 3:", "    f{N}(a, d, o)"],
    "dunder": ["G = G * 10 + (o + 1)"],
    "iter": ["for x in o:", "    G = G * 10 + x"],
    "with": ["with o:", "    a.append(2)"],
    "prop": ["o.p = 5"],
    "str": ["G = G * 10 + len(str(o))"],
    "clsm": ["type(o).bump()"],
    "inner": ["def inner():", "    a.append(2)", "    o.v = 6", "inner()"],
    "gen": ["G = G * 10 + sum(gn{N}(a, o))"],
}
# pairs only of these (every single effect alone)
PAIRED = ["g", "app", "pop", "dset", "ddel", "attr", "keep", "give", "acc", "helper", "method", "boom", "getattr", "store", "dunder", "gen"]
CONDS = ["plain", "c1"]
RESULTS = {
    "none": [],
    "lit": ["return 3"],
    "arg": ["return a"],
    "new": ["return [7, 7]"],
    "glob": ["return G"],
    "attr": ["return o.v"],
    "dict": ["return d"],
    "elem": ["return a[0] if a else -3"],
    "fn": ["return lambda: a.append(3)"],
}

ARGS = "xs, dd, ob"
CALLS = {
    "pos": ["r = f{N}(xs, dd, ob)"],
    "kw": ["r = f{N}(o=ob, a=xs, d=dd)"],
    "star": ["t = (xs, dd, ob)", "r = f{N}(*t)"],
    "starkw": ["kw = {'a': xs, 'd': dd, 'o': ob}", "r = f{N}(**kw)"],
    "twice": ["r = f{N}(xs, dd, ob)", "r = f{N}(xs, dd, ob)"],
    "name": ["fn = f{N}", "r = fn(xs, dd, ob)"],
    "loop": ["for i in range(2):", "    r = f{N}(xs, dd, ob)"],
    "wrap": ["r = w{N}(xs, dd, ob)"],
    "try": ["r = 0", "try:", "    r = f{N}(xs, dd, ob)", "except Exception:", "    O = r", "    r = 4"],
    "bound": ["r = ob.call(f{N}, xs, dd)"],
    "partial": ["r = functools.partial(f{N}, xs)(dd, ob)"],
    "lambda": ["r = (lambda: f{N}(xs, dd, ob))()"],
    "table": ["table = {'f': f{N}}", "r = table['f'](xs, dd, ob)"],
    "comp": ["r = [f{N}(xs, dd, ob) for _ in range(1)][0]"],
}


def callee(effects, cond, result):
    body = [ln for e in effects for ln in EFFECTS[e]]
    if cond == "c1":
        body = ["if c1:", *("    " + ln for ln in body)]
    return ["def f{N}(a, d, o, acc=[]):", "    global G, A", *("    " + ln for ln in body + RESULTS[result])]


def program(effects, cond, result, call) -> Prog:
    lines = [
        "import functools",
        "K{N} = []",
        "GD{N} = {}",
        "",
        "class B{N}:",
        "    v = 0",
        "    count = 0",
        "    def m(self, a):",
        "        self.v = len(a)",
        "        a.append(6)",
        "    def call(self, f, a, d):",
        "        return f(a, d, self)",
        "    def __add__(self, x):",
        "        self.v = 7",
        "        return x",
        "    def __iter__(self):",
        "        self.v = 8",
        "        return iter([1, 2])",
        "    def __enter__(self):",
        "        self.v = 9",
        "        return self",
        "    def __exit__(self, *e):",
        "        self.v = self.v * 10 + 1",
        "        return False",
        "    @property",
        "    def p(self):",
        "        return self.v",
        "    @p.setter",
        "    def p(self, x):",
        "        self.v = x",
        "    def __str__(self):",
        "        self.v = 11",
        "        return 'b'",
        "    @classmethod",
        "    def bump(cls):",
        "        cls.count = cls.count + 3",
        "",
        "def gn{N}(a, o):",
        "    a.append(5)",
        "    yield 1",
        "    o.v = 12",
        "    yield 2",
        "",
        "def h{N}(a, o):",
        "    a.append(4)",
        "    o.v = 4",
        "",
        *callee(effects, cond, result),
        "",
        "def w{N}(a, d, o):",
        "    return f{N}(a, d, o)",
        "",
        "def p{N}():",
        "    global E, G, O, A",
        "    xs = [0]",
        "    dd = {'x': 0}",
        "    ob = B{N}()",
        *("    " + ln for ln in CALLS[call]),
        "    if isinstance(r, list):",
        "        r.append(8)",
        "    if isinstance(r, dict):",
        "        r['z'] = 1",
        "    if callable(r):",
        "        r()",
        "    for v in dd.values():",
        "        if isinstance(v, list):",
        "            v.append(9)",
        "    cb = getattr(ob, 'cb', None)",
        "    if cb:",
        "        cb()",
        "    for k in K{N}:",
        "        k.append(9)",
        "    if A:",
        "        A.append(9)",
        "    n = len(xs)",
        "    O = n",
        "    n = len(dd)",
        "    O = n",
        "    n = ob.v if hasattr(ob, 'v') else -1",
        "    O = n",
        "    O = G",
        "    n = B{N}.count",
        "    O = n",
        "    n = len(GD{N})",
        "    O = n",
        "    n = r if isinstance(r, int) else len(r) if isinstance(r, (list, dict)) else -2",
        "    O = n",
    ]
    return Prog(lines, end=["O = G", "O = O"], desc=f"{'+'.join(effects)} {cond} {result} {call}")


def programs():
    seqs = [(e,) for e in EFFECTS] + [p for p in itertools.product(PAIRED, repeat=2) if p[0] != "boom"]
    for effects in seqs:
        for cond, result, call in itertools.product(CONDS, RESULTS, CALLS):
            yield program(effects, cond, result, call)


LIMITS = (None, "budget=0", "budget=6")

if __name__ == "__main__":
    cli(programs, NAMES, limit_sets=LIMITS)
