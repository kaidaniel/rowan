"""Weakness: relational facts. `dict(zip(names, vals))` with the same unknown names: each name
is a key (the lengths are assumed equal, design case d30).

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""

names = list(names_in)
d = dict(zip(names, vals_in))
for n in names:
    d[n]                                     #@ none
