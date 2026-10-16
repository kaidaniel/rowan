# A list changed in place (`xs[i] = v`, a slice, insert, del, reverse, sort, `*=`) and then
# grown by a loop of unknown length: a later loop over it sees every value a position holds,
# not only the values it was made with. While the length is known the elements are exactly
# the positions' values (a value overwritten is gone).
import sys

n = len(sys.argv)
key = sys.argv[1]

b = [1, 2]
b[0] = "b"
for _ in range(n):
    b.append(0)
for x in b:
    vb = x                                   #@value vb: exactly={"b", 2, 0}

h = [1, 2]
h[-1] = "h"
for _ in range(n):
    h.append(0)
for x in h:
    vh = x                                   #@value vh: exactly={1, "h", 0}

a = [1, 2]
a.insert(0, "a")
for _ in range(n):
    a.append(0)
for x in a:
    va = x                                   #@value va: exactly={"a", 1, 2, 0}

c = [1, 2]
c[0:1] = ["c", "d"]
for _ in range(n):
    c.append(0)
for x in c:
    vc = x                                   #@value vc: exactly={"c", "d", 2, 0}

d = [1, 2, "x"]
del d[2]
d.insert(1, "d")
for _ in range(n):
    d.append(0)
for x in d:
    vd = x                                   #@value vd: exactly={1, "d", 2, 0}

g = [2, 1]
g.reverse()
g[1] = "g"
for _ in range(n):
    g.append(0)
for x in g:
    vg = x                                   #@value vg: exactly={1, "g", 0}

s = [3, 1]
s.sort()
s[1] = "s"
for _ in range(n):
    s.append(0)
for x in s:
    vs = x                                   #@value vs: exactly={1, "s", 0}

m = [1, 2]
m *= 2
m[3] = "m"
for _ in range(n):
    m.append(0)
for x in m:
    vm = x                                   #@value vm: exactly={1, 2, "m", 0}

# one of two lengths: position 0 is "u" on both paths
u = [1]
if n > 2:
    u.append(2)
u[0] = "u"
for _ in range(n):
    u.append(0)
for x in u:
    vu = x                                   #@value vu: includes={"u", 2, 0}
#@todo value vu: exactly={"u", 2, 0}

# one of two lengths, then copied by a comprehension
e = [1, 2]
e[0] = "e"
if n > 2:
    e.append(3)
ce = [x for x in e]
for y in ce:
    ve = y                                   #@value ve: exactly={"e", 2, 3}

# membership in a list whose item was replaced: the key is one of its items now
allowed = ["a", "b"]
allowed[1] = "c"
if key in allowed:
    k = key                                  #@value k: exactly={"a", "c"}
