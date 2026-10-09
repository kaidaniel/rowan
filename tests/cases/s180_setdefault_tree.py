"""Hypothesis: building a tree from known paths with `node = node.setdefault(part, {})` walks
the right nested dicts (aliasing through the `node` variable).

Expectations were written from the program text before running the engine."""

PATHS = ["a/b", "a/c", "d"]
tree = {}
for p in PATHS:
    node = tree
    for part in p.split("/"):
        node = node.setdefault(part, {})
_ = 0                                        #@row tree: present={a,d} definite={a,d} closed
a = tree["a"]                                #@row a: present={b,c} definite={b,c} closed
leaf = a["b"]                                #@row leaf: present={} definite={} closed
dd = tree["d"]                               #@row dd: present={} definite={} closed
