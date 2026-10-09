"""Weakness: relational facts. Counting over the same unknown keys the dict was made from.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""

keys = list(keys_in)
counts = {k: 0 for k in keys}
for k in keys:
    counts[k] += 1                           #@ none
