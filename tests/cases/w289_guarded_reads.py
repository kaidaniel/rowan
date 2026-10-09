"""Hypothesis: a read guarded by `in` or by `try/except KeyError` puts no requirement on the
input.

Expectations were written from the program text before running the engine."""
import json

cfg = json.loads(text_in)
if "debug" in cfg:
    d = cfg["debug"]                         #@ ok debug
try:
    t = cfg["timeout"]                       #@ none timeout
except KeyError:
    t = 30
#@schema json.loads@L7: none
