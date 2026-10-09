# nn.BatchNorm1d (2-D or 3-D input, channels at dim 1) and nn.BatchNorm2d (4-D only)
import torch
from torch import nn


def bn1d(x):
    bn = nn.BatchNorm1d(16)
    y = bn(x)  #@shape y: [x.shape[0], 16, *x.shape[2:]]
    #@requires x.dim() in (2, 3); x.shape[1] == 16
    return y


def bn1d_seq(x):
    N, C, L = x.shape
    y = nn.BatchNorm1d(C)(x)  #@shape y: [x.shape[0], x.shape[1], x.shape[2]]
    #@requires none
    return y


def bn2d(x):
    bn = nn.BatchNorm2d(8)
    y = bn(x)  #@shape y: [x.shape[0], 8, x.shape[2], x.shape[3]]
    #@requires x.dim() == 4; x.shape[1] == 8
    return y


def bad_rank():
    y = nn.BatchNorm2d(8)(torch.randn(8, 5, 5))  #@shape-error
    return y


def bad_channels():
    y = nn.BatchNorm1d(16)(torch.randn(4, 8))  #@shape-error
    return y


if __name__ == "__main__":
    bn1d(torch.randn(4, 16))
    bn1d(torch.randn(4, 16, 3))
    bn1d_seq(torch.randn(3, 5, 7))
    bn2d(torch.randn(2, 8, 4, 4))
    for f in (bad_rank, bad_channels):
        try:
            f()
        except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
            pass
