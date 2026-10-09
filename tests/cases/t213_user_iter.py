"""Hypothesis: iterating an instance whose `__iter__` returns an iterator over known names
gives those names.

Expectations were written from the program text before running the engine."""


class Fields:
    def __init__(self, *names):
        self.names = list(names)

    def __iter__(self):
        return iter(self.names)


f = Fields("a", "b")
d = dict.fromkeys(f)                         #@row d: present={a,b} definite={a,b} closed
seen = {}
for n in f:
    seen[n] = 1
_ = 0                                        #@row seen: present={a,b} definite={a,b} closed
