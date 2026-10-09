"""Hypothesis: `"%s_%d" % (name, n)` with known values gives known keys.

Expectations were written from the program text before running the engine."""

d = {}
for name, n in [("a", 1), ("b", 2)]:
    d["%s_%d" % (name, n)] = n
_ = 0                                        #@row d: present={a_1,b_2} definite={a_1,b_2} closed
