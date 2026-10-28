"""A descriptor read through its class runs `__get__(None, cls)`, also through `type(o)`, and
on one of two classes per class; `hasattr(o, name)` runs the descriptor's (or a property's)
getter. (Before, `A.v` was the descriptor object itself, `__get__` ran on neither path, and
`hasattr` ran no getter: the counter stayed 0.)

Expectations were written from the program text before running the engine."""
import sys

RUNS = 0


class Counted:
    def __get__(self, obj, owner=None):
        global RUNS
        RUNS = RUNS + 1
        if obj is None:
            return 40
        return 41


class A:
    v = Counted()


class B(A):
    pass


got = A.v
#@value got: exactly={40}
runs = RUNS
#@value runs: exactly={1}
o = A() if len(sys.argv) > 1 else B()
through = type(o).v
#@value through: exactly={40}
runs2 = RUNS
#@value runs2: exactly={2}
has = hasattr(o, "v")
runs3 = RUNS
#@value runs3: exactly={3}
