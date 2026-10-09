"""A function applying its callback: inside, the callback's result over both calls.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def apply(f, d):
    r = f(d)
    _ = 0                                    #@row r: present={x,y} definite={} closed
    return r


apply(lambda d: {**d, "x": 1}, {})
apply(lambda d: {"y": 2}, {})
