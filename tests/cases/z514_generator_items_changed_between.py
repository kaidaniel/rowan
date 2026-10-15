# What code between a generator's items changes, the generator sees after its yield: unknown
# code given what it reads, a `del` of a key it reads (also of an object it changes itself), and
# code run while a partly consumed generator was returned out of the call that started it, or
# kept in an attribute for the next call.
# A consumer holding the item of an earlier step still holds that item, not the next one.
import json
import os

drop = os.environ.get("Z514_DROP", "1") == "1"

# unknown code between the items
state = {"mode": "a"}


def modes():
    yield 1
    yield state["mode"]  #@ likely mode


op = getattr(dict, os.environ.get("Z514_OP", "clear"))
try:
    for x in modes():
        op(state)
except KeyError:
    pass

# a key the generator reads deleted between its items
cfg = {"n": 1}


def counts():
    yield 0
    yield cfg["n"]  #@ possibly n


try:
    for x in counts():
        if drop:
            del cfg["n"]
except KeyError:
    pass

# the generator stores the key itself before its yield
log = {}


def logged():
    log["n"] = 0
    yield 1
    yield log["n"]  #@ possibly n


try:
    for x in logged():
        if drop:
            del log["n"]
except KeyError:
    pass

# a generator started in a call and returned from it
cfg2 = {"k": 1}


def reader():
    yield 0
    yield cfg2["k"]  #@ possibly k


def started():
    g = reader()
    next(g)
    return g


g2 = started()
if drop:
    del cfg2["k"]
try:
    print(next(g2))
except KeyError:
    pass

# a generator kept in an attribute: each call of a method takes an item
cfg3 = {"k": 1}


class Feeder:
    def __init__(self):
        self.g = self._gen()

    def _gen(self):
        yield 0
        yield cfg3["k"]  #@ possibly k

    def step(self):
        return next(self.g)


feeder = Feeder()
feeder.step()
if drop:
    del cfg3["k"]
try:
    feeder.step()
except KeyError:
    pass

# items of a loop over records yielded one after the other
raw = json.loads(os.environ.get("Z514_ROWS", '[{"id": 2}, {"id": 1, "w": 0}]'))
recs = [{"id": r["id"], "w": r.get("w")} for r in raw]


def each(rows):
    for r in rows:
        if r["w"] is None:
            del r["w"]
        yield r


prev = None
try:
    for r in each(recs):
        if "w" in r and prev is not None:
            print(prev["w"])  #@ possibly w
        prev = r
except KeyError:
    pass

# the consumer keeps the previous item: another record than the next one
def plain(rows):
    for r in rows:
        yield r


prev_rec = None
for rec in plain([{"id": r["id"]} for r in raw]):
    if prev_rec is not None:
        prev_rec["seen"] = 1
        print(rec["seen"])  #@ possibly seen
        #@todo missing seen (rowan does not know the two items are different records)
    prev_rec = rec
