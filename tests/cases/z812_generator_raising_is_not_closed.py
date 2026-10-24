"""A generator that raises before its first item is done: the raise leaving the consumer that
stops early (`next` of a generator expression over it) does not close it; one that gave its item
is closed where the generator expression is dropped."""
import os

B = os.environ.get("B", "")
log = []


def g():
    try:
        if B == "K":
            raise KeyError("k")
        yield 2
    except GeneratorExit:
        log.append("closed")
        raise


def p():
    try:
        return next((v for v in g() if v == 2), 0)
    except KeyError:
        return -1


r = p()
#@value r: exactly={2, -1}
n = len(log)
#@value n: exactly={0, 1}
