# A consumer that takes a generator's first items sees what the generator changed up to them,
# also what a later step changes back to how it was before the call (a store at each step).
G = [0]


def steps(xs):
    for w in xs:
        G[0] = w
        yield w


g = steps([1, 2, 0])
first = next(g)
seen = G[0]
#@value seen: exactly={1}
rest = list(g)
after = G[0]
#@value after: exactly={0}
