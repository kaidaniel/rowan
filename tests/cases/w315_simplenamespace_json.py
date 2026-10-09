"""Hypothesis: `SimpleNamespace(**d)` turns the json dict's keys into attributes, so reading an
attribute requires the key.

Expectations were written from the program text before running the engine."""
import json
from types import SimpleNamespace

d = json.loads(text_in)
ns = SimpleNamespace(**d)
ns.host                                      #@ input host
#@schema json.loads@L8: always={host}
