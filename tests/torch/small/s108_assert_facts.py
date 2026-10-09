# asserts give facts: rank and last dim (named or literal); a later layer is then decided, even wrong
import torch
import torch.nn as nn


def with_dim(x, d):
    assert x.dim() == 3
    #@requires none
    assert x.shape[-1] == d
    #@requires none
    y = x.sum(dim=1)  #@shape y: [x.shape[0], d]
    lin = nn.Linear(d, 5)
    z = lin(x)  #@shape z: [x.shape[0], x.shape[1], 5]
    #@requires none
    return y, z


def with_literal(x):
    assert x.shape[-1] == 512
    lin = nn.Linear(512, 10)
    y = lin(x)  #@shape y: [*x.shape[:-1], 10]
    #@requires none
    w = x * 2.0  #@shape w: [*x.shape[:-1], 512]
    return y, w


def wrong_after_assert(x):
    assert x.shape[-1] == 512
    lin = nn.Linear(256, 10)
    y = lin(x)  #@shape-error
    return y


if __name__ == "__main__":
    y, z = with_dim(torch.randn(2, 3, 4), 4)  #@shape y: [2, 4]; z: [2, 3, 5]
    y2, w = with_literal(torch.randn(6, 512))  #@shape y2: [6, 10]; w: [6, 512]
    try:
        wrong_after_assert(torch.randn(2, 512))
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass
