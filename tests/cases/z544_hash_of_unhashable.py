"""`hash` raises TypeError for an unhashable value: a list, dict or set, or a tuple holding
one, whatever the items are. A value of unknown type may be a list (`s`, an environment
variable, is a str, but rowan does not know that); `name`, the result of `str`, is a str.

Expectations were written from the program text before running the engine."""
import os
from dataclasses import dataclass

s = os.environ.get("Z544_S", "a")
name = str(len(s))                       #@value name: types={str}


def tag(f, x):
    try:
        f(x)
        return "hashed"
    except TypeError:
        return "unhashable"


r1 = tag(hash, [s])                      #@value r1: exactly={"unhashable"}
r2 = tag(hash, (s, [1]))                 #@value r2: exactly={"unhashable"}
r3 = tag(hash, (name, "b"))                 #@value r3: exactly={"hashed"}
r4 = tag(hash, {s: 1})                   #@value r4: exactly={"unhashable"}


def probe(x):
    try:
        hash(x)
        return "hashed"
    except TypeError:
        return "unhashable"


p1 = probe([s])                          #@value p1: exactly={"unhashable"}
p2 = probe((s, {s}))                     #@value p2: exactly={"unhashable"}
p3 = probe((1, 2))                       #@value p3: exactly={"hashed"}
p4 = probe(frozenset([s]))               #@value p4: exactly={"hashed"}
p5 = probe(json_like := eval(os.environ.get("Z544_V", "[1]")))
#@value p5: includes={"unhashable"}


def build(kind, k):
    try:
        if kind == "set":
            x = {k}
        elif kind == "key":
            x = {k: 1}
        elif kind == "store":
            x = {}
            x[k] = 1
        elif kind == "in":
            x = k in {"a"}
        else:
            x = set([k])
        return "built"
    except TypeError:
        return "unhashable"


b1 = build("set", [s])                   #@value b1: exactly={"unhashable"}
b2 = build("key", [s])                   #@value b2: exactly={"unhashable"}
b3 = build("store", [s])                 #@value b3: exactly={"unhashable"}
b4 = build("in", [s])                    #@value b4: exactly={"unhashable"}
b5 = build("list", [s])                  #@value b5: exactly={"unhashable"}
b6 = build("set", (s, [s]))              #@value b6: exactly={"unhashable"}
b7 = build("store", name)                 #@value b7: exactly={"built"}


def member(k):
    try:
        if k in {"a": 1}:
            return "in"
        return "out"
    except TypeError:
        return "unhashable"


m1 = member([s])                         #@value m1: exactly={"unhashable"}
m2 = member("a")                         #@value m2: exactly={"in"}


@dataclass
class Point:
    x: int


@dataclass(frozen=True)
class Frozen:
    x: int


def keyed(p):
    try:
        return {p: 1} and "built"
    except TypeError:
        return "unhashable"


k1 = keyed(Point(1))                     #@value k1: includes={"unhashable"}
k2 = keyed(Frozen(1))                    #@value k2: exactly={"built"}
