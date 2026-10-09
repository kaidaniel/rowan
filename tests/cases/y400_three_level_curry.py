"""Three nested closures, each returned by the one around it, called in a chain.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def a(x):
    def b(y):
        def c(z):
            out = {"x": x, "y": y, "z": z}
            _ = 0                            #@row out: present={x,y,z} definite={x,y,z} closed
            out["w"]                         #@ missing w
            return out
        return c
    return b


r = a(1)(2)(3)                               #@row r: present={x,y,z} definite={x,y,z} closed
