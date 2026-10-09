"""Hypothesis: `True == 1 == 1.0` hash alike, so they are one dict key; the key object kept is
the first one inserted.

Expectations were written from the program text before running the engine."""

e = {}
e[1] = "a"
e[True] = "b"
e[1.0] = "c"
_ = 0                                        #@row e: present={1} definite={1} closed
d2 = {True: 1, 1: 2}                         #@row d2: present={True} definite={True} closed
z = {0: "zero"}
z[False]                                     #@ ok False
