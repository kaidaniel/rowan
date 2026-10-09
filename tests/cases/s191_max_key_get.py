"""Hypothesis: `max(d, key=d.get)` / `min(...)` on known values pick a known key.

Expectations were written from the program text before running the engine."""

d = {"a": 3, "b": 7, "c": 1}
k = max(d, key=d.get)
w = {k: 1}                                   #@row w: present={b} definite={b} closed
m = min(d, key=lambda x: d[x])
v = {m: 1}                                   #@row v: present={c} definite={c} closed
