# In a handler that may catch an exception rowan knows nothing of, `type(e).__name__` is some
# value (it was taken for a method object).
import sys


class E1(Exception):
    pass


def boom(v):
    if v == 1:
        raise E1("one")
    return v


n = len(sys.argv)
x = 0
try:
    x = boom(n)
except (E1, KeyError) as e:
    x = type(e).__name__
r = x                                        #@value r: includes={"E1", 2}
