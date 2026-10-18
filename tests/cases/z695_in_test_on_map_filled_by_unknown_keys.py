# One path puts "k" in under a key not known (kx is "k" there), the other stores "k" by name:
# after `"k" in d`, d["k"] is either value — 0 from the first path, 2 from the second.
import sys
n = len(sys.argv)
kx = "m" if n > 2 else (sys.argv[0][:0] + "K").lower()
d = {}
if n == 1:
    d |= {kx: 0}
else:
    for _i in range(n):
        d.update({"k": 2})
def fill(d, k):
    if k not in d:
        d[k] = 9
    return d[k]
r = fill(d, "k")
#@value r: includes={0, 2}
if "k" in d:
    r2 = d["k"]
    #@value r2: includes={0, 2}
