"""`a + b` where a side may be a list gave a list alone, and `a | b` where a side may be a
dict a dict alone: where neither side is one (`x` a text, an unknown value) the operator
gives what it gives on those, `"k" + "s"` is "ks" (found by the fuzzer, tries area: `x` a
caught KeyError's "k", a skipped call's result or a list)."""
#@limits budget=0
import json
import os
import sys

n = len(sys.argv)
a = json.loads(os.environ.get("ROWAN_Q", '"q"'))


def boom(v):
    if v == "s":
        raise KeyError(v)
    return v


boom(0)
z = "s"
try:
    z = [boom(w) for w in (z, "s")]
except Exception:
    pass
x = 0
try:
    x = boom(z)
except KeyError:
    x = "k"
y = 1
try:
    y = x + "s"
except TypeError:
    pass
p = y
#@value p: includes={'ks'}

l = [1] if n == 99 else a
y2 = l + "s"
#@value y2: includes={'qs'}
y3 = "s" + l
#@value y3: includes={'sq'}
y4 = l + l
#@value y4: includes={'qq'}
d = {"a": 1} if n == 99 else len(sys.argv)
o1 = d | 2
#@value o1: includes={3}
k = [1] if n == 99 else "k"
y5 = k + "s"
#@value y5: includes={'ks'}
