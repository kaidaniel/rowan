# `next(it, default)` gives the default wherever the iterator may have ended: a filtered
# generator expression over a list of one of two contents, a generator that may yield nothing.
# A number compared with `==` narrows by value (`w == 2` keeps 2 of `2 | 3`).
import sys
def counter(k):
    i = 0
    while i < k:
        yield i
        i += 1
n = len(sys.argv)
xs = [1, 2]
if n % 2 == 0:
    xs = [1, 3]
x = next((w for w in xs if w == 2), -1)
#@value x: exactly={2, -1}
a = next(counter(n - 1), -1)
#@value a: includes={0, -1}
#@todo value a: exactly={0, -1}  (the first yield of a loop of unknown count: i is still 0)
it = iter(xs)
b = next(it, -1)
#@value b: exactly={1}
c = next(it, -1)
#@value c: exactly={2, 3}
d = next(it, -1)
#@value d: exactly={-1}
v = xs[1]
q = -1
if v == 2:
    q = v
q2 = q
#@value q2: exactly={-1, 2}
t = True if n > 2 else 0
r = -1
if t == 1:
    r = t
r2 = r
#@value r2: exactly={-1, True}
f = 2.0 if n > 2 else 3
g = -1
if f != 2:
    g = f
g2 = g
#@value g2: exactly={-1, 3}
