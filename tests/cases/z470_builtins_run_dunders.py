"""Hypothesis: builtins and f-strings run the dunder methods of the program's classes, as Python
calls them implicitly: their effects happen and their results are the builtin's. `str` /
`repr` / `format` / f-strings / `print`, `hash`, `len`, `int`, `abs`. A class that caches its
hash on the instance is the realistic case; `str` and an f-string of an object whose class
defines only `__repr__` use it.

Expectations were written from the program text before running the engine."""

log = {}


class Point:
    def __init__(self, x):
        self.x = x
        self._h = None

    def __str__(self):
        log["str"] = 1
        return "pt"

    def __repr__(self):
        log["repr"] = 1
        return "Point()"

    def __format__(self, spec):
        log["format"] = spec
        return "F"

    def __hash__(self):
        if self._h is None:
            self._h = 7
        return self._h

    def __len__(self):
        log["len"] = 1
        return 2

    def __int__(self):
        log["int"] = 1
        return 5

    def __abs__(self):
        log["abs"] = 1
        return 9


class Tag:
    def __repr__(self):
        return "Tag!"


p = Point(1)
s = str(p)
#@value s: exactly={"pt"}
r = repr(p)
#@value r: exactly={"Point()"}
f = f"{p:>3}"
#@value f: exactly={"F"}
spec = log["format"]                      #@ ok format
#@value spec: exactly={">3"}
g = format(p, "x")
#@value g: exactly={"F"}
h = hash(p)
#@value h: exactly={7}
cached = p._h
#@value cached: exactly={7}
n = len(p)
#@value n: exactly={2}
i = int(p)
#@value i: exactly={5}
a = abs(p)
#@value a: exactly={9}
print(p)
done = log
#@row done: present={str,repr,format,len,int,abs} definite={str,repr,format,len,int,abs} closed

t = Tag()
ts = str(t)
#@value ts: exactly={"Tag!"}
tf = f"<{t}>"
#@value tf: exactly={"<Tag!>"}
