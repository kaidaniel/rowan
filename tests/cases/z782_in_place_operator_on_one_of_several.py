# `t op= v` changes t's object in place (a list, a dict, a set, an instance whose class
# defines `__iop__`), also where t may hold an object of another kind on other paths, and so
# does `operator.iadd` & co.; code rowan cannot see may run the program's in-place and item
# dunders. (Found by the fuzzer: `d = {"m": None} if c else e; d |= {...}` beside a skipped
# call's result left `e` unchanged.)
import marshal
import operator
import sys

n = len(sys.argv)
raw = sys.argv[-1].encode()

# marshal.loads may give a dict, a list, a set or anything else: on its path e is untouched
# (`includes={True, False} types={bool}`: a bool either way, `bool(?)` or `True | False`)
e = {}
d = marshal.loads(raw) if n > 5 else e
d |= {"m": None}
p1 = "m" in e
#@value p1: includes={True, False} types={bool}

xs = []
ys = marshal.loads(raw) if n > 5 else xs
ys += [1]
p2 = len(xs)
#@value p2: exactly={0, 1}

s = set()
t = marshal.loads(raw) if n > 5 else s
t |= {2}
p3 = 2 in s
#@value p3: includes={True, False} types={bool}


class Acc:
    def __init__(self):
        self.items = []

    def __iadd__(self, x):
        self.items.append(x)
        return self

    def __add__(self, x):
        return "added"


# `a += 1` calls `__iadd__`, which changes the object b holds too; `__add__` does not run
a = Acc()
b = a
a += 1
p4 = len(b.items)
#@value p4: exactly={1}
p5 = a is b
#@value p5: exactly={True}

# operator's in-place functions are `+=` and `|=`
ws = []
vs = ws
operator.iadd(vs, [3])
p6 = len(ws)
#@value p6: exactly={1}
f = {}
g = f
r7 = operator.ior(g, {"k": 1})
p7 = "k" in f
#@value p7: exactly={True}
p8 = r7 is f
#@value p8: exactly={True}
c = Acc()
operator.iadd(c, 5)
p9 = len(c.items)
#@value p9: exactly={1}

# a callable not known (on the oracle's run, `print`) may be `operator.iadd` or
# `operator.setitem`: it may run Box's `__iadd__` / `__setitem__`, which change LOG
LOG = []


class Box:
    def __setitem__(self, k, v):
        LOG.append(k)

    def __iadd__(self, v):
        LOG.append(v)
        return self


hook = getattr(operator, raw.decode()) if n > 5 else print
box = Box()
hook(box, 7)
p10 = len(LOG)
#@value p10: includes={0, 1} types={int}
