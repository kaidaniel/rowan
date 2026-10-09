"""Hypothesis: a function returning two records through a tuple unpacks to the right names,
a swap `a, b = b, a` exchanges them, and a missing key inside the callee is reported at the
calling line.

Expectations were written from the program text before running the engine."""


def split(rec):
    pub = {k: rec[k] for k in ("id", "name")}
    priv = {"secret": rec["secret"]}
    return pub, priv


p, s = split({"id": 1, "name": "n", "secret": "x"})
_ = 0                                        #@row p: present={id,name} definite={id,name} closed
_ = 1                                        #@row s: present={secret} definite={secret} closed
p, s = s, p
_ = 2                                        #@row p: present={secret} definite={secret} closed
p["id"]                                      #@ missing id
_, only = split({"id": 1, "name": "n"})      #@ missing secret
