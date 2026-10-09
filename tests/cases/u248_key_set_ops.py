"""Hypothesis: `&`, `|`, `-` on `keys()` views of known dicts give known key sets.

Expectations were written from the program text before running the engine."""

a = {"x": 1, "y": 2}
b = {"y": 3, "z": 4}
both = dict.fromkeys(a.keys() & b.keys())    #@row both: present={y} definite={y} closed
either = dict.fromkeys(a.keys() | b.keys())  #@row either: present={x,y,z} definite={x,y,z} closed
only_a = dict.fromkeys(a.keys() - b.keys())  #@row only_a: present={x} definite={x} closed
