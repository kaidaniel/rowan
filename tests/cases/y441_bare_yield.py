"""A bare `yield` produces None; a `yield` expression's value is None
when the generator is iterated (nothing is sent)."""


def g():
    yield 1
    yield
    yield 3


xs = list(g())
n = len(xs)
#@value n: exactly={3}
mid = xs[1]
#@value mid: exactly={None}
last = xs[2]
#@value last: exactly={3}


def h():
    got = yield
    yield got


ys = list(h())
ny = len(ys)
#@value ny: exactly={2}
y1 = ys[1]
#@value y1: exactly={None}
