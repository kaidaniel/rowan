"""`o.__dict__.get(name, default)` on an instance unknown code may have given attributes (here an
`__init__` that does not run, at `budget=0`, which stores through `self.__dict__`): the entry may
be there, so the result is the default or anything. (Before, the default only: `-1`, where
Python gives 3.)

Expectations were written from the program text before running the engine."""
#@limits budget=0


class A:
    def __init__(self):
        self.__dict__["v"] = 3


def make():
    o = A()
    got = o.__dict__.get("v", -1)
    #@general got: includes={-1} unknown
    return got


make()
