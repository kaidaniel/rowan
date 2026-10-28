"""Soundness of what an attribute holds for any call (z914): where the program may store
attributes by names or ways its text does not say (`setattr` given as a value, `vars(o)`,
`__dict__`), or a store's value is not said, the attribute may hold anything; `+= 1` keeps an
int an int only where every value it applies to is one (`0.5 + 1` is a float).

Expectations were written from the program text before running the engine."""
import functools


class Box:
    def __init__(self):
        self.v = 0
        self.ratio = 0.5

    def bump(self):
        self.v = 1
        self.ratio += 1

    def show(self):
        x = self.v
        #@general x: untyped
        #@todo general x: shows={0, 1} untyped  (the unknown such a store leaves is an object, which names no literals beside it)
        r = self.ratio
        #@general r: shows={0.5} untyped
        return x


def later(b: Box):
    f = functools.partial(setattr, b, "v")
    f("text")


b = Box()
b.bump()
b.show()
later(b)
b.show()
