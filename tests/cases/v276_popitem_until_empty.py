"""Hypothesis: `while d: k, v = d.popitem()` drains a known dict: every key is collected and
the dict ends empty.

Expectations were written from the program text before running the engine."""

d = {"a": 1, "b": 2}
seen = {}
while d:
    k, v = d.popitem()
    seen[k] = v
_ = 0                                        #@row seen: present={a,b} definite={a,b} closed
_ = 1                                        #@row d: present={} definite={} closed
