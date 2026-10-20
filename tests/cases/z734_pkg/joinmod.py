import functools


def tagged(fn):
    fn.tag = "t"
    return fn


def with_tag(t):
    def deco(fn):
        fn.tag = t
        return fn
    return deco


def logged(fn):
    @functools.wraps(fn)
    def inner(*a):
        return fn(*a)
    inner.calls = 0
    return inner


@tagged
def f(v):
    return v


@with_tag("w")
def g(v):
    return v + 1


@logged
def h(v):
    return v * 2


def tagged(fn):
    return 7
