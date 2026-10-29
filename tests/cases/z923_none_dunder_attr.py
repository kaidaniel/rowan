"""None has the attributes every object has (`__class__`, `__eq__`, `__doc__`, …) and
`__bool__`, but no `__name__`: `f.__name__` with f a function or None raises AttributeError
where it is None.

Expectations were written from the program text before running the engine."""
import os


def h(x):
    return x


f = h if os.environ.get("Z923") else None


def name_of():
    try:
        return f.__name__
    except AttributeError:
        return "AE"


n = name_of()                                #@value n: exactly={"h", "AE"}

try:
    q = f.__qualname__
except AttributeError:
    q = "AE"
vq = q                                       #@value vq: exactly={"h", "AE"}
cls = f.__class__.__name__
#@todo value cls: exactly={"function", "NoneType"}   (the classes of functions and of None are not modelled)
