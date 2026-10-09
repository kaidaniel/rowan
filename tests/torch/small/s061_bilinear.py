# nn.Bilinear: two inputs with their own feature sizes, batch dims must agree
import torch
from torch import nn


def bilinear_2d(x1, x2):
    N, A = x1.shape
    M, B = x2.shape
    bil = nn.Bilinear(5, 7, 3)
    y = bil(x1, x2)  #@shape y: [x1.shape[0], 3]
    #@requires x1.shape[1] == 5; x2.shape[1] == 7; x1.shape[0] == x2.shape[0]
    w = bil.weight  #@shape w: [3, 5, 7]
    return y, w


def bilinear_params(n, a, b, o):
    bil = nn.Bilinear(a, b, o)
    y = bil(torch.randn(n, a), torch.randn(n, b))  #@shape y: [n, o]
    #@requires none
    return y


def concrete():
    bil = nn.Bilinear(4, 6, 2)
    y = bil(torch.randn(2, 9, 4), torch.randn(2, 9, 6))  #@shape y: [2, 9, 2]
    return y


def bad_features():
    bil = nn.Bilinear(5, 7, 3)
    y = bil(torch.randn(4, 5), torch.randn(4, 8))  #@shape-error
    return y


if __name__ == "__main__":
    bilinear_2d(torch.randn(4, 5), torch.randn(4, 7))
    bilinear_params(3, 2, 6, 5)
    concrete()
    try:
        bad_features()
    except (RuntimeError, ValueError, IndexError, TypeError, AssertionError):
        pass
