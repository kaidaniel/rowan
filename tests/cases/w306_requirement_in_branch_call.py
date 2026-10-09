"""Hypothesis: a requirement made by a callee that is called in one branch only is required
sometimes.

Expectations were written from the program text before running the engine."""
import json


def need_db(c):
    return c["db"]


cfg = json.loads(text_in)
if mode_in == "full":
    need_db(cfg)
cfg["name"]
#@schema json.loads@L12: always={name} sometimes={db}
