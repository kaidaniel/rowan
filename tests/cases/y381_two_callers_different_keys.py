"""A function called from two places with different dicts and labels: the row inside is the
join of both calls.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def tag(d, label):
    d[label] = True
    _ = 0                                    #@row d: present={x,a,b} definite={} closed
    return d


tag({"x": 1}, "a")
tag({}, "b")
