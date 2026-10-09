"""Weakness: path correlation with identity: after storing through `t`, the dict `t` is has
the key; `t is a` says which one.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""

a = {}
b = {}
t = a if cond else b
t["x"] = 1
if t is a:
    # Out of reach for now: the store through `t` is weak on both dicts, and `t is a` does
    # not bring back which one it hit (store/alias correlation, see h72 in AGENTS.md).
    a["x"]                                   #@todo ok x
    a["x"]                                   #@ sound x
