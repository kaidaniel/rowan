"""Hypothesis: a dataclass field with `default_factory=dict` gives each instance its own dict,
so a store through one instance is not seen through another.

Expectations were written from the program text before running the engine."""

from dataclasses import dataclass, field


@dataclass
class Bag:
    name: str
    items: dict = field(default_factory=dict)


a = Bag("a")
b = Bag("b")
a.items["x"] = 1
ai = a.items                                 #@row ai: present={x} definite={x} closed
bi = b.items                                 #@row bi: present={} definite={} closed
b.items["x"]                                 #@ missing x
