"""Hypothesis: keys read in the two branches of an unknown test are required sometimes, a key
read after the branches always.

Expectations were written from the program text before running the engine."""
import json

cfg = json.loads(text_in)
if flag_in:
    a = cfg["a"]
else:
    b = cfg["b"]
c = cfg["c"]
#@schema json.loads@L7: always={c} sometimes={a,b}
