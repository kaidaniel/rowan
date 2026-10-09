"""An uncalled function reading an unknown key under try/except: the result is the value or
the fallback dict.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def safe_get(d, k):
    try:
        v = d[k]
    except KeyError:
        v = {"missing": True}
    _ = 0                                    #@row v: possible={missing} definite_within={missing}
    v["missing"]                             #@ sound missing
    return v
