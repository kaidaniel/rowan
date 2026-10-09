"""Hypothesis: `defaultdict(lambda: defaultdict(int))` filled in a known loop has per-user inner
dicts that are distinct objects with exactly their items.

Expectations were written from the program text before running the engine."""

from collections import defaultdict

counts = defaultdict(lambda: defaultdict(int))
for user, item in [("ann", "x"), ("ann", "y"), ("bob", "x")]:
    counts[user][item] += 1
ann = dict(counts["ann"])                    #@row ann: present={x,y} definite={x,y} closed
bob = dict(counts["bob"])                    #@row bob: present={x} definite={x} closed
