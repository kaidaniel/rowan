# A generator used up by `list(...)` in a loop of unknown count, or by a comprehension that
# runs to a fixpoint (its iterable one of two lists): what it changes is changed after.
import sys

G = [0]


def bump():
    G[0] += 1
    yield G[0]
    G[0] += 10
    yield G[0]


def evens(xs):
    for v in xs:
        if v % 2 == 0:
            yield v
        else:
            G[0] = v


n = len(sys.argv)
for _i in range(n - 1):
    x = list(bump())[-1]
a = G[0]                                     #@value a: includes={0, 11, 22, 33, 44}
xs = [1, 2]
if n > 2:
    xs = [1, 3]
G[0] = 0
y = sum(1 for _ in evens(xs))
b = G[0]                                     #@value b: includes={1, 3}
G[0] = 0
z = [w for w in evens(xs) if w > 0]
c = G[0]                                     #@value c: includes={1, 3}
#@todo value c: exactly={1, 3}  (one of two lists iterated position by position: 2 joins in)
