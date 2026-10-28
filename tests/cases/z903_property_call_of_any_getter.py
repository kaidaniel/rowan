"""`property(fget, fset, fdel)` called with a getter that is not one function of the program's
(one of two chosen by a test, a library function), or a setter of two: a read runs whichever
getter it may be, a store whichever setter, a store with no setter raises AttributeError, and
`.setter(f)` makes a property with that setter. (Before, the read gave the property object
itself: `vy` was claimed `property(...)` and `n` 0 only; a setter of two made every store raise; and
`isinstance(A.y, property)` was False.)

Expectations were written from the program text before running the engine."""
import sys

log = []


def g1(self):
    log.append("g1")
    return 1


def g2(self):
    return "two"


def s1(self, v):
    self._v = v


def s2(self, v):
    self._w = v


pick = len(sys.argv) > 1


class A:
    def __init__(self):
        self._x = 5

    y = property(g1 if pick else g2)
    w = property(fget=g2, fset=s1 if pick else s2)
    u = property(id)


class B:
    b = property(g1 if pick else g2)
    b = b.setter(s1)


o = A()
vy = o.y
#@value vy: exactly={1, "two"}
n = len(log)
#@value n: exactly={0, 1}
o.w = 7
has_v = getattr(o, "_v", None)
#@value has_v: exactly={7, None}
vw = o.w
#@value vw: exactly={"two"}
vu = o.u
#@value vu: types={int}
try:
    o.y = 3
    stored = "stored"
except AttributeError:
    stored = "refused"
#@value stored: exactly={"refused"}
q = B()
q.b = 9
vb = q._v
#@value vb: exactly={9}
isprop = isinstance(A.y, property)
#@value isprop: exactly={True}
