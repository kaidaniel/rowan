"""Every statement kind, with the record facts it should produce."""
import json
import os as operating_system
from json import loads as parse

# Assign / AnnAssign / AugAssign / chained
a = {"x": 1}
b: dict = {"y": 2}
c = a
c["z"] = 3
a["z"]                               #@ ok z
b["z"]                               #@ missing z
n = 1
n += 1
a["x"] += 5
a["x"]                               #@ ok x
a["q"] += 1                          #@ missing q
p = q = {"k": 1}
q["m"] = 2
p["m"]                               #@ ok m

# tuple / list targets
(d1, d2) = ({"a": 1}, {"b": 2})
[e1, e2] = [{"c": 1}, {"d": 2}]
d1["a"]                              #@ ok a
d2["a"]                              #@ missing a
e2["d"]                              #@ ok d
e1["d"]                              #@ missing d

# Delete
del a["x"]
a["x"]                               #@ missing x
a["z"]                               #@ ok z
del c
# Pass / Assert
pass
assert "z" in a
a["z"]                               #@ ok z

# If / elif / else
cfg = {"mode": "fast"}
if flag1:
    cfg["a"] = 1
elif flag2:
    cfg["b"] = 2
else:
    cfg["a"] = 3
    cfg["c"] = 4
cfg["mode"]                          #@ ok mode
cfg["a"]                             #@ possibly a
cfg["c"]                             #@ possibly c
cfg["d"]                             #@ missing d

# While / else
w = {"n": 0}
while w["n"] < 10:                   #@ ok n
    w["n"] += 1
    if w["n"] > 5:
        w["big"] = True
        break
else:
    w["done"] = True
# Corrected on re-reading (r140 unrolls `while` loops whose test is decided at every head): n reaches 6 > 5,
# which sets "big" and breaks, so the else never runs.
w["big"]                             #@ ok big
w["done"]                            #@ missing done
w["n"]                               #@ ok n

# For / else / continue
f = {}
for k in ["a", "b"]:
    if k == "a":
        f[k] = 1
        continue
    f["other"] = k
f["a"]                               #@ ok a
f["other"]                           #@ ok other
f["b"]                               #@ missing b

# Try / except / else / finally
t = {"a": 1}
try:
    # loads inside try/except KeyError are guarded: nothing is reported for them
    v = t["a"]                       #@ none a
    v2 = t["missing"]                #@ none missing
except KeyError:
    t["fallback"] = True
else:
    t["ok"] = True
finally:
    t["always"] = True
t["always"]                          #@ ok always
# t["missing"] definitely raises (closed record): the except branch runs, the else does not.
t["fallback"]                        #@ ok fallback
t["ok"]                              #@ missing ok

# Raise inside a branch: the rest of that path is dead
r = {"a": 1}
if cond:
    raise ValueError("no")
else:
    r["b"] = 2
r["b"]                               #@ ok b

# With
with open("f") as fh:
    cfg2 = json.load(fh)
cfg2["key"]                          #@ input key
#@schema json.load@L107: always={key}

# Global / Nonlocal
G = {"g": 1}


def set_global():
    global G
    G = {"h": 2}


def outer():
    inner_d = {"i": 1}

    def inner():
        nonlocal inner_d
        inner_d = {**inner_d, "j": 2}

    inner()
    return inner_d


set_global()
G["h"]                               #@ ok h
G["g"]                               #@ missing g
o = outer()
o["i"]                               #@ ok i
o["j"]                               #@ ok j
o["k"]                               #@ missing k

# Import forms
parsed = parse(text)
parsed["k"]                          #@ input k
env_home = operating_system.environ["HOME"]      #@ input HOME
