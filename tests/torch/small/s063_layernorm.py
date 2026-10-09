# nn.LayerNorm with an int and a tuple normalized_shape, from a parameter, and F.layer_norm
import torch
import torch.nn.functional as F
from torch import nn


def ln_int(x):
    ln = nn.LayerNorm(64)
    y = ln(x)  #@shape y: [*x.shape[:-1], 64]
    #@requires x.shape[-1] == 64
    return y


def ln_param(x, d):
    y = nn.LayerNorm(d)(x)  #@shape y: [*x.shape[:-1], d]
    #@requires x.shape[-1] == d
    return y


def ln_tuple(x):
    ln = nn.LayerNorm((4, 8))
    y = ln(x)  #@shape y: [*x.shape[:-2], 4, 8]
    #@requires x.dim() >= 2; x.shape[-2] == 4; x.shape[-1] == 8
    w = ln.weight
    return y, w


def f_ln(x):
    B, T, C = x.shape
    y = F.layer_norm(x, (C,))  #@shape y: [x.shape[0], x.shape[1], x.shape[2]]
    #@requires none
    return y


def bad_size():
    ln = nn.LayerNorm(64)
    y = ln(torch.randn(2, 32))  #@shape-error
    return y


if __name__ == "__main__":
    ln_int(torch.randn(3, 64))
    ln_param(torch.randn(2, 5, 7), 7)
    ln_tuple(torch.randn(2, 4, 8))
    f_ln(torch.randn(2, 3, 5))
    try:
        bad_size()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass
