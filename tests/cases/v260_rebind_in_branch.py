"""Hypothesis: a name rebound to another dict under an unknown condition holds either dict: a
key only the new one has is possible.

Expectations were written from the program text before running the engine."""

d = {}
if unknown_flag:
    d = {"a": 1}
d["a"]                                       #@ possibly a
_ = 0                                        #@row d: present={a} definite={} closed
