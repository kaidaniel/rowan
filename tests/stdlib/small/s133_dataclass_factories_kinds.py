# Dataclass default factories of other kinds each make a new object per field and per
# instance: a lambda, one of the program's functions used by two fields, `dict` and `set`.
# A field(init=False) with a factory too. Expectations written from the program text and
# the dataclasses documentation.
from dataclasses import dataclass, field


def fresh():
    return []


@dataclass
class Box:
    a: list = field(default_factory=fresh)
    b: list = field(default_factory=fresh)
    c: list = field(default_factory=lambda: [0])
    d: dict = field(default_factory=dict)
    e: dict = field(default_factory=dict)
    f: set = field(default_factory=set, init=False)
    g: set = field(default_factory=set, init=False)


x = Box()
x.a.append(1)
x.d["k"] = 1
x.f.add(3)
na = len(x.a)
#@value na: exactly={1}
nb = len(x.b)
#@value nb: exactly={0}
nc = len(x.c)
#@value nc: exactly={1}
ne = len(x.e)
#@value ne: exactly={0}
ng = len(x.g)
#@value ng: exactly={0}
y = Box()
ny = len(y.a) + len(y.d) + len(y.f)
#@value ny: exactly={0}
