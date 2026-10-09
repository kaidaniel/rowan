"""Soundness of branch facts: the object a test reads changes between two identical tests,
so the first test's branch says nothing about the second.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""
import json

cfg = json.loads(t_in)
out = {}
if "x" in cfg:
    out["x"] = 1
cfg["x"] = 0
if "x" in cfg:
    out["x"]                                 #@ possibly x
    out["x"]                                 #@ sound x
