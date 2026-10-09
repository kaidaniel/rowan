"""Weakness: relational facts. `{**a, **b}` has every key of a and of b.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""

a = dict(a_in)
b = dict(b_in)
merged = {**a, **b}
for k in a:
    merged[k]                                #@ none
for k in b:
    merged[k]                                #@ none
