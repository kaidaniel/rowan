"""A folding function passed in and used by `reduce` inside.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""

from functools import reduce


def fold(fn, items, start):
    acc = reduce(fn, items, start)
    _ = 0                                    #@row acc: present={a,b} definite={a,b} closed
    return acc


fold(lambda acc, k: {**acc, k: True}, ["a", "b"], {})
