# A dict filled under keys not known: a read under a literal key takes what those stores put
# there, and keeps it under the key. A handler that stores the key on the other path then
# joins with that value instead of replacing it.
import sys
n = len(sys.argv)
kx = "m" if n > 2 else (sys.argv[0][:0] + "Z").lower()
d = {"k": 0, "m": 1}
for _i in range(n):
    d[_i % 3] = _i
d = {vv if isinstance(vv, str) else kk: kk for kk, vv in d.items()}
try:
    d[kx] = d["k"]
except KeyError:
    d["k"] = 0
p = d.get("k", -1)
#@value p: includes={"k"}
e = {"k": 0, "m": 1}
try:
    e["m"] = e[kx]
except KeyError:
    e[kx] = None
try:
    e["m"] = e["z"]
except KeyError:
    e["z"] = 0
q = e["z"]
#@value q: includes={None, 0}
