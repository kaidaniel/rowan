"""Hypothesis: two closures from two calls of one factory have their own captured dicts.

Expectations were written from the program text before running the engine."""


def make_counter():
    counts = {}

    def bump(k):
        counts[k] = counts.get(k, 0) + 1
        return counts
    return bump


c1 = make_counter()
c2 = make_counter()
c1("a")
r2 = c2("b")                                 #@row r2: present={b} definite={b} closed
r1 = c1("c")                                 #@row r1: present={a,c} definite={a,c} closed
