"""Hypothesis: a class whose __setattr__ stores into an inner dict gets no instance attributes
from `b.k = v` / setattr(): vars(b) stays {_data}; the keys land in b._data; __getattr__ reads
them back and turns a missing one into AttributeError (hasattr False). Precise: vars(bag) is
{_data}; bag._data is {ab, xyz, extra}; bag.ab is 2."""
import json
import os


class Bag:
    def __init__(self):
        object.__setattr__(self, "_data", {})

    def __setattr__(self, k, v):
        self._data[k] = v

    def __getattr__(self, k):
        try:
            return self._data[k]
        except KeyError:
            raise AttributeError(k)


def fill(b, keys):
    for k in keys:
        setattr(b, k, len(k))
    b.extra = 0
    return b


if __name__ == "__main__":
    bag = fill(Bag(), ["ab", "xyz"])
    #@row bag: present={_data} definite={_data} absent={ab,xyz,extra} closed
    d = bag._data
    #@row d: present={ab,xyz,extra} definite={ab,xyz,extra} closed
    v = bag.ab                                   #@value v: exactly={2}
    e = bag.extra                                #@value e: exactly={0}
    has = hasattr(bag, "zz")                     #@value has: exactly={False}
    inv = "xyz" in vars(bag)                     #@value inv: exactly={False}
    keys = json.loads(os.environ.get("Z430_KEYS", '["q"]'))
    bag2 = fill(Bag(), keys)
    #@row bag2: present={_data} definite={_data} absent={extra} closed
    e2 = bag2.extra                              #@value e2: exactly={0}
