"""Hypothesis: sorting json rows by `itemgetter("rank")` requires rank on every element.

Expectations were written from the program text before running the engine."""
import json
from operator import itemgetter

rows = json.loads(text_in)
ordered = sorted(rows, key=itemgetter("rank"))
#@schema json.loads@L7['*']: always={rank}
