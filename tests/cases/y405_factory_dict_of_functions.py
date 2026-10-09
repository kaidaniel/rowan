"""A factory returning a dict of closures; one of them is called through the dict.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def make_ops(prefix):
    def get(d):
        return d.get(prefix + "x")

    def put(d, v):
        d[prefix + "x"] = v
        _ = 0                                #@row d: present={p_x} definite={p_x} closed
        return d
    return {"get": get, "put": put}


ops = make_ops("p_")
ops["put"]({}, 1)
