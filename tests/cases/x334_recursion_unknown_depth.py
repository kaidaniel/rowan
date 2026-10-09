"""Weakness: recursion of unknown depth. The result is a leaf or a node; neither key is
definite, both are possible, and a nested access is never "definitely missing".

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""


def build(n):
    if n == 0:
        return {"leaf": True}
    return {"child": build(n - 1), "depth": n}


t = build(n_in)                              #@row t: present={leaf,child,depth} definite={} closed
_ = 0                                        #@row t: possible={leaf,child,depth} definite_within={}
t["child"]                                   #@ possibly child
t["child"]                                   #@ sound child
