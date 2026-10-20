# What uses up a one-shot iterator: dict(), str.join, max, sum, unpacking; `x in it` takes
# items up to the first equal one; next() takes one. An iterator has no length and no
# positions: len() and subscripts raise TypeError.
z = zip("ab", "cd")
d1 = len(dict(z))
#@value d1: exactly={2}
d2 = len(dict(z))
#@value d2: exactly={0}
m = map(str, [1, 2])
j1 = ",".join(m)
#@value j1: exactly={"1,2"}
j2 = ",".join(m)
#@value j2: exactly={""}
it = iter(["1", "2"])
mx = max(it)
#@value mx: exactly={"2"}
mx2 = max(it, default="-")
#@value mx2: exactly={"-"}
it4 = iter([1, 2])
a, b = it4
u = len(list(it4))
#@value u: exactly={0}
it5 = iter([1, 2, 3])
h = 2 in it5
#@value h: exactly={True}
h2 = list(it5)
h3 = len(h2)
#@value h3: exactly={1}
h4 = h2[0]
#@value h4: exactly={3}
en = enumerate("abc")
e1 = next(en)
e2 = next(en)
i2 = e2[0]
#@value i2: exactly={1}
c2 = e2[1]
#@value c2: exactly={"b"}
e3 = len(list(en))
#@value e3: exactly={1}
it6 = iter([1, 2, 3])
next(it6)
first, *rest = it6
#@value first: exactly={2}
nrest = len(rest)
#@value nrest: exactly={1}
fm = filter(None, [0, 1])
try:
    n = len(fm)
except TypeError:
    n = -1
#@value n: exactly={-1}
g = (i for i in [1, 2])
try:
    k = g[0]
except TypeError:
    k = "-"
#@value k: exactly={"-"}
