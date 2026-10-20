"""A fake installed package for z705_inert_decorators.py: its import needs more work than an
entry gets (`expensive`), and the decorators after that point are an inert one (`wrap`: it
makes a wrapper and stores an attribute) and a registry (`register`)."""
import functools

ROWS = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
REG = {}


def _cell(i, j):
    return i * j


def expensive():
    out = {}
    for i in ROWS:
        for j in ROWS:
            out[i] = _cell(i, j)
    return out


def wrap(fn):
    @functools.wraps(fn)
    def inner(*args, **kwargs):
        return fn(*args, **kwargs)

    inner.tagged = True
    return inner


def register(fn):
    REG[fn.__name__] = fn
    return fn


TABLE = expensive()


@wrap
def double(x):
    return 2 * x


@register
def plain(x):
    return x + 1
