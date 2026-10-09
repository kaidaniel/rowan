"""Hypothesis: a dict reached through a list element or a dict value is the same object, so a
store through that path is visible through the original name (and vice versa).

Expectations were written from the program text before running the engine."""

d = {"a": 1}
holder = [d]
wrap = {"inner": d}
holder[0]["b"] = 2
wrap["inner"]["c"] = 3
d["d"] = 4
_ = 0                                     #@row d: present={a,b,c,d} definite={a,b,c,d} closed
again = holder[0]                         #@row again: present={a,b,c,d} definite={a,b,c,d} closed
via = wrap["inner"]                       #@row via: present={a,b,c,d} definite={a,b,c,d} closed
d["e"]                                    #@ missing e
