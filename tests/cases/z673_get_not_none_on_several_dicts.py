# `if d.get("k") is not None:` and `if d.get("k"):` where d is one of several dicts, one filled
# under keys rowan does not know: whichever dict d is, it has "k" in the branch (as with
# `"k" in d`); after the test, on both paths, d may lack it. python z673.py → pick is m, which
# has "k": a = b = c = 1; z673.py '{}' → pick is j = {}: a = b = 0, c raises (-1).
import json
import sys

m = {}
for name in sys.argv + ["k"]:
    m[name] = 1
j = json.loads(sys.argv[1]) if len(sys.argv) == 2 else {}
pick = m if len(sys.argv) != 2 else j


def not_none(d):
    if d.get("k", None) is not None:
        return d["k"]
        #@ ok k
    return 0


def truthy(d):
    if d.get("k"):
        return d["k"]
        #@ ok k
    return 0


def after(d):
    if d.get("k") is not None:
        pass
    return d["k"]
    #@ fails k


a = not_none(pick)
#@value a: includes={0, 1}
b = truthy(pick)
#@value b: includes={0, 1}
try:
    c = after(pick)
except KeyError:
    c = -1
p = c
#@value p: includes={-1, 1}
