"""Hypothesis: `update` with keywords and with pairs, `clear`, and `popitem` (which removes
the most recently inserted key) keep the row exact.

Expectations were written from the program text before running the engine."""

d = {"a": 1}
d.update(b=2, c=3)
_ = 0                                        #@row d: present={a,b,c} definite={a,b,c} closed
d.update([("e", 5)])
_ = 1                                        #@row d: present={a,b,c,e} definite={a,b,c,e} closed
d.clear()
_ = 2                                        #@row d: present={} definite={} closed
d["a"]                                       #@ missing a
d.update({"z": 1}, y=2)
_ = 3                                        #@row d: present={z,y} definite={z,y} closed
last = d.popitem()
_ = 4                                        #@row d: present={z} definite={z} closed
d["y"]                                       #@ missing y
