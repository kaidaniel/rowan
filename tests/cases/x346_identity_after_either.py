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
    # (`t is a` makes t that dict, and the key stored through t is then a's: the name's keys
    # hold on whichever object it is)
    a["x"]                                   #@ ok x
    a["x"]                                   #@ sound x
