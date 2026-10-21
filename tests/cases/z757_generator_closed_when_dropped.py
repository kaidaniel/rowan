# A suspended generator nothing holds any more is closed by CPython there (its reference count
# reaches zero): when the only name holding it is rebound or deleted, when the function whose
# local it is returns, when a temporary's statement ends. Its `finally` blocks run then. One
# that some path may still hold may or may not have been closed.
import itertools
import sys

log = []


def guarded(tag):
    try:
        yield 1
        log.append(tag + " after 1")
        yield 2
    finally:
        log.append(tag + " closed")


g = guarded("a")
r1 = next(g)
#@value r1: exactly={1}
n1 = len(log)
#@value n1: exactly={0}
g = None
n2 = len(log)
#@value n2: exactly={1}
w2 = log[-1]
#@value w2: exactly={'a closed'}
g = guarded("b")
next(g)
next(g)
del g
n3 = len(log)
#@value n3: exactly={3}
w3 = log[-1]
#@value w3: exactly={'b closed'}


def take_one(tag):
    h = guarded(tag)
    return next(h)


r4 = take_one("c")
#@value r4: exactly={1}
n4 = len(log)
#@value n4: exactly={4}


def consume(it):
    return next(it)


r5 = consume(guarded("d"))
#@value r5: exactly={1}
n5 = len(log)
#@value n5: exactly={5}
r6 = list(itertools.islice(guarded("e"), 1))
n6 = len(log)
#@value n6: exactly={6}
w6 = log[-1]
#@value w6: exactly={'e closed'}
ge = (w * 3 for w in guarded("f"))
r7 = next(ge)
#@value r7: exactly={3}
n7 = len(log)
#@value n7: exactly={6}
ge = None
n7b = len(log)
#@value n7b: exactly={7}
w7 = log[-1]
#@value w7: exactly={'f closed'}
keep = []
g = guarded("g")
next(g)
if len(sys.argv) > 5:
    keep.append(g)
g = None
n8 = len(log)
#@value n8: exactly={7, 8}
