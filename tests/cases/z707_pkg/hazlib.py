"""Decorated definitions in a module's code, for z707_deferred_hazards.py: `tag` is plain,
`count` appends to a shared list, `checks` reads an attribute a function may not have."""
import functools

ORDER = []
STATE = {"k": 1}
LABEL = "first"


def tag(fn):
    @functools.wraps(fn)
    def inner(*args, **kwargs):
        return fn(*args, **kwargs)

    inner.label = LABEL
    return inner


def count(fn):
    ORDER.append(fn.__name__)
    return fn


def checks(fn):
    fn.seen = fn.custom
    return fn


@tag
def used(x):
    return x + 1


@tag
def unused(x):
    return x * 2


@tag
def bump(x):
    STATE["k"] = 2
    return x


@count
def first():
    return 1


@count
def second():
    return 2


try:
    @checks
    def checked():
        return 0
except AttributeError:
    checked = None

LABEL = "second"
