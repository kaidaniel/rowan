"""An uncalled method filling a cache on its instance: the cached entry has the literal keys.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


class Svc:
    def __init__(self):
        self.cache = {}

    def lookup(self, k):
        if k not in self.cache:
            self.cache[k] = {"k": k}
        hit = self.cache[k]
        _ = 0                                #@row hit: present={k} definite={k} closed
        hit["k"]                             #@ ok k
        return hit
