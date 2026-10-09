"""Hypothesis: a recursive deep merge of two known nested dicts gives the union at every
level and leaves the inputs unchanged (it copies with `dict(a)`).

Expectations were written from the program text before running the engine."""


def merge(a, b):
    out = dict(a)
    for k, v in b.items():
        if k in out and isinstance(out[k], dict) and isinstance(v, dict):
            out[k] = merge(out[k], v)
        else:
            out[k] = v
    return out


A = {"db": {"host": "h"}, "x": 1}
B = {"db": {"port": 1}, "y": 2}
m = merge(A, B)                              #@row m: present={db,x,y} definite={db,x,y} closed
db = m["db"]                                 #@row db: present={host,port} definite={host,port} closed
adb = A["db"]                                #@row adb: present={host} definite={host} closed
A["db"]["port"]                              #@ missing port
A["y"]                                       #@ missing y
