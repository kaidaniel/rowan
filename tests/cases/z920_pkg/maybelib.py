"""For z920_unbound_globals.py: a plain decorator (it only stores an attribute) reading a name
this module binds on some paths only."""
import os

if os.environ.get("Z920_LABEL"):
    LABEL = "set"


def tag(fn):
    fn.label = LABEL
    return fn


@tag
def f(x):
    return x + 1
