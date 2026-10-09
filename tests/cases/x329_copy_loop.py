"""Weakness: relational facts. A key-by-key copy of an unknown dict has its keys.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""

d = dict(src_in)
d2 = {}
for k in d:
    d2[k] = d[k]
for k in d:
    d2[k]                                    #@ none
