# nn.Linear with literal sizes on inputs whose rank the function fixes (2-D, 3-D), and concrete calls
import torch
from torch import nn


def linear_2d(x):
    N, D = x.shape
    #@requires x.dim() == 2
    lin = nn.Linear(32, 10)
    y = lin(x)  #@shape y: [x.shape[0], 10]
    #@requires x.shape[1] == 32
    z = lin(x)  #@shape z: [x.shape[0], 10]
    #@requires none
    return y, z


def linear_3d(x):
    B, T, C = x.shape
    #@requires x.dim() == 3
    up = nn.Linear(C, 4 * C)
    h = up(x)  #@shape h: [x.shape[0], x.shape[1], 4*x.shape[2]]
    #@requires none
    back = nn.Linear(4 * C, C)(h)  #@shape back: [x.shape[0], x.shape[1], x.shape[2]]
    #@requires none
    return back


def concrete():
    lin = nn.Linear(20, 5)
    a = lin(torch.randn(3, 7, 20))  #@shape a: [3, 7, 5]
    b = lin(torch.randn(20))  #@shape b: [5]
    c = lin(torch.randn(2, 2, 2, 20))  #@shape c: [2, 2, 2, 5]
    return a, b, c


if __name__ == "__main__":
    linear_2d(torch.randn(4, 32))
    linear_3d(torch.randn(2, 5, 6))
    concrete()
