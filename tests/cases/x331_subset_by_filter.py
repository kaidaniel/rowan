"""Weakness: relational facts. A filtered sub-dict's keys are keys of the original.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""

d = dict(src_in)
sub = {k: d[k] for k in d if pred(k)}
for k in sub:
    d[k]                                     #@ none
