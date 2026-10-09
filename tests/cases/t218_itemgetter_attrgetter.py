"""Hypothesis: `operator.itemgetter` / `attrgetter` work as key functions with known results,
and an itemgetter applied to a dict without the key reports the missing key.

Expectations were written from the program text before running the engine."""

from operator import attrgetter, itemgetter

rows = [{"n": 2, "b": 1}, {"n": 1, "a": 1}]
low = sorted(rows, key=itemgetter("n"))[0]   #@row low: present={n,a} definite={n,a} closed


class O:
    def __init__(self, n, tag):
        self.n = n
        self.tag = tag


objs = [O(2, {"two": 1}), O(1, {"one": 1})]
first = min(objs, key=attrgetter("n"))
t = first.tag                                #@row t: present={one} definite={one} closed
get_n = itemgetter("n")
v = get_n({"m": 1})                          #@ missing n
