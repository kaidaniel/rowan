# A read through a value that is one of several inputs' parts reads one of them on a run: what
# it needs of each is needed only sometimes (rowan said always). A read through a key computed
# from data reads some item, maybe one read before by name. A list of two inputs' elements has
# each read: an element's needs stay per element. Expectations written from the program text.
import sys


def pick_one(a, b, use_a):
    x = a if use_a else b
    return x["k"]
#@schema param pick_one.a: sometimes={k}
#@schema param pick_one.b: sometimes={k}


def by_key(d, k):
    first = d["a"]
    return first, d[k]["b"]
#@schema param by_key.d: always={a}
#@schema param by_key.d['a']: sometimes={b}


def both(left, right):
    return [r["id"] for r in [*left, *right]]
#@schema param both.left['*']: always={id} only={id}
#@schema param both.right['*']: always={id} only={id}


def first_or_default(rows):
    row = rows[0] if rows else {"id": 0}
    return row["id"]
#@schema param first_or_default.rows[0]: sometimes={id}  (first written as nothing required of rows: rows[0] is read when rows is not empty)


p = pick_one({"k": 1}, {}, True)
q = pick_one({}, {"k": 2}, False)
f, g = by_key({"a": {"b": 1}, "c": {"b": 2}, "e": {}}, "c")
ids = both([{"id": 1}], [{"id": 2, "x": 0}])
z = first_or_default([{"id": 3}]) + first_or_default([])
w = by_key({"a": {}}, sys.argv[0]) if len(sys.argv) > 5 else None
