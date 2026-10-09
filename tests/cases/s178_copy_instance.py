"""Hypothesis: `copy.copy` of an instance makes a new attribute set (a new attribute on the
copy is not on the original) sharing the attribute values (a store into a shared dict is
seen through both); `copy.deepcopy` shares nothing.

Expectations were written from the program text before running the engine."""

import copy


class P:
    def __init__(self):
        self.x = 1
        self.tags = {}


p = P()
q = copy.copy(p)
q.y = 2
q.tags["t"] = 1
p.y                                          #@ missing y
pt = p.tags                                  #@row pt: present={t} definite={t} closed
r = copy.deepcopy(p)
r.tags["u"] = 1
pt2 = p.tags                                 #@row pt2: present={t} definite={t} closed
