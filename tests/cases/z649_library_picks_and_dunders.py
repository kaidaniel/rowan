"""A library call that hands back items it was given (`random.choice`, `random.sample`,
`random.choices`) gives back the program's instances: formatting one runs its `__repr__`. They
were unknowns, so `f"{o!r}"` ran no code and `CALLS` was claimed unchanged. A library call taken
to change nothing it is given may still run the dunders of the instances it is given (`os.fspath`
calls `__fspath__`, `logging` formats `%s` with `__str__`, a list's items with `__repr__`): what
those change may change.

Expectations were written from the program text before running the engine."""
import logging
import os
import random

CALLS = {"n": 0}
LOG = []


class A:
    def __init__(self, k):
        self.k = k

    def __repr__(self):
        CALLS["n"] = CALLS["n"] + 1
        return "A" + str(self.k)


xs = [A(1), A(2)]
o = random.choice(xs)
s = f"{o!r}"
#@value s: exactly={"A1", "A2"}
n1 = CALLS["n"]
#@value n1: exactly={1}
ys = random.sample(xs, 1)
t = repr(ys[0])
#@value t: exactly={"A1", "A2"}
n2 = CALLS["n"]
#@value n2: exactly={2}
zs = random.choices(xs, k=3)
u = repr(zs[2])
#@value u: exactly={"A1", "A2"}
n3 = CALLS["n"]
#@value n3: exactly={3}


class P:
    def __fspath__(self):
        CALLS["n"] = CALLS["n"] + 100
        return "/tmp"

    def __str__(self):
        LOG.append("str")
        return "p"


class Q:
    def __repr__(self):
        LOG.append("repr")
        return "q"


p = P()
path = os.fspath(p)
n4 = CALLS["n"]
# (103: __fspath__ ran)
#@value n4: includes={103}
logging.warning("%s", p)
m1 = len(LOG)
# (1: the warning was formatted by the last-resort handler)
#@value m1: includes={1}
logging.warning("%s", [Q()])
m2 = len(LOG)
#@value m2: includes={2}
