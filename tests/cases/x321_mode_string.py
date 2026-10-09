"""Weakness: correlated equality tests on an unknown string.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""

d = {}
if mode == "a":
    d["a"] = 1
elif mode == "b":
    d["b"] = 1
if mode == "a":
    d["a"]                                   #@ ok a
    d["a"]                                   #@ sound a
    # Corrected on re-reading: under `mode == "a"` the elif did not run, so "b" really is
    # missing; the `sound b` floor written here contradicted that and is dropped.
    d["b"]                                   #@ missing b
