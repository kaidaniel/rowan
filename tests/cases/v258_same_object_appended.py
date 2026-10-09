"""Hypothesis: appending one dict an unknown number of times makes every element that dict, so
a store through an element is a store on it.

Expectations were written from the program text before running the engine."""

base = {"k": 1}
items = []
for _ in range(n_in):
    items.append(base)
first = items[0]
first["new"] = 1
_ = 0                                        #@row base: present={k,new} definite={k,new} closed
