# Code past the budget that sets an attribute by a computed name (`setattr(self, name, v)`)
# may set any: a later read of an attribute set on some paths, or one the class has, may give
# what it set.
#@limits budget=0
import sys


class A:
    k = "A"

    def __init__(self, v):
        self.v = v

    def put(self, name, v):
        setattr(self, name, v)


n = len(sys.argv)
a = A(1)
a.put("u", 0)
if n > 2:
    a.u = None
u = getattr(a, "u", -1)                      #@value u: includes={0, None}
b = A(1)
b.put("k", "s")
k = b.k                                      #@value k: includes={"s"}
