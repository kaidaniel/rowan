"""Hypothesis: `copy.deepcopy` keeps sharing inside the copy (one inner object reached twice is
copied once) and leaves the original alone.

Expectations were written from the program text before running the engine."""

import copy

inner = {"v": 1}
outer = {"a": inner, "b": inner}
c = copy.deepcopy(outer)
c["a"]["new"] = True
cb = c["b"]                                  #@row cb: present={v,new} definite={v,new} closed
_ = 0                                        #@row inner: present={v} definite={v} closed
