# sum / any / all over records: generator callbacks read keys of each record


def total(lines):
    return sum(l["qty"] * l["price"] for l in lines)
#@schema param total.lines['*']: always={qty,price} only={qty,price}


def flags(recs):
    some = any(r.get("urgent") for r in recs)
    every = all("id" in r for r in recs)
    return some, every


def counts():
    n = sum([1, 2, 3], 10)                       #@value n: exactly={16}
    f = sum(x / 2 for x in (1, 3))               #@value f: exactly={2.0}
    e = any([])                                  #@value e: exactly={False}
    a = all([])                                  #@value a: exactly={True}
    b = all(k in {"a": 1, "b": 2} for k in "ab") #@value b: exactly={True}
    return n


if __name__ == "__main__":
    t = total([{"qty": 2, "price": 3}, {"qty": 1, "price": 4, "sku": "x"}])  #@value t: exactly={10}
    z = total([])                                #@value z: exactly={0}
    s1, e1 = flags([{"id": 1, "urgent": True}, {"id": 2}])
    #@value s1: exactly={True}
    s2, e2 = flags([{"name": "x"}])
    #@value e2: exactly={False}
    counts()
