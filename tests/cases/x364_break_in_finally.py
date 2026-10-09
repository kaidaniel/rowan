"""Weakness: model corner. `break` in a `finally` swallows the exception: the code after the
loop runs.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""

d = {}
for x in [1, 2]:
    try:
        d["tried"] = x
        raise ValueError("x")
    finally:
        break
after = {"reached": True}                    #@row after: present={reached} definite={reached} closed
d["tried"]                                   #@ ok tried
d["tried"]                                   #@ sound tried
