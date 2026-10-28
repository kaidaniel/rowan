"""Narrowing and `match`, checked exhaustively against CPython (AGENTS.md, Statements: Branches —
a test splits the state into where it is true and where it is false, both branches run and
join; Soundness: `isinstance` against a builtin class, None, typed unknowns):

- a name `x` bound to one or two values (`VALUES`: None, 0, 3, `''`, `'a'`, True, `[]`, `[1]`, a
  dict, an instance; two of them chosen by `c1`), or, for any call, a parameter that may be any
  of them;
- tested (`TESTS`: `is None`, `is not None`, truth, `==` / `!=`, `in` a tuple, `isinstance`
  against one or two builtin classes, `type(x) is int`, `hasattr`, a comparison guarded by a
  None test, `len`, `in` x, `bool`, `not not`) in one of the statement forms (`FORMS`: `if` /
  `else`, a branch that rebinds `x`, an early `return`, a conditional expression, `not`, `and` /
  `or` with `c2`, `elif`, `assert`, `while`), observing `x` in each branch and after the join;
- or matched (`match x:`) against two patterns of `PATTERNS` (literals, None, class patterns of
  builtins and of the program's class, sequence and mapping patterns, a guard) and `case _`.

`--pairs`: two tests of `TESTS` on the same `x` (`PAIR_FORMS`: nested, in sequence after the
first's join, `and` / `or`, an early return on the first then the second, the first rebinding
`x` on a path then the second).

    uv run --no-project python tests/narrow_protocol.py [--pairs] [--rowan N | --all] [--seed S] [-v]
"""
from __future__ import annotations

import itertools

from protocol_lib import Prog, cli

NAMES = ["O"]

VALUES = ["None", "0", "3", "''", "'a'", "True", "[]", "[1]", "{'k': 1}", "Ob{N}()"]

TESTS = {
    "none": "x is None",
    "notnone": "x is not None",
    "truth": "x",
    "eq": "x == 3",
    "ne": "x != 3",
    "in": "x in (3, 'a', None)",
    "int": "isinstance(x, int)",
    "str": "isinstance(x, str)",
    "intstr": "isinstance(x, (int, str))",
    "list": "isinstance(x, list)",
    "type": "type(x) is int",
    "hasattr": "hasattr(x, 'v')",
    "gt": "x is not None and x > 2",
    "len": "len(x) > 0",
    "kin": "'k' in x",
    "bool": "bool(x)",
    "notnot": "not not x",
}

FORMS = {
    "if": ["if T:", "    O = x", "else:", "    O = x", "O = x"],
    "rebind": ["if T:", "    O = x", "    x = 4", "O = x"],
    "early": ["if T:", "    O = x", "    return", "O = x"],
    "ternary": ["y = x if T else -9", "O = y", "O = x"],
    "not": ["if not (T):", "    O = x", "else:", "    O = x"],
    "and": ["if T and c2:", "    O = x", "else:", "    O = x", "O = x"],
    "or": ["if T or c2:", "    O = x", "else:", "    O = x", "O = x"],
    "elif": ["if x is None:", "    O = x", "elif T:", "    O = x", "else:", "    O = x", "O = x"],
    "assert": ["try:", "    assert T", "    O = x", "except AssertionError:", "    O = x", "O = x"],
    "while": ["k = 0", "while T and k < 2:", "    O = x", "    k = k + 1", "    x = 0 if k == 1 else x", "O = x"],
}

PATTERNS = {
    "none": "None",
    "three": "3",
    "int": "int()",
    "strg": "str() if x",
    "one": "[a]",
    "empty": "[]",
    "map": "{'k': v}",
    "cls": "Ob{N}(v=1)",
    "bool": "bool()",
    "or": "0 | 'a'",
}


def value(pair) -> str:
    return pair[0] if len(pair) == 1 else f"{pair[0]} if c1 else {pair[1]}"


def pairs():
    yield from ((v,) for v in VALUES)
    yield from itertools.combinations(VALUES, 2)


def header() -> list:
    return ["class Ob{N}:", "    v = 1", ""]


def test_program(pair, test, form) -> Prog:
    body = [ln.replace("T", TESTS[test]) if ln.lstrip().startswith(("if", "elif", "while", "assert", "y =")) else ln for ln in FORMS[form]]
    lines = [*header(), "def p{N}():", "    global O", f"    x = {value(pair)}", *("    " + ln for ln in body), "",
             # (the same for any call: `q` is called with each value, and its run for any call
             # has `x` as any of them)
             "def q{N}(x):", "    global O", *("    " + ln for ln in body), ""]
    end = [f"run(lambda: q{{N}}({v}))" for v in pair]  # ({N}: replaced in `module`)
    return Prog(lines, end=end + ["O = O"], desc=f"{'/'.join(pair)} {test} {form}")


def match_program(pair, pats) -> Prog:
    cases = []
    for p in pats:
        cases += [f"    case {PATTERNS[p]}:", "        O = x"]
    lines = [*header(), "def p{N}():", "    global O", f"    x = {value(pair)}", "    match x:", *("    " + ln for ln in cases),
             "        case _:", "            O = x", "    O = x", ""]
    return Prog(lines, end=["O = O"], desc=f"{'/'.join(pair)} match {'+'.join(pats)}")


PAIR_FORMS = {
    "nested": ["if T1:", "    if T2:", "        O = x", "    else:", "        O = x", "    O = x", "O = x"],
    "seq": ["if T1:", "    O = x", "if T2:", "    O = x", "else:", "    O = x", "O = x"],
    "and": ["if T1 and T2:", "    O = x", "else:", "    O = x", "O = x"],
    "or": ["if T1 or T2:", "    O = x", "else:", "    O = x", "O = x"],
    "guard": ["if not (T1):", "    return", "if T2:", "    O = x", "O = x"],
    "rebind": ["if T1:", "    x = 4 if c2 else x", "if T2:", "    O = x", "else:", "    O = x", "O = x"],
}


def pair_program(pair, t1, t2, form) -> Prog:
    body = [ln.replace("T1", TESTS[t1]).replace("T2", TESTS[t2]) for ln in PAIR_FORMS[form]]
    lines = [*header(), "def p{N}():", "    global O", f"    x = {value(pair)}", *("    " + ln for ln in body), "",
             "def q{N}(x):", "    global O", *("    " + ln for ln in body), ""]
    end = [f"run(lambda: q{{N}}({v}))" for v in pair]
    return Prog(lines, end=end + ["O = O"], desc=f"{'/'.join(pair)} {t1}+{t2} {form}")


def pair_programs():
    for pair, (t1, t2), form in itertools.product(pairs(), itertools.permutations(TESTS, 2), PAIR_FORMS):
        yield pair_program(pair, t1, t2, form)


def programs():
    for pair, test, form in itertools.product(pairs(), TESTS, FORMS):
        yield test_program(pair, test, form)
    for pair, pats in itertools.product(pairs(), itertools.permutations(PATTERNS, 2)):
        yield match_program(pair, pats)


if __name__ == "__main__":
    cli(programs, NAMES, pairs=pair_programs)
