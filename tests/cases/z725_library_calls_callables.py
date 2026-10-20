# A library function rowan takes for one that changes nothing it is given (`operator.call`,
# `os.walk`'s `onerror`, a thread's target) may call what it is given: a dict's or list's bound
# method changes its receiver, a library function like `setattr` the object it is given, a
# class of the program's runs its `__init__`, an instance with `__call__` runs it.
import operator

d = {}
operator.call(d.setdefault, "s", [])
v = len(d)
#@value v: includes={1}
#@row d: possible={s}
G = {"a": 0}
operator.call(G.update, {"x": 2})
w = G.get("x", -1)
#@value w: includes={2}
L = []
operator.call(L.append, 1)
n = len(L)
#@value n: includes={1}


class Box:
    made = 0

    def __init__(self):
        Box.made += 1
        self.v = 0

    def __call__(self):
        self.v = 7


b = Box()
operator.call(setattr, b, "v", 5)
bv = b.v
#@value bv: includes={5}
operator.call(Box)
m = Box.made
#@value m: includes={2}
operator.call(b)
bv2 = b.v
#@value bv2: includes={7}
