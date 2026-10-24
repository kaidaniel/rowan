"""Closures and `nonlocal`, checked exhaustively against CPython (AGENTS.md, Calls: cells, what a
skipped call may change through the cells it is given, `effects.rs` `Level` / `KeptPlace`; the
budget):

- a maker `mkN(a)` with a cell `x` (an int) and `acc` (a list) and two closures over them: `inc`,
  whose body is one or two effects (`EFFECTS`: `nonlocal x` built on or set under `c1`, the list
  appended to or rebound by `nonlocal`, the argument changed, a global, a local of the same name
  that changes no cell, a raise after a change), and `get`, which reads both cells;
- `inc` is called (`USES`) after the maker returned, inside the maker, through a global list it
  was stored in, through an attribute, by a helper it is given, after the maker rebound the
  cell, from a closure nested one level deeper, or alongside another maker call's own cells;
- the number of calls (`CALLS`: once, twice, under `c2`, in a loop, in a `try`);
- then the caller observes `get()` (the cells), `inc`'s last result, the argument's length, `G`.

Each batch runs under every limit set (`LIMITS`): the default, `budget=0` and `budget=6`.

    uv run --no-project python tests/closure_protocol.py [--rowan N | --all] [--seed S] [--limits L] [-v]
"""
from __future__ import annotations

import itertools

from protocol_lib import Prog, cli

NAMES = ["G", "O"]

EFFECTS = {
    "inc": ["nonlocal x", "x = x + 1"],
    "build": ["nonlocal x", "x = x * 10 + 2"],
    "cond": ["nonlocal x", "if c1:", "    x = x + 5"],
    "list": ["acc.append(len(acc))"],
    "rebind": ["nonlocal acc", "acc = acc + [9]"],
    "arg": ["a.append(1)"],
    "glob": ["global G", "G = G * 10 + 1"],
    "shadow": ["x = 7"],
    "boom": ["nonlocal x", "x = x + 3", "boom()"],
}
# pairs only of these (every single effect alone)
PAIRED = ["inc", "build", "list", "rebind", "glob", "boom"]

USES = ["later", "inside", "stored", "attr", "helper", "after", "nested", "two"]

CALLS = {
    "once": ["r = F()"],
    "twice": ["r = F()", "r = F()"],
    "cond": ["if c2:", "    r = F()"],
    "loop": ["for i in range(2):", "    r = F()"],
    "try": ["try:", "    r = F()", "except KeyError:", "    O = r", "    r = -1"],
}


def inc_def(effects) -> list:
    """`inc`'s definition: its `nonlocal` / `global` lines first, then the effects, then `x`."""
    lines = [ln for e in effects for ln in EFFECTS[e]]
    decl = [ln for ln in lines if ln.startswith(("nonlocal", "global"))]
    names = {"nonlocal": [], "global": []}
    for d in decl:
        kw, rest = d.split(" ", 1)
        names[kw] += [n for n in rest.split(", ") if n not in names[kw]]
    head = [f"{kw} {', '.join(ns)}" for kw, ns in names.items() if ns]
    body = [ln for ln in lines if ln not in decl]
    return ["def inc():", *("    " + ln for ln in head + body + ["return x"])]


def program(effects, use, call) -> Prog:
    calls = CALLS[call]
    ind = lambda ls, k=1: ["    " * k + ln for ln in ls]
    inc = inc_def(effects)
    if use == "nested":
        inner = ["def mid():", *ind(inc), "    return inc", "inc = mid()"]
    else:
        inner = inc
    mk = ["def mk{N}(a):", "    global G, O", "    r = 0", "    x = 1", "    acc = []", *ind(inner), "    def get():", "        return x * 100 + len(acc)"]
    pre = ["r = 0", "inc, get = mk{N}(xs)"]
    caller = [ln.replace("F()", "inc()") for ln in calls]
    if use == "inside":
        mk += ind([ln.replace("F()", "inc()") for ln in calls])
        caller = []
        pre = ["r = 0", "inc, get = mk{N}(xs)", "r = inc()"]
    elif use == "stored":
        mk += ["    H{N}.append(inc)"]
        caller = ["for h in H{N}:", *ind([ln.replace("F()", "h()") for ln in calls])]
    elif use == "attr":
        caller = ["ob = Ob{N}()", "ob.cb = inc", *[ln.replace("F()", "ob.cb()") for ln in calls]]
    elif use == "helper":
        caller = [ln.replace("F()", "ap{N}(inc)") for ln in calls]
    elif use == "after":
        mk += ["    x = 5", "    acc = [4]"]
    elif use == "two":
        pre = ["r = 0", "inc0, get = mk{N}(xs)", "inc, get2 = mk{N}(xs)", "inc0()"]
    mk += ["    return inc, get"]
    lines = [
        "H{N} = []",
        "",
        "class Ob{N}:",
        "    pass",
        "",
        "def ap{N}(f):",
        "    return f()",
        "",
        *mk,
        "",
        "def p{N}():",
        "    global G, O",
        "    xs = [0]",
        *ind(pre + caller),
        "    n = get()",
        "    O = n",
        "    O = r",
        "    n = len(xs)",
        "    O = n",
        "    O = G",
    ]
    return Prog(lines, end=["O = G"], desc=f"{'+'.join(effects)} {use} {call}")


def programs():
    seqs = [(e,) for e in EFFECTS] + [p for p in itertools.product(PAIRED, repeat=2) if p[0] != "boom"]
    for effects, use, call in itertools.product(seqs, USES, CALLS):
        yield program(effects, use, call)


LIMITS = (None, "budget=0", "budget=6")

if __name__ == "__main__":
    cli(programs, NAMES, limit_sets=LIMITS)
