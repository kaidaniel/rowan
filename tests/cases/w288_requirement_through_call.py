"""Hypothesis: keys a callee reads from its parameter become requirements on the json value
passed in.

Expectations were written from the program text before running the engine."""
import json


def area(shape):
    return shape["w"] * shape["h"]


data = json.loads(text_in)
a = area(data)                               #@ input w ; input h
#@schema json.loads@L12: always={w,h}
