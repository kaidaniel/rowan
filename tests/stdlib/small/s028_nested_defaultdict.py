# nested defaultdicts (lambda factory) pivoting records; converting back to plain dicts
from collections import defaultdict


def pivot(rows):
    t = defaultdict(lambda: defaultdict(int))
    for r in rows:
        t[r["region"]][r["product"]] += r["qty"]
    return t
#@schema param pivot.rows['*']: always={region,product,qty} only={region,product,qty}


def to_plain(t):
    return {k: dict(v) for k, v in t.items()}


if __name__ == "__main__":
    rows = [
        {"region": "north", "product": "apple", "qty": 2},
        {"region": "north", "product": "pear", "qty": 1},
        {"region": "south", "product": "apple", "qty": 5},
    ]
    t = pivot(rows)                              #@row t: present={north,south} definite={north,south} closed
    north = t["north"]                           #@row north: present={apple,pear} definite={apple,pear} closed
    z = t["east"]["fig"]                         #@ ok east ; ok fig
    #@value z: exactly={0}
    _ = 0                                        #@row t: present={north,south,east} definite={north,south,east} closed
    plain = to_plain(t)
    south = plain["south"]                       #@row south: present={apple} definite={apple} closed
    try:
        south["pear"]                            #@ missing pear
    except KeyError:
        pass
    a = plain["north"]["apple"]                  #@value a: exactly={2}
    e = pivot([])                                #@row e: present={} closed
