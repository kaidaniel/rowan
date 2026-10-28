"""An instance of a class whose MRO has `__slots__` everywhere has no `__dict__`; a subclass that
declares no `__slots__` has one. For any call of `use(o: Slotted)` the parameter may be either,
and `o.__dict__` or `o.extra = 1` may fail or not; on one of two receivers it fails for one only.
(Before, both were claimed to raise AttributeError for any call.)

Expectations were written from the program text before running the engine."""
import sys


class Slotted:
    __slots__ = ("a",)

    def __init__(self):
        self.a = 1


class Free(Slotted):
    pass


def use(o: Slotted):
    try:
        d = len(o.__dict__)
    except AttributeError:
        d = "attr"
    got = d
    #@general got: includes={'attr', 0}
    try:
        o.extra = 1
        e = "stored"
    except AttributeError:
        e = "attr"
    stored = e
    #@general stored: includes={'attr', 'stored'}


use(Slotted())
use(Free())
o = Slotted() if len(sys.argv) > 1 else Free()
try:
    n = len(o.__dict__)
except AttributeError:
    n = "attr"
either = n
#@value either: includes={'attr', 0}
