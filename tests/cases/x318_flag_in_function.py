"""Weakness: correlated conditions inside a function called with an unknown flag.

Expectations were written from the program text before running the engine: the precise
answer, and a soundness floor (`sound`, `possible=`, `definite_within=`) that holds even
where the precise answer is out of reach."""


def f(d, flag):
    if flag:
        d["a"] = 1
    if flag:
        return d["a"]                        #@ ok a
    return None


def g(d, flag):
    if flag:
        d["a"] = 1
    if flag:
        return d["a"]                        #@ sound a
    return None


f({}, unknown)
g({}, unknown)
