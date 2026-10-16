# A class attribute that is a tuple in the base class and a number stored in the subclass in a
# loop (which may not run, as far as rowan knows): either, read from the class or an instance.
# A guarded read on an instance of either class fails only where the class lacks it.
import sys


class A:
    def __init__(self, v):
        self.v = v


class B(A):
    pass


n = len(sys.argv)
A.u = (3,)
for _i in range(n):
    B.u = 1
b = B("s")
p = getattr(b, "u", -1)                      #@value p: includes={1}
q = B.u                                      #@value q: includes={1}
C = type("C", (), {})
a = B("s")
if n == 1:
    a = C()
try:
    r = a.u                                  #@ possibly u
except AttributeError:
    r = -1
r1 = r                                       #@value r1: includes={1, -1}
