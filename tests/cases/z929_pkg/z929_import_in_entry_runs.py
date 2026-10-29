"""Two functions nobody calls import the same module inside their bodies. Each run for any
call starts where the module may not have been imported yet: its import runs the module's code
there. Rowan ran the module's code in the first such run only; the second found a module it
knew nothing of, and storing an attribute on it made the module one of that attribute only:
`m.LIMIT` was claimed to raise AttributeError. (Now such a module holds anything there.)
Expectations were written from the program text before running the engine."""


def first():
    import limitslib as m

    return m.LIMIT


def second():
    import limitslib as m

    m.SEEN = True
    try:
        r = m.LIMIT
    except AttributeError:
        r = "AE"
    return r
    #@general r: includes={10}
    #@todo general r: exactly={10}   (a module another run imported holds anything here)
