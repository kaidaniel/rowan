"""A memoising closure over a function parameter: the cached value is the function's result.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def memoize(fn):
    cache = {}

    def wrapped(k):
        if k not in cache:
            cache[k] = fn(k)
        val = cache[k]
        _ = 0                                #@row val: present={key} definite={key} closed
        return val
    return wrapped


lookup = memoize(lambda k: {"key": k})
lookup("x")
lookup("y")
