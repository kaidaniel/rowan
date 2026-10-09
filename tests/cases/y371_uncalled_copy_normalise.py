"""An uncalled function copying its open parameter: the copy is open, with its own stores
definite and popped keys absent.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def normalise(rec):
    out = dict(rec)
    out["id"] = str(out.get("id", ""))
    out.pop("tmp", None)
    _ = 0                                    #@row out: present={id} definite={id} absent={tmp} open
    out["tmp"]                               #@ missing tmp
    return out
