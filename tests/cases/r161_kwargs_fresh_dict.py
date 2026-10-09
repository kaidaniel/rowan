"""Hypothesis: `**kw` in a callee is a fresh dict: mutating it does not touch the caller's
splatted dict, and its keys are exactly the keyword arguments passed.

Expectations were written from the program text before running the engine."""


def opts(**kw):
    kw["extra"] = True
    return kw


cfg = {"a": 1}
o = opts(**cfg, b=2)                         #@row o: present={a,b,extra} definite={a,b,extra} closed
_ = 0                                        #@row cfg: present={a} definite={a} closed
cfg["extra"]                                 #@ missing extra
o2 = opts()                                  #@row o2: present={extra} definite={extra} closed
o2["a"]                                      #@ missing a
