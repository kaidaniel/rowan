"""Hypothesis: dict-building idioms keep exact keys and values: `{**a, **b, "k": …}` keeps the
first position of an overridden key and the last value; chained `setdefault(…).setdefault(…)`
nests; a `defaultdict(lambda: {...})` gives each new key its own fresh inner dict; `dict(zip())`
stops at the shorter list; a store into `ChainMap.new_child(over)` writes into `over` (the
caller's dict gains the key) and leaves the parent map alone. A user dict from input keeps the
default keys definite and opens the row."""
import json
import os
from collections import ChainMap, defaultdict

DEFAULTS = {"color": "red", "size": 1, "debug": False}


def merge(user):
    return {**DEFAULTS, **user, "size": max(DEFAULTS["size"], user.get("size", 0))}


def index(rows):
    idx = {}
    for r in rows:
        idx.setdefault(r["g"], {}).setdefault(r["k"], []).append(r["v"])
    return idx


def stats(pairs):
    acc = defaultdict(lambda: {"n": 0, "sum": 0})
    for k, v in pairs:
        acc[k]["n"] += 1
        acc[k]["sum"] += v
    return dict(acc)


def layered(over):
    child = ChainMap(DEFAULTS).new_child(over)
    child["debug"] = True
    return child["color"], DEFAULTS["debug"], dict(child)


if __name__ == "__main__":
    m1 = merge({"color": "blue", "size": 3})
    #@row m1: present={color,size,debug} definite={color,size,debug} closed
    c1 = m1["color"]                             #@value c1: exactly={"blue"}
    m2 = merge({"x": 1})
    order = tuple(m2)                            #@value order: exactly={("color","size","debug","x")}
    sz = m2["size"]                              #@value sz: exactly={1}
    ix = index([{"g": "a", "k": "x", "v": 1}, {"g": "a", "k": "x", "v": 2}, {"g": "b", "k": "y", "v": 3}])
    #@row ix: present={a,b} definite={a,b} closed
    ia = ix["a"]
    #@row ia: present={x} definite={x} absent={y} closed
    nx = len(ia["x"])                            #@value nx: exactly={2}
    st = stats([("p", 2), ("q", 5), ("p", 4)])
    #@row st: present={p,q} definite={p,q} closed
    psum = st["p"]["sum"]                        #@value psum: exactly={6}
    qn = st["q"]["n"]                            #@value qn: exactly={1}
    z = dict(zip(["a", "b", "c"], [1, 2]))
    #@row z: present={a,b} definite={a,b} absent={c} closed
    ov = {"color": "green"}
    col, base_dbg, flat = layered(ov)            #@value col: exactly={"green"} ; base_dbg: exactly={False}
    _ = 0                                        #@row ov: present={color,debug} definite={color,debug} closed
    fl = tuple(flat)                             #@value fl: exactly={("color","size","debug")}
    m3 = merge(json.loads(os.environ.get("USER_CFG", '{"size": 5, "theme": "dark"}')))
    #@schema json.loads@L61: only={}
    _ = 0                                        #@row m3: present={color,size,debug} definite={color,size,debug} open
