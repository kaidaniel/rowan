"""Hypothesis: a data descriptor's `__get__` / `__set__` run on attribute access and
assignment through the instance (storing into the instance's `__dict__`).

Expectations were written from the program text before running the engine."""


class Field:
    def __set_name__(self, owner, name):
        self.name = name

    def __get__(self, obj, objtype=None):
        return obj.__dict__.get(self.name, {"default": True})

    def __set__(self, obj, value):
        obj.__dict__[self.name] = value


class Model:
    tag = Field()


m = Model()
before = m.tag                               #@row before: present={default} definite={default} closed
m.tag = {"set": 1}
after = m.tag                                #@row after: present={set} definite={set} closed
