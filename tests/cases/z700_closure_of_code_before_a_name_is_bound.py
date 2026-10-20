"""Soundness: what unknown code given a function may change is the closure of its code, and
that closure depends on the code the names it reads hold. Given `f` before `helper` is
defined, unknown code can only fail calling it; given `f` again after `def helper`, it may
run `helper` through `f` (CPython: `timeit` calls `f` once, `STATE["k"]` is 2). The closure
computed the first time must not stand for the second.

Expectations were written from the program text before running the engine."""
import timeit

STATE = {"k": 1}
LOG = []


def f():
    helper()


def g():
    later(LOG)


try:
    timeit.timeit(f, number=0)
except NameError:
    pass
k1 = STATE["k"]                              #@value k1: exactly={1}
timeit.timeit(g, number=0)
n1 = len(LOG)                                #@value n1: includes={0}
#@todo value n1: exactly={0}   (`later` is not bound yet: calling it raises before LOG is given)


def helper():
    STATE["k"] = 2


def later(xs):
    xs.append("ran")


timeit.timeit(f, number=1)
k2 = STATE["k"]                              #@value k2: includes={1,2}
timeit.timeit(g, number=1)
n2 = len(LOG)                                #@value n2: includes={0,1}
