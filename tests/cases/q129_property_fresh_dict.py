"""Hypothesis: a property that builds a new dict on each access gives independent dicts:
a key added to one access's result is not on the next access's result.

Expectations were written from the program text before running the engine."""


class Conf:
    def __init__(self):
        self.base = {"a": 1}

    @property
    def view(self):
        return dict(self.base)


c = Conf()
v1 = c.view
v1["extra"] = True
v2 = c.view                               #@row v2: present={a} definite={a} closed
_ = 0                                     #@row v1: present={a,extra} definite={a,extra} closed
base = c.base                             #@row base: present={a} definite={a} closed
v2["extra"]                               #@ missing extra
