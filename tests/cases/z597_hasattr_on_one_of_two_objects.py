# `hasattr(b, "w")` where b is an instance without w or one whose class answers every name
# (`__getattr__`): the test holds for the latter, so its branch runs. (The read on the
# instance without w fails there: rowan goes on with a value not known.)
import sys


class A:
    def __init__(self, v):
        self.v = v


class C:
    def __getattr__(self, name):
        return "dyn"


n = len(sys.argv)
r = 0
if n == 1:
    b = A(0)
else:
    b = C()
if hasattr(b, "w"):
    r = b.w
r1 = r                                       #@value r1: includes={0, "dyn"}
