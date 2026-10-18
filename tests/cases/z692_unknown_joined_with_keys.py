# A value that is one of a dict's keys (some not known: an input's text), joined with an int
# not known: the join may be any of the keys, so it is no longer an int.
import sys
n = len(sys.argv)
d = {"k": 0}
d[sys.argv[0][:0] + "m"] = 1
for kk in d:
    x = kk
y = x if n > 2 else n % 3
#@value y: includes={"k", "m", 0, 1, 2}
#@value y: untyped
e = {kk: kk for kk in d}
e[n % 3] = n
b = e.get("k", -1)
#@value b: includes={"k"}
