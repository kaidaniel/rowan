# A dict made from pairs not known (`dict(reversed(list(...)))` of a dict filtered by a key
# not known) holds values no slot has; a later store under a key not known adds one value
# beside them, it does not replace them: d["k"] is still 0 where kx is not "k".
import sys
n = len(sys.argv)
kx = "m" if n > 2 else (sys.argv[0][:0] + "Z").lower()
d = {"k": 0, "m": 1}
if n <= 2:
    d = {kk: vv for kk, vv in d.items() if kk != kx}
d = dict(reversed(list(d.items())))
for _i in range(n - 1):
    d |= {kx: 2}
c = d.get("k", -1)
#@value c: includes={0}
vs = list(d.values())
v0 = vs[0] if vs else -1
#@value v0: includes={0, 1, 2}
