"""Hypothesis: a defaultdict read never fails, also when the key is one of several (a loop
over a list whose length depends on the path); the list's first items, there on every path,
run first, so their keys are sure; dict(sorted(d.items())) keeps d's keys (the sure ones
sure) in sorted order."""
import sys
from collections import defaultdict


class E:
    def __init__(self, region, amount):
        self.region = region
        self.amount = amount


def totals_by(events, attr):
    acc = defaultdict(float)
    for e in events:
        acc[getattr(e, attr)] += e.amount            #@ ok region
    return dict(sorted(acc.items()))


evs = [E("west", 1.0), E("north", 2.0)]
if len(sys.argv) > 2:
    evs.append(E("south", 3.0))
t = totals_by(evs, "region")
#@row t: present={north,south,west} definite={north,west} closed
first = list(t)[0]                                   #@value first: exactly={"north"}

counts = defaultdict(int)
for name in evs:
    counts[name.region] += 1                         #@ ok region
c = dict(counts)
#@row c: present={north,south,west} definite={north,west} closed
s = dict(sorted(c.items(), reverse=True))
#@row s: present={north,south,west} definite={north,west} closed
