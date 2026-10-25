"""A comprehension over a generator called anew in each round of a loop reads, per item, what
the generator changed by then. In the first round `bump()` makes G[0] 1 before its item, so
the comprehension sees 1; the loop's body then sets G[0] to 7, and in the second round the
generator makes it 8. (Before, from the second round on the generator object stood for
several runs and the comprehension's items saw G[0] as before the generator ran: 0 or 7,
never 8.)

Expectations were written from the program text before running the engine."""
import sys

G = [0]


def bump():
    G[0] += 1
    yield G[0]


n = len(sys.argv)
y = 5
for _i in range(n):
    y = [G[0] for w in bump()][-1]
    G[0] = 7
seen = y
#@value seen: includes={1, 8}
after = G[0]
#@value after: includes={7}


def counted(xs):
    for v in xs:
        G[0] = v
        yield v


got = 0
for _j in range(n):
    got = [G[0] + 100 for v in counted([3, 4])][0]
    G[0] = 9
first = got
#@value first: includes={103}
