"""Hypothesis: `functools.reduce` over known items runs the function item by item, so
merging dicts with `{**acc, **p}` gives the union and summing known numbers a known total.

Expectations were written from the program text before running the engine."""

from functools import reduce

parts = [{"a": 1}, {"b": 2}, {"c": 3}]
merged = reduce(lambda acc, p: {**acc, **p}, parts, {})   #@row merged: present={a,b,c} definite={a,b,c} closed
merged["d"]                                  #@ missing d
total = reduce(lambda x, y: x + y, [1, 2, 3])
k = {total: 1}                               #@row k: present={6} definite={6} closed
