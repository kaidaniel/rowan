"""Hypothesis: slices with a step (`[::-1]`, `[::2]`) on a known list keep positions.

Expectations were written from the program text before running the engine."""

xs = [{"a": 1}, {"b": 1}, {"c": 1}]
rev = xs[::-1]
r0 = rev[0]                                  #@row r0: present={c} definite={c} closed
every = xs[::2]
e1 = every[1]                                #@row e1: present={c} definite={c} closed
