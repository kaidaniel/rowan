"""A generator a function starts and keeps in a global, resumed by a later call: what the code
between its items changes (a key it reads after a yield deleted, a value it reads set) is
what it sees when resumed. rowan stopped watching the generator once the call that started
it returned (it reached it through no returned value, argument or caller): the later item
was the one the first run computed.

Expectations were written from the program text before running the engine."""
import os

CFG = {"k": 1, "n": 10}
_gen = None


def numbers():
    while True:
        yield CFG["n"]


def start():
    global _gen
    _gen = numbers()
    return next(_gen)


def step():
    return next(_gen)


a = start()                              #@value a: exactly={10}
CFG["n"] = 20
b = step()                               #@value b: includes={20}
if os.environ.get("Z550_DROP", "1") == "1":
    del CFG["n"]
try:
    c = step()
except KeyError:
    c = "gone"
d = c                                    #@value d: includes={"gone"}
