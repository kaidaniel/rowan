# zip_longest(fillvalue=): header names paired with row values; short sides get the fill value

from itertools import zip_longest


def to_record(header, values):
    rec = dict(zip_longest(header, values, fillvalue=None))
    return rec


def merge_columns(left, right):
    out = []
    for a, b in zip_longest(left, right, fillvalue={}):
        out.append({**a, **b})
    return out


r = to_record(["id", "name", "email"], [1, "ann"])   #@row r: present={id,name,email} definite={id,name,email} closed
em = r["email"]                                     #@value em: exactly={None}
r2 = to_record(["id"], [2, "extra"])                #@row r2: present={id,None} definite={id,None} closed
x = r2[None]                                        #@value x: exactly={"extra"}
m = merge_columns([{"id": 1}, {"id": 2}], [{"score": 9}])
m0 = m[0]                                           #@row m0: present={id,score} definite={id,score} closed
m1 = m[1]                                           #@row m1: present={id} definite={id} closed
nm = len(merge_columns([], [{"a": 1}, {"b": 2}]))   #@value nm: exactly={2}
try:
    m1["score"]                                     #@ missing score
except KeyError:
    pass
