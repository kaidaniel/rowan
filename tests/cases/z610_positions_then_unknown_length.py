"""Hypothesis: a list whose positions were set together (a slice assignment of known items, a
partial's args, a known index store) keeps its elements when it later loses its length (more
appends than rowan counts): iterating it, or testing membership, sees every value it may hold.

Expectations were written from the program text before running the engine."""
import functools


def spliced():
    xs = [1, 2, 3]
    xs[0:1] = [7, 8]
    for i in range(20):
        xs.append(i + 100)
    has7 = 7 in xs
    #@value has7: includes={True}
    eights = [x for x in xs if x == 8]
    n = len(eights)
    #@value n: includes={1}
    return has7, n


def stored():
    ys = [0, 0]
    ys[1] = 5
    for i in range(20):
        ys.append(-i)
    has5 = 5 in ys
    #@value has5: includes={True}
    return has5


def add(a, b, c):
    return a + b + c


def from_partial():
    p = functools.partial(add, 3, 4)
    zs = list(p.args)
    for i in range(20):
        zs.append(i + 50)
    has3 = 3 in zs
    #@value has3: includes={True}
    return has3


if __name__ == "__main__":
    assert spliced() == (True, 1)
    assert stored() is True
    assert from_partial() is True
