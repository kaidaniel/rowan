"""A method past its budget that binds its receiver to a local (`box = self`) uses it through
the local as through `self`: only the names it reads, stores or calls on it change what the
receiver holds. When the local is passed on, the receiver escapes: anything about it may
have changed. (Each method's second call is past the budget.)

Expectations were written from the program text before running the engine."""
#@limits budget=2
import os
import reprlib

peek = getattr(reprlib, os.environ.get("Z527_FN", "repr"))


class Box:
    def __init__(self):
        self.a = 1
        self.b = 2

    def pick(self, other):
        box = self
        if other is not None:
            box = other
        return box.a

    def leak(self):
        box = self
        peek(box)
        return 0


b = Box()
b.pick(None)
b.pick(None)
vb = b.b  #@ ok b
#@value vb: exactly={2}
c = Box()
c.leak()
c.leak()
vc = c.b  #@ likely b
