"""Hypothesis: reads through nested json objects give a schema per access path.

Expectations were written from the program text before running the engine."""
import json

cfg = json.loads(text_in)
db = cfg["db"]
host = db["host"]
port = cfg["db"]["port"]
#@schema json.loads@L6: always={db}
#@schema json.loads@L6['db']: always={host,port}
