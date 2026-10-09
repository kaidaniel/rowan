"""Hypothesis: a generator yielding different records in order gives `next()` the first
one, and `list()` of the rest keeps their order.

Expectations were written from the program text before running the engine."""


def records():
    yield {"kind": "head", "n": 0}
    yield {"kind": "body", "n": 1}
    yield {"kind": "tail", "n": 2, "last": True}


g = records()
head = next(g)                            #@row head: present={kind,n} definite={kind,n} closed
rest = list(g)
tail = rest[1]                            #@row tail: present={kind,n,last} definite={kind,n,last} closed
head["last"]                              #@ missing last
