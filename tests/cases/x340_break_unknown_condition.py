"""Weakness: a loop that stores and breaks under an unknown condition: the key is possible.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""

found = {}
for x in xs_in:
    if pred(x):
        found["x"] = x
        break
found["x"]                                   #@ possibly x
found["x"]                                   #@ sound x
