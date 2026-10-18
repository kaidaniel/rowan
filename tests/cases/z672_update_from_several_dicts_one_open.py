# `out.update(b)` where b is one of two dicts (a loop of unknown length rebound it), one with a
# key "k" and one holding a key not known (`f[kx] = 9`): that key may be "k". The value under
# "k" from the dict that has it was taken as the only one; the other dict's joins in (found by
# the fuzzer, seed 424242).
import sys


def fill(d, k):
    if k not in d:
        d[k] = 9
    return d[k]


def merge(a, b):
    out = dict(a)
    out.update(b)
    return out


n = len(sys.argv)
kx = (sys.argv[0][:0] + "K").lower()
d = {"k": 0}
f = {}
if n == 1:
    fill(f, kx)
for _i in range(n - 1):
    f = merge(f, d)
d = merge(d, f)
p = d.get("k", -1)
#@value p: includes={0, 9}
#@value p: exactly={0, 9}
