"""Hypothesis: `OrderedDict.move_to_end` and `popitem(last=False)` follow insertion order.

Expectations were written from the program text before running the engine."""

from collections import OrderedDict

od = OrderedDict()
od["a"] = 1
od["b"] = 2
od["c"] = 3
od.move_to_end("a")
first = next(iter(od))
f = {first: 1}                               #@row f: present={b} definite={b} closed
k, v = od.popitem(last=False)
g = {k: 1}                                   #@row g: present={b} definite={b} closed
_ = 0                                        #@row od: present={c,a} definite={c,a} closed
