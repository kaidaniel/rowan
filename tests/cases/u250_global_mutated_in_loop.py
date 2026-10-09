"""Hypothesis: a function called in a loop over known items that stores into a module-level
dict leaves every item's key there.

Expectations were written from the program text before running the engine."""

SEEN = {}


def mark(k):
    SEEN[k] = True


for k in ["a", "b"]:
    mark(k)
_ = 0                                        #@row SEEN: present={a,b} definite={a,b} closed
SEEN["c"]                                    #@ missing c
