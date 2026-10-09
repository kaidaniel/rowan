# map / filter with named functions and lambdas reading keys: the callback sees each record


def price_of(item):
    return item["qty"] * item["unit"]
#@schema param price_of.item: always={qty,unit} only={qty,unit}


def is_active(user):
    return user.get("active", False) and user["role"] != "guest"
#@schema param is_active.user: sometimes={role} only={role}


def totals(items):
    prices = list(map(price_of, items))
    n = len(prices)                               #@value n: exactly={2,0}
    s = sum(prices)                               #@value s: exactly={23,0}
    return s


def actives(users):
    kept = list(filter(is_active, users))
    names = [u["name"] for u in kept]             #@ ok name
    first = kept[0] if kept else {}
    #@row first: present={name,active,role} definite={} closed
    return names


def labels(users):
    out = list(map(lambda u: {"label": u["name"].upper(), **u}, users))
    row = out[0]
    #@row row: present={label,name,active,role} definite={label,name} closed
    row["label"]                                  #@ ok label
    return out


if __name__ == "__main__":
    totals([{"qty": 2, "unit": 4}, {"qty": 3, "unit": 5, "note": "x"}])
    totals([])
    actives([{"name": "a", "active": True, "role": "admin"}, {"name": "b", "active": False}])
    actives([{"name": "c", "role": "guest", "active": True}])
    labels([{"name": "ann", "active": True, "role": "dev"}])
    lb = labels([{"name": "bob"}])[0]["label"]    #@value lb: exactly={"BOB"}
