# A list's or set's methods called by name (`xs.__setitem__(0, v)`, `s.__ior__(t)`) change
# it as the statements they stand for do: what a later loop sees includes what they stored.
d = [1, 2]
d.__setitem__(0, "d")
for x in d:
    vd = x                                   #@value vd: exactly={"d", 2}

e = [1, 2]
e.__delitem__(0)
ne = len(e)                                  #@value ne: exactly={1}

f = [1, 2]
f.__iadd__(["f"])
for x in f:
    vf = x                                   #@value vf: exactly={1, 2, "f"}

g = {1}
g.__ior__({"g"})
for x in g:
    vg = x                                   #@value vg: exactly={1, "g"}

# reading methods change nothing
h = [1, 2, 1]
c = h.count(1)
for x in h:
    vh = x                                   #@value vh: exactly={1, 2}
