"""The (key, value) pairs of `d.items()` through `enumerate`, `zip`, `list` and a starred
item keep their values. A pair element of an iteration was dropped when another iteration
wrapped it (`enumerate(d.items())` gave items `(i, nothing)`): reads on the value raised
nothing, and the values were empty."""
import json
import os

d = {"a": {"x": 1}, "b": {"x": 2}}
src = json.loads(os.environ.get("ROWS", '{"r": {"x": 3}}'))

for i, (k, v) in enumerate(src.items()):
    v["y"]                                       #@ input y
    w = v["x"]

for kv, n in zip(src.items(), range(3)):
    k2, v2 = kv
    v2["z"]                                      #@ input z

pairs = list(d.items())
first = pairs[0]
fk, fv = first
fx = fv["x"]
#@value fx: includes={1}

#@schema json.loads@L9['*']: always={x,y,z}
