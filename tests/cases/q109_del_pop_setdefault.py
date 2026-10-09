"""Hypothesis: `del`, `pop` with and without a default, and `setdefault` keep the row exact:
a deleted key is missing until re-added, pop of a missing key with a default is fine,
setdefault returns the existing value when the key is there.

Expectations were written from the program text before running the engine."""

d = {"a": 1, "b": 2, "c": {"n": 0}}
del d["a"]
d["a"]                                    #@ missing a
d["a"] = 10
d["a"]                                    #@ ok a
b = d.pop("b")
d["b"]                                    #@ missing b
z = d.pop("zzz", {"default": True})       #@row z: present={default} definite={default} closed
c = d.setdefault("c", {"fresh": True})    #@row c: present={n} definite={n} closed
e = d.setdefault("e", {"fresh": True})    #@row e: present={fresh} definite={fresh} closed
_ = 0                                     #@row d: present={a,c,e} definite={a,c,e} closed
