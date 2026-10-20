"""Soundness: a pure test's branch rows come back when the test recurs only while nothing
changed them since the join (`CondFact`). Unknown code given the same values twice (`G`) may
remove `d["x"]` the second time; the rows the first `if flag:` left must not come back in the
second (CPython: the second `timeit` pops "x", `has` is False).

Expectations were written from the program text before running the engine."""
import os
import timeit

d = {"x": 1}
G = {"h": [d]}
flag = os.environ.get("ROWAN_Z704_UNSET") is None
timeit.timeit("pass", globals=G, number=1)
if flag:
    assert "x" in d
timeit.timeit("h[0].pop('x')", globals=G, number=1)
if flag:
    has = "x" in d                           #@value has: includes={False}
