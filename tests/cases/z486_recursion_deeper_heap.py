"""Hypothesis: a recursive call made from a heap the recursion changed (a dict that grows with
each level) may take paths the first level's heap rules out: the key the deepest level
stores is there after the call. Rowan's running activation answers its recursive calls, so
it starts again from the heaps they are made in."""


def fill(d):
    if len(d) >= 6:
        d["z"] = 1
        return
    d[len(d)] = 1
    fill(d)


def depth(node):
    if not node["kids"]:
        return 1
    return 1 + max(depth(k) for k in node["kids"])


r = {}
fill(r)
z = r["z"]                                       #@ sound z
#@value z: includes={1}
tree = {"kids": [{"kids": []}, {"kids": [{"kids": []}]}]}
dd = depth(tree)                                 #@value dd: includes={3}
