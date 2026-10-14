"""Hypothesis: calling a value that is one of several classes makes instances only of the
classes whose construction can complete: a class whose `__init__` raises on every path
gives no instance (timm builds attention layers from a class chosen by name: the ones
that require an argument not given went on, their attributes "never set", 2149 false ✗).

Expectations were written from the program text before running the engine."""

import sys


class Strict:
    def __init__(self, size=None):
        if size is None:
            raise ValueError("size required")
        self.size = size

    def area(self):
        return self.size * 2


class Loose:
    def __init__(self, size=None):
        self.size = 1

    def area(self):
        return self.size


pick = Strict if len(sys.argv) > 5 else Loose
obj = pick()
a = obj.area()                               #@value a: exactly={1}
