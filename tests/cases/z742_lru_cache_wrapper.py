# functools.lru_cache / cache as decorators make a new function object: CPython's wrapper,
# which runs the function's code through its cache. It is not the function it wraps (even
# where an inner decorator gave back another function), `__wrapped__` is that function, its
# attributes are copied from it, and attributes set on the wrapper are its own.
import functools


def f0():
    return 0


def swap(f):
    return f0


@functools.lru_cache(maxsize=None)
@swap
def f3():
    return 3


same = f3 is f0
#@value same: includes={False}
#@todo value same: exactly={False}  (identity of the program's functions is not decided)
r3 = f3()
#@value r3: exactly={0}
back = f3.__wrapped__ is f0
#@value back: includes={True}
#@todo value back: exactly={True}  (as same)
f3.tag = 1
t0 = getattr(f0, "tag", "-")
#@value t0: exactly={"-"}
# (the store was into the wrapper, not into f0)
t3 = f3.tag
#@value t3: exactly={1}
f0.mark = "m"


@functools.cache
def g(x):
    return x + 1


g.mark = 2
gm = g.mark
#@value gm: exactly={2}
r = g(1)
#@value r: exactly={2}
again = g(1)
#@value again: exactly={2}
info = g.cache_info()
g.cache_clear()
gw = g.__wrapped__(5)
#@value gw: exactly={6}
name = g.__name__
#@value name: exactly={"g"}


def h0():
    return "h"


h0.color = "red"
hc = functools.lru_cache(maxsize=None)(h0)
hcol = hc.color
#@value hcol: exactly={"red"}
hc.color = "blue"
h0col = h0.color
#@value h0col: exactly={"red"}
