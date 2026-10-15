# A slice from the start to a bound not known (`recs[:batch]`, the bound from the
# environment) holds the list's own first items: item i of it, where there is one, is item i
# of the list. A list one of a few lengths (an append on some paths) has its items at their
# positions too: `zs[1]` exists only where the append ran.
import os

batch = int(os.environ.get("Z517_BATCH", "2"))
recs = [{"id": 1}, {"id": 2, "tags": ["a"]}, {"id": 3}]
first = recs[:batch]
r1 = first[1]                                    #@row r1: present={id,tags} definite={id,tags} closed
t = r1["tags"]                                   #@ ok tags
last = first[-1]                                 #@row last: present={id,tags} definite={id} closed
whole = recs[0:batch:1]
w0 = whole[0]["id"]                              #@value w0: exactly={1}

zs = ["x"]
if batch > 1:
    zs.append("y")
z1 = zs[1]                                       #@value z1: exactly={"y"}
zl = zs[-1]                                      #@value zl: exactly={"x","y"}
z0 = zs[0]                                       #@value z0: exactly={"x"}
