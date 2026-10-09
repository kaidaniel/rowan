"""Hypothesis: a class-syntax `NamedTuple` with a default has its fields as attributes and
`_asdict()` / `_replace()` like the functional form.

Expectations were written from the program text before running the engine."""

from typing import NamedTuple


class P(NamedTuple):
    x: int
    y: int = 0


p = P(1)
d = p._asdict()                              #@row d: present={x,y} definite={x,y} closed
q = p._replace(y=5)
q.y                                          #@ ok y
p.z                                          #@ missing z
