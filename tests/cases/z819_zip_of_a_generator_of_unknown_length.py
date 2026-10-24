"""`zip(gen, xs)` of a generator whose number of items depends on the path: it asks the generator
as the loop goes, so a raise after the generator's first item comes after the first round."""
import os

c1 = os.environ.get("C1") == "1"


def g():
    if c1:
        return
    yield 2
    raise ValueError("after the first")


def p():
    n = 0
    try:
        for a, b in zip(g(), (1,)):
            n = n + a
    except ValueError:
        return n + 100
    return n


r = p()
#@value r: includes={0, 102}
#@todo value r: exactly={0, 102}  (claimed: 0, 2 or any int)
