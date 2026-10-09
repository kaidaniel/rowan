"""Hypothesis: the truth of an instance is decided by its `__bool__` / `__len__` when they
give known values, and an instance without either is true.

Expectations were written from the program text before running the engine."""


class Bag:
    def __init__(self, items):
        self.items = items

    def __len__(self):
        return len(self.items)


class Flag:
    def __bool__(self):
        return False


class Plain:
    pass


def check(x):
    out = {}
    if x:
        out["truthy"] = True
    else:
        out["falsy"] = True
    return out


a = check(Bag([]))                           #@row a: present={falsy} definite={falsy} closed
b = check(Bag([1]))                          #@row b: present={truthy} definite={truthy} closed
c = check(Flag())                            #@row c: present={falsy} definite={falsy} closed
d = check(Plain())                           #@row d: present={truthy} definite={truthy} closed
