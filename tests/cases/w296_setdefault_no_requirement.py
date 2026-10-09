"""Hypothesis: `setdefault` makes the key present without requiring it; `.get` requires
nothing.

Expectations were written from the program text before running the engine."""
import json

o = json.loads(text_in)
o.setdefault("k", 1)
o["k"]                                       #@ ok k
o.get("m")                                   #@ none m
#@schema json.loads@L7: none
