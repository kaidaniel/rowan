import functools


def tagged(fn):
    fn.tag = "t"
    return fn


def logged(fn):
    @functools.wraps(fn)
    def inner(*a):
        return fn(*a)
    return inner


@tagged
def f(v):
    return v


@logged
def g(v):
    return v + 1
