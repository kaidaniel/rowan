"""Hypothesis: a dict iterates in insertion order: the first key is the first inserted, a key
deleted and re-inserted moves to the end, and re-assigning an existing key keeps its place.

Expectations were written from the program text before running the engine."""

d = {"a": 1, "b": 2, "c": 3}
first = next(iter(d))
out = {first: True}                          #@row out: present={a} definite={a} closed
del d["a"]
d["a"] = 9
last = list(d)[-1]
out2 = {last: True}                          #@row out2: present={a} definite={a} closed
d["b"] = 20
k0 = list(d)[0]
out3 = {k0: 1}                               #@row out3: present={b} definite={b} closed
