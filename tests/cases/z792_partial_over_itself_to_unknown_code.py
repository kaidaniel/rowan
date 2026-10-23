"""A `functools.partial` built over the previous one at the same place, a number of times not
known (`cb = partial(cb, i)` in a loop: one object that holds itself), given to unknown code
twice: whether it is a library's code is asked once per object, not again for itself without
end (rowan overflowed its stack). (Found by the held-out corpus: transitions/core.py, whose
analysis crashed.)"""
import os
from functools import partial

try:
    import notinstalled_runner  # (not installed here: unknown code to rowan)
except ImportError:
    notinstalled_runner = None


def f(*a):
    return a


def build(n):
    cb = f
    for i in range(n):
        cb = partial(cb, i)
    return cb


def use(cb):
    try:
        notinstalled_runner.run(cb)
        notinstalled_runner.run(cb)
    except Exception:
        pass
    return cb


n = len(os.environ.get("Z792_N", "xx"))
got = use(build(n))()  #@value got: includes={(0, 1)}
