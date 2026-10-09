"""Hypothesis: reads on a copy of an input (`dict(src)`, `{**src, ...}`) are requirements on
the input, except keys the copy itself adds.

Expectations were written from the program text before running the engine."""
import json

src = json.loads(text_in)
cp = dict(src)
cp["x"]                                      #@ input x
merged = {**src, "y": 1}
merged["y"]                                  #@ ok y
merged["z"]                                  #@ input z
#@schema json.loads@L7: always={x,z} only={x,z}
