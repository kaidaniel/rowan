# A list's iterator reads the list as it is at each step, by index: an append after iter()
# is seen, a replaced item is the new one, and an iterator once at its end stays there.
# reversed() starts from the last index at creation and stops at an index past the end; dict
# and set iterators raise RuntimeError once the size changed. map, zip, enumerate and
# generators read their inputs at each step too.
xs = [1, 2]
it = iter(xs)
xs.append(3)
a = list(it)
la = len(a)
#@value la: exactly={3}
sa = sum(a)
#@value sa: exactly={6}
ys = [1, 2]
it2 = iter(ys)
f = next(it2)
#@value f: exactly={1}
ys.append(9)
rest = list(it2)
sr = sum(rest)
#@value sr: exactly={11}
zs = [1, 2]
it3 = iter(zs)
zs[0] = 5
first = next(it3)
#@value first: exactly={5}
ws = [1]
it4 = iter(ws)
done = list(it4)
ws.append(2)
le = len(list(it4))
#@value le: exactly={0}
ms = [1, 2]
m = map(str, ms)
ms.append(3)
mj = "-".join(m)
#@value mj: exactly={"1-2-3"}
p = [1, 2]
q = [3, 4, 5]
z = zip(p, q)
p.append(9)
zl = len(list(z))
#@value zl: exactly={3}
es = [1]
en = enumerate(es)
es.append(2)
el = len(list(en))
#@value el: exactly={2}
rs = [1, 2]
r = reversed(rs)
rs.append(3)
rl = list(r)
rsum = sum(rl)
#@value rsum: exactly={3}
rn = len(rl)
#@value rn: exactly={2}
rs2 = [1, 2]
r2 = reversed(rs2)
rs2[1] = 7
rf = next(r2)
#@value rf: exactly={7}
rs3 = [1, 2, 3]
r3 = reversed(rs3)
rs3.pop()
rl3 = len(list(r3))
#@value rl3: exactly={0}
d = {"a": 1}
di = iter(d)
d["b"] = 2
try:
    dl = list(di)
    dr = "ok"
except RuntimeError:
    dr = "err"
#@value dr: exactly={"err"}
d2 = {"a": 1}
dv = iter(d2.values())
d2["a"] = 5
v = next(dv)
#@value v: exactly={5}
s = {1}
si = iter(s)
s.add(2)
try:
    sl = list(si)
    sr2 = "ok"
except RuntimeError:
    sr2 = "err"
#@value sr2: exactly={"err"}


def gen(src):
    for x in src:
        yield x * 10


gs = [1, 2]
g = gen(gs)
g1 = next(g)
#@value g1: includes={10}
#@todo value g1: exactly={10}  (the generator runs again, its list changed at every yield)
gs.append(3)
gt = sum(g)
#@value gt: includes={50}
#@todo value gt: exactly={50}  (as g1)
hs = [1, 2]
h = (x * 10 for x in hs)
hs.append(3)
ht = sum(h)
#@value ht: includes={60}
#@value ht: exactly={60}
