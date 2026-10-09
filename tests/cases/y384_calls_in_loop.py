"""A function called in a loop on one dict: inside, keys grow call by call.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""


def bump(counter, key):
    counter[key] = counter.get(key, 0) + 1
    _ = 0                                    #@row counter: present={a,b} definite={a} closed
    return counter


c = {}
for k in ["a", "b", "a"]:
    bump(c, k)
