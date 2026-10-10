"""Hypothesis: indexing a parameter is decided per call site when the argument is a literal
string (summaries gave an unknown): characters by position, from the end, and a slice."""


def ix(k):
    return k[0]


def ixn(k):
    return k[-1]


def pair(k):
    return k[0] + k[1]


e = ix("xy")                                      #@value e: exactly={"x"}
c = ixn("xyz")                                    #@value c: exactly={"z"}
p = pair("ab")                                    #@value p: exactly={"ab"}
a = ix(["l0"])                                    #@value a: exactly={"l0"}
t = ix(("t0", "t1"))                              #@value t: exactly={"t0"}
