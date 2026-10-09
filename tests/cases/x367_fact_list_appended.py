"""Soundness of loop facts: an unknown list appended to after the loop is non-empty even if
the loop did not run, so `if xs:` does not mean the key was stored.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""

d = {}
for v in xs_in:
    d["v"] = v
xs_in.append(1)
if xs_in:
    d["v"]                                   #@ possibly v
    d["v"]                                   #@ sound v
