# Iterating a set: Python does not promise the order of its items (a str hashes differently
# in each run), so whatever depends on the order may be any of them; what does not (sorted,
# len, membership) stays exact.
names = {"ann", "bob"}

out = []
for nm in names:
    out.append(nm)
first = out[0]                               #@value first: exactly={"ann", "bob"}
last = nm                                    #@value last: exactly={"ann", "bob"}

idx = [i for i, nm2 in enumerate(names) if nm2 == "ann"]
pos = idx[0]                                 #@value pos: exactly={0, 1}

joined = ",".join(names)                     #@value joined: includes={"ann,bob", "bob,ann"}
#@todo value joined: exactly={"ann,bob", "bob,ann"}

taken = set(names).pop()                     #@value taken: exactly={"ann", "bob"}

by = dict(zip(names, [1, 2]))
v = by.get("ann")                            #@value v: includes={1, 2}
#@todo value v: exactly={1, 2}  (each pair's name is either: the dict's keys are each maybe there)

comp = [nm3.upper() for nm3 in names]
c0 = comp[0]                                 #@value c0: exactly={"ANN", "BOB"}
nc = len(comp)                               #@value nc: exactly={2}

s0 = sorted(names)[0]                        #@value s0: exactly={"ann"}
