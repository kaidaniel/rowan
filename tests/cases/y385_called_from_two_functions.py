"""A helper called from two other functions with different dicts: the join inside.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def norm(d):
    d.setdefault("v", 0)
    _ = 0                                    #@row d: present={v,p,q} definite={v} closed
    return d


def p():
    return norm({"p": 1})


def q():
    return norm({"q": 1})


p()
q()
