"""Hypothesis: `d.keys()` is a live view: a key added after taking the view is in it, and
its length follows the dict.

Expectations were written from the program text before running the engine."""

d = {"a": 1}
ks = d.keys()
d["b"] = 2
flag = "b" in ks
out = {"yes": 1} if flag else {"no": 1}      #@row out: present={yes} definite={yes} closed
n = len(ks)
o2 = {n: 1}                                  #@row o2: present={2} definite={2} closed
