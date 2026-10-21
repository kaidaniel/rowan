# A generator that another generator iterates (`for v in inner():`, `yield from inner()`, a
# generator expression's first iterable) is closed with it: closing the outer one ends its
# loop by an exception, which drops the inner one, so the inner one's `finally` runs first,
# then the outer one's. Consumers that stop early (`next`, `any`) close what they stop at.
log = []


def guarded(tag):
    try:
        yield 1
        log.append(tag + " after 1")
        yield 2
    finally:
        log.append(tag + " closed")


def outer(tag):
    try:
        for v in guarded(tag):
            yield v * 10
    finally:
        log.append(tag + " outer")


r1 = next(outer("a"))
#@value r1: exactly={10}
n1 = len(log)
#@value n1: exactly={2}
w1 = log[0]
#@value w1: exactly={'a closed'}
w1b = log[1]
#@value w1b: exactly={'a outer'}
o = outer("b")
r2 = next(o)
#@value r2: exactly={10}
n2 = len(log)
#@value n2: exactly={2}
o = None
n2b = len(log)
#@value n2b: exactly={4}
w2 = log[-1]
#@value w2: exactly={'b outer'}
r3 = any(w > 0 for w in guarded("c"))
#@value r3: exactly={True}
n3 = len(log)
#@value n3: exactly={5}
r4 = any(w > 1 for w in guarded("d"))
#@value r4: exactly={True}
n4 = len(log)
#@value n4: exactly={7}
w4 = log[-1]
#@value w4: exactly={'d closed'}


def deleg(tag):
    yield from guarded(tag)


o = deleg("e")
r5 = next(o)
#@value r5: exactly={1}
n5 = len(log)
#@value n5: exactly={7}
o = None
n5b = len(log)
#@value n5b: exactly={8}
w5 = log[-1]
#@value w5: exactly={'e closed'}


def lines(tag):
    for v in guarded(tag):
        yield v


def first_line(tag):
    for v in lines(tag):
        return v


r6 = first_line("f")
#@value r6: exactly={1}
n6 = len(log)
#@value n6: exactly={9}
w6 = log[-1]
#@value w6: exactly={'f closed'}
