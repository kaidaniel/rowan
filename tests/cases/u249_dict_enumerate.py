"""Hypothesis: `dict(enumerate(xs))` has the indices as keys and the items as values.

Expectations were written from the program text before running the engine."""

names = ["a", "b"]
idx = dict(enumerate(names))                 #@row idx: present={0,1} definite={0,1} closed
inv = {v: k for k, v in idx.items()}         #@row inv: present={a,b} definite={a,b} closed
