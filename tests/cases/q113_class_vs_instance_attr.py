"""Hypothesis: a class-level dict is shared by instances (a store through one instance's
reference is seen by another), while an instance attribute assigned in `__init__` shadows
it per instance.

Expectations were written from the program text before running the engine."""


class Registry:
    shared = {}

    def __init__(self, name):
        self.own = {"name": name}

    def put(self, key):
        self.shared[key] = self.own["name"]
        self.own[key] = True


a = Registry("a")
b = Registry("b")
a.put("x")
b.put("y")
shared = Registry.shared                  #@row shared: present={x,y} definite={x,y} closed
a_own = a.own                             #@row a_own: present={name,x} definite={name,x} closed
b_own = b.own                             #@row b_own: present={name,y} definite={name,y} closed
a.own["y"]                                #@ missing y
