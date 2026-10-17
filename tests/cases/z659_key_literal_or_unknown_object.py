# A key that is a literal on one path and an unknown (an unknown call's result) on the other:
# stored into a dict it may or may not be "a". Found by the fuzzer (seed 52001, comps_52234):
# `{t: i for i, t in enumerate(ys)}` over ys = ["a", "b"] | [t.upper(), …] claimed "b" in d.
# python z659.py → k is the script's path in capitals: every p is False / 0; z659.py x y → k
# is "a": every p is True / 1. (`types={bool}`: False or True, rowan's bool(?).)
import sys

n = len(sys.argv)
k = "a" if n > 2 else sys.argv[0].upper()

d1 = {k: 1}
p1 = "a" in d1
#@value p1: types={bool}
d2 = {k: 1 for _ in range(1)}
p2 = "a" in d2
#@value p2: types={bool}
d3 = dict([(k, 1)])
p3 = "a" in d3
#@value p3: types={bool}
d4 = {}
d4.update([(k, 1)])
p4 = "a" in d4
#@value p4: types={bool}
d5 = dict.fromkeys([k], 1)
p5 = "a" in d5
#@value p5: types={bool}
d6 = dict(zip([k], [1]))
p6 = "a" in d6
#@value p6: types={bool}


class C:
    pass


c = C()
setattr(c, k.lower(), 1)
p7 = hasattr(c, "a")
#@value p7: types={bool}
# (hasattr on a row a key not known was stored into: undecided)
e = {"a": 1}
p8 = k in e
#@value p8: types={bool}
p9 = e.get(k, 0)
#@value p9: exactly={0, 1}
ys = ["a", "b"]
if n > 2:
    ys = [sys.argv[1].upper(), sys.argv[2].upper()]
d10 = {t: i for i, t in enumerate(ys)}
p10 = "b" in d10
#@value p10: types={bool}
# removing and reading with such a key: "a" may stay
e = {"a": 1, "b": 2}
try:
    del e[k]
except KeyError:
    pass
p11 = "a" in e
#@value p11: types={bool}
f = {"a": 1}
f.pop(k, None)
p12 = "a" in f
#@value p12: types={bool}
g = {"a": 1}
try:
    v = g[k]
except KeyError:
    v = 0
p13 = v
#@value p13: exactly={0, 1}
h = {"a"}
h.discard(k)
p14 = "a" in h
#@value p14: types={bool}
s = {k}
p15 = "a" in s
#@value p15: types={bool}
