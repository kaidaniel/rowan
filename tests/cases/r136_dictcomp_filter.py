"""Hypothesis: a dict comprehension over known pairs with a decided filter keeps exactly the
keys whose values pass; with an unknown filter every key is possible, none definite, and
the result is still closed.

Expectations were written from the program text before running the engine."""

PAIRS = [("a", 1), ("b", 0), ("c", 3)]
kept = {k: v for k, v in PAIRS if v}             #@row kept: present={a,c} definite={a,c} closed
kept["b"]                                        #@ missing b
kept["c"]                                        #@ ok c
maybe = {k: v for k, v in PAIRS if pick(k)}      #@row maybe: present={a,b,c} definite={} closed
maybe["a"]                                       #@ possibly a
maybe["z"]                                       #@ missing z
big = {k: {"v": v} for k, v in PAIRS if v > 1}   #@row big: present={c} definite={c} closed
inner = big["c"]                                 #@row inner: present={v} definite={v} closed
