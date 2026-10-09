"""Hypothesis: a `return` inside `finally` swallows the exception raised in the try body: the
call returns normally, with the keys stored before the raise and in the finally.

Expectations were written from the program text before running the engine."""


def risky(d):
    try:
        d["start"] = 1
        raise ValueError("x")
    finally:
        d["fin"] = 1
        return d


out = risky({})                              #@row out: present={start,fin} definite={start,fin} closed
after = {"reached": True}                    #@row after: present={reached} definite={reached} closed
