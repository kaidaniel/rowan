"""Hypothesis: a generator runs in step with its consumer, so what the consumer's code changes
between two items is what the generator reads after its yield (rowan runs the body whole at
the first item, then again with those changes made at each yield). Changes the generator
does not read, and consumers that change nothing it reaches, keep its items exact."""


def reader(d):
    yield 1
    v = d["x"]                                   #@ sound x
    yield v


def maybe(e):
    yield 1
    yield e.get("y", "none")


def ids(rows):
    for r in rows:
        yield r["id"]                           #@ ok id


def fresh(n):
    for i in range(n):
        yield {"i": i}


d = {}
got = []
for item in reader(d):
    d["x"] = 5
    got.append(item)
last = got[-1]                                   #@value last: includes={5}

e = {}
g = maybe(e)
first = next(g)                                  #@value first: exactly={1}
e["y"] = "set"
second = next(g)                                 #@value second: includes={"set"}

out = []
for x in ids([{"id": 1}, {"id": 2}]):
    out.append(x)
n = len(out)                                     #@value n: exactly={2}

tagged = []
for rec in fresh(2):
    rec["tag"] = True
    tagged.append(rec)
t1 = tagged[1]
#@row t1: present={i,tag} definite={i,tag} closed
