"""Hypothesis: `dict(d)`, `d.copy()`, `{**d}` and `copy.deepcopy(d)` are new objects: keys
added to a copy do not appear on the original, and nested dicts are shared by shallow copies
but not by the deep copy.

Expectations were written from the program text before running the engine."""
import copy

orig = {"a": 1, "nested": {"n": 1}}
c1 = dict(orig)
c2 = orig.copy()
c3 = {**orig}
c4 = copy.deepcopy(orig)
c1["b"] = 1
c2["c"] = 1
c3["d"] = 1
c4["e"] = 1
c1["nested"]["shallow"] = True
c4["nested"]["deep"] = True
_ = 0                                     #@row orig: present={a,nested} definite={a,nested} closed
nested = orig["nested"]                   #@row nested: present={n,shallow} definite={n,shallow} closed
deep_nested = c4["nested"]                #@row deep_nested: present={n,deep} definite={n,deep} closed
_ = 0                                     #@row c1: present={a,nested,b} definite={a,nested,b} closed
orig["b"]                                 #@ missing b
deep_nested["shallow"]                    #@ missing shallow
