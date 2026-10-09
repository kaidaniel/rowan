"""A lambda stored in a dict that returns another lambda, used inside a function.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""

handlers = {"double": lambda: (lambda d: {**d, "twice": True})}


def use(name, d):
    maker = handlers[name]                   #@ ok double
    fn = maker()
    out = fn(d)
    _ = 0                                    #@row out: present={a,twice} definite={a,twice} closed
    return out


use("double", {"a": 1})
