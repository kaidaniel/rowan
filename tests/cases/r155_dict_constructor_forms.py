"""Hypothesis: every form of the `dict(...)` constructor (mapping plus keywords, `**`,
list of pairs, `items()`) gives a fresh exact row and leaves its argument alone.

Expectations were written from the program text before running the engine."""

base = {"a": 1}
d1 = dict(base, b=2)                         #@row d1: present={a,b} definite={a,b} closed
d2 = dict(**base)                            #@row d2: present={a} definite={a} closed
d3 = dict([("x", 1), ("y", 2)])              #@row d3: present={x,y} definite={x,y} closed
d4 = dict(a=1, **{"c": 3})                   #@row d4: present={a,c} definite={a,c} closed
d5 = dict(d3.items())                        #@row d5: present={x,y} definite={x,y} closed
d2["z"] = 0
base["b"]                                    #@ missing b
base["z"]                                    #@ missing z
d1["c"]                                      #@ missing c
