"""A ChainMap looks up its maps when it is read, so a key stored into one of them after the
ChainMap was made is there (before, the ChainMap kept the keys its maps had when it was made:
`cm["z"]` was claimed to fail always). The same holds for ChainMaps made in a loop.

Expectations were written from the program text before running the engine."""
import sys
from collections import ChainMap

base = {"a": 1}
cm = ChainMap({}, base)
base["z"] = 5
z = cm["z"]  #@ ok z
#@value z: exactly={5}
n = len(cm)
#@value n: exactly={2}
has = "z" in cm
#@value has: exactly={True}
keys = sorted(cm)
got = cm.get("z")
#@value got: exactly={5}

pairs = []
for i in range(len(sys.argv) + 2):
    b = {"a": i}
    pairs.append((ChainMap({}, b), b))
cm0, b0 = pairs[0]
b0["q"] = 7
q = cm0["q"]
#@value q: includes={7}
