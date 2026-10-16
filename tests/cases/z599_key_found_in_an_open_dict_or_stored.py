# `if k not in d: d[k] = 9` on a dict code rowan cannot see changed: where the key was there,
# its value is whatever that code put there, not the 9 stored on the other path.
import sys
import xml.dom.minidom as somelib


def fill(d, k):
    if k not in d:
        d[k] = 9
    return d[k]


n = len(sys.argv)
d = {"k": 0}
somelib.something(d)
v = fill(d, "m")                             #@value v: unknown
q = d.get("m", -1)                           #@value q: unknown
# (the same for an input's dict: a default stored where the key is missing)
import json

data = json.loads(sys.argv[1] if n > 50 else '{"m": 1}')
if "m" not in data:
    data["m"] = 9
m = data["m"]                                #@value m: unknown
