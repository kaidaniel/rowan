#@engine rust
"""Variables rebound to different records: in sequence, in branches, in loops, after calls,
through aliases, shadowed in nested scopes."""
x = {"a": 1}
x["a"]                                   #@ ok a
x = {"b": 2}
x["a"]                                   #@ missing a
x["b"]                                   #@ ok b
y = x
x = {"c": 3}
y["b"]                                   #@ ok b
y["c"]                                   #@ missing c
x["c"]                                   #@ ok c

if cond:
    z = {"p": 1}
else:
    z = {"q": 2}
z["p"]                                   #@ possibly p
z["r"]                                   #@ missing r
z = {"s": 1}
z["p"]                                   #@ missing p
z["s"]                                   #@ ok s

acc = {"start": True}
for i in range(n):
    acc = {"iter": i}
acc["start"]                             #@ possibly start
acc["iter"]                              #@ possibly iter
acc["never"]                             #@ missing never

w = {"v": 1}
w = w
w["v"]                                   #@ ok v


def rebind(d):
    d = {"inner": 1}
    return d


orig = {"outer": 1}
got = rebind(orig)
orig["outer"]                            #@ ok outer
orig["inner"]                            #@ missing inner
got["inner"]                             #@ ok inner


def mutate(d):
    d["touched"] = True
    d = {"new": 1}
    d["new2"] = 2
    return d


m = {"m": 1}
mr = mutate(m)
m["touched"]                             #@ ok touched
m["new2"]                                #@ missing new2
mr["new2"]                               #@ ok new2

name = "k1"
tbl = {"k1": 1, "k2": 2}
tbl[name]                                #@ ok k1
name = "k2"
tbl[name]                                #@ ok k2
name = "k3"
tbl[name]                                #@ missing k3

shadow = {"module": 1}


def shadows():
    shadow = {"local": 1}
    return shadow["local"]               #@ ok local


shadows()
shadow["module"]                         #@ ok module
shadow["local"]                          #@ missing local

swap_a, swap_b = {"a": 1}, {"b": 1}
swap_a, swap_b = swap_b, swap_a
swap_a["b"]                              #@ ok b
swap_b["a"]                              #@ ok a
swap_a["a"]                              #@ missing a

val = {"first": 1}
val = val.copy()
val["second"] = 2
val["first"]                             #@ ok first
val["second"]                            #@ ok second

cur = {"level": 0}
cur = {"level": 1, "child": cur}
cur["child"]["level"]                    #@ ok child ; ok level
cur["child"]["child"]                    #@ missing child
