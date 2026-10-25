"""`super(Mid, self)` looks past Mid in self's class's order, not past the class whose code
runs: here it calls Base's `__init__`, skipping Mid's (which would want two arguments).
(Before, the class argument was ignored: Mid's `__init__` ran, and once arguments that do not
fit raised TypeError, so did the construction.)

Expectations were written from the program text before running the engine."""


class Base:
    def __init__(self):
        self.base = 1


class Mid(Base):
    def __init__(self, a, b):
        self.a = a
        super().__init__()


class Leaf(Mid):
    def __init__(self, a, b):
        self.b = b
        super(Mid, self).__init__()


x = Leaf(1, 2)
v = x.base                                       #@ ok base
#@value v: exactly={1}
has_a = hasattr(x, "a")
#@value has_a: exactly={False}
