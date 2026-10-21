# A generator closed after the program changed what its `finally` reads runs it on what the
# program left there, not on what it saw when it gave its item.
G = [0]


def closing(xs):
    try:
        for v in xs:
            yield v
            G[0] = v
    finally:
        G[0] = G[0] + 100


g = closing([1, 2])
x = next(g)
#@value x: exactly={1}
G[0] = 5
g = None
h = G[0]
#@value h: includes={105}
#@todo value h: exactly={105}  (the program's change is seen by running the module again with the generator's facts changed at its yields: any value)
