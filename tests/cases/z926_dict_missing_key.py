"""`obj.__dict__[k]` for a name the object lacks raises KeyError, not AttributeError (the
`__dict__` is a dict); a module's too.

Expectations were written from the program text before running the engine."""
import os


class A:
    pass


a = A()
a.x = 1
try:
    v = a.__dict__["nope"]
except KeyError:
    v = "KE"
except AttributeError:
    v = "AE"
vv = v                                       #@value vv: exactly={"KE"}
x = a.__dict__["x"]                          #@value x: exactly={1}

try:
    w = globals()["nope"]
except KeyError:
    w = "KE"
except AttributeError:
    w = "AE"
vw = w                                       #@value vw: exactly={"KE"}
