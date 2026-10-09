"""Weakness: dynamic code. `exec` may define or change names and records: a dict it can see is
not claimed closed afterwards.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""

d = {"a": 1}
exec("d['b'] = 2")
d["b"]                                       #@ sound b
d["a"]                                       #@ sound a
