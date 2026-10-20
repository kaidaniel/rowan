# A `functools.wraps` wrapper made in a loop of rounds not known is a wrapper on every round:
# its name is the wrapped function's. In the main module (`counted(f2)` per round, kept in a
# list) and in an imported module, whose `@counted def f2` runs when `m.f2` is first read
# (deferred), here in such a loop. The wrapper's object made in a round becomes the loop's
# summary of it, which is still a wrapper of `f2` (not "inner", the wrapper's own name).
import functools
import sys

import wrapmod as m


def counted(fn):
    @functools.wraps(fn)
    def inner(*a):
        return fn(*a)
    return inner


def f2(v):
    return v + 2


n = len(sys.argv)
gs = []
for _i in range(n + 1):
    gs.append(counted(f2))
p = gs[0].__name__
#@value p: exactly={"f2"}
w = gs[-1](1)
#@value w: exactly={3}

for _j in range(n - 1):
    y = m.f2
q = m.f2.__name__
#@value q: exactly={"f2"}
r = m.f2(3)
#@value r: exactly={5}
