"""Weakness: correlated membership tests on an input: a key copied when the input has it is
there whenever the input has it.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""
import json

cfg = json.loads(text_in)
out = {}
if "x" in cfg:
    out["x"] = cfg["x"]
if "x" in cfg:
    out["x"]                                 #@ ok x
    out["x"]                                 #@ sound x
