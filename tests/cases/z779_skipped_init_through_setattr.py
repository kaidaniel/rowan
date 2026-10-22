"""A class whose `__setattr__` takes every store (decimal's `Context`, which also has a
`__delattr__` that only raises): a call of its `__init__` that does not run (budget 0)
recorded none of the attributes `self.x = …` stores, so reading one was "likely never set",
a false warning on every run. The stores go to `__setattr__` with their names: likely there."""
#@limits budget=0


class Ctx:
    def __init__(self, prec=None):
        self.prec = prec if prec is not None else 28
        self.flags = {}

    def __setattr__(self, name, value):
        if name == "prec" and not isinstance(value, int):
            raise TypeError("prec must be an integer")
        return object.__setattr__(self, name, value)

    def __delattr__(self, name):
        raise AttributeError("%s cannot be deleted" % name)


Ctx(1)
c = Ctx(2)
p = c.prec
#@ likely prec
f = c.flags
#@ likely flags
