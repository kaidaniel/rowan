"""Weakness: a loop over an unknown list stores a key; a later `if xs:` means it ran.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""

last = {}
for x in xs_in:
    last["v"] = x
if xs_in:
    last["v"]                                #@ ok v
    last["v"]                                #@ sound v
