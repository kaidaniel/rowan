"""Weakness: a callee storing into one of two dicts passed under an unknown condition.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""


def mark(d):
    d["marked"] = True
    return d


a = {}
b = {}
r = mark(a if cond else b)
# Out of reach for now: the callee's store reaches one of two dicts (weak on each) and `r` is
# that same dict, which the weak stores do not record (store/alias correlation, h72).
r["marked"]                                  #@todo ok marked
r["marked"]                                  #@ sound marked
a["marked"]                                  #@ possibly marked
b["marked"]                                  #@ sound marked
