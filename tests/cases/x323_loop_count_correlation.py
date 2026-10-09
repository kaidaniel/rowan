"""Weakness: a loop over `range(n)` with unknown n stores a key; `n > 0` later means it ran.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""

y = {}
for i in range(n_in):
    y["k"] = i
if n_in > 0:
    y["k"]                                   #@ ok k
    y["k"]                                   #@ sound k
