"""A read of a dict under a key not known may raise KeyError, whatever the dict holds: a
handler for it runs. It never raised (the handler was dead code), also in a callee."""
import sys

d = {"a": 1, "b": 2}
k = sys.argv[-1]
try:
    v = d[k]
    r = "found"
except KeyError:
    r = "missing"
rr = r
#@value rr: exactly={"found", "missing"}


def get(m, key):
    return m[key]


try:
    w = get(d, k)
    s = "found"
except KeyError:
    s = "missing"
ss = s
#@value ss: exactly={"found", "missing"}
