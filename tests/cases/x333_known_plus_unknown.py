"""Weakness: relational facts. A dict with a literal key plus unknown ones keeps the literal
key definite and stays open.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""

d = {"id": 1}
for k in extra_in:
    d[k] = True
d["id"]                                      #@ ok id
_ = 0                                        #@row d: definite={id} open
