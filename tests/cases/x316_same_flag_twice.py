"""Weakness: correlated conditions. The same unknown flag tested twice: the key stored under
the first test is there under the second.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""

d = {}
if flag:
    d["a"] = 1
if flag:
    d["a"]                                   #@ ok a
    d["a"]                                   #@ sound a
d["a"]                                       #@ possibly a
