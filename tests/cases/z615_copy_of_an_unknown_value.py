"""Hypothesis: `.copy()` of a value rowan knows nothing of (an unannotated parameter) is not a
dict's copy: the parameter may be a DataFrame, a list, a set, any object with a `copy` method.
The copy is as unknown as the value (`?`, not `{**?}`), what is read from it is still needed
of the parameter, and a method a dict lacks (`assign`, `append`) is the unknown value's own
method, not `dict.assign`. The same holds for `copy.copy` of an unknown value.

Expectations were written from the program text before running the engine."""
import copy


def add_revenue(m):
    m = m.copy()
    #@value m: unknown
    m["revenue"] = m["gross"] * 2                #@ input gross
    m = m.assign(cost=1)
    return m
#@schema param add_revenue.m: always={gross} only={}


def grow(xs):
    ys = copy.copy(xs)
    #@value ys: unknown
    ys.append(0)
    return ys
