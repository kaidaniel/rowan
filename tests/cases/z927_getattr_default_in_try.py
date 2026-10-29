"""`getattr(x, name, default)` gives the default where x lacks the attribute, None included:
inside a `try` catching AttributeError, the path where it is missing goes on with the
default (it does not end in the handler).

Expectations were written from the program text before running the engine."""
import os


def h(x):
    return x


f = h if os.environ.get("Z927") else None


def calls_of():
    try:
        return getattr(f, "calls", -1)
    except AttributeError:
        return "AE"


v = calls_of()                               #@value v: includes={-1}
#@todo value v: exactly={-1}   (the read on the copy still raises its AttributeError to the try)
try:
    w = getattr(f, "calls", -1)
except AttributeError:
    w = "AE"
vw = w                                       #@value vw: includes={-1}
#@todo value vw: exactly={-1}
