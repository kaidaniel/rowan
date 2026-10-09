"""Hypothesis: a TypedDict is a plain dict at run time: constructed with keywords it has those
keys, and a declared key that was not given is missing.

Expectations were written from the program text before running the engine."""

from typing import TypedDict


class Opts(TypedDict, total=False):
    verbose: bool
    level: int


o = Opts(verbose=True)                       #@row o: present={verbose} definite={verbose} closed
o2: Opts = {"level": 1}
o2["level"]                                  #@ ok level
o["level"]                                   #@ missing level
