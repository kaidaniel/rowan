# nn.Upsample(scale_factor / size) and F.interpolate(size= / scale_factor=) on 4-D and 3-D inputs
import torch
import torch.nn.functional as F
from torch import nn


def up2d(x):
    N, C, H, W = x.shape
    y = nn.Upsample(scale_factor=2)(x)  #@shape y: [x.shape[0], x.shape[1], 2*x.shape[2], 2*x.shape[3]]
    z = F.interpolate(x, size=(8, 8))  #@shape z: [x.shape[0], x.shape[1], 8, 8]
    w = F.interpolate(x, scale_factor=2, mode="bilinear", align_corners=False)  #@shape w: [x.shape[0], x.shape[1], 2*x.shape[2], 2*x.shape[3]]
    v = nn.Upsample(size=(5, 10), mode="nearest")(x)  #@shape v: [x.shape[0], x.shape[1], 5, 10]
    h = F.interpolate(x, scale_factor=0.5)  #@shape h: [x.shape[0], x.shape[1], x.shape[2]//2, x.shape[3]//2]
    return y, z, w, v, h


def up1d(x):
    N, C, L = x.shape
    y = F.interpolate(x, scale_factor=3)  #@shape y: [x.shape[0], x.shape[1], 3*x.shape[2]]
    z = F.interpolate(x, size=16)  #@shape z: [x.shape[0], x.shape[1], 16]
    return y, z


def concrete():
    y = nn.Upsample(scale_factor=2)(torch.randn(1, 3, 4, 5))  #@shape y: [1, 3, 8, 10]
    return y


if __name__ == "__main__":
    up2d(torch.randn(2, 3, 5, 6))
    up1d(torch.randn(2, 4, 7))
    concrete()
