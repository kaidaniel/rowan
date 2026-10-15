# A loop over records that one object stands for (a summary, an input's elements) takes each
# item as an object of its own: a store into it and a key a test proves hold for that item.
# Another path to the same records (taken before the loop, through the list, the item of
# another loop) may be the item or another record: what the item got may be there or not.
import json
import os

raw = json.loads(os.environ.get("ROWAN_Z513", '[{"id": 1}, {"id": 2}]'))


def ahead():
    # `nxt` is the last record; the first iteration stores into the first one
    rows = [{"id": r["id"]} for r in raw]
    nxt = rows[-1]
    for r in rows:
        r["x"] = 1
        print(r["x"])  #@ ok x
        print(nxt["x"])  #@ possibly x


def through_list():
    # a store through the list reaches the first record only
    rows = [{"id": r["id"]} for r in raw]
    for r in rows:
        rows[0]["w"] = 1
        print(r["w"])  #@ possibly w


def two_loops():
    # the inner item is another record than the outer one, and loses its key again
    rows = [{"id": r["id"]} for r in raw]
    for a in rows:
        for b in rows:
            b["z"] = 1
            print(a["z"])  #@ possibly z
            del b["z"]


def after_break():
    # the item a loop left by `break` is still one of the records
    rows = [{"id": r["id"]} for r in raw]
    r = {"id": 0}
    for r in rows:
        break
    r["k"] = 1
    print(r["k"])  #@ ok k
    print(rows[-1]["k"])  #@ possibly k


def done_with_alias():
    # every record went through the body; only the first one got `t`
    rows = [{"id": r["id"]} for r in raw]
    for r in rows:
        r["s"] = 1
        rows[0]["t"] = 1
    for r in rows:
        print(r["s"])  #@ sound s
        #@todo ok s (a store through the list may hit the item: rowan joins the records' row into the item's, not that one key)
        print(r["t"])  #@ possibly t


def previous_item():
    # the item of the previous iteration is another record than this one
    rows = [{"id": r["id"]} for r in raw]
    prev = None
    for r in rows:
        if prev is not None:
            prev["seen"] = 1
            print(r["seen"])  #@ possibly seen
            #@todo missing seen (rowan does not know the two items are different records)
        prev = r


def last_of_input(items):
    # `last` is the input's last element; the first iteration stores into the first one
    last = None
    for it in items:
        last = it
    for it in items:
        it["y"] = 1
        if last is not None:
            print(last["y"])  #@ input y


for f in (ahead, through_list, two_loops, after_break, done_with_alias, previous_item):
    try:
        f()
    except KeyError:
        pass
try:
    last_of_input(raw)
except KeyError:
    pass
