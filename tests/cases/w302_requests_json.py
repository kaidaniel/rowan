"""Hypothesis: `requests.get(...).json()` is an input whose reads are requirements.

Expectations were written from the program text before running the engine."""
import requests

r = requests.get("https://example.invalid/api")
data = r.json()
items = data["items"]                        #@ input items
#@schema requests.get@L6: always={items}
