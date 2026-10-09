"""Hypothesis: a classmethod alternative constructor `cls(**d)` builds an instance of the class
it is called on (a subclass too) with the keyword arguments' attributes.

Expectations were written from the program text before running the engine."""


class Cfg:
    def __init__(self, host, port=80):
        self.host = host
        self.port = port

    @classmethod
    def from_dict(cls, d):
        return cls(**d)


class Sub(Cfg):
    pass


c = Cfg.from_dict({"host": "h"})
c.port                                       #@ ok port
c.user                                       #@ missing user
s = Sub.from_dict({"host": "h", "port": 1})
kind = {type(s).__name__: 1}                 #@row kind: present={Sub} definite={Sub} closed
s.host                                       #@ ok host
