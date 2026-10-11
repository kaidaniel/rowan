"""A subscript by an index not known to be in range may raise IndexError: a handler for it
runs. Only an index known to be out of range on a list of known length raised; `sys.argv[3]`,
`t[j]`, `"ab"[5]` never did, so their handlers were dead code."""
import sys

try:
    a = sys.argv[3]
    r = "ok"
except IndexError:
    r = "missing"
rr = r
#@value rr: exactly={"ok", "missing"}

t = [1, 2, 3]
j = len(sys.argv) * 2
try:
    v = t[j]
    s = "in"
except IndexError:
    s = "out"
ss = s
#@value ss: exactly={"in", "out"}

try:
    c = "ab"[5]
    u = "no"
except IndexError:
    u = "raised"
uu = u
#@value uu: exactly={"raised"}

w = "x" if len(sys.argv) > 1 else "xyz"
try:
    c2 = w[2]
    z = "fine"
except IndexError:
    z = "short"
zz = z
#@value zz: exactly={"fine", "short"}
