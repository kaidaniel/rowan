"""Hypothesis: a factory called in a comprehension over `range(3)` makes three distinct
objects (a store on one is not seen on another); over an unknown range the element objects
still have the factory's keys.

Expectations were written from the program text before running the engine."""


def make(i):
    return {"i": i}


items = [make(i) for i in range(3)]
items[0]["tag"] = True
i1 = items[1]                                #@row i1: present={i} definite={i} closed
i0 = items[0]                                #@row i0: present={i,tag} definite={i,tag} closed
loose = [make(i) for i in range(n_in)]
l0 = loose[0]                                #@row l0: present={i} definite={i} closed
