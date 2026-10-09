# `!=`, `<`, `>`, `>=` on ranks and dims in asserts and ifs learn no equality facts
import torch
import torch.nn as nn


def ne_rank(x):
    assert x.dim() != 2
    #@requires none
    y = x.sum(dim=-1)  #@shape y: [*x.shape[:-1]]
    return y


def lt_pad(x, d):
    B, T, C = x.shape
    if C < d:
        pad = torch.zeros(B, T, d - C)  #@shape pad: [x.shape[0], x.shape[1], d - x.shape[2]]
        #@requires none
        x2 = torch.cat([x, pad], dim=-1)  #@shape x2: [x.shape[0], x.shape[1], d]
        #@requires none
    else:
        x2 = x[:, :, :d]  #@shape x2: [x.shape[0], x.shape[1], min(d, x.shape[2])]
    return x2


def gt_dim(x):
    assert x.shape[-1] > 4
    lin = nn.Linear(4, 2)
    # `> 4` is no fact: the slice stays min(4, d) (a wrong `== 4` fact would print 4)
    h = x[..., :4]  #@shape h: [*x.shape[:-1], min(4, x.shape[-1])]
    y = lin(h)  #@shape y: [*x.shape[:-1], 2]
    return y


def ge_rank(x, w):
    if x.dim() >= 3:
        z = x @ w  #@shape z: [*broadcast(x.shape[:-2], w.shape[:-2]), x.shape[-2], w.shape[-1]] if w.dim() >= 2
    else:
        z = x  #@shape z: [*x.shape]
    return z


def ne_size(x, n):
    if x.size(0) != n:
        y = x[:n]  #@shape y: [min(n, x.shape[0]), *x.shape[1:]]
    else:
        y = x  #@shape y: [n, *x.shape[1:]]
    return y


if __name__ == "__main__":
    a = ne_rank(torch.randn(2, 3, 4))  #@shape a: [2, 3]
    b = lt_pad(torch.randn(2, 3, 4), 6)  #@shape b: [2, 3, 6]
    c = lt_pad(torch.randn(2, 3, 8), 6)  #@shape c: [2, 3, 6]
    d = gt_dim(torch.randn(5, 7))  #@shape d: [5, 2]
    e = ge_rank(torch.randn(2, 3, 4), torch.randn(4, 5))  #@shape e: [2, 3, 5]
    f = ge_rank(torch.randn(3, 4), torch.randn(4, 5))  #@shape f: [3, 4]
    g = ne_size(torch.randn(6, 2), 4)  #@shape g: [4, 2]
    h = ne_size(torch.randn(4, 2), 4)  #@shape h: [4, 2]
