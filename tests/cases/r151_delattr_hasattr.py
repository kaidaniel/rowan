"""Hypothesis: `del obj.attr` removes the attribute, `hasattr` narrows, `getattr` with a
default is safe, `getattr` without a default and `setattr` with a literal name act like
plain attribute access and store.

Expectations were written from the program text before running the engine."""


class Box:
    def __init__(self):
        self.a = 1
        self.b = 2


box = Box()
del box.a
box.a                                        #@ missing a
if hasattr(box, "b"):
    box.b                                    #@ ok b
v = getattr(box, "a", None)                  #@ none a
w = getattr(box, "b")                        #@ ok b
setattr(box, "c", {"k": 1})
c = box.c                                    #@row c: present={k} definite={k} closed
_ = 0                                        #@row box: present={b,c} definite={b,c} closed
