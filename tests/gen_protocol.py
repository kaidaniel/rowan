"""Generators, checked exhaustively against CPython: closing, raises replayed at their consumers,
per-item effects, every consumer kind, `break` / return / raise out of loops over them, and a
`try` around the consumption (AGENTS.md, Generators; genclose.rs, genraise.rs, stopearly.rs).

The bounded grammar (`programs`): a generator `gN` whose body is one to three statements (`yield
1`, `yield 2`, a marker `G = G * 10 + 1`, `raise ValueError()`, `if c1: return`, `boom()`; at
least one yield, a raise only last), plain or wrapped in a try (finally; `except GeneratorExit`
re-raising; `except ValueError`; `except BaseException` swallowing, then a marker or another
yield), consumed by `pN` in one of the consumer forms (`CONSUMERS`), plain or inside a try
(`except ValueError`, `except Exception`, `finally`). The generator marks `G`, the consumer `C`
(two traces: one shared global is the interleaving rowan joins, `Gens::watch`). Observation
points: after each consumer step, at each handler and finally (the generator's too), and after
the run.

    uv run --no-project python tests/gen_protocol.py [--rowan N | --all] [--seed S] [-v]

runs CPython on every program and rowan on N of them (default 2000, `--all`: every one).
"""
from __future__ import annotations

import itertools

from protocol_lib import Prog, cli

NAMES = ["G", "C", "OG", "OC"]

LEAVES = {
    "y1": ["yield 1"],
    "y2": ["yield 2"],
    "m": ["G = G * 10 + 1"],
    "raise": ["raise ValueError()"],
    "ret": ["if c1:", "    return"],
    "boom": ["boom()"],
}


def seqs():
    for n in (1, 2, 3):
        for s in itertools.product(LEAVES, repeat=n):
            if not any(x.startswith("y") for x in s) or "raise" in s[:-1]:
                continue
            yield list(s)


def indent(lines, k=1):
    return ["    " * k + ln for ln in lines]


def gen_body(seq, wrap):
    body = [ln for x in seq for ln in LEAVES[x]]
    if wrap == "plain":
        return body
    if wrap == "finally":
        return ["try:", *indent(body), "finally:", "    OG = G", "    G = G * 10 + 6", "G = G * 10 + 5"]
    if wrap == "genexit":
        return ["try:", *indent(body), "except GeneratorExit:", "    OG = G", "    G = G * 10 + 7", "    raise", "G = G * 10 + 5"]
    if wrap == "valueerror":
        return ["try:", *indent(body), "except ValueError:", "    OG = G", "    G = G * 10 + 8", "G = G * 10 + 5"]
    if wrap == "swallow":
        return ["try:", *indent(body), "except BaseException:", "    OG = G", "    G = G * 10 + 9", "G = G * 10 + 5"]
    if wrap == "swallow_yield":
        return ["try:", *indent(body), "except BaseException:", "    OG = G", "    G = G * 10 + 9", "    yield 3"]
    raise ValueError(wrap)


WRAPS = ["plain", "finally", "genexit", "valueerror", "swallow", "swallow_yield"]

# consumers of `g{N}()`: lines of pN's body (after its global line)
CONSUMERS = {
    "for": ["for v in g{N}():", "    C = C * 10 + v", "    OG = G", "OC = C", "OG = G"],
    "for_break": ["for v in g{N}():", "    C = C * 10 + v", "    if c2:", "        break", "    OG = G", "OG = G", "OC = C"],
    "for_return": ["for v in g{N}():", "    C = C * 10 + v", "    if c2:", "        return", "OG = G", "OC = C"],
    "for_raise": ["for v in g{N}():", "    C = C * 10 + v", "    if v == 2:", "        raise KeyError()", "OG = G"],
    "held_break": ["it = g{N}()", "for v in it:", "    C = C * 10 + v", "    break", "OG = G", "it = None", "OG = G"],
    "next2": ["it = g{N}()", "C = C * 10 + next(it)", "OG = G", "C = C * 10 + next(it)", "OG = G", "OC = C"],
    "next_default": ["it = g{N}()", "C = C * 10 + next(it, 0)", "OG = G", "C = C * 10 + next(it, 0)", "OG = G", "it.close()", "OG = G", "OC = C"],
    "list": ["xs = list(g{N}())", "C = len(xs)", "OG = G", "OC = C"],
    "sum": ["C = sum(v for v in g{N}())", "OG = G", "OC = C"],
    "comp": ["C = len([v for v in g{N}() if v != 2])", "OG = G", "OC = C"],
    "any": ["if any(v == 2 for v in g{N}()):", "    C = 1", "OG = G", "OC = C"],
    "all": ["if all(v == 1 for v in g{N}()):", "    C = 1", "OG = G", "OC = C"],
    "in": ["if 2 in g{N}():", "    C = 1", "OG = G", "OC = C"],
    "next_genexp": ["C = next((v for v in g{N}() if v == 2), 0)", "OG = G", "OC = C"],
    "yield_from": ["for v in h{N}():", "    C = C * 10 + v", "    if c2:", "        break", "OG = G", "OC = C"],
    "zip": ["for a, b in zip(g{N}(), (1,)):", "    C = C * 10 + a", "OG = G", "OC = C"],
    "unpack": ["a, b = g{N}()", "C = a * 10 + b", "OG = G", "OC = C"],
    "throw": ["it = g{N}()", "C = next(it)", "OG = G", "C = C * 10 + it.throw(ValueError())", "OG = G", "OC = C"],
}

CTRY = ["none", "valueerror", "exception", "finally"]

HELPER = ["def h{N}():", "    global G", "    G = G * 10 + 4", "    yield from g{N}()", "    G = G * 10 + 4"]


def consumer(kind, ctry):
    body = [ln for ln in CONSUMERS[kind]]
    if ctry == "none":
        return body
    if ctry == "valueerror":
        return ["try:", *indent(body), "except ValueError:", "    OC = C", "    OG = G", "    C = C * 10 + 8", "OC = C"]
    if ctry == "exception":
        return ["try:", *indent(body), "except Exception:", "    OC = C", "    OG = G", "    C = C * 10 + 7", "OC = C"]
    return ["try:", *indent(body), "finally:", "    OG = G", "    C = C * 10 + 6"]


def program(seq, wrap, kind, ctry) -> Prog:
    lines = ["def g{N}():", "    global G, OG", *indent(gen_body(seq, wrap))]
    if kind == "yield_from":
        lines += [""] + HELPER
    lines += ["", "def p{N}():", "    global C, G, OG, OC", *indent(consumer(kind, ctry))]
    return Prog(lines, end=["OG = G", "OC = C"], desc=f"{'+'.join(seq)} {wrap} / {kind} {ctry}")


def programs():
    # (`throw` last: a program that calls `.throw(` makes every generator of its batch raise at
    # its yields, `close_point`; batched apart, the others keep their precision)
    for kinds in ([k for k in CONSUMERS if k != "throw"], ["throw"]):
        for seq in seqs():
            for wrap in WRAPS:
                for kind in kinds:
                    for ctry in CTRY:
                        yield program(seq, wrap, kind, ctry)


if __name__ == "__main__":
    cli(programs, NAMES)
