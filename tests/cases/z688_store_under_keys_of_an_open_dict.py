# A store under a key of a dict that holds keys not known (`e[kx]`, kx a text from input) may
# replace any key's value: `d["k"]` is 0 or None (None on runs where kx is "k").
import sys
n = len(sys.argv)
kx = "m" if n > 2 else sys.argv[0][:0].lower() + "k"
d = {"k": 0}
e = {}
e[kx] = 0
for kk, vv in list(e.items()):
    d[kk] = None
a = d.get("k", -1)
#@value a: exactly={0, None}
d2 = {"k": 0}
for kk in e:
    d2[kk] = None
b = d2.get("k", -1)
#@value b: exactly={0, None}
d4 = {"k": 0, "m": 1}
for kk in e:
    del d4[kk]
h = "k" in d4
#@value h: includes={True, False}
