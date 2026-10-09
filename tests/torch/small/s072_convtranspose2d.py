# nn.ConvTranspose2d output size (H-1)*s - 2p + d(k-1) + output_padding + 1, on known and unknown ranks
import torch
from torch import nn


def up2(x):
    N, C, H, W = x.shape
    dec = nn.ConvTranspose2d(C, C // 2, 2, stride=2)
    y = dec(x)  #@shape y: [x.shape[0], x.shape[1]//2, 2*x.shape[2], 2*x.shape[3]]
    #@requires none
    return y


def paddings(x):
    N, C, H, W = x.shape
    a = nn.ConvTranspose2d(16, 8, 3, stride=2, padding=1, output_padding=1)(x)  #@shape a: [x.shape[0], 8, 2*x.shape[2], 2*x.shape[3]]
    #@requires x.shape[1] == 16
    b = nn.ConvTranspose2d(16, 8, 3, stride=2, padding=1)(x)  #@shape b: [x.shape[0], 8, 2*x.shape[2] - 1, 2*x.shape[3] - 1]
    #@requires none
    c = nn.ConvTranspose2d(16, 4, 4)(x)  #@shape c: [x.shape[0], 4, x.shape[2] + 3, x.shape[3] + 3]
    return a, b, c


def any_rank(x):
    y = nn.ConvTranspose2d(8, 4, 3)(x)  #@shape y: [*x.shape[:-3], 4, x.shape[-2] + 2, x.shape[-1] + 2]
    #@requires x.dim() in (3, 4); x.shape[-3] == 8
    return y


def concrete():
    y = nn.ConvTranspose2d(4, 2, 3, stride=2, padding=1, output_padding=1)(torch.randn(1, 4, 7, 7))  #@shape y: [1, 2, 14, 14]
    return y


if __name__ == "__main__":
    up2(torch.randn(2, 6, 3, 5))
    paddings(torch.randn(1, 16, 4, 6))
    any_rank(torch.randn(8, 3, 3))
    concrete()
