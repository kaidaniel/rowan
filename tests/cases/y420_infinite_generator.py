"""Hypothesis: calling an infinite generator (`while True: yield …`) does not run forever: the
code after the call, after `next(g)` and after a `for` that breaks out of it runs.

Expectations were written from the program text before running the engine."""


def counter(start):
    n = start
    while True:
        yield n
        n += 1


def first_over(g, stop):
    for x in g:
        if x > stop:
            return x
    return None


if __name__ == "__main__":
    g = counter(0)
    a = next(g)
    after_next = "yes"                           #@value after_next: exactly={"yes"}
    for y in counter(5):
        if y > 6:
            break
    after_for = "yes"                            #@value after_for: exactly={"yes"}
    f = first_over(counter(1), 3)
    after_call = "yes"                           #@value after_call: exactly={"yes"}
