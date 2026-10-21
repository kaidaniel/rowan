"""Hypothesis: a raise no running try can see (every handler surely misses its class and nothing
runs on its way out: no finally, a `with` of managers that run nothing, `redirect_stdout`) is
not recorded: it leaves the program. A driver that runs everything under
`with redirect_stdout(out): try: … except SystemExit:` (pip's) then joins none of the
ValueErrors its callees may raise. What a try does see is unchanged: a finally on the way out,
the program's manager's `__exit__`, and a handler further out that catches what an inner try
misses each start from the raises that reach them; one that cannot be raised there is not.

Expectations were written from the program text before running the engine."""
import io
import json
import os
from contextlib import redirect_stdout

ROWS = json.loads(os.environ.get("Z768_ROWS", '[{"n": 1, "a": "x", "b": "y", "c": "z"}]'))
STAGE = ["start"]


class Stop(Exception):
    pass


def check(r):
    if r.get("bad"):
        raise ValueError("bad row")
    return r


def count(rows):
    seen = []
    for r in rows:
        check(r)
        seen.append(r["n"])
        check(r)
        seen.append(r["a"])
        check(r)
        seen.append(r["b"])
        seen.append(r["c"])
        if r["a"] == "x":
            seen.append(r["b"])
    return len(seen)


def driver(rows):
    out = io.StringIO()
    with redirect_stdout(out):
        try:
            n = count(rows)
            m = count(rows)
            k = count(rows)
        except SystemExit:
            n = -1
    return n


r1 = driver(ROWS)
#@value r1: types={int}


def with_finally(rows):
    stage = "start"
    try:
        stage = "reading"
        for r in rows:
            check(r)
        stage = "done"
    finally:
        at = stage
        #@value at: exactly={"reading", "done"}
    return at


r2 = with_finally(ROWS)
#@value r2: exactly={"done"}


class Tracker:
    def __enter__(self):
        return self

    def __exit__(self, *exc):
        got = STAGE[0]
        #@value got: exactly={"reading", "done"}
        self.got = got
        return False


def with_manager(rows):
    t = Tracker()
    with t:
        STAGE[0] = "reading"
        for r in rows:
            check(r)
        STAGE[0] = "done"
    return t.got


r3 = with_manager(ROWS)
#@value r3: exactly={"done"}


def inner(rows):
    stage = "a"
    try:
        stage = "b"
        check(rows[0])
        stage = "c"
    except Stop:
        stage = "stop"
    return stage


def outer(rows):
    try:
        return inner(rows)
    except ValueError:
        return "caught"


r4 = outer(ROWS)
#@value r4: exactly={"c", "caught"}
#@joins <= 40
#@visits <= 200
