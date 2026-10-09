# itertools.pairwise (consecutive records) and tee (two independent iterators over one stream)

from itertools import pairwise, tee


def deltas(readings):
    out = []
    for prev, cur in pairwise(readings):
        out.append({"at": cur["t"], "delta": cur["v"] - prev["v"]})
    return out
#@schema param deltas.readings['*']: always={t,v} only={t,v}


def peek_and_sum(it):
    a, b = tee(it)
    first = next(a)
    total = sum(x["n"] for x in b)
    return first, total


d = deltas([{"t": 1, "v": 10}, {"t": 2, "v": 13}, {"t": 3, "v": 11, "note": "x"}])
nd = len(d)                                      #@value nd: exactly={2}
d1 = d[1]                                        #@row d1: present={at,delta} definite={at,delta} closed
dv = d1["delta"]                                 #@value dv: exactly={-2}
e = deltas([{"t": 0, "v": 1}])
ne = len(e)                                      #@value ne: exactly={0}
f, tot = peek_and_sum(iter([{"n": 2, "tag": "x"}, {"n": 5}]))
#@row f: present={n,tag} definite={n,tag} closed
#@value tot: exactly={7}
ps = ["".join(p) for p in pairwise("abc")]
p1 = ps[1]                                       #@value p1: exactly={"bc"}
try:
    f["m"]                                       #@ missing m
except KeyError:
    pass
