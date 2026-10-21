# A loop over a generator runs the generator's code between its items: the body sees, at each
# item, what the generator did up to the yield that gave it, not what it does by its end; so
# does a comprehension's element. Its `finally` runs once it is used up, or when the generator
# holding it is closed.
G = [0]


def closing(xs):
    try:
        for v in xs:
            yield v
            G[0] = v
    finally:
        G[0] = G[0] + 100


seen = []
for w in closing([1, 2]):
    seen.append(G[0])
a = seen[0]
#@value a: exactly={0}
b = seen[1]
#@value b: exactly={1}
c = G[0]
#@value c: exactly={102}
G[0] = 0
seen2 = [G[0] + w for w in closing([1, 2])]
d = seen2[0]
#@value d: exactly={1}
e = seen2[1]
#@value e: exactly={3}
f = G[0]
#@value f: exactly={102}
G[0] = 0


def wrap(it):
    for v in it:
        yield v * 2


g = wrap(closing([1, 2]))
y = next(g)
#@value y: exactly={2}
k = G[0]
#@value k: exactly={0}
g = None
m = G[0]
#@value m: exactly={100}
