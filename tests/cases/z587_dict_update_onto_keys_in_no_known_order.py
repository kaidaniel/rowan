# Keys copied onto a dict whose own keys are in no known order (here d's "k", popped on some
# paths by a loop) are not first: "k" may come before "m".
import sys


def pick(x, y):
    return x if n > 2 else y


n = len(sys.argv)
d = {"k": 0}
f = {"m": None}
if n == 1:
    d["m"] = 2
    f["k"] = 1
for _i in range(n - 1):
    pick(f, d).pop("k", None)
out = dict(d)
out.update(f)
first = list(out)[0]                         #@value first: includes={"k", "m"}
#@todo value first: exactly={"k", "m"}  (the keys of a dict in no known order are not taken as literals)
