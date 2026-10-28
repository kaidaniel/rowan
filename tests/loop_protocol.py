"""Loops, checked exhaustively against CPython (AGENTS.md, Statements: Loops — over at most 16
known items unrolled, else a fixpoint, widened past 40 rounds; `loops.rs`):
- a loop (`ITERS`): a `for` over no, one, three or twenty known items, items some paths add
  (`for_known_prefix`), items on one path or the other, an unknown count (`range(len(B) * 2)`),
  partly unknown items, a dict's keys, `.items()` unpacked, `enumerate`, a set, a generator
  expression, a `while` on a counter (known bound, unknown bound, `while True` with a break),
  and a loop nested in another;
- a body (`BODIES`): a trace `G = G * 10 + x`, the last item, an append, a dict store, a counter,
  the loop variable rebound, `break` / `continue` / `return` (surely, on a path, or through a
  `finally`), a raise, `boom()` (raising on some inputs), a handler that continues or breaks,
  an inner loop's break (also its `else` continuing the outer loop), the inner loop variable
  read after it, an append undone by `pop`, a store on some rounds, and pairs of these;
- a form (`FORMS`): plain, with an `else:`, or inside a `try` catching ValueError;
- and, for any call, the same loop over a parameter `xs` (`param_program`), called with the
  items.
Observation points: the loop variable and `G` at the end of each round (the join over rounds),
in the `else`, and after the loop: `G`, the loop variable (bound to -1 before the loop), the
list's length and the counter. Each batch runs under the default limits and `budget=0`.
    uv run --no-project python tests/loop_protocol.py [--rowan N | --all] [--seed S] [--limits L] [-v]
"""
from __future__ import annotations

import itertools

from protocol_lib import Prog, cli

NAMES = ["G", "OX", "OG", "OL", "ON"]

# name: (lines before the loop, the loop's header lines, the body's first lines)
ITERS = {
    "empty": ([], ["for x in []:"], []),
    "one": ([], ["for x in [1]:"], []),
    "three": ([], ["for x in [1, 2, 3]:"], []),
    "twenty": ([], ["for x in range(20):"], []),
    "prefix": (["xs = [1, 2]", "if c1:", "    xs.append(3)"], ["for x in xs:"], []),
    "either": ([], ["for x in ([1, 2] if c1 else [3]):"], []),
    "unkcount": ([], ["for x in range(len(B) * 2):"], []),
    "unkitems": ([], ["for x in [len(B), 2, len(B) + 1]:"], []),
    "dict": ([], ["for x in {1: 'a', 2: 'b'}:"], []),
    "items": ([], ["for _, x in {1: 2, 2: 3}.items():"], []),
    "enum": ([], ["for i, x in enumerate([5, 6]):"], []),
    "set": ([], ["for x in {1, 2}:"], []),
    "genexp": ([], ["for x in (v for v in [1, 2, 3]):"], []),
    "while": (["i = 0"], ["while i < 3:"], ["i += 1", "x = i"]),
    "whileunk": (["i = 0"], ["while i < len(B) * 2:"], ["i += 1", "x = i"]),
    "whiletrue": (["i = 0"], ["while True:"], ["i += 1", "x = i", "if i > 2:", "    break"]),
    "nested": ([], ["for a in [1, 2]:", "    for x in [10, 20]:"], []),
}

BODIES = {
    "mark": ["G = G * 10 + x"],
    "last": ["G = x"],
    "append": ["acc.append(x)"],
    "store": ["d[x] = G", "G = G + 1"],
    "count": ["n += 1"],
    "rebind": ["x = x + 10"],
    "brk": ["if x == 2:", "    break"],
    "brkc": ["if c1 and x == 2:", "    break"],
    "cont": ["if x == 2:", "    continue", "G = G * 10 + x"],
    "contc": ["if c1:", "    continue", "G = G * 10 + x"],
    "ret": ["if x == 2:", "    return"],
    "raise": ["if x == 2:", "    raise ValueError()"],
    "boom": ["boom()", "G = G * 10 + x"],
    "finbrk": ["try:", "    if x == 2:", "        break", "finally:", "    G = G * 10 + 9"],
    "fincont": ["try:", "    if x == 2:", "        continue", "finally:", "    G = G * 10 + 9"],
    "handcont": ["try:", "    boom()", "except KeyError:", "    continue", "G = G * 10 + x"],
    "handbrk": ["try:", "    boom()", "except ValueError:", "    break", "G = G * 10 + x"],
    "inner": ["for y in [1, 2]:", "    if y == x:", "        break", "    G = G * 10 + y"],
    "some": ["if c2:", "    G = x"],
    "markbrk": ["G = G * 10 + x", "if x == 2:", "    break"],
    "appendcont": ["acc.append(x)", "if c1:", "    continue", "n += 1"],
    "leak": ["for y in [1, 2]:", "    pass", "G = G * 10 + y"],
    "pop": ["acc.append(x)", "if c1:", "    acc.pop()"],
    "nestedbrk": ["for y in [1, 2]:", "    if y == x:", "        break", "else:", "    continue", "break"],
}

FORMS = {
    "plain": [],
    "else": ["else:", "    G = G * 10 + 7", "    OG = G"],
    "try": None,  # (the loop inside `try: … except ValueError: G = -1`)
}


def loop_lines(iter_, body, form) -> list:
    """The loop's lines (inside a function body, unindented), its observations included."""
    pre, heads, first = ITERS[iter_]
    depth = len(heads)
    ind = "    " * depth
    inner = [*first, *BODIES[body], "OX = x", "OG = G"]
    lines = [*pre, *heads, *(ind + ln for ln in inner)]
    if form == "else":
        lines += FORMS["else"]
    if form == "try":
        lines = ["try:", *("    " + ln for ln in lines), "except ValueError:", "    G = -1"]
    return lines + ["OG = G", "OX = x", "OL = len(acc)", "ON = n"]


def program(iter_, body, form) -> Prog:
    lines = ["def p{N}():", "    global G, OX, OG, OL, ON", "    acc = []", "    d = {}", "    n = 0", "    x = -1",
             *("    " + ln for ln in loop_lines(iter_, body, form)), ""]
    return Prog(lines, end=["OG = G"], desc=f"{iter_} {body} {form}")


PARAM_ITERS = {"empty": "[]", "one": "[1]", "three": "[1, 2, 3]", "twenty": "list(range(20))", "either": "[1, 2] if c1 else [3]",
               "dict": "{1: 'a', 2: 'b'}", "set": "{1, 2}"}


def param_program(iter_, body) -> Prog:
    """The loop over a parameter: its rows for any call must cover each call's run."""
    lines = ["def q{N}(xs):", "    global G, OX, OG, OL, ON", "    acc = []", "    d = {}", "    n = 0", "    x = -1",
             "    for x in xs:", *("        " + ln for ln in [*BODIES[body], "OX = x", "OG = G"]),
             "    OG = G", "    OX = x", "    OL = len(acc)", "    ON = n", "",
             "def p{N}():", f"    q{{N}}({PARAM_ITERS[iter_]})", ""]
    return Prog(lines, end=["OG = G"], desc=f"param {iter_} {body}")


def programs():
    for iter_, body, form in itertools.product(ITERS, BODIES, FORMS):
        yield program(iter_, body, form)
    for iter_, body in itertools.product(PARAM_ITERS, BODIES):
        yield param_program(iter_, body)


if __name__ == "__main__":
    cli(programs, NAMES, limit_sets=(None, "budget=0"))
