"""Twenty functions decorated by a plain decorator (`tag`: a `functools.wraps` wrapper), for
z708_unused_definitions_cost_nothing.py: the program uses one."""
import functools


def tag(fn):
    @functools.wraps(fn)
    def inner(*args, **kwargs):
        return fn(*args, **kwargs)

    return inner


@tag
def f0(x):
    return x + 0


@tag
def f1(x):
    return x + 1


@tag
def f2(x):
    return x + 2


@tag
def f3(x):
    return x + 3


@tag
def f4(x):
    return x + 4


@tag
def f5(x):
    return x + 5


@tag
def f6(x):
    return x + 6


@tag
def f7(x):
    return x + 7


@tag
def f8(x):
    return x + 8


@tag
def f9(x):
    return x + 9


@tag
def f10(x):
    return x + 10


@tag
def f11(x):
    return x + 11


@tag
def f12(x):
    return x + 12


@tag
def f13(x):
    return x + 13


@tag
def f14(x):
    return x + 14


@tag
def f15(x):
    return x + 15


@tag
def f16(x):
    return x + 16


@tag
def f17(x):
    return x + 17


@tag
def f18(x):
    return x + 18


@tag
def f19(x):
    return x + 19
