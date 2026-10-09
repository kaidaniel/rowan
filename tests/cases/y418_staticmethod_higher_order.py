"""A staticmethod returning a closure that applies a function twice.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


class Util:
    @staticmethod
    def twice(fn):
        def run(x):
            once = fn(x)
            again = fn(once)
            _ = 0                            #@row again: present={n} definite={n} closed
            return again
        return run


inc = Util.twice(lambda d: {"n": d.get("n", 0) + 1})
r = inc({})                                  #@row r: present={n} definite={n} closed
k = {r["n"]: 1}                              #@row k: present={2} definite={2} closed
