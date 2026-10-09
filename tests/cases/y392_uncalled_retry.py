"""An uncalled higher-order function: the callback is unknown, its own result is exact.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def retry(fn, times):
    last = None
    for _ in range(times):
        last = fn()
    result = {"last": last, "times": times}
    _ = 0                                    #@row result: present={last,times} definite={last,times} closed
    return result
