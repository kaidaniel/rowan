"""Hypothesis: `User(**data)` with a json dict needs the fields without defaults (a missing one
is a TypeError), and the instance has every field.

Expectations were written from the program text before running the engine."""
import json
from dataclasses import dataclass


@dataclass
class User:
    name: str
    age: int = 0


data = json.loads(text_in)
u = User(**data)
u.name                                       #@ ok name
u.age                                        #@ ok age
#@schema json.loads@L15: always={name} only={name}
