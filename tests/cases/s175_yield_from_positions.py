"""Hypothesis: `yield from` chains of generators with known yields keep positions in
`list(...)`.

Expectations were written from the program text before running the engine."""


def first():
    yield {"a": 1}
    yield {"b": 2}


def second():
    yield {"c": 3}


def both():
    yield from first()
    yield from second()


items = list(both())
i0 = items[0]                                #@row i0: present={a} definite={a} closed
i2 = items[2]                                #@row i2: present={c} definite={c} closed
n = {len(items): 1}                          #@row n: present={3} definite={3} closed
