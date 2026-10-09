"""`compose(f, g)` returning a closure: inside it, the intermediate value is g's result.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def compose(f, g):
    def h(x):
        mid = g(x)
        _ = 0                                #@row mid: present={g} definite={g} closed
        return f(mid)
    return h


def add_g(d):
    return {**d, "g": 1}


def add_f(d):
    return {**d, "f": 1}


r = compose(add_f, add_g)({})                #@row r: present={g,f} definite={g,f} closed
