"""Weakness: a boolean set alongside the key in the same branch.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""

d = {}
has = False
if cond:
    d["k"] = 1
    has = True
if has:
    d["k"]                                   #@ ok k
    d["k"]                                   #@ sound k
