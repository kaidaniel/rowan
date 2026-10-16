# An attribute set on some paths only, read in a `try` that handles AttributeError: where it
# is not set, the handler runs and the name keeps its value.
import sys


class A:
    def __init__(self, v):
        self.v = v


n = len(sys.argv)
b = A(1)
q = 0
if n > 2:
    b.w = "s"
try:
    q = b.w                                  #@ possibly w
except AttributeError:
    pass
q1 = q                                       #@value q1: exactly={"s", 0}
