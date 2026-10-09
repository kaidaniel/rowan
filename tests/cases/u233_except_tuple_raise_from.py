"""Hypothesis: `except (KeyError, IndexError)` catches a missing key; `raise ... from e`
raises the new exception, caught by its own handler one level up.

Expectations were written from the program text before running the engine."""


def get(d, k):
    try:
        return d[k]
    except (KeyError, IndexError) as e:
        raise LookupError("missing") from e


def safe(d, k):
    try:
        return {"v": get(d, k)}
    except LookupError:
        return {"err": True}


a = safe({"x": 1}, "x")                      #@row a: present={v} definite={v} closed
b = safe({"x": 1}, "y")                      #@row b: present={err} definite={err} closed
