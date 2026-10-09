"""Hypothesis: `isinstance` on items of a list of known literals of mixed types is decided per
item.

Expectations were written from the program text before running the engine."""

vals = ["a", 1, "b", 2.5]
by_type = {}
for v in vals:
    if isinstance(v, str):
        by_type.setdefault("str", []).append(v)
    elif isinstance(v, int):
        by_type.setdefault("int", []).append(v)
_ = 0                                        #@row by_type: present={str,int} definite={str,int} closed
by_type["float"]                             #@ missing float
