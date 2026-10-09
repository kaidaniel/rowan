"""Weakness: mutual recursion on an unknown counter: every shape is possible, none definite.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""


def even(n):
    if n == 0:
        return {"even_end": True}
    return {"odd": odd(n - 1)}


def odd(n):
    if n == 0:
        return {"odd_end": True}
    return {"even": even(n - 1)}


r = even(n_in)                               #@row r: present={even_end,odd} definite={} closed
_ = 0                                        #@row r: possible={even_end,odd} definite_within={}
r["odd"]                                     #@ sound odd
