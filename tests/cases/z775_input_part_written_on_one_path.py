"""A part of an input (`R["r1"]`, an annotated list's element) is made where a path first
reads it. Written on one path only, the joined state kept that path's facts of it as if the
other path never had it, though there it is the input's part as it was: `R["r1"]["a"]` read
after `if c: R["r1"]["a"] = "s"` was claimed to be "s" alone. The other path holds it as it
starts: "s" or what the input holds."""
import json
import os
import sys
from dataclasses import dataclass

DATA = json.loads(os.environ.get("ROWAN_DATA", '{"r1": {"a": 1}}'))
n = len(sys.argv)

if n % 2 == 0:
    DATA["r1"]["a"] = "s"
p1 = DATA["r1"]["a"]
#@value p1: includes={'s'} unknown


def f(R, c):
    if c:
        R["r1"]["a"] = "s"
    q = R["r1"]["a"]
    #@general q: includes={'s'} unknown
    return q


def g(R, c):
    if c:
        R.r1.a = "s"
    q = R.r1.a
    #@general q: includes={'s'} unknown
    return q


@dataclass
class P:
    v: int


def h(ps: list[P], c: bool):
    if c:
        ps[0].v = 5
    q = ps[0].v
    #@general q: includes={5} types={int}
    return q


class B:
    def __init__(self, v):
        self.v = v


def k(bs: list[B], c: bool):
    if c:
        bs[0].v = 5
    q = bs[0].v
    #@general q: includes={5} unknown
    return q


r = f({"r1": {"a": 1}}, n > 5)
#@value r: exactly={1, 's'}
s = h([P(2)], n > 5)
#@value s: exactly={2, 5}
