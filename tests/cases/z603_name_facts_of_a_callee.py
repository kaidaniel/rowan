# A callee's facts about its own names do not hold for the caller's names of the same
# spelling: `fill`'s `d` has "m" after `fill(f, "m")`, the module's `d` does not.
import sys


def fill(d, k):
    if k not in d:
        d[k] = 9
    return d[k]


n = len(sys.argv)
d = {"k": 0}
f = {"k": 1, "m": None}
if n > 2:
    f = dict.fromkeys(["k", "k"], "s")
r = fill(f, "m")
try:
    r = d["m"]                               #@ missing m
except KeyError:
    pass
b = r                                        #@value b: exactly={9, None}
