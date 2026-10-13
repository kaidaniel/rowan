"""Hypothesis: `f.__get__(obj)` of a library function is f bound to obj, so calling it
calls f with obj first: attrs sets the attributes of its slotted, frozen classes with
`object.__setattr__.__get__(self)(name, value)`. rowan took the call for one without
effects on self: its attributes were "never set" (false ✗ in attrs).

Expectations were written from the program text before running the engine."""

_SET = object.__setattr__


class Frozen:
    __slots__ = ("name", "size")

    def __init__(self, name):
        bound = _SET.__get__(self)
        bound("name", name)
        _SET(self, "size", 1)

    def __setattr__(self, key, value):
        raise AttributeError(key)


f = Frozen("a")
n = f.name                                   #@value n: exactly={a}
s = f.size                                   #@value s: exactly={1}
