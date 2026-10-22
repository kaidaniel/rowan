import functools


def with_tag(t):
    def deco(fn):
        fn.tag = t
        return fn
    return deco


def wrapped(fn):
    def inner(*a, **k):
        return fn(*a, **k)
    return functools.update_wrapper(inner, fn)


def tagged(fn):
    fn.tag = "t"
    return fn


def f0(v):
    return -v


@wrapped
@with_tag("w")
def f2(v):
    return v + 1


@tagged
def f3(v):
    return v + 3
