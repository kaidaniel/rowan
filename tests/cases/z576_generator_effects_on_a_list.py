# A generator that reads and changes a global list's item. Code between its items that
# stores into the list changes what it yields next; in a loop of unknown count, each new
# generator's first step (`G[0] += 1`) happens every round.
import sys

G = [0]


def watch():
    for _ in range(3):
        yield G[0]


def bump():
    G[0] += 1
    yield G[0]
    G[0] += 10
    yield G[0]


n = len(sys.argv)
g = watch()
x = next(g)                                  #@value x: exactly={0}
G[0] = 5
y = next(g, -1)                              #@value y: includes={5}
#@todo value y: exactly={5}  (the generator runs again with the item any value, not the one stored)
G[0] = 0
for _i in range(n - 1):
    h = bump()
    z = next(h)                              #@value z: includes={1, 2, 3, 4}
c = G[0]                                     #@value c: includes={0, 1, 2, 3, 4}
