# Calls of `functools.lru_cache` / `functools.cache` functions: a call that hits the cache
# gives back the cached result and does not run. A wrapper's `fn(*a)` (arguments not known one
# by one), arguments not known, a method (its receiver is part of the key), and a bounded cache
# (`maxsize=1`: it drops results, so the call may run again) are each a hit or a miss.
import functools
import sys

CALLS = {"n": 0}


def counted(fn):
    @functools.wraps(fn)
    def inner(*a):
        CALLS["n"] += 1
        return fn(*a)
    return inner


@counted
@functools.lru_cache(maxsize=None)
def f3(v):
    return CALLS["n"] + v


x = f3(2)
#@value x: exactly={3}
y = f3(2)
#@value y: includes={3}

L = []


@functools.cache
def g(v):
    L.append(v)
    return [v]


k = sys.argv[0][:0] + "a"
r1 = g(k)
r2 = g(k)
same = r1 is r2
#@value same: includes={True}
ln = len(L)
#@value ln: includes={1}
r1.append(9)
z = len(r2)
#@value z: includes={2}

M = []


@functools.lru_cache(maxsize=1)
def h(v):
    M.append(v)
    return len(M)


h(1)
h(2)
a = h(1)
#@value a: includes={3}


class C:
    def __init__(self):
        self.n = 0

    @functools.lru_cache(maxsize=None)
    def get(self, v):
        self.n += 1
        return self.n


c = C()
c.get(1)
b = c.get(1)
#@value b: includes={1}
cn = c.n
#@value cn: includes={1}
