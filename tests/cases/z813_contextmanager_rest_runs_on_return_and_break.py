"""A `@contextmanager` generator's code after its yield runs however the with body leaves: at
its end, and before a `return` or a `break` leaves the with statement (as `__exit__` would)."""
import contextlib

log = []


@contextlib.contextmanager
def cm():
    log.append("enter")
    yield
    log.append("exit")


def returns():
    with cm():
        return len(log)


r = returns()
#@value r: exactly={1}
n1 = len(log)
#@value n1: exactly={2}
last1 = log[-1]
#@value last1: exactly={"exit"}

for i in (1, 2):
    with cm():
        break
n2 = len(log)
#@value n2: exactly={4}
