"""Hypothesis: an `lru_cache` function returns the cached object for repeated arguments, so a
store on the first result is seen on the second; other arguments get a fresh object.

Expectations were written from the program text before running the engine."""

from functools import lru_cache


@lru_cache(maxsize=None)
def load(name):
    return {"name": name}


a = load("x")
a["seen"] = True
b = load("x")                                #@row b: present={name,seen} definite={name,seen} closed
c = load("y")                                #@row c: present={name} definite={name} closed
