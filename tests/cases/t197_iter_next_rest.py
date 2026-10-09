"""Hypothesis: `next(it)` on an iterator over a known list consumes the first item, so the
rest of the iteration starts at the second.

Expectations were written from the program text before running the engine."""

it = iter([{"h": 1}, {"a": 1}, {"b": 1}])
head = next(it)                              #@row head: present={h} definite={h} closed
rest = list(it)
r0 = rest[0]                                 #@row r0: present={a} definite={a} closed
n = {len(rest): 1}                           #@row n: present={2} definite={2} closed
