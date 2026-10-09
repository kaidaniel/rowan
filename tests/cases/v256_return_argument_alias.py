"""Hypothesis: a function that mutates its argument and returns it gives back the same
object, so later stores through the result reach the caller's dict.

Expectations were written from the program text before running the engine."""


def tag(d):
    d["tagged"] = True
    return d


c = {"a": 1}
e = tag(c)
e["more"] = 1
_ = 0                                        #@row c: present={a,tagged,more} definite={a,tagged,more} closed
