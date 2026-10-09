"""A closure over a tuple of step functions, applying them in order.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def pipeline(*steps):
    def run(d):
        for s in steps:
            d = s(d)
        _ = 0                                #@row d: present={a,b} definite={a,b} closed
        return d
    return run


p = pipeline(lambda d: {**d, "a": 1}, lambda d: {**d, "b": 2})
r = p({})                                    #@row r: present={a,b} definite={a,b} closed
