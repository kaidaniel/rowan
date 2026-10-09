"""Hypothesis: keys built with `str.format` (positional, named, format specs), `str()` of a
number and `"_".join` on literals are known.

Expectations were written from the program text before running the engine."""

d = {}
for i, p in enumerate(["a", "b"]):
    d["{}_{}".format(p, i)] = i
_ = 0                                        #@row d: present={a_0,b_1} definite={a_0,b_1} closed
e = {"{name}-{n:02d}".format(name="x", n=3): 1}
key = "_".join(["k", str(5)])
e[key] = 2
_ = 1                                        #@row e: present={x-03,k_5} definite={x-03,k_5} closed
e["k_6"]                                     #@ missing k_6
