# nn.Flatten (default and explicit dims) and nn.Unflatten, on known ranks and on an unknown last dim
import torch
from torch import nn


def flat4(x):
    N, C, H, W = x.shape
    y = nn.Flatten()(x)  #@shape y: [x.shape[0], x.shape[1]*x.shape[2]*x.shape[3]]
    z = nn.Unflatten(1, (C, H, W))(y)  #@shape z: [x.shape[0], x.shape[1], x.shape[2], x.shape[3]]
    a = nn.Flatten(0, 1)(x)  #@shape a: [x.shape[0]*x.shape[1], x.shape[2], x.shape[3]]
    b = nn.Flatten(start_dim=2)(x)  #@shape b: [x.shape[0], x.shape[1], x.shape[2]*x.shape[3]]
    return z, a, b


def unflat_last(x):
    y = nn.Unflatten(-1, (4, 8))(x)  #@shape y: [*x.shape[:-1], 4, 8]
    #@requires x.shape[-1] == 32
    return y


def concrete():
    y = nn.Flatten()(torch.randn(2, 3, 4, 5))  #@shape y: [2, 60]
    z = nn.Unflatten(1, (3, 20))(y)  #@shape z: [2, 3, 20]
    return z


if __name__ == "__main__":
    flat4(torch.randn(2, 3, 4, 5))
    unflat_last(torch.randn(3, 32))
    concrete()
