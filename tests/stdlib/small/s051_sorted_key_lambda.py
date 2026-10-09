# sorted / min / max with key= lambdas reading dict keys: the lambda sees each record the caller passed


def top(rows):
    ranked = sorted(rows, key=lambda r: r["score"], reverse=True)
    best = ranked[0]
    #@row best: present={name,score,team} definite={name,score} closed
    n = len(ranked)                               #@value n: exactly={2,3}
    best["name"]                                  #@ ok name
    best["team"]                                  #@ possibly team
    #@row best: present={name,score,team} definite={name,score,team} closed
    return best
#@schema param top.rows['*']: always={name,score,team} only={name,score,team}


def cheapest(items):
    lo = min(items, key=lambda it: it["price"])
    hi = max(items, key=lambda it: it["price"])
    #@row lo: present={sku,price} definite={sku,price} closed
    spread = hi["price"] - lo["price"]            #@value spread: exactly={4,0}
    hi["sku"]                                     #@ ok sku
    return lo
#@schema param cheapest.items['*']: always={price,sku} only={price,sku}


def by_len(words):
    longest = max(words, key=len)                 #@value longest: exactly={"pear","banana"}
    first = sorted(words, key=lambda w: (len(w), w))[0]  #@value first: exactly={"fig"}
    return longest, first


if __name__ == "__main__":
    try:
        top([{"name": "a", "score": 3}, {"name": "b", "score": 5}])
    except KeyError:
        pass
    top([{"name": "c", "score": 1, "team": "x"}, {"name": "d", "score": 0, "team": "y"}, {"name": "e", "score": 9, "team": "z"}])
    cheapest([{"sku": "x", "price": 3}, {"sku": "y", "price": 7}])
    cheapest([{"sku": "z", "price": 2}])
    by_len(["fig", "pear", "kiwi"])
    by_len(["banana", "fig"])
