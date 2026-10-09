"""A key function passed in and used by `max` inside.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def top(rows, key):
    best = max(rows, key=key)
    _ = 0                                    #@row best: present={n,big} definite={n,big} closed
    return best


top([{"n": 1}, {"n": 5, "big": True}], lambda r: r["n"])
