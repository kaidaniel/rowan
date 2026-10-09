"""Hypothesis: `isinstance(x, (dict, list))` with a known kind takes exactly one branch, and
the `else` of an isinstance on a known dict is dead.

Expectations were written from the program text before running the engine."""


def shape(x):
    out = {"kind": "other"}
    if isinstance(x, (dict, list)):
        out["container"] = True
        if isinstance(x, dict):
            out["keys"] = sorted(x)
        else:
            out["len"] = len(x)
    else:
        out["scalar"] = True
    return out


d = shape({"a": 1})                       #@row d: present={kind,container,keys} definite={kind,container,keys} closed
l = shape([1, 2])                         #@row l: present={kind,container,len} definite={kind,container,len} closed
s = shape(3)                              #@row s: present={kind,scalar} definite={kind,scalar} closed
d["scalar"]                               #@ missing scalar
l["keys"]                                 #@ missing keys
