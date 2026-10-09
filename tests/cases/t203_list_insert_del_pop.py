"""Hypothesis: `insert(0, x)`, `del xs[0]` and `pop()` on a list with known positions keep
the positions exact.

Expectations were written from the program text before running the engine."""

rows = [{"a": 1}, {"b": 1}]
rows.insert(0, {"z": 1})
r0 = rows[0]                                 #@row r0: present={z} definite={z} closed
r1 = rows[1]                                 #@row r1: present={a} definite={a} closed
del rows[0]
f = rows[0]                                  #@row f: present={a} definite={a} closed
p = rows.pop()                               #@row p: present={b} definite={b} closed
n = {len(rows): 1}                           #@row n: present={1} definite={1} closed
