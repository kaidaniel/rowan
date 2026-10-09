"""Hypothesis: a memoising function returns the same object on the second call with the
same key, so a store made by the caller after the first call is visible through the
second result; a different key gives a fresh object.

Expectations were written from the program text before running the engine."""

_cache = {}


def get(name):
    if name not in _cache:
        _cache[name] = {"name": name}
    return _cache[name]


a = get("x")
a["touched"] = True
b = get("x")                                 #@row b: present={name,touched} definite={name,touched} closed
c = get("y")                                 #@row c: present={name} definite={name} closed
c["touched"]                                 #@ missing touched
_ = 0                                        #@row _cache: present={x,y} definite={x,y} closed
