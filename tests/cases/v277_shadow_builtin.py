"""Hypothesis: a module-level function named like a builtin (`len`) is the one called.

Expectations were written from the program text before running the engine."""


def len(x):
    return 99


k = {len([1]): 1}                            #@row k: present={99} definite={99} closed
