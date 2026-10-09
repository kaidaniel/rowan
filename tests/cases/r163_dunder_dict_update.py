"""Hypothesis: `self.__dict__.update(kw)` in `__init__` gives the instance one attribute per
keyword; `vars(obj)` is that attribute dict.

Expectations were written from the program text before running the engine."""


class Rec:
    def __init__(self, **kw):
        self.__dict__.update(kw)


r = Rec(a=1, b=2)
r.a                                          #@ ok a
r.c                                          #@ missing c
v = vars(r)                                  #@row v: present={a,b} definite={a,b} closed
