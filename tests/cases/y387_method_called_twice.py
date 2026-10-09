"""A method called twice on one instance: a snapshot inside grows.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


class Reg:
    def __init__(self):
        self.items = {}

    def add(self, k):
        self.items[k] = len(self.items)
        snap = dict(self.items)
        _ = 0                                #@row snap: present={x,y} definite={x} closed
        return snap


r = Reg()
r.add("x")
r.add("y")
