"""A data descriptor (its class defines `__set__` or `__delete__`) takes reads over the
instance's own `__dict__` entry of the name; a non-data descriptor (only `__get__`) does not.
An entry put there through `__dict__` (bypassing `__set__`) is read back through `__dict__`
only. (Before, the `__dict__` entry won: `o.v` was claimed 3, Python gives 11.)

Expectations were written from the program text before running the engine."""


class Data:
    def __get__(self, obj, owner=None):
        if obj is None:
            return self
        return obj.__dict__.get("_v", 1) + 10

    def __set__(self, obj, value):
        obj.__dict__["_v"] = value * 2


class NonData:
    def __get__(self, obj, owner=None):
        return 51


class A:
    v = Data()
    w = NonData()

    def __init__(self):
        self.__dict__["v"] = 3
        self.__dict__["w"] = 4


o = A()
v = o.v
#@value v: exactly={11}
raw = o.__dict__["v"]
#@value raw: exactly={3}
w = o.w
#@value w: exactly={4}
o.v = 5
v2 = o.v
#@value v2: exactly={20}
