"""A class with its own `__new__`: `Cls(args)` calls `__new__(Cls, args)`, and `__init__` on its
result when that is an instance of `Cls`; the instance is what `__new__` returns.
`object.__new__(cls)` (also as `super().__new__(cls)`) is a fresh instance of `cls` without
attributes. `__new__` was ignored: the instance was a fresh one with no attributes, so
everything `__new__` set read as definitely missing (`_pydecimal.Decimal` builds itself in
`__new__`: hundreds of false `self._is_special` / `self._exp` verdicts on stdlib12k)."""


class Dec:
    __slots__ = ("_int", "_is_special")

    def __new__(cls, value="0"):
        self = object.__new__(cls)
        if isinstance(value, int):
            self._int = str(value)
            self._is_special = False
            return self
        if isinstance(value, Dec):
            self._int = value._int
            self._is_special = value._is_special
            return self
        raise TypeError("cannot convert")

    def special(self):
        return self._is_special  #@ ok _is_special


class Point:
    def __new__(cls, *args):
        inst = super().__new__(cls)
        inst.made = True
        return inst

    def __init__(self, x):
        self.x = x


class Shared:
    _one = None

    def __new__(cls):
        if cls._one is None:
            cls._one = object.__new__(cls)
            cls._one.count = 0
        return cls._one


d = Dec(3)
i = d._int  #@value i: exactly={"3"}
e = Dec(d)
s = e.special()  #@value s: exactly={False}
p = Point(4)  #@row p: present={made,x} definite={made,x} closed
s1 = Shared()
n = s1.count  #@ ok count
print(i, s, p.x, n)
