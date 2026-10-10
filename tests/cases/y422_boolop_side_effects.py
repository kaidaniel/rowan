"""Hypothesis: the operands of `and` / `or` that surely run (decided by the earlier operands)
keep their side effects exact: appends inside them are positional, and an undecided operand
makes the rest happen on some paths only.

Expectations were written from the program text before running the engine."""
import sys

LOG = []


def note(tag, v):
    LOG.append(tag)
    return v


if __name__ == "__main__":
    a = note("a", 1) and note("b", 2)            #@value a: exactly={2}
    t1 = tuple(LOG)                              #@value t1: exactly={("a","b")}
    o = note("c", 0) or note("d", "z")           #@value o: exactly={"z"}
    t2 = "".join(LOG)                            #@value t2: exactly={"abcd"}
    s = note("e", 0) and note("f", 1)            #@value s: exactly={0}
    t3 = "".join(LOG)                            #@value t3: exactly={"abcde"}
    u = note("g", len(sys.argv) > 3) and note("h", 1)
    n = len(LOG)                                 #@value n: includes={6,7}
    #@value n: exactly={6,7}
