# A read on a value that is one of two instances, one of whose class answers every name
# through `__getattr__`: it fails only where the value is the other one.
import sys


class B:
    def __init__(self):
        self.v = 1


class C:
    def __getattr__(self, name):
        return "dyn"


n = len(sys.argv)
b = B()
if n == 1:
    b = C()
try:
    r = b.u                                  #@ possibly u
except AttributeError:
    r = "ae"
r1 = r                                       #@value r1: exactly={"dyn", "ae"}
