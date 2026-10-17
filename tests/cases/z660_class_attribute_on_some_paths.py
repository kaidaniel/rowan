# A class attribute stored on some paths only (with, on another, the instance's): reads that
# may fail. Companion of z658. python z660.py → p1 3, p3 0, p4 3; z660.py x → p1 0, p3 0, no p4;
# z660.py x y → p1 1, p3 1, p4 1.
import sys


class A:
    pass


class B:
    pass


n = len(sys.argv)
a = A()
b = B()
r = 0
if n > 2:
    A.u = 1
    B.u = 1
elif n == 1:
    a.u = 3
p1 = getattr(a, "u", 0)
#@value p1: exactly={0, 1, 3}
p2 = hasattr(a, "u")
#@value p2: types={bool}
# (hasattr where the class or the instance may have it: undecided)
try:
    q = b.u
except AttributeError:
    q = 0
p3 = q
#@value p3: exactly={0, 1}
if hasattr(a, "u"):
    p4 = a.u
    #@value p4: exactly={1, 3}
