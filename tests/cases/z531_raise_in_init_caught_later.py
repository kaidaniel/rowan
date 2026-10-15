"""A constructor that may raise after setting some attributes, inside a `try` whose body later
stores the object and may raise again: the handler joins both raise points, but on the first
the half-built object is garbage (nothing outside the constructor holds it), so the object
the program keeps has all its attributes (rich's `Style(bgcolor=…)` in a property read under
a `try`: ~70 false "only set on some paths"). One the constructor stored somewhere before the
raise (`REG.append(self)`) is held: on that path it lacks what was not set yet.

Expectations were written from the program text before running the engine."""
import os
import reprlib

peek = getattr(reprlib, os.environ.get("Z531_FN", "repr"))


class S:
    def __init__(self, c):
        self.a = 1
        self.b = parse(c)
        self.c = 2


def parse(c):
    return peek(c)


out = []


def build(c):
    s = S(c)
    out.append(s)
    peek(c)


try:
    build("x")
except Exception:
    pass
for s in out:
    v = s.c  #@ ok c
    w = s.a  #@ ok a

REG = []


class T:
    def __init__(self, c):
        self.a = 1
        REG.append(self)
        self.b = parse(c)
        self.c = 2


try:
    T("y")
except Exception:
    pass
for t in REG:
    tc = t.c  #@ possibly c
    ta = t.a  #@ ok a
