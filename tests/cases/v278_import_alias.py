"""Hypothesis: `from collections import defaultdict as dd` is the defaultdict model under
another name.

Expectations were written from the program text before running the engine."""

from collections import defaultdict as dd

g = dd(list)
g["a"].append(1)
x = dict(g)                                  #@row x: present={a} definite={a} closed
g["zz"]                                      #@ ok zz
