# defaultdict with a user factory, setdefault, Counter over a key read from each record

from collections import Counter, defaultdict


def new_bucket():
    return {"count": 0, "items": []}


def bucket_by(rows, field):
    groups = defaultdict(new_bucket)
    for r in rows:
        b = groups[r[field]]
        #@row b: present={count,items} definite={count,items} closed
        b["count"] += 1
        b["items"].append(r)
    return groups


def tally(events):
    return Counter(e["kind"] for e in events)
#@schema param tally.events['*']: always={kind} only={kind}


g = bucket_by([{"k": "x", "v": 1}, {"k": "y"}, {"k": "x"}], "k")
#@row g: present={x,y} definite={x,y} closed
cx = g["x"]["count"]                              #@value cx: exactly={2}
z = g["z"]                                        #@row z: present={count,items} definite={count,items} closed
#@row g: present={x,y,z} definite={x,y,z} closed
t = tally([{"kind": "a"}, {"kind": "b", "u": 1}, {"kind": "a"}])
#@row t: present={a,b} definite={a,b} closed
ta = t["a"]                                       #@value ta: exactly={2}
tm = t["missing"]                                 #@value tm: exactly={0}
idx = {}
for word in ["ab", "ac", "b"]:
    idx.setdefault(word[0], []).append(word)
ni = len(idx)                                     #@row idx: present={a,b} definite={a,b} closed
na = len(idx["a"])                                #@value na: exactly={2}
