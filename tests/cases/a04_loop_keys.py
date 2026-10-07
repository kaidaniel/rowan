d = {}
for c in ["a", "b", "c"]:
    d[c] = compute(c)
d["a"]                               #@ ok a
x = d                                #@row d: present={a,b,c} definite={a,b,c} closed
d["z"]                               #@ missing z

counts = {}
for w in words:
    counts[w] = counts.get(w, 0) + 1
counts["the"]                        #@ none the
