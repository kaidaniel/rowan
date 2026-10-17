"""`"…%s…" % obj` runs obj's `__str__` (`%r`: `__repr__`), as `str(obj)` and `f"{obj}"` do: the
text is what it returns, and what it changes changes. It ran no code: `s` was an unknown str and
`SEEN` was claimed empty. A `%d` of an instance runs `__index__`; a format whose text is not
known may run any of them.

Expectations were written from the program text before running the engine."""
import os

SEEN = []


class Item:
    def __init__(self, name):
        self.name = name

    def __str__(self):
        SEEN.append("str")
        return "item " + self.name

    def __repr__(self):
        SEEN.append("repr")
        return "Item(" + repr(self.name) + ")"

    def __index__(self):
        SEEN.append("index")
        return 7


it = Item("a")
s = "got %s" % it
#@value s: exactly={"got item a"}
n1 = len(SEEN)
#@value n1: exactly={1}
r = "%r and %s" % (it, "x")
#@value r: exactly={"Item('a') and x"}
n2 = len(SEEN)
#@value n2: exactly={2}
d = "%d" % it
#@value d: types={str}
n3 = len(SEEN)
# (3: __index__ ran)
#@value n3: includes={3}
fmt = os.environ.get("Z650_FMT", "<%s>")
u = fmt % it
n4 = len(SEEN)
# (4: __str__ ran for the default format)
#@value n4: includes={4}
