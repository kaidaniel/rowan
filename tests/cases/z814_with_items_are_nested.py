"""`with a, b: body` is `with a: with b: body`: an exception b's `__exit__` raises goes through
a's `__exit__` (which runs); an `__enter__` of b that raises runs a's `__exit__` but not b's."""
log = []


class A:
    def __enter__(self):
        log.append("a in")
        return self

    def __exit__(self, t, v, tb):
        log.append("a out")


class B:
    def __init__(self, fail_in):
        self.fail_in = fail_in

    def __enter__(self):
        if self.fail_in:
            raise KeyError("b in")
        log.append("b in")
        return self

    def __exit__(self, t, v, tb):
        log.append("b out")
        raise ValueError("b out")


try:
    with A(), B(False):
        log.append("body")
except ValueError:
    pass
n1 = len(log)
#@value n1: exactly={5}
last1 = log[-1]
#@value last1: exactly={"a out"}

try:
    with A(), B(True):
        log.append("body")
except KeyError:
    pass
n2 = len(log)
#@value n2: exactly={7}
last2 = log[-1]
#@value last2: exactly={"a out"}
