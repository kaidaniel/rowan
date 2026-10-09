"""Weakness: stores on loop elements under an unknown condition: possible on each element.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""

rows = [{"id": 1}, {"id": 2}]
for r in rows:
    if pred(r):
        r["hit"] = True
rows[0]["hit"]                               #@ possibly hit
rows[1]["hit"]                               #@ sound hit
