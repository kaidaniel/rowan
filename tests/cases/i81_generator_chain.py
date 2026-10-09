"""Generators yielding record dicts, chained (source -> filter -> enrich), consumed into a
list and a dict; a generator with send-free state in a closure.

Expectations were written from the program text before running the engine."""


def source():
    yield {"id": 1, "kind": "a", "v": 10}
    yield {"id": 2, "kind": "b", "v": 20}
    yield {"id": 3, "kind": "a", "v": 30}


def only_kind(records, kind):
    for r in records:
        if r["kind"] == kind:
            yield r


def enrich(records):
    n = 0
    for r in records:
        n += 1
        out = dict(r)
        out["seq"] = n
        if r["v"] > 15:
            out["big"] = True
        yield out


def counter():
    seen = {"n": 0}

    def bump():
        seen["n"] += 1
        return seen

    return bump


rows = list(enrich(only_kind(source(), "a")))
# The chain keeps positions: only records 1 and 3 have kind "a"; the first has v=10, so
# 10 > 15 is decided and it gets no "big".
r0 = rows[0]                              #@row r0: present={id,kind,v,seq} definite={id,kind,v,seq} closed
by_id = {r["id"]: r for r in rows}        #@row by_id: present={1,3} definite={1,3} closed
raw = next(source())                      #@row raw: present={id,kind,v} definite={id,kind,v} closed
bump = counter()
state = bump()                            #@row state: present={n} definite={n} closed
state2 = bump()                           #@row state2: present={n} definite={n} closed
r0["seq"]                                 #@ ok seq
r0["big"]                                 #@ missing big
raw["seq"]                                #@ missing seq
