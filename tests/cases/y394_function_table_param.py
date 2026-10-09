"""A table of functions passed in and looked up by a known key.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def route(table, kind, ev):
    h = table[kind]                          #@ ok create
    out = h(ev)
    _ = 0                                    #@row out: present={created} definite={created} closed
    return out


route({"create": lambda e: {"created": e}, "drop": lambda e: {"dropped": e}}, "create", 1)
