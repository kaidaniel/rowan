"""`getattr(obj, name)` by a name nothing is known of may be any attribute obj has: an
instance's own, a method of its class (bound to it: calling it runs the method), what its
`__getattr__` makes of the name, a library object's method (a list's `append`). Rowan read
only the instance's own attributes for one of several names (and the shared unknown for a
name known in part): the method's pop went unseen and the read after it was proven, though it
fails here.

Expectations were written from the program text before running the engine."""
import os


class Box:
    def __init__(self):
        self.items = {"k": 1}

    def clear(self):
        self.items.pop("k")
        return "cleared"


class Dyn:
    def __getattr__(self, n):
        return "dyn"


b = Box()
f = getattr(b, "clear" if os.getpid() > 0 else "items")
r = f()
#@value r: includes={"cleared"}
try:
    x = b.items["k"]                         #@ possibly k
except KeyError:
    x = 0

d = getattr(Dyn(), os.environ.get("Z622_B", "anything"))
#@value d: includes={"dyn"}

ops = []
getattr(ops, os.environ.get("Z622_C", "append"))("x")
c = Box()
getattr(c, os.environ.get("Z622_E", "clear"))()
try:
    y = c.items["k"]                         #@ possibly k
except KeyError:
    y = 0
n = len(ops)
#@value n: includes={1}

caught = False
try:
    getattr(b, os.environ.get("Z622_D", "nope"))
except AttributeError:
    caught = True
#@value caught: includes={True}
