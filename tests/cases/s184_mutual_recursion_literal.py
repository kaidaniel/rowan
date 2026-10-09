"""Hypothesis: mutual recursion on a literal counter is decided call by call, so the nesting
of the result is exact.

Expectations were written from the program text before running the engine."""


def even(n):
    if n == 0:
        return {"even_end": True}
    return {"odd": odd(n - 1)}


def odd(n):
    if n == 0:
        return {"odd_end": True}
    return {"even": even(n - 1)}


r = even(3)                                  #@row r: present={odd} definite={odd} closed
deep = r["odd"]["even"]["odd"]               #@row deep: present={odd_end} definite={odd_end} closed
r["odd"]["odd"]                              #@ missing odd
