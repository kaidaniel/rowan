# helpers calling helpers through library callbacks: records pass map(partial) -> filter -> sorted(key) -> reduce

from functools import partial, reduce


def normalise(rec, defaults):
    out = {**defaults, **rec}
    out["name"] = out["name"].lower()
    return out


def is_valid(rec):
    return rec["qty"] > 0
#@schema param is_valid.rec: always={qty} only={qty}


def weight(rec):
    return rec["qty"] * rec.get("unit", 1)
#@schema param weight.rec: always={qty} only={qty}


def total(acc, rec):
    acc["sum"] += weight(rec)
    acc["names"].append(rec["name"])              #@ ok name
    return acc


def pipeline(records, defaults):
    norm = map(partial(normalise, defaults=defaults), records)
    valid = filter(is_valid, norm)
    ranked = sorted(valid, key=weight, reverse=True)
    summary = reduce(total, ranked, {"sum": 0, "names": []})
    #@row summary: present={sum,names} definite={sum,names} closed
    best = ranked[0] if ranked else None
    return summary, best


s, b = pipeline([{"name": "A", "qty": 2, "unit": 3}, {"name": "B", "qty": 0}], {"qty": 1})
#@row b: present={name,qty,unit} definite={name,qty,unit} closed
tot = s["sum"]                                    #@value tot: exactly={6}
s2, b2 = pipeline([{"name": "C"}], {"qty": 4, "unit": 2, "src": "d"})
#@row b2: present={name,qty,unit,src} definite={name,qty,unit,src} closed
t2 = s2["sum"]                                    #@value t2: exactly={8}
bn = b2["name"]                                   #@value bn: exactly={"c"}
try:
    pipeline([{"qty": 1}], {})                    #@ missing name
except KeyError:
    pass
