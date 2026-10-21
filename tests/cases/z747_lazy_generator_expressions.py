# A generator expression runs when its items are taken, not where it is written: its first
# iterable is evaluated (and iter() taken of it) where it is made, everything else reads the
# names as they are at each step, and one never consumed runs nothing.
d = {}
g = (d[k] for k in ["a"])            #@ ok a
d["a"] = 1
r1 = list(g)
v1 = r1[0]
#@value v1: exactly={1}
y = 1
h = (x + y for x in [10, 20])
y = 2
r2 = list(h)
a2 = r2[0]
#@value a2: exactly={12}
e = {}
never = (e[k] for k in ["zz"])       #@ none zz
log = []


def note(x):
    log.append(x)
    return x


m = (note(x) for x in [1, 2])
n_before = len(log)
#@value n_before: exactly={0}
r4 = list(m)
n_after = len(log)
#@value n_after: exactly={2}
xs = [1, 2]
g5 = (x * 10 for x in xs)
xs = [7]
s5 = sum(list(g5))
#@value s5: exactly={30}
ys = [1]
g6 = (v for v in ys)
ys.append(2)
n6 = len(list(g6))
#@value n6: exactly={2}
caught = 0
try:
    bad = (x for x in None)
except TypeError:
    caught = 1
#@value caught: exactly={1}


def make(n):
    k = 1
    out = (x + k for x in range(n))
    k = 5
    return out


r8 = list(make(2))
t8 = r8[1]
#@value t8: exactly={6}
first = next(x for x in [1, 5, 7] if x > 3)
#@value first: exactly={5}
gw = ((last := x) for x in [4, 9])
for _ in gw:
    pass
lw = last
#@value lw: exactly={9}


def walrus():
    total = 0
    for _ in ((total := total + x) for x in [1, 2, 3]):
        pass
    return total


wt = walrus()
#@value wt: exactly={6}


def consume(it, store):
    store["ready"] = 1
    return list(it)


st = {}
r12 = consume((st["ready"] for _ in [0]), st)   #@ ok ready
v12 = r12[0]
#@value v12: exactly={1}
