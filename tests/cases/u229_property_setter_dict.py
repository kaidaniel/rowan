"""Hypothesis: a property whose setter stores into a private dict and whose getter reads it:
after the assignment the key is there; on a fresh instance the getter's read is missing.

Expectations were written from the program text before running the engine."""


class Rec:
    def __init__(self):
        self._data = {}

    @property
    def name(self):
        return self._data["name"]

    @name.setter
    def name(self, v):
        self._data["name"] = v


r = Rec()
r.name = "x"
d = r._data                                  #@row d: present={name} definite={name} closed
r.name                                       #@ ok name
r2 = Rec()
r2.name                                      #@ missing name
