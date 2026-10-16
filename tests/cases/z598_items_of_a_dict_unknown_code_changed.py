# The items of a dict code rowan cannot see may have changed (a store into what a call past
# the budget returned, which may be the dict) include what that code may have put there: a
# comprehension filtering on the values may keep any of them.
#@limits budget=0
import sys


def pick(x, y):
    return x if n > 2 else y


n = len(sys.argv)
d = {"k": 0}
pick(d, {}).pop("z", None)
f = {kk: d[kk] for kk in d if kk != "k"}
pick(f, f)["m"] = [1]
f.update(dict.fromkeys(["k", "z"], 0))
g = {kk: vv for kk, vv in f.items() if vv != 0}
#@row g: possible={m}
