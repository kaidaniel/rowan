"""A call past its budget changes an argument only where its code may change it: read, its
items looked up, iterated (a list), only tested, the argument stays as it was; stored into,
appended to, passed on, it may have changed. The code an argument holds still runs (a
callback called, a property read). (Each function's second call is past the budget.)

Expectations were written from the program text before running the engine."""
#@limits budget=1

CFG = {"a": 1, "b": 2}
KEYS = ["a", "b"]


def total(d, keys):
    s = 0
    for k in keys:
        s += d.get(k, 0)
    return s


total(CFG, KEYS)
total(CFG, KEYS)
x = CFG["a"]  #@ ok a
#@value x: exactly={1}
n = len(KEYS)  #@value n: exactly={2}


def fill(d, v):
    a = 1
    d["k"] = v


fill({}, 1)
D = {"z": 0}
fill(D, 2)
dk = D.get("k")  #@value dk: includes={2}


def keep(items, v):
    a = 1
    items.append(v)


L = []
keep(L, 1)
keep(L, 2)
ln = len(L)  #@value ln: includes={2}

HITS = []


def on_hit(v):
    HITS.append(v)


def run(cb, v):
    a = 1
    cb(v)


run(on_hit, 1)
run(on_hit, 2)
nh = len(HITS)  #@value nh: includes={2}

SEEN = []


class P:
    @property
    def v(self):
        SEEN.append(1)
        return 1


def peek(p):
    a = 1
    if p.v:
        return 0
    return 1


p = P()
peek(p)
peek(p)
ns = len(SEEN)  #@value ns: includes={2}
