"""Weakness: correlated conditions through a copy of the flag.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""

ok = flag
d = {}
if ok:
    d["a"] = 1
if ok:
    d["a"]                                   #@ ok a
    d["a"]                                   #@ sound a
