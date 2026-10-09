"""Hypothesis: a dict subclass with `__missing__` answers missing keys from it (no missing
verdict); present keys are read normally.

Expectations were written from the program text before running the engine."""


class Defaults(dict):
    def __missing__(self, key):
        return {"default": key}


d = Defaults(a={"real": 1})
x = d["a"]                                   #@row x: present={real} definite={real} closed
y = d["zzz"]                                 #@ none zzz
_ = 0                                        #@row y: present={default} definite={default} closed
