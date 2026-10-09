"""A classmethod building a dict that depends on its string argument, called twice.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


class Rec:
    def __init__(self, data):
        self.data = data

    @classmethod
    def parse(cls, raw):
        data = {"raw": raw}
        if raw.startswith("#"):
            data["comment"] = True
        _ = 0                                #@row data: present={raw,comment} definite={raw} closed
        return cls(data)


Rec.parse("#x")
Rec.parse("y")
