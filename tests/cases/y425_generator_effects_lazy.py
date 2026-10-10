"""Hypothesis: a generator's body runs only as it is consumed: right after the call nothing of it
has run, after one next() only up to the first yield. A generator that changes the caller's list
or dict must not make those changes definite at the call (rowan runs the body eagerly; the caller
may only see the states at its suspension points). A generator without side effects keeps exact
items; a generator under @contextmanager keeps running around its with block."""
import contextlib


def tracked(log):
    for i in range(3):
        log.append(i)
        yield i


def fill(d):
    d["a"] = 1
    yield 1
    d["b"] = 2
    yield 2


def pure(xs):
    for x in xs:
        yield x * 2


@contextlib.contextmanager
def opened(state):
    state["open"] = True
    yield state
    state["open"] = False


if __name__ == "__main__":
    log = []
    g = tracked(log)
    n0 = len(log)                                #@value n0: includes={0}
    #@todo value n0: exactly={0}
    a = next(g)                                  #@value a: exactly={0}
    d = {}
    h = fill(d)
    first = next(h)                              #@value first: exactly={1}
    hasb = "b" in d                              #@value hasb: includes={False}
    _ = 0                                        #@row d: present={a,b} closed
    #@todo row d: present={a,b} definite={a} closed
    doubled = list(pure([1, 2, 3]))
    last = doubled[2]                            #@value last: exactly={6}
    nd = len(doubled)                            #@value nd: exactly={3}
    log2 = []
    total = sum(tracked(log2))                   #@value total: exactly={3}
    n2 = len(log2)                               #@value n2: includes={3}
    #@todo value n2: exactly={3}
    st = {}
    with opened(st) as s:
        inside = s["open"]                       #@value inside: exactly={True}
    after = st["open"]                           #@value after: exactly={False}
