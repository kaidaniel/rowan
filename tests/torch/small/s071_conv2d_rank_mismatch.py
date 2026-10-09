# nn.Conv2d on unknown rank (3-D or 4-D), on unbatched 3-D input, and channel / rank mismatches
import torch
from torch import nn


def conv_any(x):
    conv = nn.Conv2d(3, 8, 3)
    y = conv(x)  #@shape y: [*x.shape[:-3], 8, x.shape[-2] - 2, x.shape[-1] - 2]
    #@requires x.dim() in (3, 4); x.shape[-3] == 3
    return y


def conv_unbatched(x):
    C, H, W = x.shape
    #@requires x.dim() == 3
    y = nn.Conv2d(C, 4, 1)(x)  #@shape y: [4, x.shape[1], x.shape[2]]
    #@requires none
    return y


def concrete():
    y = nn.Conv2d(3, 6, 3, padding=1)(torch.randn(3, 10, 12))  #@shape y: [6, 10, 12]
    return y


def bad_channels():
    x = torch.randn(2, 4, 16, 16)
    y = nn.Conv2d(3, 8, 3)(x)  #@shape-error
    return y


def bad_rank():
    x = torch.randn(16, 16)
    y = nn.Conv2d(3, 8, 3)(x)  #@shape-error
    return y


if __name__ == "__main__":
    conv_any(torch.randn(2, 3, 7, 9))
    conv_any(torch.randn(3, 7, 9))
    conv_unbatched(torch.randn(5, 6, 7))
    concrete()
    for f in (bad_channels, bad_rank):
        try:
            f()
        except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
            pass
