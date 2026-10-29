"""Code rowan does not see (here `operator.delitem`, which rowan leaves unknown because it
changes its argument) given `obj.__dict__` or `vars(obj)` may change obj's attributes; given a
dict's `values()` it may change what the dict holds (not the dict itself). (Before,
a view given to such code was walked as if it held nothing: the attribute stayed as it was.
Reading the attribute in a `try` is not checked here: rowan takes it that unknown code removes
no attribute, `likely` in AGENTS.md.)

Expectations were written from the program text before running the engine."""
import operator


class P:
    def __init__(self, k):
        self.k = k


def through_dict(p):
    operator.delitem(p.__dict__, "k")
    kept1 = "k" in vars(p)
    #@value kept1: includes={False}


def through_vars(p):
    operator.delitem(vars(p), "k")
    kept2 = hasattr(p, "k")
    #@value kept2: includes={False}


def through_values():
    d = {"a": [1, 2]}
    for xs in d.values():
        operator.delitem(xs, 0)
    n = len(d["a"])
    #@value n: includes={1}


through_dict(P(1))
through_vars(P(1))
through_values()
