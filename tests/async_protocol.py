"""Coroutines, `await`, tasks, async generators, `async for` and `async with`, checked exhaustively
against CPython (`asyncio.run` drives each program; AGENTS.md, Calls):

- a coroutine `coN(a)` whose body is one or two effects (`EFFECTS`: a global changed, the
  argument appended to, an `await asyncio.sleep(0)` before a change, a change under `c1`, a raise
  after a change, nothing) and returns `G * 100 + len(a)`;
- an async generator `agN(a)` yielding twice with the same effects before each yield, a context
  manager class `CmN` whose `__aenter__` has them (its `__aexit__` changes `G` too), and a class
  with `__aiter__` / `__anext__` that has them per item;
- a method `SvN.run` and an `@asynccontextmanager` `acmN` with them;
- what the entry coroutine does with them (`SHAPES`): awaits the call (also in a loop, a stored
  list of them, a method), calls it and never awaits
  it, awaits it later, makes a task of it and awaits that later (also after a yield to the loop,
  two tasks, one never awaited, a task a library's loop makes),
  gathers two (also two coroutines whose callee changes `G` around a suspension), `wait_for`,
  awaits twice; iterates the generator (also breaking out, with a
  comprehension, with `anext`), makes it and never iterates it; `async with`; `async for` over
  the class; `async with` the `@asynccontextmanager`;
- in a `try` catching the KeyError the raising effect may raise (`WRAPS`), or not;
- observing `G` between the steps, then the result, the argument's length and `G`.

Each batch runs under every limit set (`LIMITS`): the default and `budget=0`.

    uv run --no-project python tests/async_protocol.py [--rowan N | --all] [--seed S] [--limits L] [-v]
"""
from __future__ import annotations

import itertools

from protocol_lib import Prog, cli

NAMES = ["G", "O"]

EFFECTS = {
    "glob": ["G = G * 10 + 1"],
    "arg": ["a.append(1)"],
    "sleep": ["await asyncio.sleep(0)", "G = G * 10 + 2"],
    "cond": ["if c1:", "    G = G * 10 + 4"],
    "boom": ["G = G * 10 + 3", "boom()"],
    "none": [],
}
# pairs only of these (every single effect alone)
PAIRED = ["glob", "arg", "sleep", "boom"]

SHAPES = {
    "await": ["r = await co{N}(xs)"],
    "never": ["c = co{N}(xs)", "O = G", "c.close()"],
    "late": ["c = co{N}(xs)", "O = G", "r = await c"],
    "task": ["t = asyncio.create_task(co{N}(xs))", "O = G", "r = await t"],
    "task_yield": ["t = asyncio.create_task(co{N}(xs))", "await asyncio.sleep(0)", "O = G", "r = await t"],
    "gather": ["rs = await asyncio.gather(co{N}(xs), co{N}(xs))", "r = rs[-1]"],
    "wait_for": ["r = await asyncio.wait_for(co{N}(xs), 5)"],
    "twice": ["r = await co{N}(xs)", "O = G", "r = await co{N}(xs)"],
    "agen": ["async for v in ag{N}(xs):", "    O = G", "    r = v"],
    "agen_break": ["async for v in ag{N}(xs):", "    r = v", "    break", "O = G"],
    "agen_comp": ["vs = [v async for v in ag{N}(xs)]", "r = vs[-1]"],
    "agen_anext": ["g = ag{N}(xs)", "r = await anext(g)", "O = G", "await g.aclose()"],
    "agen_never": ["g = ag{N}(xs)", "O = G"],
    "awith": ["async with Cm{N}(xs) as v:", "    O = G", "    r = v"],
    "aiter": ["async for v in It{N}(xs):", "    r = v"],
    "method": ["r = await Sv{N}(xs).run()"],
    "acm": ["async with acm{N}(xs) as v:", "    O = G", "    r = v"],
    "stored": ["cs = [co{N}(xs), co{N}(xs)]", "O = G", "for c in cs:", "    r = await c"],
    "loop": ["for i in range(2):", "    r = await co{N}(xs)"],
    "task_never": ["t = asyncio.create_task(co{N}(xs))", "O = G"],
    "tasks_two": ["t1 = asyncio.create_task(co{N}(xs))", "t2 = asyncio.create_task(co{N}(xs))", "O = G", "r = await t2", "O = G", "r = await t1"],
    "lib_task": ["t = asyncio.get_running_loop().create_task(co{N}(xs))", "G = 7", "O = G", "await asyncio.sleep(0)", "O = G", "r = await t"],
    "gather_callee": ["rs = await asyncio.gather(ci{N}(xs), ci{N}(xs))", "r = rs[0]", "O = r", "r = rs[-1]"],
}

