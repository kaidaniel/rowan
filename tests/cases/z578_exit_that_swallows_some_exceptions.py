# A manager whose `__exit__` returns True for some exceptions only: those it swallows, the
# others go on to the enclosing handler.
import sys


class E1(Exception):
    pass


class E2(E1):
    pass


class Sup:
    def __enter__(self):
        return self

    def __exit__(self, t, e, tb):
        return t is not None and issubclass(t, E2)


def boom(v):
    if v == 1:
        raise E1("one")
    if v == 2:
        raise E2("two")
    return v


n = len(sys.argv)
y = n % 3
try:
    with Sup():
        x = boom(y)
    y = "after"
except E1:
    y = "caught"
r = y                                        #@value r: exactly={"after", "caught"}
