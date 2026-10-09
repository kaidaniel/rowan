"""Hypothesis: `hasattr(o, "__dataclass_fields__")` is true for dataclass instances (the
decorator sets it on the class) and false for plain dicts, so each call takes its own path.

Expectations were written from the program text before running the engine."""

from dataclasses import asdict, dataclass


@dataclass
class P:
    x: int


def to_dict(o):
    if hasattr(o, "__dataclass_fields__"):
        return asdict(o)
    return dict(o)


a = to_dict(P(1))                            #@row a: present={x} definite={x} closed
b = to_dict({"y": 2})                        #@row b: present={y} definite={y} closed
