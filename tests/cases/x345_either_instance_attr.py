"""Weakness: an attribute store through one of two instances: possible on each.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""


class Box:
    def __init__(self):
        self.items = {}


p = Box()
q = Box()
which = p if cond else q
which.items["k"] = 1
p.items["k"]                                 #@ possibly k
q.items["k"]                                 #@ sound k
