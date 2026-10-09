"""Weakness: a success flag set by a try/except around an input read.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""
import json

d = json.loads(text_in)
ok = True
try:
    v = d["k"]
except KeyError:
    ok = False
if ok:
    d["k"]                                   #@ ok k
    d["k"]                                   #@ sound k
