"""Hypothesis: `setattr` / `getattr` with names taken from a known list act as literal
attribute stores and loads.

Expectations were written from the program text before running the engine."""


class C:
    pass


o = C()
for name in ("x", "y"):
    setattr(o, name, {"n": name})
o.x                                          #@ ok x
ox = getattr(o, "x")                         #@row ox: present={n} definite={n} closed
vals = {n: getattr(o, n) for n in ("x", "y")}  #@row vals: present={x,y} definite={x,y} closed
o.z                                          #@ missing z
