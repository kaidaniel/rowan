"""Hypothesis: assigning through a property setter stores what the setter stores (here _c and
_k), never an instance attribute named after the property; reading goes through the getter.
Precise: vars(t) is {_c, _k} (no `celsius`); after warm(t, 5) on 20 the getter gives 25 and
_k is 298."""
import json
import os


class Temp:
    def __init__(self, c):
        self.celsius = c

    @property
    def celsius(self):
        return self._c

    @celsius.setter
    def celsius(self, v):
        self._c = v
        self._k = v + 273


def warm(t, by):
    t.celsius += by
    return t


def describe(t):
    return {"c": t.celsius, "k": t._k}


if __name__ == "__main__":
    t = Temp(20)
    #@row t: present={_c,_k} definite={_c,_k} absent={celsius} closed
    warm(t, 5)
    c = t.celsius                                #@value c: exactly={25}
    k = t._k                                     #@value k: exactly={298}
    inv = "celsius" in vars(t)                   #@value inv: exactly={False}
    d = describe(t)
    #@row d: present={c,k} definite={c,k} closed
    by = json.loads(os.environ.get("Z431_BY", '3'))
    t2 = warm(Temp(0), by)
    #@row t2: present={_c,_k} definite={_c,_k} absent={celsius} closed
    t3 = Temp(-273)
    k3 = t3._k                                   #@value k3: exactly={0}
