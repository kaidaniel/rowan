"""Iterators and aliasing, checked exhaustively against CPython (AGENTS.md, Generators: one-shot
iterators and live ones; Values / Objects for aliasing):

- one-shot iterators (`iter`, `map` with a function that marks `E`, `filter`, `reversed`,
  `enumerate`, `zip`, a generator expression) taken in two steps (`next`, a loop that may break,
  `sum`, `list`, `in`, `max`), with the list they read changed in between (append, pop, a store,
  clear) or not;
- live iterators: a loop over a list, dict or set that changes it (the size: RuntimeError for a
  dict or set; a store only: none), on every round, the first, or a path's only; in a try or not;
- aliasing: `+=` / `|=` and in-place methods on a name that is, may be, or is not the same object
  as another (list, dict, set, tuple, str), then the other name read.

Observation points (copies of a name, `O = r`: the tracer reads them once the line ran, and a
line that calls code would be read inside the callee): after each step (`O`, the items / lengths taken; `OE`, the trace of the
function `map` calls), at handlers, after the run.

    uv run --no-project python tests/iter_protocol.py [--rowan N | --all] [--seed S] [-v]
"""
from __future__ import annotations

import itertools

from protocol_lib import Prog, cli

NAMES = ["E", "O", "OE"]

SETUPS = {"fixed": ["xs = [1, 2, 3]"], "path": ["xs = [1, 2] if c1 else [1, 2, 3]"]}
MAKERS = {
    "iter": "iter(xs)",
    "map": "map(f{N}, xs)",
    "filter": "filter(lambda v: v != 2, xs)",
    "reversed": "reversed(xs)",
    "enumerate": "(i * 10 + v for i, v in enumerate(xs))",
    "zip": "(a * 10 + b for a, b in zip(it0, xs))",
    "genexp": "(v * 2 for v in xs)",
}
STEPS = {
    "next": ["r = next(it, -1)", "O = r"],
    "next2": ["r = next(it)", "O = r", "r = next(it)", "O = r"],
    "loop": ["for v in it:", "    O = v", "    if v == 2 or c2:", "        break"],
    "sum": ["r = sum(it)", "O = r"],
    "list": ["r = len(list(it))", "O = r"],
    "in": ["r = 2 in it", "O = r"],
    "max": ["r = max(it, default=-1)", "O = r"],
}
CHANGES = {"none": [], "append": ["xs.append(4)"], "pop": ["xs.pop()"], "store": ["xs[0] = 9"], "clear": ["xs.clear()"]}


def iter_program(setup, maker, s1, change, s2) -> Prog:
    lines = ["def f{N}(v):", "    global E", "    E = E * 10 + v", "    return v", ""]
    body = list(SETUPS[setup])
    if maker == "zip":
        body.append("it0 = iter([1, 2])")
    body += [f"it = {MAKERS[maker]}", *STEPS[s1], "OE = E", *CHANGES[change], *STEPS[s2], "OE = E"]
    lines += ["def p{N}():", "    global E, O, OE", *("    " + ln for ln in body)]
    return Prog(lines, end=["O = O", "OE = E"], desc=f"{setup} {maker} {s1} {change} {s2}")


CONTAINERS = {
    "list": ("c = [1, 2, 3]", {"append": "c.append(9)", "store": "c[0] = 9", "pop": "c.pop()", "remove": "c.remove(v)"}),
    "dict": ("c = {1: 1, 2: 2, 3: 3}", {"add": "c[9] = 9", "store": "c[v] = 0", "del": "del c[v]", "pop": "c.pop(v)", "update": "c.update({1: 5})"}),
    "set": ("c = {1, 2, 3}", {"add": "c.add(9)", "readd": "c.add(v)", "discard": "c.discard(v)", "clear": "c.clear()"}),
}
WHEN = {"always": "True", "first": "n == 1", "c1": "c1"}


def live_program(kind, change, when, guarded) -> Prog:
    make, changes = CONTAINERS[kind]
    loop = ["n = 0", "for v in c:", "    n = n + 1", "    if n > 5:", "        break", f"    if {WHEN[when]}:", f"        {changes[change]}", "    O = n"]
    if guarded:
        loop = ["try:", *("    " + ln for ln in loop), "except RuntimeError:", "    r = -n", "    O = r", "    E = 1"]
    body = [make, *loop, "O = n", "r = len(c)", "O = r"]
    lines = ["def p{N}():", "    global E, O, OE", *("    " + ln for ln in body)]
    return Prog(lines, end=["O = O", "OE = E"], desc=f"live {kind} {change} {when} {guarded}")


ALIAS_TYPES = {
    "list": ("[1]", "[2]", "append(3)"),
    "dict": ("{1: 1}", "{2: 2}", "update({3: 3})"),
    "set": ("{1}", "{2}", "add(3)"),
    "tuple": ("(1,)", "(2,)", None),
    "str": ("'a'", "'b'", None),
}
ALIASES = {"same": "b = a", "maybe": "b = a if c1 else {copy}", "copy": "b = {copy}"}
OPS = {"iadd": "b += {other}", "ior": "b |= {other}", "rebind": "b = b + {other}", "method": "b.{method}"}


def alias_program(kind, alias, op) -> Prog:
    lit, other, method = ALIAS_TYPES[kind]
    copy = {"list": "list(a)", "dict": "dict(a)", "set": "set(a)", "tuple": "tuple(list(a))", "str": "''.join(a)"}[kind]
    stmt = OPS[op].format(other=other, method=method)
    body = [f"a = {lit}", ALIASES[alias].format(copy=copy), stmt, "r = len(a)", "O = r", "r = len(b)", "O = r", "r = a is b", "O = r"]
    lines = ["def p{N}():", "    global E, O, OE", *("    " + ln for ln in body)]
    return Prog(lines, end=["O = O"], desc=f"alias {kind} {alias} {op}")


def programs():
    for setup, maker, s1, change, s2 in itertools.product(SETUPS, MAKERS, STEPS, CHANGES, STEPS):
        yield iter_program(setup, maker, s1, change, s2)
    for kind, (_, changes) in CONTAINERS.items():
        for change, when, guarded in itertools.product(changes, WHEN, (False, True)):
            yield live_program(kind, change, when, guarded)
    for kind, alias, op in itertools.product(ALIAS_TYPES, ALIASES, OPS):
        if op == "method" and ALIAS_TYPES[kind][2] is None:
            continue
        yield alias_program(kind, alias, op)


if __name__ == "__main__":
    cli(programs, NAMES)
