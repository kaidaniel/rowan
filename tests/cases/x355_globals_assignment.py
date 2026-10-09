"""Weakness: `globals()["X"] = ...` defines a module name.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""

globals()["X"] = {"a": 1}
X["a"]                                       #@ ok a
X["a"]                                       #@ sound a
