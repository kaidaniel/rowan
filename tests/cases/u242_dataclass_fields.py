"""Hypothesis: `dataclasses.fields(Cls)` gives the fields in order, with known `.name`.

Expectations were written from the program text before running the engine."""

from dataclasses import dataclass, fields


@dataclass
class Pt:
    x: int
    y: int = 0


names = {f.name: f for f in fields(Pt)}      #@row names: present={x,y} definite={x,y} closed
