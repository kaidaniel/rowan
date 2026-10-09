"""A closure that is made but never called: it runs with an open parameter and the captured
field, so its read is a requirement and its own dict is exact.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def make_reader(field):
    def read(rec):
        v = rec[field]                       #@ input name
        out = {"field": field, "value": v}
        _ = 0                                #@row out: present={field,value} definite={field,value} closed
        return out
    return read


reader = make_reader("name")
