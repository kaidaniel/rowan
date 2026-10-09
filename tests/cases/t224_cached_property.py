"""Hypothesis: a `functools.cached_property` returns the same object on every access, so a
store through the first access is seen by the next.

Expectations were written from the program text before running the engine."""

from functools import cached_property


class Conf:
    @cached_property
    def data(self):
        return {"loaded": True}


c = Conf()
c.data["extra"] = 1
d = c.data                                   #@row d: present={loaded,extra} definite={loaded,extra} closed
