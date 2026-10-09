"""Hypothesis: `next(gen, default)` gives the default once the generator is exhausted.

Expectations were written from the program text before running the engine."""


def gen():
    yield {"a": 1}


g = gen()
first = next(g)                              #@row first: present={a} definite={a} closed
second = next(g, {"done": True})             #@row second: present={done} definite={done} closed
