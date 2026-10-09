"""Hypothesis: `dst.update(src)` copies the keys (later keys of src are not in dst) and shares
the values (a store into a shared inner dict is seen through both).

Expectations were written from the program text before running the engine."""

src = {"a": {"inner": 1}}
dst = {}
dst.update(src)
src["b"] = 2
dst["a"]["x"] = 1
sa = src["a"]                                #@row sa: present={inner,x} definite={inner,x} closed
dst["b"]                                     #@ missing b
