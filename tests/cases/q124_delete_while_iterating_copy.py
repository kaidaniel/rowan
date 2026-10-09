"""Hypothesis: deleting keys from a dict while iterating over `list(d)` (a snapshot) removes
exactly the keys the predicate selects, leaving the rest definite.

Expectations were written from the program text before running the engine."""

d = {"a": 1, "b": 2, "_tmp": 3, "_x": 4}
for k in list(d):
    if k.startswith("_"):
        del d[k]
_ = 0                                     #@row d: present={a,b} definite={a,b} closed
d["_tmp"]                                 #@ missing _tmp
d["a"]                                    #@ ok a
