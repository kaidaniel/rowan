"""Hypothesis: bound methods of different instances stored in a dict and called through it
act on their own instances.

Expectations were written from the program text before running the engine."""


class Counter:
    def __init__(self, name):
        self.name = name
        self.hits = {}

    def hit(self, key):
        self.hits[key] = self.hits.get(key, 0) + 1
        return self


a = Counter("a")
b = Counter("b")
handlers = {"first": a.hit, "second": b.hit}
handlers["first"]("x")
handlers["second"]("y")
handlers["first"]("z")
a_hits = a.hits                           #@row a_hits: present={x,z} definite={x,z} closed
b_hits = b.hits                           #@row b_hits: present={y} definite={y} closed
a.hits["y"]                               #@ missing y
