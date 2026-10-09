"""Hypothesis: a `@contextmanager` generator's yielded value is bound by `with ... as`, the
body's stores reach it, and the code after the `yield` (in a finally) runs at the end of the
with.

Expectations were written from the program text before running the engine."""

from contextlib import contextmanager


@contextmanager
def session():
    s = {"open": True}
    try:
        yield s
    finally:
        s["closed"] = True


with session() as s:
    s["used"] = 1
    inside = dict(s)                         #@row inside: present={open,used} definite={open,used} closed
_ = 0                                        #@row s: present={open,used,closed} definite={open,used,closed} closed
