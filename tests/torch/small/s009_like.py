# zeros_like / ones_like / randn_like / rand_like / full_like / empty_like copy the input's shape
import torch


def like(x):
    a = torch.zeros_like(x)  #@shape a: [*x.shape]
    b = torch.ones_like(x, dtype=torch.float32)  #@shape b: [*x.shape]
    c = torch.randn_like(x)  #@shape c: [*x.shape]
    d = torch.full_like(x, 3.0)  #@shape d: [*x.shape]
    e = torch.empty_like(x)  #@shape e: [*x.shape]
    #@requires none
    return a, b, c, d, e


def like_known(x):
    B, T = x.shape
    #@requires x.dim() == 2
    m = torch.ones_like(x)  #@shape m: [x.shape[0], x.shape[1]]
    t = torch.zeros(B, T + 1)
    u = torch.rand_like(t)  #@shape u: [x.shape[0], x.shape[1] + 1]
    return m, u


def concrete():
    z = torch.ones_like(torch.zeros(2, 5))  #@shape z: [2, 5]
    return z


if __name__ == "__main__":
    like(torch.randn(2, 3, 4))
    like(torch.randn(()))
    like_known(torch.randn(3, 4))
    concrete()
