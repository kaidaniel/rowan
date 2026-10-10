"""`t is a` with `a` one object narrows `t` to it (and `t is not a` takes `a` out of it); a key
stored through `t` while it was one of several dicts is then a key of that dict."""
import sys

a = {}
b = {}
t = a if len(sys.argv) > 2 else b
t["x"] = 1
if t is a:
    va = a["x"]
#@ ok x
    which = "a"
else:
    vb = b["x"]
#@ ok x
    which = "b"
if t is not a:
    wb = b["x"]
#@ ok x
_ = 0
#@value which: exactly={"a", "b"}

c = {"k": 1}
d = {"k": 2}
u = c if len(sys.argv) > 3 else d
if u is d:
    got = u["k"]
#@value got: exactly={2}
else:
    got2 = u["k"]
#@value got2: exactly={1}
