# A generator that changes an object it made and yielded before (a buffer it fills on): the
# consumer sees it, at each item, as it is when that item is given; after the generator is used
# up, as at its end.
def gen():
    box = {"v": 0}
    yield box
    box["v"] = 7
    yield box


seen = []
for b in gen():
    seen.append(b["v"])
a = seen[0]
#@value a: exactly={0}
c = seen[1]
#@value c: exactly={7}
end = b["v"]
#@value end: exactly={7}


def rows():
    buf = []
    buf.append(1)
    yield buf
    buf.append(2)
    yield buf


lens = [len(r) for r in rows()]
l0 = lens[0]
#@value l0: exactly={1}
l1 = lens[1]
#@value l1: exactly={2}


def nexts():
    g = gen()
    b = next(g)
    v0 = b["v"]
    next(g)
    v1 = b["v"]
    return v0, v1


v0, v1 = nexts()
#@value v0: exactly={0}
#@value v1: exactly={7}


def tagged(n):
    row = {"id": 0}
    for i in range(n):
        row["id"] = i
        yield row


ids = [r["id"] for r in tagged(len(seen) + 1)]
first = ids[0]
#@value first: includes={0}
