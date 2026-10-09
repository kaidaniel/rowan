"""A decorator factory three levels deep: inside the wrapper, the wrapped result plus the tag.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def tagged(tag):
    def deco(fn):
        def wrapper(*a, **k):
            out = fn(*a, **k)
            out["tag"] = tag
            _ = 0                            #@row out: present={v,tag} definite={v,tag} closed
            return out
        return wrapper
    return deco


@tagged("t1")
def make(v):
    return {"v": v}


r = make(1)                                  #@row r: present={v,tag} definite={v,tag} closed
