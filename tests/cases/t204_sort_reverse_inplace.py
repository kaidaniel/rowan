"""Hypothesis: `list.sort(key=...)` with known literal keys and `list.reverse()` reorder the
positions in place.

Expectations were written from the program text before running the engine."""

items = [{"n": 3, "c": 1}, {"n": 1, "a": 1}, {"n": 2, "b": 1}]
items.sort(key=lambda d: d["n"])
lo = items[0]                                #@row lo: present={n,a} definite={n,a} closed
items.reverse()
hi = items[0]                                #@row hi: present={n,c} definite={n,c} closed
