"""`map(f, xs)` calls `f` as each item is taken, not when it is made: what `f` changes comes with
the items taken so far. Taken again after the list grew, it calls `f` on the items after those
taken, not on those again."""
log = []


def f(v):
    log.append(v)
    return v * 10


xs = [1, 2, 3]
it = map(f, xs)
n0 = len(log)
#@value n0: exactly={0}
first = next(it)
#@value first: exactly={10}
n1 = len(log)
#@value n1: exactly={1}
xs.append(4)
rest = list(it)
n2 = len(log)
#@value n2: exactly={4}
last = rest[-1]
#@value last: exactly={40}
