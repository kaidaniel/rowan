"""Hypothesis: a recursive helper that stores one key per depth into the same dict leaves every
level's key (not only the outermost); a recursive builder nests one dict per level. Precise:
nest({}, 2) is {level2, level1, level0}; tree(2)["child"] is {depth, child} with depth 1 and
its child {leaf}; a runtime depth (a list's length, >= 0) always leaves level0."""
import json
import os


def nest(d, depth):
    d["level" + str(depth)] = depth
    if depth > 0:
        nest(d, depth - 1)
    return d


def tree(depth):
    if depth == 0:
        return {"leaf": True}
    return {"depth": depth, "child": tree(depth - 1)}


if __name__ == "__main__":
    r = nest({}, 2)
    #@row r: present={level2,level1,level0} definite={level2,level1,level0} closed
    l0 = r["level0"]                             #@value l0: exactly={0}
    one = nest({"base": 1}, 0)
    #@row one: present={base,level0} definite={base,level0} absent={level1} closed
    t = tree(2)
    c = t["child"]
    #@row c: present={depth,child} definite={depth,child} absent={leaf} closed
    cd = c["depth"]                              #@value cd: exactly={1}
    g = c["child"]
    #@row g: present={leaf} definite={leaf} absent={depth,child} closed
    items = json.loads(os.environ.get("Z434_ITEMS", '[1, 2]'))
    ru = nest({}, len(items))
    z = ru["level0"]                             #@ sound level0
    _ = 0                                        #@value z: includes={0}
    #@todo value z: exactly={0}  (and `ok level0` at L36: the recursion over an unknown depth ends at 0; the computed keys make ru a map)
