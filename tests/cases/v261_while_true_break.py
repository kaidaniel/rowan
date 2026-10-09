"""Hypothesis: `while True` whose body stores and then breaks runs the body once.

Expectations were written from the program text before running the engine."""

d = {}
while True:
    d["k"] = 1
    break
_ = 0                                        #@row d: present={k} definite={k} closed
d["k"]                                       #@ ok k
