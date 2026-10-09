"""Weakness: model corner. An exception raised in a `with` body over a `@contextmanager` is
seen by the generator's `except` around its yield.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""
from contextlib import contextmanager


@contextmanager
def guard(log):
    try:
        yield
    except ValueError:
        log["caught"] = True


log = {}
with guard(log):
    raise ValueError("x")
log["caught"]                                #@ ok caught
log["caught"]                                #@ sound caught
