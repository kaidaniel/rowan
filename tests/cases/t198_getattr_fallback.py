"""Hypothesis: a class with `__getattr__` answers attributes it does not have: no missing
verdict, and the value is what `__getattr__` returns.

Expectations were written from the program text before running the engine."""


class Lazy:
    def __init__(self):
        self.known = {"k": 1}

    def __getattr__(self, name):
        return {"name": name}


o = Lazy()
o.known                                      #@ ok known
v = o.other                                  #@ none other
_ = 0                                        #@row v: present={name} definite={name} closed
