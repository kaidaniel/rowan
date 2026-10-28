"""An instance of a class with `__slots__` on every class of its MRO (and no `"__dict__"` among
them) has no `__dict__`: `o.__dict__` raises AttributeError. Before, rowan gave it the instance
dict view, so a test `"v" in o.__dict__` in a `try` was taken to succeed (the class enumerator's
`__dict__` membership ops found it).

Expectations were written from the program text before running the engine."""


class Point:
    __slots__ = ("x", "y")

    def __init__(self, x):
        self.x = x
        self.y = 0


class Loose:
    def __init__(self, x):
        self.x = x


def own_x(o):
    try:
        r = o.x if "x" in o.__dict__ else -1
    except AttributeError:
        r = "attr"
    return r


p = own_x(Point(3))
#@value p: exactly={"attr"}
q = own_x(Loose(4))
#@value q: exactly={4}
