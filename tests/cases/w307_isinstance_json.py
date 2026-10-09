"""Hypothesis: a read under `isinstance(v, dict)` on a json value is required on that path
only.

Expectations were written from the program text before running the engine."""
import json

v = json.loads(text_in)
if isinstance(v, dict):
    v["k"]                                   #@ input k
#@schema json.loads@L7: sometimes={k}
