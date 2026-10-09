# nn.MaxPool2d / AvgPool2d (conv formula) and AdaptiveAvgPool2d((1, 1)) / (7), known and unknown ranks
import torch
from torch import nn


def pools(x):
    N, C, H, W = x.shape
    a = nn.MaxPool2d(2)(x)  #@shape a: [x.shape[0], x.shape[1], (x.shape[2] - 2)//2 + 1, (x.shape[3] - 2)//2 + 1]
    b = nn.MaxPool2d(3, stride=2, padding=1)(x)  #@shape b: [x.shape[0], x.shape[1], (x.shape[2] - 1)//2 + 1, (x.shape[3] - 1)//2 + 1]
    c = nn.AvgPool2d(3, stride=1, padding=1)(x)  #@shape c: [x.shape[0], x.shape[1], x.shape[2], x.shape[3]]
    d = nn.AdaptiveAvgPool2d((1, 1))(x)  #@shape d: [x.shape[0], x.shape[1], 1, 1]
    e = nn.AdaptiveAvgPool2d(7)(x)  #@shape e: [x.shape[0], x.shape[1], 7, 7]
    #@requires none
    return a, b, c, d, e


def pool_any(x):
    y = nn.AdaptiveAvgPool2d((1, 1))(x)  #@shape y: [*x.shape[:-2], 1, 1]
    #@requires x.dim() in (3, 4)
    z = nn.MaxPool2d(2)(x)  #@shape z: [*x.shape[:-2], (x.shape[-2] - 2)//2 + 1, (x.shape[-1] - 2)//2 + 1]
    #@requires none
    return y, z


def concrete():
    a = nn.MaxPool2d(2)(torch.randn(1, 3, 7, 7))  #@shape a: [1, 3, 3, 3]
    b = nn.AdaptiveAvgPool2d((2, 3))(torch.randn(4, 9, 9))  #@shape b: [4, 2, 3]
    return a, b


if __name__ == "__main__":
    pools(torch.randn(2, 3, 9, 8))
    pool_any(torch.randn(2, 3, 9, 8))
    pool_any(torch.randn(3, 6, 5))
    concrete()
