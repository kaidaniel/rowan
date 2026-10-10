"""A list whose length differs between two joined paths has one of those lengths (not an
unknown one); any change to its elements after the join makes the alternatives void."""
import sys

flag = len(sys.argv) > 2


def make():
    xs = [1]
    if flag:
        xs.append(2)
    return xs


a = make()
na = len(a)
#@value na: exactly={1, 2}
t = (1, 2) if flag else (1,)
nt = len(t)
#@value nt: exactly={1, 2}

b = make()
b.append(3)
nb = len(b)
#@value nb: exactly={2, 3}
c = make()
c.pop()
nc = len(c)
#@value nc: includes={0, 1}
d = make()
d.extend([7, 8])
nd = len(d)
#@value nd: exactly={3, 4}
e = make()
del e[0]
ne = len(e)
#@value ne: includes={0, 1}
f = make()
f.insert(0, 9)
nf = len(f)
#@value nf: includes={2, 3}
g = make()
g += [5]
ng = len(g)
#@value ng: includes={2, 3}
h = make()
h.clear()
nh = len(h)
#@value nh: exactly={0}
k = make()
k[1:] = []
nk = len(k)
#@value nk: includes={1}
m = make()
m.remove(1)
nm = len(m)
#@value nm: includes={0, 1}
cp = list(make())
ncp = len(cp)
#@value ncp: exactly={1, 2}
