# A function's attributes set other than by `f.x = …`: through its `__dict__` (update, a
# store under a key), through `vars(f)`, by `setattr` under a name not known, and by code rowan
# cannot see that was given the function. Reading one must not surely fail.
import pickle
import sys


def f():
    return 1


def g():
    return 2


def h():
    return 3


def k():
    return 4


def m():
    return 5


f.__dict__.update(x=1)
fx = f.x
#@value fx: exactly={1}
vars(g)["y"] = 2
gy = g.y
#@value gy: exactly={2}
setattr(h, sys.argv[0][:0] + "z", 3)
hz = h.z
#@value hz: includes={3}
k.__dict__["w"] = 4
kw = k.w
#@value kw: exactly={4}
fn = pickle.loads(pickle.dumps(setattr))
fn(m, "flag", True)
fl = m.flag
#@value fl: includes={True}
has = hasattr(m, "flag")
#@value has: includes={True}
done = True
#@value done: exactly={True}
