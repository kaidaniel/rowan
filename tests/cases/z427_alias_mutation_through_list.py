"""Hypothesis: a dict reached through a second name or through a list that holds it is the same
object: a helper that stores into `holder[0]` or into every item of a list changes the caller's
`base`; a copy taken before the stores keeps the old keys. Precise: build(True) returns
{a, b, c, t}, build(False) {a, b, t}; the copy keeps {a} only; a runtime flag makes c possible."""
import json
import os


def add_key(holder, k):
    holder[0][k] = 1


def tag_all(items, name):
    for it in items:
        it[name] = True


def build(extra):
    base = {"a": 1}
    snap = dict(base)
    alias = base
    box = [base]
    add_key(box, "b")
    if extra:
        alias["c"] = 3
    tag_all(box, "t")
    return base, snap


def via_pairs():
    shared = {}
    pairs = [("x", shared), ("y", shared)]
    for name, target in pairs:
        target[name] = len(name)
    return shared


if __name__ == "__main__":
    extra = json.loads(os.environ.get("Z427_EXTRA", 'false'))
    r1, s1 = build(True)
    #@row r1: present={a,b,c,t} definite={a,b,c,t} closed
    _ = 0                                        #@row s1: present={a} definite={a} absent={b,c,t} closed
    b1 = r1["b"]                                 #@value b1: exactly={1}
    r2, s2 = build(False)
    #@row r2: present={a,b,t} definite={a,b,t} absent={c} closed
    n2 = len(r2)                                 #@value n2: exactly={3}
    r3, s3 = build(extra)
    #@row r3: present={a,b,c,t} definite={a,b,t} closed
    t3 = r3["t"]                                 #@value t3: exactly={True}
    sh = via_pairs()
    #@row sh: present={x,y} definite={x,y} closed
    shy = sh["y"]                                #@value shy: exactly={1}
