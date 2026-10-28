"""An instance attribute set on some paths only, under the name of a method, a cached property
or a class attribute a base's cached property is shadowed by: on the other paths the read is
the class's, bound, run or as it is. (Before, the own value alone: `r` was claimed 2 only, and
for any call `second` 80 only once unknown code may have deleted `B.v`.)

Expectations were written from the program text before running the engine."""
import functools
import sys


class A:
    def m(self):
        return 1

    @functools.cached_property
    def v(self):
        return 80


class B(A):
    v = 2


o = A()
if len(sys.argv) > 1:
    o.m = lambda: 2
r = o.m()
#@value r: exactly={1, 2}


def run(f):
    f()


def drop(c):
    del c.v


def read(o: A):
    o.v
    second = o.v
    #@general second: includes={2, 80}
    return second


run(lambda: read(B()))
try:
    drop(B)
except AttributeError:
    pass
