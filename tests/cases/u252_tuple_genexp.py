"""Hypothesis: `tuple(genexp)` over `range(3)` keeps positions and length.

Expectations were written from the program text before running the engine."""

t = tuple({"i": i} for i in range(3))
t2 = t[2]
k = {t2["i"]: 1}                             #@row k: present={2} definite={2} closed
n = {len(t): 1}                              #@row n: present={3} definite={3} closed
