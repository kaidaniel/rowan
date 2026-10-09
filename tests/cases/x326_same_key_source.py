"""Weakness: relational facts. Two dicts built from the same unknown list have the same keys.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""

names = list(names_in)
a = {k: 0 for k in names}
b = {k: 1 for k in names}
for k in a:
    b[k]                                     #@ none
_ = 0                                        #@row b: definite_within={}
