"""An uncalled function with `**kw`: the merged dict has its literal key definite and is open.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def build(**kw):
    out = {"base": 1, **kw}
    _ = 0                                    #@row out: present={base} definite={base} open
    out["base"]                              #@ ok base
    out["extra"]                             #@ sound extra
    return out
