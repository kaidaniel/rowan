"""Nested loops with break/continue/else, early returns, flags, try inside loops, while with
compound conditions."""


def find(items, want):
    for it in items:
        if it["id"] == want:             #@ ok id
            return it
    return None


found = find([{"id": 1, "a": 1}, {"id": 2, "b": 2}], 2)
found["id"]                              #@ ok id
found["a"]                               #@ missing a


def scan(rows):
    out = {"n": 0}
    for r in rows:
        if "skip" in r:
            continue
        if r.get("stop"):
            out["stopped"] = True
            break
        out["n"] += 1
    else:
        out["complete"] = True
    return out


s = scan([{"v": 1}, {"skip": 1}, {"stop": True}])
s["n"]                                   #@ ok n
s["stopped"]                             #@ ok stopped
s["complete"]                            #@ missing complete
s2 = scan(rows)
s2["n"]                                  #@ ok n
s2["stopped"]                            #@ possibly stopped
s2["complete"]                           #@ possibly complete


def nested(grid):
    hit = {"found": False}
    for row in grid:
        for cell in row:
            if cell["x"] > 0:            #@ input x
                hit["cell"] = cell
                hit["found"] = True
                break
        else:
            continue
        break
    return hit


h = nested(g)
h["found"]                               #@ ok found
h["cell"]                               #@ possibly cell


def early(d):
    if "a" not in d:
        return {"err": "no a"}
    if d["a"] < 0:                       #@ ok a
        return {"err": "neg"}
    return {"ok": d["a"]}


e1 = early({"a": 1})
# 1 < 0 is decided (comparisons between known numbers are evaluated): only the ok return.
e1["ok"]                                 #@ ok ok
e1["err"]                                #@ missing err
e2 = early({})
e2["err"]                                #@ ok err
e2["ok"]                                 #@ missing ok
e3 = early(unknown)
e3["ok"]                                 #@ possibly ok


def retry(fn):
    result = {"attempts": 0}
    while result["attempts"] < 3:        #@ ok attempts
        result["attempts"] += 1
        try:
            result["value"] = fn()
            break
        except Exception:
            result["error"] = True
            continue
    return result


rr = retry(lambda: 1)
rr["attempts"]                           #@ ok attempts
rr["value"]                              #@ possibly value
# The lambda never raises, so the except branch is dead (the callee reported no raise).
rr["error"]                              #@ missing error
rr["other"]                              #@ missing other

state = {"phase": 0}
while state["phase"] < 2 and not done:   #@ ok phase
    if state["phase"] == 0:
        state["init"] = True
    state["phase"] += 1
state["init"]                            #@ possibly init
state["phase"]                           #@ ok phase

flags = {}
for name in ["a", "b", "c"]:
    if name == "b":
        continue
    flags[name] = True
flags["a"]                               #@ ok a
flags["c"]                               #@ ok c
flags["b"]                               #@ missing b

first = {}
for name in ["x", "y"]:
    first[name] = 1
    break
first["x"]                               #@ ok x
first["y"]                               #@ missing y
