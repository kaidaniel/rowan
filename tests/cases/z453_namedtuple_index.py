"""Hypothesis: an instance of a NamedTuple class (class syntax, a subclass of one, or
`collections.namedtuple`) indexed by position reads its fields in order, from either end; an
index past the fields raises IndexError. (pip's vendored `packaging` indexes its
`_GLibCVersion` NamedTuple: rowan said item 0 "is never set".)

Expectations were written from the program text before running the engine."""

import sys
from collections import namedtuple
from typing import NamedTuple


class V(NamedTuple):
    major: int
    minor: int = 0


class W(V):
    def total(self):
        return self[0] + self[-1]


Pt = namedtuple("Pt", "x y")

a = V(2, 3)
x = a[0]                                     #@value x: exactly={2}
y = a[-1]                                    #@value y: exactly={3}
t = W(4).total()                             #@value t: exactly={4}
n = len(sys.argv)
b = V(n, 7)
i = b[1]                                     #@value i: exactly={7}
try:
    m = a[n - 1]                             #@value m: includes={2, 3}
except IndexError:
    m = 0
p = Pt(5, 6)
px = p[1]                                    #@value px: exactly={6}
try:
    a[2]
    hit = False
except IndexError:
    hit = True
caught = hit                                 #@value caught: exactly={True}
