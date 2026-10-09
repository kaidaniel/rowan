"""An uncalled function merging an unknown number of dicts: the result is open, nothing is
definitely missing.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def merge(*dicts):
    out = {}
    for d in dicts:
        out.update(d)
    _ = 0                                    #@row out: present={} definite={} open
    out["x"]                                 #@ sound x
    return out
