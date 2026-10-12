"""A comprehension over known items with a filter undecided for an item may skip that item:
the key it stores is possible, as the same `for` loop's is."""
import sys

a = {"id": 1, "name": "a"}
b = {"id": 2, "email": "x"}
r = a if len(sys.argv) > 1 else b
q = {k: r[k] for k in ["id", "name"] if k in r}  #@row q: present={id,name} definite={id} closed
w = {}
for k in ["id", "name"]:
    if k in r:
        w[k] = r[k]
_ = 0                                            #@row w: present={id,name} definite={id} closed
