# sorted / min / max with key= and default= over records; the key callback reads each record


def by_price(items):
    ranked = sorted(items, key=lambda r: r["price"])
    cheapest = min(items, key=lambda r: r["price"], default=None)
    return ranked, cheapest
#@schema param by_price.items['*']: always={price} only={price}


def top(items):
    best = max(items, key=lambda r: r.get("score", 0), default={"name": "none", "score": 0})
    _ = 0                                        #@row best: present={name,score,price} definite={name,score} closed
    best["name"]                                 #@ ok name
    return best


def literals():
    s = tuple(sorted(["pear", "fig", "apple"], key=len))   #@value s: exactly={("fig","pear","apple")}
    m = max([3, 9, 4])                              #@value m: exactly={9}
    e = min([], default=-1)                         #@value e: exactly={-1}
    r = tuple(sorted({"b": 1, "a": 2}, reverse=True))      #@value r: exactly={("b","a")}
    k = max({"x": 1, "y": 5}.items(), key=lambda kv: kv[1])[0]  #@value k: exactly={"y"}
    return s


if __name__ == "__main__":
    rows = [{"name": "a", "price": 3, "score": 1}, {"name": "b", "price": 1}]
    ranked, cheap = by_price(rows)
    #@row cheap: present={name,price} definite={name,price} closed
    by_price([{"name": "c", "price": 2}])
    top(rows)
    top([])
    literals()
