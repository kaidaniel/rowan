# min / max with default= (empty inputs) and key= callbacks: the result is an element or the default


def newest(events):
    ev = max(events, key=lambda e: e["ts"], default={"ts": 0, "kind": "none"})
    #@row ev: present={ts,kind,user} definite={ts,kind} closed
    ev["kind"]                                    #@ ok kind
    return ev
#@schema param newest.events['*']: always={ts,kind} only={ts,kind}


def smallest_gap(nums):
    gap = min((b - a for a, b in zip(nums, nums[1:])), default=-1)
    return gap


def busiest(counts):
    name = max(counts, key=counts.get)
    return name


if __name__ == "__main__":
    a = newest([{"ts": 5, "kind": "login", "user": "u1"}, {"ts": 9, "kind": "logout", "user": "u2"}])
    ak = a["kind"]                                #@value ak: exactly={"logout"}
    b = newest([])                                #@row b: present={ts,kind} definite={ts,kind} closed
    bk = b["kind"]                                #@value bk: exactly={"none"}
    newest([{"ts": 1, "kind": "ping"}])
    g = smallest_gap([1, 4, 6])                   #@value g: exactly={2}
    g2 = smallest_gap([7])                        #@value g2: exactly={-1}
    hi = max(3, 8, 5)                             #@value hi: exactly={8}
    lo = min("pear", "apple")                     #@value lo: exactly={"apple"}
    w = busiest({"a": 1, "b": 3, "c": 2})         #@value w: exactly={"b"}
