"""`compose` never called: its inner closure still runs with open functions and argument, and
the dict it builds is exact.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def compose(f, g):
    def h(x):
        mid = g(x)
        out = {"mid": mid}
        _ = 0                                #@row out: present={mid} definite={mid} closed
        return f(out)
    return h
