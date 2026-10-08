"""dict / set methods: pop, popitem, clear, copy, fromkeys, keys/values/items views,
setdefault chains, key-set arithmetic."""
d = {"a": 1, "b": 2, "c": 3}
d.pop("a")                               #@ ok a
d["a"]                                   #@ missing a
d.pop("zz", None)                        #@ none zz
d.popitem()
d["b"]                                   #@ possibly b
d2 = {"x": 1}
d2.clear()
d2["x"]                                  #@ missing x
d3 = dict.fromkeys(["p", "q"], 0)
d3["p"]                                  #@ ok p
d3["r"]                                  #@ missing r
d4 = {"k": 1}.copy()
d4["k"]                                  #@ ok k
d4["j"]                                  #@ missing j
e = {"m": 1, "n": 2}
for k in e.keys():
    e[k]                                 #@ ok m ; ok n
for v in e.values():
    pass
ks = list(e.keys())
# list(e.keys()) keeps the literal order: ks[0] is "m"
e[ks[0]]                                 #@ ok m ; none n
if "m" in e.keys():
    e["m"]                               #@ ok m
inv = {v: k for k, v in e.items()}
inv[1]                                   #@ ok 1
inv["m"]                                 #@ missing m
missing = set(["m", "zz"]) - set(e)
need = {"m", "n"} <= e.keys()
if {"m", "n"} <= e.keys():
    e["n"]                               #@ ok n
chain = {}
chain.setdefault("l1", {}).setdefault("l2", {})["l3"] = 1
chain["l1"]["l2"]["l3"]                  #@ ok l1 ; ok l2 ; ok l3
chain["l1"]["l3"]                        #@ missing l3
g = {"a": {"b": 1}}
g.get("a", {}).get("b")                  #@ none b
g.get("x", {}).get("b")                  #@ none b
up = {"a": 1}
up |= {"b": 2}
up["b"]                                  #@ ok b
up["c"]                                  #@ missing c
