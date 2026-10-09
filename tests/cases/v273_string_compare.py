"""Hypothesis: ordering comparisons of known strings decide the branch.

Expectations were written from the program text before running the engine."""


def grade(name):
    out = {}
    if name < "m":
        out["first_half"] = True
    else:
        out["second_half"] = True
    return out


a = grade("alice")                           #@row a: present={first_half} definite={first_half} closed
z = grade("zed")                             #@row z: present={second_half} definite={second_half} closed