WRAPS = ["plain", "try"]


def program(effects, shape, wrap) -> Prog:
    ind = lambda ls, k=1: ["    " * k + ln for ln in ls]
    body = [ln for e in effects for ln in EFFECTS[e]] or ["pass"]
    body_v = [ln.replace("a.append", "self.a.append") for ln in body]
    steps = SHAPES[shape]
    if wrap == "try":
        steps = ["try:", *ind(steps), "except KeyError:", "    O = G", "    r = -1"]
    lines = [
        "import asyncio",
        "",
        "async def co{N}(a):",
        "    global G",
        *ind(body),
        "    return G * 100 + len(a)",
        "",
        "def bump{N}(a):",
        "    global G",
        "    G = G * 10 + 5",
        "",
        "async def ci{N}(a):",
        "    bump{N}(a)",
        "    x = G",
        "    await asyncio.sleep(0)",
        "    bump{N}(a)",
        "    return x * 100 + len(a)",
        "",
        "async def ag{N}(a):",
        "    global G",
        *ind(body),
        "    yield G",
        *ind(body),
        "    yield G + 1",
        "",
        "class Cm{N}:",
        "    def __init__(self, a):",
        "        self.a = a",
        "    async def __aenter__(self):",
        "        global G",
        *ind(body_v, 2),
        "        return G",
        "    async def __aexit__(self, *exc):",
        "        global G",
        "        G = G * 10 + 9",
        "        return False",
        "",
        "class Sv{N}:",
        "    def __init__(self, a):",
        "        self.a = a",
        "    async def run(self):",
        "        global G",
        *ind(body_v, 2),
        "        return G",
        "",
        "@contextlib.asynccontextmanager",
        "async def acm{N}(a):",
        "    global G",
        *ind(body),
        "    yield G",
        "    G = G * 10 + 9",
        "",
        "class It{N}:",
        "    def __init__(self, a):",
        "        self.a = a",
        "        self.n = 0",
        "    def __aiter__(self):",
        "        return self",
        "    async def __anext__(self):",
        "        global G",
        "        if self.n == 2:",
        "            raise StopAsyncIteration",
        "        self.n += 1",
        *ind(body_v, 2),
        "        return G + self.n",
        "",
        "async def main{N}():",
        "    global G, O",
        "    r = 0",
        "    xs = [0]",
        *ind(steps),
        "    O = r",
        "    n = len(xs)",
        "    O = n",
        "    O = G",
        "",
        "def p{N}():",
        "    asyncio.run(main{N}())",
    ]
    return Prog(lines, end=["O = G"], desc=f"{'+'.join(effects)} {shape} {wrap}")


def programs():
    seqs = [(e,) for e in EFFECTS] + [p for p in itertools.product(PAIRED, repeat=2) if p[0] != "boom"]
    for effects, shape, wrap in itertools.product(seqs, SHAPES, WRAPS):
        yield program(effects, shape, wrap)


LIMITS = (None, "budget=0")

if __name__ == "__main__":
    cli(programs, NAMES, quick=600, limit_sets=LIMITS)
