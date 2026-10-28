"""`o.v = 5` where the class's `v` is a property without a setter raises AttributeError; where
the run for any call no longer knows the class attribute for certain (unknown code may have
run after the module's end), the store may raise. (Before, a class attribute that was the
property on some paths only let the store go on without raising, so `except AttributeError:`
was unreachable for any call.)

Expectations were written from the program text before running the engine."""
import sys


class Meter:
    def __init__(self):
        self.a = 1

    @property
    def v(self):
        return self.a + 100


class Fixed(Meter):
    v = 2


def set_v(o: Meter):
    try:
        o.v = 5
        r = o.v
    except AttributeError:
        r = "attr"
    seen = r
    #@general seen: includes={"attr", 5}
    return r


def run(f):
    try:
        f()
    except BaseException:
        pass


run(lambda: set_v(Meter()))
run(lambda: set_v(Fixed() if len(sys.argv) > 2 else Meter()))
