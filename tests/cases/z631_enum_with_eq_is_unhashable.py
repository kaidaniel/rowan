"""An Enum class whose body defines `__eq__` without `__hash__` has unhashable members (CPython
sets the class's `__hash__` to None): as a dict key or a set item they raise TypeError. A plain
Enum's members hash. Rowan took every member as a hashable key.

Expectations were written from the program text before running the engine."""
from enum import Enum


class C(Enum):
    A = 1
    B = 2

    def __eq__(self, other):
        return self is other


class D(Enum):
    A = 1


try:
    d = {C.A: 1}
    r = "ok"
except TypeError:
    r = "te"
print(r)  #@value r: exactly={"te"}
try:
    s = {C.B}
    r2 = "ok"
except TypeError:
    r2 = "te"
print(r2)  #@value r2: exactly={"te"}
try:
    e = {D.A: 1}
    r3 = "ok"
except TypeError:
    r3 = "te"
print(r3)  #@value r3: exactly={"ok"}
print(e[D.A])  #@ ok D.A
