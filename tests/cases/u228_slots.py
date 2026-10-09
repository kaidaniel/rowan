"""Hypothesis: a class with `__slots__` has the attributes its `__init__` sets; a declared
slot that was never assigned is missing (AttributeError).

Expectations were written from the program text before running the engine."""


class P:
    __slots__ = ("x", "y")

    def __init__(self, x):
        self.x = x


p = P(1)
p.x                                          #@ ok x
p.y                                          #@ missing y
