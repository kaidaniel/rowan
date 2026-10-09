# contextlib.suppress(KeyError) around a possibly missing key: the rest of the block is skipped when it fails

from contextlib import suppress


def extract(rec):
    out = {"id": rec["id"]}                       #@ ok id
    with suppress(KeyError):
        out["email"] = rec["contact"]["email"]    #@ none contact ; none email
        out["verified"] = True
    n = len(out)                                  #@value n: exactly={1,3}
    #@row out: present={id,email,verified} definite={id} closed
    return out
#@schema param extract.rec: always={id} only={id}


def first_ok(cands, key):
    for c in cands:
        with suppress(KeyError, TypeError):
            return c[key]
    return None


a = extract({"id": 1, "contact": {"email": "e"}})   #@row a: present={id,email,verified} definite={id,email,verified} closed
b = extract({"id": 2})                              #@row b: present={id} definite={id} closed
c = extract({"id": 3, "contact": {}, "x": 0})       #@row c: present={id} definite={id} closed
v = first_ok([{}, {"k": 2}, {"k": 3}], "k")         #@value v: exactly={2}
w = first_ok([{}], "k")                             #@value w: exactly={None}
try:
    b["email"]                                      #@ missing email
except KeyError:
    pass
