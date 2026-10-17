# Attributes stored and removed under ax: "w" with three or more arguments (argv[0] included),
# else an unknown call's result ("u" at run time; for rowan any text). Found by the fuzzer once
# its attrs area got such a name (tests/fuzz.py, ax); z664 is the dict side.
# (`types={bool}`: False or True, rowan's bool(?).)
import sys

n = len(sys.argv)
ax = "w" if n > 2 else (sys.argv[0][:0] + "U").lower()


class A:
    u = "class"

    def __init__(self):
        self.w = 0


class C:
    def __getattr__(self, name):
        return "dyn"


# delattr under a name not known: w may be gone (n <= 2: ax is "u", the instance has none)
a = A()
try:
    delattr(a, ax)
except AttributeError:
    pass
p1 = hasattr(a, "w")
#@value p1: types={bool}
p2 = "w" in vars(a)
#@value p2: types={bool}
# the instance's own value over the class's
b = A()
b.__dict__[ax] = 1
p3 = b.u
#@value p3: exactly={1, 'class'}
# `__getattr__` answers only what the instance lacks
c = C()
setattr(c, ax, 2)
p4 = c.u
#@value p4: exactly={2, 'dyn'}
# a store on one path, beside a store under a name not known
d = A()
d.__dict__[ax] = 3
if n == 2:
    d.u = "s"
p5 = d.u
#@value p5: exactly={3, 's', 'class'}
e = {}
e[ax] = 1
if n == 2:
    e["u"] = "s"
p6 = e.get("u", -1)
#@value p6: exactly={1, 's', -1}
