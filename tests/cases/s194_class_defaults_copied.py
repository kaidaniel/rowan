"""Hypothesis: `dict(self.DEFAULTS)` in `__init__` copies the class-level defaults per
instance; updating the copy with keywords does not touch the class dict or other instances.

Expectations were written from the program text before running the engine."""


class Svc:
    DEFAULTS = {"retries": 3}

    def __init__(self, **over):
        self.cfg = dict(self.DEFAULTS)
        self.cfg.update(over)


a = Svc(timeout=5)
b = Svc()
ac = a.cfg                                   #@row ac: present={retries,timeout} definite={retries,timeout} closed
bc = b.cfg                                   #@row bc: present={retries} definite={retries} closed
D = Svc.DEFAULTS                             #@row D: present={retries} definite={retries} closed
