# itertools.starmap unpacks each tuple into the callback's parameters: (record, field) and (record, factor)

from itertools import starmap


def pick(rec, field):
    return rec[field]


def project(rec, fields):
    vals = list(starmap(pick, [(rec, f) for f in fields]))
    return vals


def scale(rec, factor):
    return {**rec, "value": rec["value"] * factor}
#@schema param scale.rec: always={value} only={value}


v = project({"a": 1, "b": 2}, ["b", "a"])
v0 = v[0]                                         #@value v0: exactly={2}
try:
    project({"a": 1}, ["a", "z"])                 #@ missing z
except KeyError:
    pass
out = list(starmap(scale, [({"value": 2, "k": "x"}, 3), ({"value": 5}, 2)]))
o0 = out[0]                                       #@row o0: present={value,k} definite={value,k} closed
ov = o0["value"]                                  #@value ov: exactly={6}
o1 = out[1]                                       #@row o1: present={value} definite={value} closed
p = list(starmap(pow, [(2, 5), (3, 2)]))
p0 = p[0]                                         #@value p0: exactly={32}
