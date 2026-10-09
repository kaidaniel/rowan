"""Hypothesis: a dataclass field with `init=False` set in `__post_init__` exists after
construction; `replace` on a frozen dataclass keeps the fields.

Expectations were written from the program text before running the engine."""

from dataclasses import dataclass, field, replace


@dataclass(frozen=True)
class Item:
    name: str
    qty: int = 1


@dataclass
class Order:
    items: list
    summary: dict = field(init=False)

    def __post_init__(self):
        self.summary = {"n": len(self.items)}


o = Order([Item("a")])
s = o.summary                                #@row s: present={n} definite={n} closed
i2 = replace(Item("b"), qty=3)
i2.qty                                       #@ ok qty
i2.price                                     #@ missing price
