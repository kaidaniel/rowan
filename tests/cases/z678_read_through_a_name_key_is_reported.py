# `if c.get("notes"):` on a name holding one of several dicts gives the name the key: the read
# `c["notes"]` after it is proven, and reported as one (its own verdict on the dicts, made
# without reporting, is no finding: it did not stand in for the proven one). python z678.py → a.
import sys

a = {"notes": ("a",)}
b = {"notes": ("b",), "x": 1}
c = a if len(sys.argv) < 3 else b
r = None
if c.get("notes"):
    r = c["notes"][0]
    #@ reported notes
    #@ ok notes
s = r
#@value s: exactly={"a", "b"}
