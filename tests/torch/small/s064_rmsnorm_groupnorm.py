# nn.RMSNorm (int and list shape) and nn.GroupNorm (channels at dim 1, any trailing dims)
import torch
from torch import nn


def rms(x, d):
    norm = nn.RMSNorm(d)
    y = norm(x)  #@shape y: [*x.shape[:-1], d]
    #@requires x.shape[-1] == d
    return y


def rms_list(x):
    B, T, C = x.shape
    y = nn.RMSNorm([T, C])(x)  #@shape y: [x.shape[0], x.shape[1], x.shape[2]]
    #@requires none
    return y


def group(x):
    gn = nn.GroupNorm(4, 32)
    y = gn(x)  #@shape y: [x.shape[0], 32, *x.shape[2:]]
    #@requires x.dim() >= 2; x.shape[1] == 32
    return y


def concrete():
    y = nn.GroupNorm(2, 6)(torch.randn(2, 6, 5, 5))  #@shape y: [2, 6, 5, 5]
    return y


def bad_channels():
    gn = nn.GroupNorm(4, 32)
    y = gn(torch.randn(2, 16, 8))  #@shape-error
    return y


if __name__ == "__main__":
    rms(torch.randn(4, 6), 6)
    rms_list(torch.randn(2, 3, 4))
    group(torch.randn(2, 32, 7))
    group(torch.randn(2, 32))
    concrete()
    try:
        bad_channels()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass
