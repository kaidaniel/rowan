"""Hypothesis: a generator that yields inside a loop under a decided condition yields exactly
the items that pass, in order.

Expectations were written from the program text before running the engine."""


def evens(xs):
    for x in xs:
        if x % 2 == 0:
            yield {"n": x}


out = list(evens([1, 2, 3, 4]))
n = {len(out): 1}                            #@row n: present={2} definite={2} closed
second = out[1]
k = {second["n"]: 1}                         #@row k: present={4} definite={4} closed
