"""Hypothesis: a `namedtuple` has its declared fields as attributes; `_asdict()` gives a
closed dict with exactly those keys, also after `_replace`.

Expectations were written from the program text before running the engine."""

from collections import namedtuple

Point = namedtuple("Point", ["x", "y"])
p = Point(1, 2)
d = p._asdict()                              #@row d: present={x,y} definite={x,y} closed
q = p._replace(x=5)
dq = q._asdict()                             #@row dq: present={x,y} definite={x,y} closed
d["z"]                                       #@ missing z
p.x                                          #@ ok x
p.z                                          #@ missing z
