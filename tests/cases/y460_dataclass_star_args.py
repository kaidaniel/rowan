"""`*args` in a call of a dataclass / NamedTuple class supplies positional fields: a sequence of
known length gives them in order; one of unknown length gives each field the args don't the
sequence's element (when the call returns, every field without a default was supplied: a
short sequence raises TypeError). The star arguments were dropped, so every field read as not
supplied and definitely missing (rich's `ColorTriplet(*background)`)."""
import sys
from dataclasses import dataclass
from typing import NamedTuple


class CT(NamedTuple):
    red: int
    green: int
    blue: int


@dataclass
class P:
    x: int
    y: int = 0


def mk(bg):
    return CT(*bg)


t = mk((1, 2, 3))
r = t.red  #@value r: exactly={1}
u = CT(*sys.argv[1:4])
#@row u: present={red,green,blue} definite={red,green,blue} closed
p = P(*[5])
py = p.y  #@value py: exactly={0}
q = P(7, *sys.argv[1:2])
#@row q: present={x,y} definite={x,y} closed
print(r, u.blue, p.x, py, q.y)
