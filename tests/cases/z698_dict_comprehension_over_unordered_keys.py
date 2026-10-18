# A dict comprehension over a dict whose insertion order depends on the path: its keys come in
# that order too, which is not known ("z" goes in last on every run here, but rowan cannot
# tell: it must not claim another order).
import sys
def fill(d, k):
    if k not in d:
        d[k] = 9
    return d[k]
n = len(sys.argv)
f = {"k": 1, "m": None}
f.setdefault(1, [1])
if n > 2:
    for _i in range(n - 1):
        if f.get("z") is None:
            f["z"] = 2
r = fill(f, "z")
e = {kk: f[kk] for kk in f if kk != "k"}
last = list(e)[-1]
#@value last: includes={"z"}
#@todo value last: exactly={"z"}  (f's order is lost where "z" is stored on some paths only)
s = {kk for kk in {"a": 1, "b": 2}}
k = len(s)
#@value k: exactly={2}
