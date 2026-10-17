"""Hypothesis: code rowan does not follow (past the budget) that changes an instance may set any
attribute on it, also one whose class holds an object (a tuple, a list) under that name: a later
read may give what it set, not only the class's object (fuzz attrs_3511: `getattr(b, "u", -1)`
read only the class's `(3,)`). A method stays the class's (rebinding a function is not
modelled), so calling it still runs the method.

Expectations were written from the program text before running the engine."""
#@limits budget=0


class A:
    u = (3,)
    tags = ["t"]
    k = "A"

    def __init__(self, v):
        self.v = v

    def put(self, name, v):
        setattr(self, name, v)

    def m(self):
        return 7


a = A(1)
a.put("x", 0)
b = A(2)
b.put("u", 0)
u = getattr(b, "u", -1)                      #@value u: includes={(3,), 0}
c = A(3)
c.put("tags", None)
t = c.tags                                   #@value t: includes={None}
k = c.k                                      #@value k: includes={"A"}
r = c.m()                                    #@value r: exactly={7}
