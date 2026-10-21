# A `for` over a generator made for it alone (`for v in gen():`) drops it when the loop ends
# other than by using it up: at a `break`, a `return` from inside it, an exception leaving it.
# CPython then closes it: GeneratorExit is raised at the yield it stands at, so its `finally`
# blocks and its with statements' `__exit__` run there, before the code after the loop, the
# caller, or the handler of the exception. A generator a name holds is not closed by the loop.
log = []


def guarded(tag):
    try:
        yield 1
        log.append(tag + " after 1")
        yield 2
        log.append(tag + " after 2")
    finally:
        log.append(tag + " closed")


for v in guarded("a"):
    break
n1 = len(log)
#@value n1: exactly={1}
w1 = log[-1]
#@value w1: exactly={'a closed'}
for v in guarded("b"):
    if v == 2:
        break
n2 = len(log)
#@value n2: exactly={3}
w2 = log[-1]
#@value w2: exactly={'b closed'}


def first(tag):
    for v in guarded(tag):
        return v


r3 = first("c")
#@value r3: exactly={1}
n3 = len(log)
#@value n3: exactly={4}
w3 = log[-1]
#@value w3: exactly={'c closed'}
try:
    for v in guarded("d"):
        raise ValueError(v)
except ValueError:
    n4 = len(log)
#@value n4: exactly={5}


class Tracker:
    def __init__(self, out):
        self.out = out

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.out.append("exit")
        return False


def managed(out):
    with Tracker(out):
        yield 1
        out.append("second")
        yield 2


o5 = []
for v in managed(o5):
    break
n5 = len(o5)
#@value n5: exactly={1}
w5 = o5[0]
#@value w5: exactly={'exit'}


def counted(k, out):
    try:
        for i in range(k):
            yield i
    finally:
        out.append(k)


o6 = []
for v in counted(5, o6):
    if v == 2:
        break
n6 = len(o6)
#@value n6: exactly={1}
o7 = []
for v in counted(2, o7):
    pass
n7 = len(o7)
#@value n7: exactly={1}
g8 = guarded("h")
for v in g8:
    break
n8 = len(log)
#@value n8: exactly={5}
r8 = next(g8)
#@value r8: exactly={2}
n8b = len(log)
#@value n8b: exactly={6}
w8 = log[-1]
#@value w8: exactly={'h after 1'}
