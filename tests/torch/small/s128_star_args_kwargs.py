# *args of tensors and **kwargs carrying dims / options: forwarded through helpers and dict(**) unpacking
import torch
import torch.nn.functional as F


def cat_all(*xs, dim=0):
    return torch.cat(xs, dim=dim)


def apply(fn, *args, **kwargs):
    return fn(*args, **kwargs)


def make(n, d):
    return torch.zeros(n, d)


def kw_shape(x, **kw):
    B, T = x.shape
    return x.view(B, kw["heads"], T // kw["heads"])


def use(x):
    B, T, C = x.shape
    c = cat_all(x, x, dim=1)  #@shape c: [x.shape[0], 2*x.shape[1], x.shape[2]]
    #@requires none
    s = cat_all(x, x)  #@shape s: [2*x.shape[0], x.shape[1], x.shape[2]]
    z = apply(torch.zeros, 2, 3)  #@shape z: [2, 3]
    p = apply(F.pad, x, (1, 1))  #@shape p: [x.shape[0], x.shape[1], x.shape[2] + 2]
    k = make(**{"n": 3, "d": C})  #@shape k: [3, x.shape[2]]
    opts = dict(dim=-1, keepdim=True)
    m = x.mean(**opts)  #@shape m: [x.shape[0], x.shape[1], 1]
    args = (B, T * C)
    r = x.reshape(*args)  #@shape r: [x.shape[0], x.shape[1]*x.shape[2]]
    v = kw_shape(r, heads=2)  #@shape v: [x.shape[0], 2, x.shape[1]*x.shape[2] // 2]
    return c, s, z, p, k, m, v


if __name__ == "__main__":
    c, s, z, p, k, m, v = use(torch.randn(2, 3, 4))  #@shape c: [2, 6, 4]; s: [4, 3, 4]; p: [2, 3, 6]; v: [2, 2, 6]
