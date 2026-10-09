"""Hypothesis: `super().__init__(role=..., **kw)` forwards the subclass's extra keywords, so
the base's `dict(kw)` has exactly the forwarded keys per construction.

Expectations were written from the program text before running the engine."""


class A:
    def __init__(self, **kw):
        self.opts = dict(kw)


class B(A):
    def __init__(self, name, **kw):
        super().__init__(role="b", **kw)
        self.name = name


b = B("n", color="red")
o = b.opts                                   #@row o: present={role,color} definite={role,color} closed
b2 = B("m")
o2 = b2.opts                                 #@row o2: present={role} definite={role} closed
o2["color"]                                  #@ missing color
