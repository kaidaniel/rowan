"""Hypothesis: a wrapping decorator that adds a key to the wrapped function's result makes the
key part of every call's result.

Expectations were written from the program text before running the engine."""


def with_meta(fn):
    def wrapper(*args, **kwargs):
        out = fn(*args, **kwargs)
        out["meta"] = {"fn": fn.__name__}
        return out
    return wrapper


@with_meta
def make(x):
    return {"x": x}


r = make(1)                                  #@row r: present={x,meta} definite={x,meta} closed
m = r["meta"]                                #@row m: present={fn} definite={fn} closed
