"""`x in gen()`, `next(gen(), d)` and `any(gen())` stop at the first item that answers them; a
generator that returns before its yield on some path gives no item there, so the answer is
the one for no items."""
import os

c1 = os.environ.get("C1") == "1"


def g():
    if c1:
        return
    yield 2


found = 2 in g()
#@value found: includes={True, False}
#@todo value found: exactly={True, False}  (`in` at module level: a bool not known)
first = next(g(), 0)
#@value first: exactly={0, 2}
some = any(g())
#@value some: exactly={True, False}
