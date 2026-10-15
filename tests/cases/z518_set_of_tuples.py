# A tuple of known values is a set's member by value: `add` keeps the set's members known, and
# `in` decides membership of such a tuple (a de-duplication loop keeps its count).
seen = set()
seen.add(("a", 1))
hit = ("a", 1) in seen                           #@value hit: exactly={True}
miss = ("a", 2) in seen                          #@value miss: exactly={False}
lit = {("b", 2)}
other = ("b", 3) not in lit                      #@value other: exactly={True}

keys = set()
kept = []
for rec in [{"s": "x", "id": 1}, {"s": "x", "id": 2}, {"s": "x", "id": 1}]:
    k = (rec["s"], rec["id"])
    if k in keys:
        continue
    keys.add(k)
    kept.append(rec)
n = len(kept)                                    #@value n: exactly={2}
last = kept[-1]["id"]                            #@value last: exactly={2}
