import functools


def counted(fn):
    @functools.wraps(fn)
    def inner(*a):
        return fn(*a)
    return inner


@counted
def f2(v):
    return v + 2
