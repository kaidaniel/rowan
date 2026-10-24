"""Unpacking a generator into names takes its items until it ends or gives one more than there
are names: too few raise ValueError once it ran to its end (all its effects done); too many
once it gave one more (its effects up to that item); as many asks for one more, so it runs to
its end."""
log = []


def few():
    log.append("a")
    yield 1
    log.append("b")


def many():
    yield 1
    yield 2
    log.append("c")
    yield 3
    log.append("d")


def two():
    yield 1
    yield 2
    log.append("e")


try:
    x, y = few()
except ValueError:
    pass
n1 = len(log)
#@value n1: exactly={2}

try:
    x, y = many()
except ValueError:
    pass
n2 = len(log)
#@value n2: exactly={3}

p, q = two()
n3 = len(log)
#@value n3: exactly={4}
last = log[-1]
#@value last: exactly={"e"}
