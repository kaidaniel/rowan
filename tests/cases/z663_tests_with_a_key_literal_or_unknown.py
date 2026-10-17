# Tests whose key is a literal on one path and an unknown on the other narrow nothing to that
# literal (`if d.get(k):`, `if hasattr(c, k):`), and an attribute read in a try after a store
# under a name not known may raise. python z663.py → k is the script's path in capitals, the
# dict and the instance hold that key only: p1, p2, p3, p7 are -1; z663.py x y → k is "a", the
# tests false: 0. p6 is -1 both ways (e.a is set with three arguments).
import sys

n = len(sys.argv)
k = "a" if n > 2 else sys.argv[0].upper()
d = {}
if n > 3:
    d["a"] = 1
d[sys.argv[0].upper()] = 2
r = 0
if d.get(k):
    try:
        r = d["a"]
    except KeyError:
        r = -1
p1 = r
#@value p1: includes={-1, 0, 1}


class C:
    pass


c = C()
if n > 3:
    c.a = 1
setattr(c, sys.argv[0].upper(), 2)
q = 0
if hasattr(c, k):
    try:
        q = c.a
    except AttributeError:
        q = -1
p2 = q
#@value p2: includes={-1, 0, 1}
s = 0
if k not in d:
    pass
else:
    try:
        s = d["a"]
    except KeyError:
        s = -1
p7 = s
#@value p7: includes={-1, 0, 1}
t = 0
if d.get(k) is not None:
    try:
        t = d["a"]
    except KeyError:
        t = -1
p3 = t
#@value p3: includes={-1, 0, 1}
u = 0
if k == "a":
    u = 1
p4 = u
#@value p4: includes={0, 1}
x = 0
match k:
    case "a":
        x = 1
    case _:
        x = 2
p5 = x
#@value p5: includes={1, 2}
e = C()
if n > 3:
    e.a = 1
setattr(e, sys.argv[0].upper(), 2)
v = 0
try:
    v = e.a
except AttributeError:
    v = -1
p6 = v
#@value p6: includes={-1, 2}
