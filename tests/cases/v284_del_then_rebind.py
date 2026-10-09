"""Hypothesis: `del d` then `d = {...}` leaves only the new dict under the name.

Expectations were written from the program text before running the engine."""

d = {"a": 1}
del d
d = {"b": 2}
_ = 0                                        #@row d: present={b} definite={b} closed
d["a"]                                       #@ missing a
