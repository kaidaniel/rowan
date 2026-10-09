# F.one_hot (literal / parameter / data-dependent class count) and F.pad on 1 and 2 trailing dims
import torch
import torch.nn.functional as F


def oh(t, k):
    y = F.one_hot(t, num_classes=10)  #@shape y: [*t.shape, 10]
    z = F.one_hot(t, k)  #@shape z: [*t.shape, k]
    return y, z


def pads(x):
    a = F.pad(x, (1, 2))  #@shape a: [*x.shape[:-1], x.shape[-1] + 3]
    #@requires none
    b = F.pad(x, (1, 1, 2, 2))  #@shape b: [*x.shape[:-2], x.shape[-2] + 4, x.shape[-1] + 2]
    #@requires x.dim() >= 2
    c = F.pad(x, (-1, -1))  #@shape c: [*x.shape[:-1], x.shape[-1] - 2]
    return a, b, c


def pads_known(x, p):
    N, C, H, W = x.shape
    y = F.pad(x, (0, 1, 0, 1), mode="constant", value=0.0)  #@shape y: [x.shape[0], x.shape[1], x.shape[2] + 1, x.shape[3] + 1]
    z = F.pad(x, (p, p))  #@shape z: [x.shape[0], x.shape[1], x.shape[2], x.shape[3] + 2*p]
    return y, z


def concrete():
    y = F.one_hot(torch.tensor([0, 2, 1]))  #@shape y: [3, ?]
    return y


if __name__ == "__main__":
    oh(torch.tensor([[1, 2], [3, 4]]), 6)
    pads(torch.randn(2, 3, 5))
    pads_known(torch.randn(1, 2, 3, 4), 2)
    concrete()
