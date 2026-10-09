"""Soundness of loop facts: a list cleared after the loop is empty even if the loop ran, so
`if not ys:` does not mean the key is missing.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""

e = {}
ys = list(src_in)
for y in ys:
    e["y"] = y
ys.clear()
if not ys:
    e["y"]                                   #@ possibly y
    e["y"]                                   #@ sound y
