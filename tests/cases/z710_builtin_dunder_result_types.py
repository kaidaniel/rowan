# A builtin carried out by a dunder of the program's class gives a value of the builtin's type
# whatever the dunder returns (CPython raises otherwise): `len(b)` and `hash(b)` an int,
# `str(b)` / `repr(b)` / `format(b)` a str, `int(b)` an int, `float(b)` a float.
import os


class Box:
    def __init__(self, size, name):
        self.size = size
        self.name = name

    def __len__(self):
        return self.size

    def __str__(self):
        return self.name

    def __repr__(self):
        return self.name

    def __hash__(self):
        return self.size

    def __int__(self):
        return self.size

    def __float__(self):
        return self.size * 1.0


def use(b: Box):
    n = len(b)                                      #@general n: types={int}
    s = str(b)                                      #@general s: types={str}
    r = repr(b)                                     #@general r: types={str}
    h = hash(b)                                     #@general h: types={int}
    i = int(b)                                      #@general i: types={int}
    f = float(b)                                    #@general f: types={float}
    t = f"{b}"                                      #@general t: types={str}
    return n, s, r, h, i, f, t


box = Box(int(os.environ.get("SIZE", "3")), os.environ.get("NAME", "box"))
n = len(box)                                        #@value n: shows={3} types={int}
s = str(box)                                        #@value s: shows={"box"} types={str}
h = hash(box)                                       #@value h: shows={3} types={int}
use(box)
