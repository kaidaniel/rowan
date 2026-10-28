"""`'{}'.format(x)` is `str(x)` inside the template: of several literals (a `Literal['a', 3]`
parameter's) each one's text, of an int unknown a str. (Before, the literals other than strs were
kept as they were, `"a" | 3`, and an int unknown's result was claimed an int.)

Expectations were written from the program text before running the engine."""
from typing import Literal


def f(x: Literal["a", 3], n: int):
    r = "{}".format(x)
    #@general r: exactly={'a', '3'}
    s = "<{}>".format(n)
    #@general s: types={str}
    t = "{}".format(n)
    #@general t: types={str}
    return r


f("a", 1)
f(3, 2)
