"""Hypothesis: a dict passed positionally through two layers of `*args` / `**kwargs`
forwarding is the same object, so a store at the bottom is visible at the top.

Expectations were written from the program text before running the engine."""


def bottom(rec, flag, label=None):
    rec["flag"] = flag
    if label is not None:
        rec["label"] = label
    return rec


def middle(*args, **kwargs):
    return bottom(*args, **kwargs)


def top(rec, *args, **kwargs):
    rec["seen"] = True
    return middle(rec, *args, **kwargs)


r1 = {"id": 1}
out1 = top(r1, True)                      #@row out1: present={id,seen,flag} definite={id,seen,flag} closed
r2 = {"id": 2}
out2 = top(r2, False, label="x")          #@row out2: present={id,seen,flag,label} definite={id,seen,flag,label} closed
_ = 0                                     #@row r1: present={id,seen,flag} definite={id,seen,flag} closed
r1["label"]                               #@ missing label
