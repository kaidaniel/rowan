"""A closure keeping a dict across calls: inside, the dict over both calls.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def counter():
    seen = {}

    def hit(k):
        seen[k] = seen.get(k, 0) + 1
        _ = 0                                #@row seen: present={a,b} definite={a} closed
        return seen
    return hit


h = counter()
h("a")
h("b")
