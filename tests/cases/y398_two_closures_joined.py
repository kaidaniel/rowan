"""Two closures from two factory calls, each called once: inside, the join of both runs.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def make_setter(key):
    def setter(d):
        d[key] = True
        _ = 0                                #@row d: present={a,b} definite={} closed
        return d
    return setter


make_setter("a")({})
make_setter("b")({})
