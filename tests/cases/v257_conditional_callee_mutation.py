"""Hypothesis: a callee that stores a key under an unknown condition makes it possible in the
caller's dict; with a literal False the store does not happen.

Expectations were written from the program text before running the engine."""


def maybe_add(d, flag):
    if flag:
        d["extra"] = 1


c = {"a": 1}
maybe_add(c, unknown_flag)
_ = 0                                        #@row c: present={a,extra} definite={a} closed
c["extra"]                                   #@ possibly extra
d2 = {"a": 1}
maybe_add(d2, False)
_ = 1                                        #@row d2: present={a} definite={a} closed
