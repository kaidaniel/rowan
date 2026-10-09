"""Weakness: recursion of unknown depth storing into an accumulator passed down: its keys come
from data, so nothing is definitely missing.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""
import json


def collect(n, acc):
    acc[n["name"]] = True
    for c in n.get("kids", []):
        collect(c, acc)
    return acc


names = collect(json.loads(text_in), {})
names["anything"]                            #@ sound anything
