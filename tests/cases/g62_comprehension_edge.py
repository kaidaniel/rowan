#@engine rust
"""Comprehension corner cases: nested generators, conditions, walrus, unpacking targets."""
pairs = [("a", {"v": 1}), ("b", {"v": 2, "w": 3})]
by_name = {name: rec for name, rec in pairs}
by_name["a"]["v"]                        #@ ok a ; ok v
by_name["b"]["w"]                        #@ ok b ; ok w
by_name["a"]["w"]                        #@ missing w
only_w = {name: rec for name, rec in pairs if "w" in rec}
only_w["b"]                              #@ any b
flat = [cell for row in [[{"c": 1}], [{"c": 2}]] for cell in row]
flat[0]["c"]                             #@ ok c
flat[0]["d"]                             #@ missing d
sq = {k: {"sq": v["v"] ** 2} for k, v in by_name.items()}      #@ ok v
sq["a"]["sq"]                            #@ ok sq
sq["a"]["v"]                             #@ missing v
walrus = [y for x in [{"n": 1}] if (y := x["n"])]             #@ ok n
keys_upper = {k.upper(): v for k, v in by_name.items()}
keys_upper["A"]["v"]                     #@ ok A ; ok v
keys_upper["a"]                          #@ missing a
dropped = {k: v for k, v in by_name.items() if k not in ("a",)}
dropped["b"]                             #@ ok b
dropped["a"]                             #@ missing a
copies = [dict(r) for r in [{"p": 1}]]
copies[0]["p"]                           #@ ok p
copies[0]["q"]                           #@ missing q
merged = {**{k: 1 for k in ["x", "y"]}, "z": 2}
merged["x"]                              #@ ok x
merged["z"]                              #@ ok z
merged["w"]                              #@ missing w
gen_sum = sum(r["v"] for r in by_name.values())              #@ ok v
nested = {outer: {inner: 0 for inner in ["i"]} for outer in ["o"]}
nested["o"]["i"]                         #@ ok o ; ok i
nested["o"]["j"]                         #@ missing j
