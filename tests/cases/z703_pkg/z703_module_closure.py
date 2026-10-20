"""Soundness: unknown code given a module may run any function the module holds. Given `m`
before the program stores `hook` on it, it can only run `m.run`; given `m` again after
`m.hook = bump`, it may run `bump` (CPython: the second `timeit` runs `m.hook()`, `STATE["k"]`
is 2). What the code a module holds may change, computed the first time, must not stand for
the second (the same as z700, for a module used as a value).

Expectations were written from the program text before running the engine."""
import timeit

import modval as m

STATE = {"k": 1}


def bump():
    STATE["k"] = 2


timeit.timeit("m.run()", globals={"m": m}, number=1)
k1 = STATE["k"]                              #@value k1: exactly={1}
m.hook = bump
timeit.timeit("m.hook()", globals={"m": m}, number=1)
k2 = STATE["k"]                              #@value k2: includes={1,2}
