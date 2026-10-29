"""For z922_module_dict.py: a module with a plain decorator (`tag`), a function and a
constant."""
import functools

X = 1


def tag(fn):
    @functools.wraps(fn)
    def inner(*a):
        return fn(*a) * 10

    inner.label = "t"
    return inner


@tag
def f(x):
    return x + 1


def g():
    return Z
