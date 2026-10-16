# A list of some of a dict's keys (a comprehension with a filter) need not hold them all: a
# dict made from it has those keys only maybe. `list(d)` holds them all.
import sys

n = len(sys.argv)
d = {"a": 1, "b": 2}
if n > 2:
    d["s"] = 0
ys = [kk for kk in d if d[kk] > 3]
e = {t: len(t) for t in ys}                  #@row e: present={a, b, s} definite={} closed
#@todo row e: present={} definite={} closed  (the filter is not decided per key)
c = {"a": 1, "b": 2}
keys = list(c)
f = {t: 0 for t in keys}                     #@row f: present={a, b} definite={a, b} closed
keys = list(d)
g = {t: 0 for t in keys}                     #@row g: present={a, b, s} definite={} closed
#@todo row g: present={a, b, s} definite={a, b} closed  (a list of keys some of which are there on some paths only)
