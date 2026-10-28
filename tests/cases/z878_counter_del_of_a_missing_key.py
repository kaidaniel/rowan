"""`del c[k]` on a `Counter` that lacks k does nothing (Counter's `__delitem__` ignores a
missing key, unlike a dict's). (Before, it was taken to raise KeyError.)

Expectations were written from the program text before running the engine."""
from collections import Counter

c = Counter(["x", "y", "x"])
try:
    del c["a"]
    n = len(c)
except KeyError:
    n = "ke"
n_seen = n
#@value n_seen: includes={2}
d = {"x": 2}
try:
    del d["a"]
    m = len(d)
except KeyError:
    m = "ke"
m_seen = m
#@value m_seen: exactly={"ke"}
