"""Hypothesis: two json values have separate schemas.

Expectations were written from the program text before running the engine."""
import json

a = json.loads(ta)
b = json.loads(tb)
a["x"]                                       #@ input x
b["y"]                                       #@ input y
#@schema json.loads@L6: always={x} only={x}
#@schema json.loads@L7: always={y} only={y}
