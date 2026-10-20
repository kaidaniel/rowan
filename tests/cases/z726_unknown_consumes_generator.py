# A generator handed to code rowan cannot see may be consumed there, whole or in part: what its
# body changes (a global list, a list it was given) may have changed, also for one the program
# consumed in part before. (`operator.call` is such code since it may call what it is given:
# z725.)
import operator

L = []


def gen_cb():
    for i in range(2):
        L.append(i)
        yield i


operator.call(list, gen_cb())
n = len(L)
#@value n: includes={2}
last = L[-1] if L else -9
#@value last: includes={1}

M = []


def fill(xs):
    xs.append(1)
    yield 1
    xs.append(2)
    yield 2


g = fill(M)
next(g)
operator.call(next, g, None)
m = len(M)
#@value m: includes={2}
