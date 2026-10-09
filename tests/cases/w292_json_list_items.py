"""Hypothesis: reads on the elements of a json list are requirements on its element path.

Expectations were written from the program text before running the engine."""
import json

data = json.loads(text_in)
ids = {}
for item in data["items"]:
    ids[item["id"]] = item.get("name")
#@schema json.loads@L6: always={items}
#@schema json.loads@L6['items']['*']: always={id} only={id}
