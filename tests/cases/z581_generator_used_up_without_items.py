# A generator that yields nothing still runs its body when it is used up: what it changes
# (here G[0] on the odd items it skips) is changed after `list(g)`, `sum(...)` or a loop.
G = [0]


def evens(xs):
    for v in xs:
        if v % 2 == 0:
            yield v
        else:
            G[0] = v


xs = [1, 3]
y = sum(1 for _ in evens(xs))                #@value y: exactly={0}
a = G[0]                                     #@value a: exactly={3}
G[0] = 0
z = list(evens(xs))
b = G[0]                                     #@value b: exactly={3}
G[0] = 0
for q in evens(xs):
    pass
c = G[0]                                     #@value c: exactly={3}
