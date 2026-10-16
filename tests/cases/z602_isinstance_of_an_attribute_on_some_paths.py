# `isinstance(a.k, str)` where the instance has k on some paths only (a tuple) and its class
# has k (a str) on the others: the test holds on those, so `a.k + "!"` runs there.
import sys


class A:
    k = "A"


class B(A):
    k = "B"


def pick(x, y):
    return x if n > 2 else y


n = len(sys.argv)
a = A()
b = B()
pick(a, b).k = (3,)
r = a.k + "!" if isinstance(a.k, str) else "x"
#@value r: includes={"A!", "x"}
