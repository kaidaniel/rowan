"""An uncalled method on a class never instantiated: self has what `__init__` sets.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


class Account:
    def __init__(self, owner):
        self.owner = owner
        self.meta = {"created": True}

    def report(self, extra):
        out = {"owner": self.owner, **self.meta}
        if extra:
            out["extra"] = extra
        _ = 0                                #@row out: present={owner,created,extra} definite={owner,created} closed
        return out
