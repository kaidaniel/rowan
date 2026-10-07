"""Symbolic summaries: name-based dispatch through an untyped parameter must not run another
class's method on this receiver; closures calling sibling closures; a library-style table lookup."""
from typing import Any


class RenameMap:
    def __init__(self, pairs):
        self.pairs = pairs

    def apply(self, k):
        return dict(self.pairs).get(k, k)         #@ ok pairs


class RenameFn:
    def __init__(self, fn):
        self.fn = fn

    def apply(self, k):
        return self.fn + k                        #@ ok fn ; none pairs


def rename_all(step, keys):
    # `step` is untyped: both `apply` methods are candidates, each only for its own class
    return [step.apply(k) for k in keys]


rename_all(RenameFn("x"), ["a"])                  #@ none pairs
rename_all(RenameMap((("a", "b"),)), ["a"])       #@ none fn


TABLE = {"a.*": 1, "b.*": 2, "c.x": 3}


def lookup(qual):
    for prefix in ("a.", "b.", "c.", "d.", "e.", "f.", "g.", "h.", "i.", "j.", "k.", "l.", "m.", "n.", "o.", "p.", "q."):
        if qual.startswith(prefix) and prefix + "*" in TABLE:
            return TABLE[prefix + "*"]            #@ ok a.* ; ok b.* ; none c.* ; none d.*
    return None


def lookup2(k):
    if k in ("a.*", "c.x", "zzz"):
        if k in TABLE:
            return TABLE[k]                       #@ ok a.* ; ok c.x ; none zzz
    return None


def make(n):
    cache = {}

    def key(i):
        return f"k{i}"

    def put(i, v):
        cache[key(i)] = v                         # calls a sibling closure through its cell

    def get(i):
        return cache[key(i)]

    put(1, "x")
    return get, put


g, p = make(3)
p(2, "y")
g(1)
