"""A subclass without `__slots__` of a class with `__slots__ = ("v",)` has a `__dict__`, but the
slot member `v` (a data descriptor) takes reads and stores of `o.v`: an entry put in `__dict__`
under that name is read back only through `__dict__`, and the slot's value is no entry of the
dict. (Before, `o.__dict__["v"] = 7` made `o.v` 7: Python raises AttributeError, the slot is
empty; and the slot's value was read back through `__dict__`.)

Expectations were written from the program text before running the engine."""


class Slotted:
    __slots__ = ("v",)


class Free(Slotted):
    pass


o = Free()
o.__dict__["v"] = 7
try:
    got = o.v
except AttributeError:
    got = "attr"
#@value got: includes={'attr'}
entry = o.__dict__.get("v", -1)
#@value entry: includes={7}

p = Free()
p.v = 5
slot = p.v
#@value slot: exactly={5}
seen = p.__dict__.get("v", -1)
#@value seen: includes={-1}
try:
    item = p.__dict__["v"]
except KeyError:
    item = "key"
#@value item: includes={'key'}
