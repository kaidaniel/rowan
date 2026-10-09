"""Hypothesis: a dict placed in two containers and mutated inside a function reached through
one container is updated for reads through the other container.

Expectations were written from the program text before running the engine."""


def stamp(container, key):
    container[key]["stamped"] = True
    return container[key]


rec = {"id": 1}
by_id = {1: rec}
queue = [rec]
stamp(by_id, 1)
first = queue[0]                          #@row first: present={id,stamped} definite={id,stamped} closed
other = {"id": 2}
by_id[2] = other
stamp(by_id, 2)
_ = 0                                     #@row other: present={id,stamped} definite={id,stamped} closed
_ = 0                                     #@row rec: present={id,stamped} definite={id,stamped} closed
