# What code between a generator's items changes, the generator sees after its yield: unknown
# code given what it reads, a `del` of a key it reads (also of an object it changes itself), and
# code run while a partly consumed generator was returned out of the call that started it.
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
