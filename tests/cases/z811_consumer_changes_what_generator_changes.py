"""A loop body that writes a global the generator writes too: the generator's later effects
(its finally and its end, here once the loop used it up) still come."""
G = 0
H = 0


def g():
    global G, H
    try:
        G = G * 10 + 1
        yield 1
        yield 1
    finally:
        H = G
        G = G * 10 + 6
    G = G * 10 + 5


def p():
    global H
    for v in g():
        H = G
    return G


r = p()
#@value r: includes={165}
#@todo value r: exactly={165}  (the body's write joins the generator's facts of H and G after it)
