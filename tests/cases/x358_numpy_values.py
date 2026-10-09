"""Weakness: unmodelled libraries. A dict holding numpy arrays keeps its own keys exact.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""
import numpy as np

d = {"xs": np.array([1, 2]), "ys": np.zeros(3)}
d["xs"]                                      #@ ok xs
d["zs"]                                      #@ missing zs
d["xs"].shape                                #@ sound shape
