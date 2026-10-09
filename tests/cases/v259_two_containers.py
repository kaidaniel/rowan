"""Hypothesis: one dict held by two containers stays reachable through the second after it is
deleted from the first; a store through the second reaches it.

Expectations were written from the program text before running the engine."""

shared = {"v": 1}
a = {"s": shared}
b = [shared]
del a["s"]
b[0]["w"] = 2
_ = 0                                        #@row shared: present={v,w} definite={v,w} closed
_ = 1                                        #@row a: present={} definite={} closed
