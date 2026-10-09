"""Hypothesis: `dict(sorted(d.items()))` rebuilds the dict in sorted key order, and sorting by
a value key orders by it.

Expectations were written from the program text before running the engine."""

d = {"b": 2, "a": 1, "c": 3}
s = dict(sorted(d.items()))
first = next(iter(s))
f = {first: 1}                               #@row f: present={a} definite={a} closed
byval = dict(sorted(d.items(), key=lambda kv: -kv[1]))
top = list(byval)[0]
t = {top: 1}                                 #@row t: present={c} definite={c} closed
