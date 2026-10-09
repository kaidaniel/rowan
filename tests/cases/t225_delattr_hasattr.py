"""Hypothesis: `delattr(obj, "a")` removes the attribute and `hasattr` on a known instance is
decided.

Expectations were written from the program text before running the engine."""


class B:
    def __init__(self):
        self.a = 1
        self.b = 2


b = B()
delattr(b, "a")
b.a                                          #@ missing a
out = {}
if hasattr(b, "a"):
    out["has_a"] = True
if hasattr(b, "b"):
    out["has_b"] = True
_ = 0                                        #@row out: present={has_b} definite={has_b} closed
