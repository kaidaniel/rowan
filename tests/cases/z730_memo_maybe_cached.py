# A cached function called with an argument not known, then with a literal one: the first call
# may have cached the second's result (here it did: `x` is 2 when the program runs with no
# arguments), so the second call may be a hit, which does not run: `CALLS["n"]` may be 1.
import functools
import sys

CALLS = {"n": 0}


@functools.lru_cache(maxsize=None)
def f(v):
    CALLS["n"] += 1
    return v * 2


x = len(sys.argv) + 1
f(x)
r = f(2)
#@value r: includes={4}
c = CALLS["n"]
#@value c: includes={1}
