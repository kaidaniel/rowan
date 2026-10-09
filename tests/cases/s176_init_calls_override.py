"""Hypothesis: a base `__init__` that calls `self.setup()` runs the subclass override, so
attributes and keys it adds exist on subclass instances only.

Expectations were written from the program text before running the engine."""


class Base:
    def __init__(self):
        self.cfg = {"base": True}
        self.setup()

    def setup(self):
        pass


class Web(Base):
    def setup(self):
        self.cfg["port"] = 80
        self.routes = {}


b = Base()
w = Web()
bc = b.cfg                                   #@row bc: present={base} definite={base} closed
wc = w.cfg                                   #@row wc: present={base,port} definite={base,port} closed
b.routes                                     #@ missing routes
w.routes                                     #@ ok routes
