# `f.update({k: 2})` and `{**f, **{k: 2}}` with k not known: f's "a" is its own 1, or 2 when k is
# "a" — a dict filled under keys not known holds only the values stored, under any key. Same for
# `dict(f, **m)`-style copies of such a dict. python z676.py → k is the script's path in
# capitals: x = y = 1.
import sys

k = sys.argv[0].upper()
f = {"a": 1}
f.update({k: 2})
x = f["a"]
#@value x: exactly={1, 2}
g = {**{"a": 1}, **{k: 3}}
y = g["a"]
#@value y: exactly={1, 3}
m = {}
m[k] = 4
h = {"a": 1}
h.update(m)
z = h["a"]
#@value z: exactly={1, 4}
