"""Weakness: correlated `is not None` tests on the same value.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""
import json

cfg = json.loads(text_in)
v = cfg.get("v")
d = {}
if v is not None:
    d["v"] = v
if v is not None:
    d["v"]                                   #@ ok v
    d["v"]                                   #@ sound v
