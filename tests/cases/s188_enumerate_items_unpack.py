"""Hypothesis: `for i, (k, rec) in enumerate(d.items())` unpacks index, key and value with
known values.

Expectations were written from the program text before running the engine."""

d = {"a": {"v": 1}, "b": {"v": 2}}
out = {}
for i, (k, rec) in enumerate(d.items()):
    out[f"{i}:{k}"] = rec["v"]               #@ ok v
_ = 0                                        #@row out: present={0:a,1:b} definite={0:a,1:b} closed
