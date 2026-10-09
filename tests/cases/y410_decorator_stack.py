"""Two stacked decorator factories: inside the decorated function its own dict; outside the
keys both wrappers add.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def add(key):
    def deco(fn):
        def w(*a):
            r = fn(*a)
            r[key] = True
            return r
        return w
    return deco


@add("x")
@add("y")
def base():
    r = {"base": True}
    _ = 0                                    #@row r: present={base} definite={base} closed
    return r


out = base()                                 #@row out: present={base,x,y} definite={base,x,y} closed
