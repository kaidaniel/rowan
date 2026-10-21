# A generator made for one consumer that stops early (next, any, all, `in`) is closed when the
# consumer is done with it (CPython drops it there): GeneratorExit is raised at the yield it
# stopped at, so its `finally` blocks and its with statements' `__exit__` run, and an
# `except GeneratorExit` handler too; nothing after that yield runs.
log = []


def guarded(tag):
    try:
        yield tag
        log.append("after " + tag)
        yield tag + "!"
    finally:
        log.append("closed " + tag)


r1 = next(guarded("a"), None)
#@value r1: exactly={'a'}
n1 = len(log)
#@value n1: exactly={1}
r2 = any(guarded("b"))
#@value r2: exactly={True}
n2 = len(log)
#@value n2: exactly={2}
r3 = all(guarded(""))
#@value r3: exactly={False}
n3 = len(log)
#@value n3: exactly={3}
r4 = list(guarded("c"))
n4 = len(log)
#@value n4: exactly={5}
last = log[-1]
#@value last: exactly={'closed c'}


def swallow(out):
    try:
        yield 1
        out.append("after")
    except GeneratorExit:
        out.append("exit")


o5 = []
r5 = next(swallow(o5))
#@value r5: exactly={1}
n5 = len(o5)
#@value n5: exactly={1}
w5 = o5[0]
#@value w5: exactly={'exit'}


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


o6 = []
r6 = 1 in managed(o6)
#@value r6: exactly={True}
n6 = len(o6)
#@value n6: exactly={1}
w6 = o6[0]
#@value w6: exactly={'exit'}


def plain(out):
    out.append("start")
    yield 1
    out.append("never")


o7 = []
r7 = next(plain(o7))
n7 = len(o7)
#@value n7: exactly={1}
