"""Hypothesis: a recursive builder to a known depth yields nested dicts whose keys at each
level are known (the leaf has no child).

Expectations were written from the program text before running the engine."""


def build(depth):
    node = {"depth": depth}
    if depth > 0:
        node["child"] = build(depth - 1)
    else:
        node["leaf"] = True
    return node


t = build(2)                              #@row t: present={depth,child} definite={depth,child} closed
c1 = t["child"]                           #@row c1: present={depth,child} definite={depth,child} closed
c2 = c1["child"]                          #@row c2: present={depth,leaf} definite={depth,leaf} closed
t["leaf"]                                 #@ missing leaf
c2["child"]                               #@ missing child
