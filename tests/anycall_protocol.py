"""Runs for any call and result summaries, checked exhaustively against CPython (AGENTS.md,
Calls: entry runs and general runs, result summaries; Soundness: what rowan assumes of
annotations):

- a function `qN(x, y)` whose body is one operation on its parameters (`OPS`: operators, `in`,
  comparisons, calls, subscripts, attributes, iteration, unpacking, builtins, formatting, a
  method of the program's class) inside a `try` catching one class (`CATCH`), returning the
  operation's result, `"caught"` or `"other"`;
- the parameters unannotated, or annotated (`ANNS`: `int`, `Optional[int]`, `str`, `list`,
  `dict`, `Optional[Ob]`, `Ob` with a subclass overriding its method and operator), called with
  two values the annotation allows (unannotated: two of `VALUES`);
- observed inside the function (rowan's rows for each call and for any call) and at module
  level (`T = qN(…)`: under `budget=0` the second call is skipped, its result the function's
  summary).

Each batch runs under the default limits and `budget=0`. Rowan's claims must cover CPython's
runs in both.

    uv run --no-project python tests/anycall_protocol.py [--rowan N | --all] [--seed S] [--limits L] [-v]
"""
from __future__ import annotations

import itertools

from protocol_lib import Prog, cli

NAMES = ["O", "T"]

VALUES = ["None", "0", "3", "''", "'ab'", "[]", "[1, 2]", "{}", "{'k': 1}", "Ob{N}()", "Sub{N}()"]
# the second operand of binary operations
OTHERS = ["None", "0", "2", "'a'", "[1]"]

OPS = {
    # one operand
    "neg": "-x",
    "not": "not x",
    "inv": "~x",
    "truth": "1 if x else 2",
    "len": "len(x)",
    "hash": "hash(x)",
    "int": "int(x)",
    "float": "float(x)",
    "str": "str(x)",
    "abs": "abs(x)",
    "call": "x()",
    "item": "x[0]",
    "key": "x['k']",
    "slice": "x[1:]",
    "attr": "x.v",
    "meth": "x.m()",
    "upper": "x.upper()",
    "get": "x.get('k')",
    "iter": "[a for a in x]",
    "unpack": "(lambda a, b: a)(*x)",
    "pair": "(lambda p: p[0])(tuple(x))",
    "sorted": "sorted(x)",
    "max": "max(x)",
    "sum": "sum(x)",
    "fmtd": "f'{x:d}'",
    "pct": "'%d' % (x,)",
    "isnone": "x is None",
    "eqnone": "x == None",
    # two operands
    "add": "x + y",
    "sub": "x - y",
    "mul": "x * y",
    "div": "x / y",
    "mod": "x % y",
    "pow": "x ** y",
    "lt": "x < y",
    "eq": "x == y",
    "in": "x in y",
    "ni": "y in x",
    "sub_item": "x[y]",
    "and": "x and y",
    "or": "x or y",
    "concat": "'p' + x",
}
TWO = {"add", "sub", "mul", "div", "mod", "pow", "lt", "eq", "in", "ni", "sub_item", "and", "or"}

CATCH = ["TypeError", "AttributeError", "LookupError", "ValueError", "ZeroDivisionError", "Exception"]

# annotation: the values passed (they satisfy it)
ANNS = {
    "int": ("int", ["0", "3"]),
    "optint": ("Optional[int]", ["None", "3"]),
    "str": ("str", ["''", "'ab'"]),
    "list": ("list", ["[]", "[1, 2]"]),
    "dict": ("dict", ["{}", "{'k': 1}"]),
    "optob": ("Optional[Ob{N}]", ["None", "Ob{N}()"]),
    "ob": ("Ob{N}", ["Ob{N}()", "Sub{N}()"]),
}


def header() -> list:
    return ["from typing import Optional", "", "class Ob{N}:", "    v = 1", "", "    def m(self):", "        return 1", "",
            "class Sub{N}(Ob{N}):", "    def m(self):", "        return [][0]", "",
            "    def __add__(self, o):", "        return 7", ""]


def program(op: str, catch: str, ann: str | None, xs: tuple, y: str, inside: bool) -> Prog:
    xa = f": {ANNS[ann][0]}" if ann else ""
    lines = [*header(), "def p{N}():", "    pass", "",
             f"def q{{N}}(x{xa}, y):", "    global O", "    try:", f"        r = {OPS[op]}", "        O = r" if inside else "        pass",
             f"    except {catch}:", "        O = 'caught'" if inside else "        pass", "        return 'caught'",
             "    except BaseException:", "        return 'other'", "    return r", ""]
    yy = y if op in TWO else "0"
    end = []
    for x in xs:
        end += [f"T = q{{N}}({x}, {yy})", "O = T"]
    return Prog(lines, end=end, desc=f"{op} {catch} {ann or '-'} {'/'.join(xs)} {yy}")


def programs(limits=None):
    # (inside the function only under the default limits: under `budget=0` the second call's
    # run is skipped, and what rowan claims there is its first call's)
    inside = not limits
    pairs = list(itertools.combinations(VALUES, 2))
    for op, catch in itertools.product(OPS, CATCH):
        ys = OTHERS if op in TWO else ["0"]
        for xs, y in itertools.product(pairs, ys):
            yield program(op, catch, None, xs, y, inside)
            yield program(op, catch, None, xs[::-1], y, inside)
        for ann, (_, vals) in ANNS.items():
            for y in ys:
                yield program(op, catch, ann, tuple(vals), y, inside)
                yield program(op, catch, ann, tuple(vals[::-1]), y, inside)


LIMITS = (None, "budget=0")

if __name__ == "__main__":
    cli(programs, NAMES, limit_sets=LIMITS)
