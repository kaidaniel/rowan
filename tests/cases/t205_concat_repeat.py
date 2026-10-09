"""Hypothesis: `a + b`, `a * 3` and `a += b` on lists with known positions give known
positions.

Expectations were written from the program text before running the engine."""

a = [{"x": 1}]
b = [{"y": 1}, {"z": 1}]
c = a + b
c2 = c[2]                                    #@row c2: present={z} definite={z} closed
d = a * 3
n = {len(d): 1}                              #@row n: present={3} definite={3} closed
d1 = d[1]                                    #@row d1: present={x} definite={x} closed
a += b
a1 = a[1]                                    #@row a1: present={y} definite={y} closed
