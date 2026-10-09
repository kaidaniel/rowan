# nn.Conv2d output sizes with int and tuple kernel/stride/padding, sizes from parameters, the weight shape
import torch
from torch import nn


def conv2d_int(x):
    N, C, H, W = x.shape
    y = nn.Conv2d(3, 16, 3, padding=1)(x)  #@shape y: [x.shape[0], 16, x.shape[2], x.shape[3]]
    #@requires x.shape[1] == 3
    s = nn.Conv2d(16, 32, 3, stride=2, padding=1)(y)  #@shape s: [x.shape[0], 32, (x.shape[2] - 1)//2 + 1, (x.shape[3] - 1)//2 + 1]
    #@requires none
    return s


def conv2d_tuple(x):
    N, C, H, W = x.shape
    conv = nn.Conv2d(C, 8, (3, 5), stride=(1, 2), padding=(1, 0))
    y = conv(x)  #@shape y: [x.shape[0], 8, x.shape[2], (x.shape[3] - 5)//2 + 1]
    return y


def conv_params(x, ci, co, k):
    N, C, H, W = x.shape
    conv = nn.Conv2d(ci, co, k)
    y = conv(x)  #@shape y: [x.shape[0], co, x.shape[2] - k + 1, x.shape[3] - k + 1]
    #@requires x.shape[1] == ci
    w = conv.weight  #@shape w: [co, ci, k, k]
    return y, w


def concrete():
    y = nn.Conv2d(3, 8, 5, stride=2, padding=2)(torch.randn(2, 3, 32, 32))  #@shape y: [2, 8, 16, 16]
    return y


if __name__ == "__main__":
    conv2d_int(torch.randn(2, 3, 9, 8))
    conv2d_tuple(torch.randn(1, 2, 6, 12))
    conv_params(torch.randn(2, 4, 10, 11), 4, 6, 3)
    concrete()
