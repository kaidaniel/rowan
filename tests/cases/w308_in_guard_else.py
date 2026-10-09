"""Hypothesis: `if "a" in c: c["a"] else: c["b"]` requires only b, and only sometimes.

Expectations were written from the program text before running the engine."""
import json

c = json.loads(text_in)
if "a" in c:
    x = c["a"]                               #@ ok a
else:
    x = c["b"]                               #@ input b
#@schema json.loads@L6: sometimes={b} only={b}
