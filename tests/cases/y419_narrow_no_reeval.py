"""Hypothesis: narrowing a test does not run the calls in it a second time: each `if` over a
call (`in`, `==`, hasattr, isinstance, `.get`, `not`) appends to CALLS once, also inside a
function called twice.

Expectations were written from the program text before running the engine."""
CALLS = []


def load():
    CALLS.append("load")
    return {"a": 1}


def val():
    CALLS.append("val")
    return "x"


def probe(k):
    if k in load():
        pass
    if k == val():
        pass
    if hasattr(load(), "x"):
        pass
    if isinstance(load(), dict):
        pass
    if load().get("a"):
        pass
    if not load():
        pass
    return len(CALLS)


if __name__ == "__main__":
    n1 = probe("a")                              #@value n1: exactly={6}
    n2 = probe("b")                              #@value n2: exactly={12}
    loads = CALLS.count("load")                  #@value loads: exactly={10}
