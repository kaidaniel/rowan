# Iterators reading their list as it is, through other consumers: a for loop whose body appends
# to the list (a worklist through iter()), unpacking, `in`, zip and filter over lists that grow,
# a list grown on one path only, a dict's items iterator after an insert, a set left as it was;
# map over two lists, islice, chain and zip of three over lists that grow; a deque's iterator
# raises RuntimeError once the deque changed.
import sys

n = len(sys.argv)
xs = [1]
it = iter(xs)
out = []
for x in it:
    out.append(x)
    if x < 3:
        xs.append(x + 1)
lo = len(out)
#@value lo: includes={3}
#@todo value lo: exactly={3}  (a list its loop changes: a fixpoint over any item)
ys = [1, 2]
yi = iter(ys)
ys.append(3)
a, b, c = yi
#@value c: exactly={3}
zs = [1, 2]
zi = iter(zs)
zs.append(7)
found = 7 in zi
#@value found: exactly={True}
p = [1, 2]
q = [3, 4]
z = zip(p, q)
p.append(5)
q.append(6)
zl = len(list(z))
#@value zl: exactly={3}
fs = [0, 1]
f = filter(None, fs)
fs.append(2)
ft = sum(f)
#@value ft: exactly={3}
ws = [1, 2]
wi = iter(ws)
if n > 5:
    ws.append(3)
wl = len(list(wi))
#@value wl: exactly={2, 3}
d = {"a": 1, "b": 2}
di = iter(d.items())
first = next(di)
d["c"] = 3
try:
    rest = list(di)
    r = "ok"
except RuntimeError:
    r = "err"
#@value r: exactly={"err"}
s = {1, 2}
si = iter(s)
s.discard(5)
sl = len(list(si))
#@value sl: exactly={2}
import collections
import itertools

a1 = [1, 2]
b1 = [3, 4]
m2 = map(lambda x, y: x + y, a1, b1)
a1.append(5)
b1.append(6)
r1 = len(list(m2))
#@value r1: exactly={3}
c1 = [1, 2]
isl = itertools.islice(c1, 5)
c1.append(3)
r2 = len(list(isl))
#@value r2: exactly={3}
d1 = [1]
ch = itertools.chain(d1, [9])
d1.append(2)
r3 = len(list(ch))
#@value r3: exactly={3}
e1 = [1, 2]
z3 = zip(e1, e1, e1)
e1.append(3)
r4 = len(list(z3))
#@value r4: exactly={3}
dq = collections.deque([1, 2])
dqi = iter(dq)
dq.append(3)
try:
    r8 = len(list(dqi))
except RuntimeError:
    r8 = -1
#@value r8: exactly={-1}
