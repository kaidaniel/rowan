"""Recursion, checked exhaustively against CPython (AGENTS.md, Calls: Recursion — a recursive
call takes the running activation's partial result, activations told apart by call site and
literal arguments, a call on the innermost activation's own arguments joins it; the budget and
skipped calls, as call_protocol.py):

- a recursive function `fN(n, acc, o, d)` of one of the shapes (`SHAPES`: on a counter, on a
  list's tail, on its own arguments with the list it is given as its progress, on a global
  counter, mutual through `gN`, twice per activation, in a loop over `range(n)`, a method on
  `self`, a closure calling itself), with an effect before or after its recursive call
  (`EFFECTS`: a global, the list, the object's attribute, the dict), a result (`RESULTS`: a count,
  a literal, the base case's value passed back, the list it is given, a new list, None, a
  maximum, a number built from the base case's), and raises (`RAISES`: none, `boom()` at the base
  case, that caught around the recursive call);
- called by `pN` with a depth of 0, 1 or 2 (`c1`, `c2`).

Observation points: each activation's entry (its depth), after its recursive call (`G`), and
after the call: the result, `G`, the list's length, the attribute, the dict's size. Each batch
runs under every limit set (`LIMITS`): the default, `budget=0` and `budget=6`.

    uv run --no-project python tests/rec_protocol.py [--rowan N | --all] [--seed S] [--limits L] [-v]
"""
from __future__ import annotations

import itertools

from protocol_lib import Prog, cli

NAMES = ["G", "A", "O"]

# per shape: the depth's start (`n0`), the entry's measure, the base case's test, the lines of
# the recursive call (binding `r`), and how the function is defined and called
SHAPES = {
    "int": dict(n0="(2 if c1 else 1) if c2 else 0", k="n", base="n <= 0", rec=["r = F(n - 1, acc, o, d)"]),
    "tail": dict(n0="([1, 2] if c1 else [1]) if c2 else []", k="len(n)", base="not n", rec=["r = F(n[1:], acc, o, d)"]),
    "same": dict(n0="(2 if c1 else 1) if c2 else 0", k="len(acc)", base="len(acc) >= n", rec=["acc.append(0)", "r = F(n, acc, o, d)"]),
    "glob": dict(n0="(2 if c1 else 1) if c2 else 0", k="A", base="A >= n", rec=["A = A + 1", "r = F(n, acc, o, d)"]),
    "mutual": dict(n0="(2 if c1 else 1) if c2 else 0", k="n", base="n <= 0", rec=["r = g{N}(n - 1, acc, o, d)"]),
    "twice": dict(n0="(2 if c1 else 1) if c2 else 0", k="n", base="n <= 0", rec=["r = F(n - 1, acc, o, d)", "r = F(n - 2, acc, o, d)"]),
    "loop": dict(n0="(2 if c1 else 1) if c2 else 0", k="n", base="n <= 0", rec=["for j in range(n):", "    r = F(j, acc, o, d)"]),
    "method": dict(n0="(2 if c1 else 1) if c2 else 0", k="n", base="n <= 0", rec=["r = self.f(n - 1, acc, o, d)"]),
    "closure": dict(n0="(2 if c1 else 1) if c2 else 0", k="n", base="n <= 0", rec=["r = go(n - 1)"]),
}

EFFECTS = {
    "g": ["G = G * 10 + 1"],
    "acc": ["acc.append(1)"],
    "attr": ["o.v = o.v * 10 + 1"],
    "dict": ["d[len(d)] = 1"],
}
WHERE = ["pre", "post"]

# per result: the base case's return, the recursive case's, and a value of its kind (a caught
# raise's)
RESULTS = {
    "count": ("return 0", "return r + 1", "0"),
    "lit": ("return 5", "return 3", "0"),
    "pass": ("return 7", "return r", "0"),
    "acc": ("return acc", "return r", "acc"),
    "new": ("return []", "return r + [1]", "[]"),
    "none": ("return", "", "None"),
    "max": ("return 0", "return max(r, len(acc))", "0"),
    "built": ("return o.v", "return r * 10 + 2", "0"),
}

RAISES = ["plain", "boom", "caught", "outer"]


def body(shape, effect, where, result, raises) -> list:
    """The recursive function's body (its parameters `n, acc, o, d`; `F` the call of itself)."""
    s = SHAPES[shape]
    base, rec_ret, fallback = RESULTS[result]
    eff = EFFECTS[effect] if effect != "none" else []
    rec = list(s["rec"])
    if raises == "caught":
        rec = ["try:", *("    " + ln for ln in rec), "except KeyError:", "    G = G * 10 + 2", f"    r = {fallback}"]
    lines = ["global G, A, O", f"k = {s['k']}", "O = k", f"if {s['base']}:"]
    if raises in ("boom", "caught", "outer"):
        lines.append("    boom()")
    lines.append("    " + base)
    lines += (eff if where == "pre" else []) + rec + ["O = G"] + (eff if where == "post" else [])
    if rec_ret:
        lines.append(rec_ret)
    return lines


def program(shape, effect, where, result, raises) -> Prog:
    s = SHAPES[shape]
    b = body(shape, effect, where, result, raises)
    ind = lambda ls, k=1: ["    " * k + ln for ln in ls]
    lines = ["class Ob{N}:", "    v = 0", ""]
    if shape == "method":
        lines += ["class R{N}:", "    def f(self, n, acc, o, d):", *ind([ln.replace("F(", "self.f(") for ln in b], 2), ""]
        call = "R{N}().f(n, acc, o, d)"
    elif shape == "closure":
        go = [ln.replace("F(", "go(") for ln in b]
        lines += ["def f{N}(n, acc, o, d):", "    def go(n):", *ind(["nonlocal acc, o, d", *go], 2), "    return go(n)", ""]
        call = "f{N}(n, acc, o, d)"
    else:
        lines += ["def f{N}(n, acc, o, d):", *ind([ln.replace("F(", "f{N}(") for ln in b]), ""]
        if shape == "mutual":
            lines += ["def g{N}(n, acc, o, d):", "    return f{N}(n, acc, o, d)", ""]
        call = "f{N}(n, acc, o, d)"
    lines += [
        "def p{N}():",
        "    global G, A, O",
        "    acc = []",
        "    o = Ob{N}()",
        "    d = {}",
        f"    n = {s['n0']}",
        *([f"    r = {call}"] if raises != "outer" else ["    try:", f"        r = {call}", "    except KeyError:", "        O = G", "        r = -3"]),
        "    n = r if isinstance(r, int) else len(r) if isinstance(r, list) else -2",
        "    O = n",
        "    O = G",
        "    n = len(acc)",
        "    O = n",
        "    n = o.v",
        "    O = n",
        "    n = len(d)",
        "    O = n",
    ]
    return Prog(lines, end=["O = G", "O = A"], desc=f"{shape} {effect} {where} {result} {raises}")


def programs():
    effects = [("none", "pre")] + list(itertools.product(EFFECTS, WHERE))
    for shape, (effect, where), result, raises in itertools.product(SHAPES, effects, RESULTS, RAISES):
        yield program(shape, effect, where, result, raises)


LIMITS = (None, "budget=0", "budget=6")

if __name__ == "__main__":
    cli(programs, NAMES, limit_sets=LIMITS)
