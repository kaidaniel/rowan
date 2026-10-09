"""Hypothesis: integer arithmetic on the loop index gives known keys.

Expectations were written from the program text before running the engine."""

d = {}
for i in range(3):
    d[i * 2 + 1] = i
_ = 0                                        #@row d: present={1,3,5} definite={1,3,5} closed
d[2]                                         #@ missing 2
