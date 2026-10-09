"""A function iterating a dict for one call and a list for another: the keys it collects.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def keys_of(x):
    ks = {}
    for k in x:
        ks[k] = True
    _ = 0                                    #@row ks: present={a,b} definite={} closed
    return ks


keys_of({"a": 1})
keys_of(["b"])
