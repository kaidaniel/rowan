"""Hypothesis: a key read unconditionally from a json config is always required; one read
only when another (optional) key is truthy is required sometimes.

Expectations were written from the program text before running the engine."""
import json

with open("c.json") as fh:
    cfg = json.load(fh)
host = cfg["host"]                           #@ input host
if cfg.get("tls"):
    cert = cfg["cert"]                       #@ input cert
#@schema json.load@L8: always={host} sometimes={cert}
