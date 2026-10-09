"""Hypothesis: a nested dict comprehension makes one inner dict per outer key (distinct
objects) with the inner keys.

Expectations were written from the program text before running the engine."""

A = ["x", "y"]
B = ["p", "q"]
grid = {a: {b: 0 for b in B} for a in A}     #@row grid: present={x,y} definite={x,y} closed
gx = grid["x"]                               #@row gx: present={p,q} definite={p,q} closed
gx["new"] = 1
gy = grid["y"]                               #@row gy: present={p,q} definite={p,q} closed
