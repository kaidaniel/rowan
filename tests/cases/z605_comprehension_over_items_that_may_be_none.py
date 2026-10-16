# A comprehension over the items of an input's dict (which may be empty), or over `enumerate`
# of a list that may be empty: what it stores under a key of its own is there only maybe.
import sys
import json

n = len(sys.argv)
data = json.loads(sys.argv[1] if n > 50 else "{}")
out = {"x": v for k, v in data.items()}
#@row out: possible={x} definite_within={}
d = {"a": 1, "b": 2}
ys = [kk for kk in d if d[kk] > 2]
for _i in range(n):
    if n > 1:
        ys.append("s")
e = {t: i for i, t in enumerate(ys)}
#@row e: possible={s} definite_within={}
