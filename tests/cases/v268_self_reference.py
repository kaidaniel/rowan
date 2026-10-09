"""Hypothesis: a dict that contains itself is followed through any number of hops.

Expectations were written from the program text before running the engine."""

d = {"a": 1}
d["self"] = d
x = d["self"]["self"]                        #@row x: present={a,self} definite={a,self} closed
x["a"]                                       #@ ok a
