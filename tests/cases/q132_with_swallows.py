"""Hypothesis: a context manager whose `__exit__` returns True swallows exceptions, so a
definitely-missing access inside the block does not stop the code after the block; one
that returns False does not swallow, so the code after it is dead for that path.

Expectations were written from the program text before running the engine."""


class Quiet:
    def __enter__(self):
        return self

    def __exit__(self, *exc):
        return True


class Loud:
    def __enter__(self):
        return self

    def __exit__(self, *exc):
        return False


d = {"a": 1}
out = {"start": True}
with Quiet():
    out["before"] = True
    x = d["missing"]
    out["after"] = True
out["done"] = True
_ = 0                                     #@row out: present={start,before,done} definite={start,before,done} closed
out["after"]                              #@ missing after
