"""Hypothesis: a class overriding `__setattr__` routes attribute assignments through it (here
into a dict made with `object.__setattr__` in `__init__`).

Expectations were written from the program text before running the engine."""


class Bag:
    def __init__(self):
        object.__setattr__(self, "data", {})

    def __setattr__(self, name, value):
        self.data[name] = value


b = Bag()
b.x = 1
b.y = 2
d = b.data                                   #@row d: present={x,y} definite={x,y} closed
