def first(d):
    if "k" not in d:
        return None
    return d["k"]                               #@ ok k


def second(d):
    try:
        v = d["a"]                              #@ none a
    except KeyError:
        v = d["b"]                              #@ ok b
    return v


def third(d):
    if d.get("x") is not None and d["x"] > 1:   #@ ok x
        return d["x"]                           #@ ok x
    return d["y"]                               #@ missing y


first({"k": 1})
second({"a": 1, "c": 2})
second({"b": 1})
third({"x": 5})
third({"x": 0})

e = {"n": 0}
while e["n"] < 3:                               #@ ok n
    e["n"] += 1
    if e["n"] == 2:
        e["two"] = True
# Corrected on re-reading (r140 unrolls `while` loops whose test is decided at every head): n runs 1, 2, 3, so
# "two" is set at n == 2 on the only path.
e["two"]                                        #@ ok two
