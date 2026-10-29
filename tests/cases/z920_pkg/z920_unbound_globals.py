"""A global bound on some paths only, read inside a function: where it is not bound, the read
raises NameError. `maybelib`'s decorator `tag` reads LABEL, bound only when Z920_LABEL is set:
at the `def` it raises, so the import fails (it cannot be put off to the first read of `f`).
`mode` reads MODE, which this module binds only when Z920_MODE is set.

Expectations were written from the program text before running the engine."""
import os

try:
    import maybelib
    lab = maybelib.f.label
except NameError:
    lab = "unbound"
v = lab                                      #@value v: exactly={"set", "unbound"}

if os.environ.get("Z920_MODE"):
    MODE = "fast"


def mode():
    return MODE


try:
    m = mode()
except NameError:
    m = "none"
w = m                                        #@value w: exactly={"fast", "none"}
