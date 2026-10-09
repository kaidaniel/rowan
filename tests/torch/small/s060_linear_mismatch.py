# nn.Linear mismatches: literal sizes, after an assert, in a chain; a module-level layer on a parameter
import torch
from torch import nn

head = nn.Linear(32, 10)


def classify(x):
    y = head(x)  #@shape y: [*x.shape[:-1], 10]
    #@requires x.shape[-1] == 32
    return y


def bad_concrete():
    lin = nn.Linear(16, 8)
    x = torch.randn(4, 10)
    y = lin(x)  #@shape-error
    return y


def bad_after_assert(x):
    assert x.shape[-1] == 64
    y = head(x)  #@shape-error
    return y


def bad_chain(x):
    l1 = nn.Linear(8, 16)
    l2 = nn.Linear(32, 4)
    h = l1(x)  #@shape h: [*x.shape[:-1], 16]
    #@requires x.shape[-1] == 8
    y = l2(h)  #@shape-error
    return y


if __name__ == "__main__":
    classify(torch.randn(5, 32))
    for f, args in [(bad_concrete, ()), (bad_after_assert, (torch.randn(2, 64),)), (bad_chain, (torch.randn(3, 8),))]:
        try:
            f(*args)
        except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
            pass
