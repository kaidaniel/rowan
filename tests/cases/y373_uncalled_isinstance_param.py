"""An uncalled function branching on the type of an open parameter: every branch is possible.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def size(x):
    out = {}
    if isinstance(x, dict):
        out["keys"] = len(x)
    elif isinstance(x, (list, tuple)):
        out["items"] = len(x)
    else:
        out["scalar"] = True
    _ = 0                                    #@row out: present={keys,items,scalar} definite={} closed
    return out
