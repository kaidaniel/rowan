"""Hypothesis: `d[k] += 1` reads d[k] first, so a missing key is reported.

Expectations were written from the program text before running the engine."""

counts = {"a": 0}
counts["a"] += 1                             #@ ok a
_ = 0                                        #@row counts: present={a} definite={a} closed
counts["b"] += 1                             #@ missing b
