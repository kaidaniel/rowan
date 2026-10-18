# `inspect` functions that look at a function they are given (its doc, file, kind) do not call
# it (scipy's docstring decorators call `inspect.getdoc` / `getsourcefile` on each function they
# wrap): they were unknown code that may run what it is given, a walk over everything the
# function's code reaches at every call (16% of importing scipy.special).
import inspect

counter = {"n": 0}


def bump():
    """Count a call."""
    counter["n"] += 1
    return counter["n"]


doc = inspect.getdoc(bump)
src = inspect.getsourcefile(bump)
kind = inspect.isfunction(bump)
n = counter["n"]
#@value n: exactly={0}
