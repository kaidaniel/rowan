# A generator without a `finally` that changes an older object between its yields: a loop or
# comprehension over it sees, at each item, what it did up to that item's yield, not its end
# (the generator runs on only when asked for the next item); after the loop, its end.
import sys

G = [0]


def gen():
    yield 1
    G[0] = 7
    yield 2


def loop():
    G[0] = 0
    seen = []
    for w in gen():
        seen.append(G[0])
    return seen, G[0]


seen, after = loop()
a = seen[0]
#@value a: exactly={0}
b = seen[1]
#@value b: exactly={7}
#@value after: exactly={7}


def comp():
    G[0] = 0
    return [G[0] for w in gen()]


c = comp()
c0 = c[0]
#@value c0: exactly={0}
c1 = c[1]
#@value c1: exactly={7}


def genexp():
    G[0] = 0
    return list(G[0] + w for w in gen())


e = genexp()
e0 = e[0]
#@value e0: exactly={1}
e1 = e[1]
#@value e1: exactly={9}


def many(n):
    for i in range(n):
        yield i
        G[0] = 7


def loop_many(n):
    G[0] = 0
    seen = []
    for w in many(n):
        seen.append(G[0])
    return seen


m = loop_many(len(sys.argv) + 1)
m0 = m[0]
#@value m0: includes={0}
