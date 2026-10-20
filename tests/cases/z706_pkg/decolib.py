"""Decorated definitions in a module's code, for z706_deferred_definitions.py: `tag` and
`flagit` are plain (they make a wrapper, store attributes, read only names), `register`
changes what others hold (a registry)."""
import functools

CALLS = []
REGISTRY = {}
LABEL = "first"


def tag(fn):
    @functools.wraps(fn)
    def inner(*args, **kwargs):
        CALLS.append(fn.__name__)
        return fn(*args, **kwargs)

    inner.label = LABEL
    return inner


def flagit(fn):
    fn.flag = True
    return fn


def register(fn):
    REGISTRY[fn.__name__] = fn
    return fn


@tag
def used(x):
    return x + 1


@tag
def unused(x):
    return x * 2


@tag
@register
def both():
    return "b"


@tag
@flagit
def flagged():
    return "f"


LABEL = "second"
