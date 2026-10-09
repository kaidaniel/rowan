"""Hypothesis: `zip(*pairs)` transposes known pairs into known columns.

Expectations were written from the program text before running the engine."""

pairs = [("a", 1), ("b", 2)]
names, nums = zip(*pairs)
d = dict.fromkeys(names)                     #@row d: present={a,b} definite={a,b} closed
idx = {n: i for i, n in enumerate(nums)}     #@row idx: present={1,2} definite={1,2} closed
