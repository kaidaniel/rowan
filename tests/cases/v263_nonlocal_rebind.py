"""Hypothesis: `nonlocal d; d = {...}` in an inner function rebinds the outer variable.

Expectations were written from the program text before running the engine."""


def outer():
    d = {"v": 1}

    def reset():
        nonlocal d
        d = {"fresh": True}
    reset()
    return d


r = outer()                                  #@row r: present={fresh} definite={fresh} closed
