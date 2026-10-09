"""The same access inside a function is fine for one call and missing for another: both
verdicts are reported on that line.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def get_port(cfg):
    return cfg["port"]                       #@ ok port ; missing port


get_port({"port": 1})
get_port({})
