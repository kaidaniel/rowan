"""Weakness: relational facts. `dict.fromkeys(a)` has exactly a's keys.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""

a = dict(src_in)
b = dict.fromkeys(a, 0)
for k in a:
    b[k]                                     #@ none
