"""`with` statements and their managers, checked exhaustively against CPython: an `__exit__` that
returns None, True or False, raises its own error, swallows one class only or on some paths;
an `__enter__` that raises (no `__exit__` then); `@contextmanager` generators that let the body's
raise through, run a finally, swallow it or raise it again; crossed with try / except / finally
around the with statement, a loop around it (`break`), and a second manager nested inside
or beside it in one with statement (AGENTS.md, Statements: `with` is a try whose finally is `__exit__`; tries.rs, gens.rs `CmHook`).

The body marks `X` (and returns, raises KeyError / a BaseException, calls `boom()`); the
managers mark `E` (two traces: one shared global is the interleaving rowan joins). Observation
points: after the with statement, at each handler and finally, after the run.

    uv run --no-project python tests/with_protocol.py [--rowan N | --all] [--seed S] [-v]
"""
from __future__ import annotations

import itertools

from protocol_lib import Prog, cli

NAMES = ["X", "E", "OX", "OE"]

EXITS = {
    "none": [],
    "true": ["return True"],
    "false": ["return False"],
    "raise": ["raise ValueError()"],
    "keyerror": ["return t is KeyError"],
    "cond": ["if c2:", "    return True"],
}
ENTERS = {"ok": [], "raise": ["if c2:", "    raise ValueError()"]}
GENS = {
    "gen_plain": ["E = E * 10 + 1", "yield", "E = E * 10 + 2"],
    "gen_finally": ["E = E * 10 + 1", "try:", "    yield", "finally:", "    E = E * 10 + 2"],
    "gen_swallow": ["E = E * 10 + 1", "try:", "    yield", "except Exception:", "    E = E * 10 + 3"],
    "gen_reraise": ["E = E * 10 + 1", "try:", "    yield", "except Exception:", "    E = E * 10 + 3", "    raise"],
}

LEAVES = {
    "m": ["X = X * 10 + 1"],
    "key": ["raise KeyError()"],
    "stop": ["raise Stop()"],
    "boom": ["boom()"],
    "ret1": ["if c1:", "    return"],
    "ret": ["return"],
}
LOOP_LEAVES = {"brk": ["if c1:", "    break"]}


def indent(lines, k=1):
    return ["    " * k + ln for ln in lines]


def manager(name: str, kind: str) -> list:
    """A manager class (or `@contextmanager` function) named `name`; `kind`: `ENTER/EXIT` or a `GENS` key."""
    if kind in GENS:
        return ["@contextlib.contextmanager", f"def {name}():", "    global E", *indent(GENS[kind])]
    enter, exit_ = kind.split("/")
    return [f"class {name}:", "    def __enter__(self):", "        global E", "        E = E * 10 + 1", *indent(ENTERS[enter], 2), "        return self", "",
            "    def __exit__(self, t, v, tb):", "        global E", "        E = E * 10 + 2", *indent(EXITS[exit_], 2)]


KINDS = [f"{e}/{x}" for e in ENTERS for x in EXITS] + list(GENS)
INNER = ["ok/none", "ok/true", "ok/raise"]


def bodies(loop: bool):
    leaves = dict(LEAVES, **(LOOP_LEAVES if loop else {}))
    for n in (1, 2):
        for s in itertools.product(leaves, repeat=n):
            if "ret" in s[:-1] or "key" in s[:-1] or "stop" in s[:-1]:
                continue
            yield [ln for x in s for ln in leaves[x]], "+".join(s)


AROUND = ["none", "keyerror", "exception", "finally", "loop"]


def wrap(stmt: list, around: str) -> list:
    if around == "none":
        return stmt
    if around == "keyerror":
        return ["try:", *indent(stmt), "except KeyError:", "    OX = X", "    OE = E", "    X = X * 10 + 7", "OX = X"]
    if around == "exception":
        return ["try:", *indent(stmt), "except Exception:", "    OX = X", "    OE = E", "    X = X * 10 + 8", "OX = X"]
    if around == "finally":
        return ["try:", *indent(stmt), "finally:", "    OX = X", "    OE = E", "    X = X * 10 + 6"]
    return ["for i in (1, 2):", *indent(stmt), "    X = X * 10 + 5", "OX = X", "OE = E"]


def program(kind: str, inner: str | None, body: list, desc: str, around: str, one: bool = False) -> Prog:
    lines = manager("M{N}", kind)
    w = ["with M{N}():"]
    if inner and one:
        lines += [""] + manager("I{N}", inner)
        w = ["with M{N}(), I{N}():", *indent(body)]
    elif inner:
        lines += [""] + manager("I{N}", inner)
        w += ["    with I{N}():", *indent(body, 2), "    OX = X", "    OE = E"]
    else:
        w += indent(body)
    w += ["OX = X", "OE = E"]
    lines += ["", "def p{N}():", "    global X, E, OX, OE", *indent(wrap(w, around))]
    return Prog(lines, end=["OX = X", "OE = E"], desc=f"{kind} {inner or '-'} / {desc} {around}")


def programs():
    for kind in KINDS:
        for around in AROUND:
            for body, desc in bodies(around == "loop"):
                yield program(kind, None, body, desc, around)
    for kind in KINDS:
        for inner in INNER:
            for around in ("none", "exception"):
                for body, desc in bodies(False):
                    yield program(kind, inner, body, desc, around)
                    # (one with statement of two managers)
                    yield program(kind, inner, body, desc + " one", around, one=True)


if __name__ == "__main__":
    cli(programs, NAMES)
