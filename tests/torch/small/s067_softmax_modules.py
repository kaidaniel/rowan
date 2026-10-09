# nn.Softmax(dim) / nn.LogSoftmax keep the shape; a dim beyond the rank is a rank requirement or an error
import torch
import torch.nn.functional as F
from torch import nn


def sm_last(x):
    y = nn.Softmax(dim=-1)(x)  #@shape y: [*x.shape]
    #@requires none
    return y


def sm_dim1(x):
    y = nn.Softmax(dim=1)(x)  #@shape y: [*x.shape]
    #@requires x.dim() >= 2
    z = nn.LogSoftmax(dim=1)(y)  #@shape z: [*x.shape]
    #@requires none
    return z


def f_sm(x):
    B, T, C = x.shape
    p = F.softmax(x, dim=2)  #@shape p: [x.shape[0], x.shape[1], x.shape[2]]
    #@requires none
    lp = F.log_softmax(x, dim=-1)  #@shape lp: [x.shape[0], x.shape[1], x.shape[2]]
    return p, lp


def bad_dim():
    x = torch.randn(4, 5)
    y = nn.Softmax(dim=2)(x)  #@shape-error
    return y


if __name__ == "__main__":
    sm_last(torch.randn(5))
    sm_dim1(torch.randn(2, 3, 4))
    f_sm(torch.randn(2, 3, 4))
    try:
        bad_dim()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass
