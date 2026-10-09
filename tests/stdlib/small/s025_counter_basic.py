# Counter: keys come from the counted values; a missing key reads 0 (no KeyError); most_common
from collections import Counter


def tally(words):
    return Counter(words)


def by_status(orders):
    c = Counter(o["status"] for o in orders)     #@ ok status
    return c
#@schema param by_status.orders['*']: always={status} only={status}


if __name__ == "__main__":
    c = tally(["a", "b", "a"])                   #@row c: present={a,b} definite={a,b} closed
    n = c["zzz"]                                 #@ ok zzz
    #@value n: exactly={0}
    top = tuple(c.most_common(1))                #@value top: exactly={(("a",2),)}
    s = by_status([{"status": "paid"}, {"status": "open", "id": 2}, {"status": "paid"}])
    #@row s: present={paid,open} definite={paid,open} closed
    p = s["paid"]                                #@value p: exactly={2}
    e = by_status([])                            #@row e: present={} closed
    kw = Counter(x=1, y=2)                       #@row kw: present={x,y} definite={x,y} closed
    tot = kw.total()                             #@value tot: exactly={3}
    del kw["x"]
    _ = 0                                        #@row kw: present={y} definite={y} absent={x} closed
    el = "".join(sorted(Counter("aab").elements()))  #@value el: exactly={"aab"}
