"""Hypothesis: `functools.partial` with positional and keyword arguments merges them with the
call's own arguments.

Expectations were written from the program text before running the engine."""

from functools import partial


def build(kind, **extra):
    return {"kind": kind, **extra}


mk = partial(build, "x", flag=True)
r = mk(size=2)                               #@row r: present={kind,flag,size} definite={kind,flag,size} closed
r2 = partial(build, kind="y")()              #@row r2: present={kind} definite={kind} closed
