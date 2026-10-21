"""Hypothesis: a `functools.cached_property` cached on some paths only (read in one branch, or
in a property that returns early on others: pip's `BaseDistribution.metadata` through
`local` / `installed_location` / `raw_name`) is read on the other paths by its getter, which
caches it there: the read never fails, its value is the cached one where it was cached and the
getter's where it runs now, and the instance holds it on every path after.

Expectations were written from the program text before running the engine."""
import functools
import os


class D:
    def __init__(self):
        self.n = 0

    @functools.cached_property
    def m(self):
        return self.n


def branch():
    d = D()
    if os.environ.get("Z769"):
        d.m
    d.n = 5
    x = d.m                              #@ ok m
    #@value x: exactly={0, 5}
    y = d.m                              #@ ok m
    #@value y: exactly={0, 5}
    #@row d: present={n, m} definite={n, m} closed
    return x


r1 = branch()
#@value r1: exactly={0, 5}


class Dist:
    def __init__(self, name):
        self._name = name

    @functools.cached_property
    def metadata(self):
        return {"Name": self._name}

    @property
    def raw_name(self):
        return self.metadata.get("Name", "?")   #@ ok metadata

    @property
    def location(self):
        return self.raw_name + "/site"

    @property
    def local(self):
        if os.environ.get("Z769"):
            return False
        return self.location.startswith("a")


def names():
    out = []
    for n in ("a", "b"):
        d = Dist(n)
        if d.local:
            out.append(d.raw_name)
    return out


r2 = len(names())
#@value r2: exactly={0, 1}
