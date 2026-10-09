"""Hypothesis: `"{host}:{port}".format(**cfg)` reads host and port from cfg.

Expectations were written from the program text before running the engine."""
import json

cfg = json.loads(text_in)
addr = "{host}:{port}".format(**cfg)
#@schema json.loads@L6: always={host,port}
