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
#@schema json.loads@L7: always={timeout} only={timeout}
# (L14 corrected from `none` for the `try` half: the guarded read raises a KeyError for timeout when it is
#  missing; caught, it still counts as required — the runtime oracle's rule. The `in` guard requires nothing.)
