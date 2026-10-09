# nn.Conv1d output length: kernel, stride, padding, dilation, padding="same"; unknown rank (2-D or 3-D)
import torch
from torch import nn


def conv1d_formulas(x):
    N, C, L = x.shape
    a = nn.Conv1d(C, 16, 3)(x)  #@shape a: [x.shape[0], 16, x.shape[2] - 2]
    #@requires none
    b = nn.Conv1d(16, 16, 3, stride=2, padding=1)(a)  #@shape b: [x.shape[0], 16, (x.shape[2] - 3)//2 + 1]
    #@requires none
    c = nn.Conv1d(C, 8, 3, dilation=2)(x)  #@shape c: [x.shape[0], 8, x.shape[2] - 4]
    d = nn.Conv1d(C, 8, 5, padding="same")(x)  #@shape d: [x.shape[0], 8, x.shape[2]]
    e = nn.Conv1d(C, 8, 3, padding="same", dilation=2)(x)  #@shape e: [x.shape[0], 8, x.shape[2]]
    return b, c, d, e


def conv1d_any(x):
    conv = nn.Conv1d(4, 8, 5, padding=2)
    y = conv(x)  #@shape y: [*x.shape[:-2], 8, x.shape[-1]]
    #@requires x.dim() in (2, 3); x.shape[-2] == 4
    return y


def concrete():
    y = nn.Conv1d(3, 6, 4, stride=2)(torch.randn(1, 3, 20))  #@shape y: [1, 6, 9]
    return y


if __name__ == "__main__":
    conv1d_formulas(torch.randn(2, 3, 21))
    conv1d_formulas(torch.randn(1, 5, 10))
    conv1d_any(torch.randn(2, 4, 9))
    conv1d_any(torch.randn(4, 9))
    concrete()
