# loops: a body that keeps the shape keeps it after the loop; a body that changes a dim (or the rank) makes it unknown
import torch


def keep(x, k):
    h = x
    for _ in range(k):
        h = torch.tanh(h) * 2 + 1  #@shape h: [*x.shape]
    z = h  #@shape z: [*x.shape]
    return z


def keep_known(x, k):
    B, C = x.shape
    #@requires x.dim() == 2
    h = torch.zeros(B, C)
    for i in range(k):
        h = h + x * i
    z = h  #@shape z: [x.shape[0], x.shape[1]]
    return z


def grow(n, k):
    h = torch.zeros(1, n)
    for i in range(k):
        h = torch.zeros(h.shape[0] + 1, n)
    z = h  #@shape z: [?, n]
    return z


def rank_change(n, k):
    h = torch.zeros(n)
    for i in range(k):
        h = torch.zeros(2, n)
    z = h  #@shape z: [*, n]
    return z


def while_halve(x):
    h = x
    while h.abs().max() > 1:
        h = h / 2
    z = h  #@shape z: [*x.shape]
    return z


if __name__ == "__main__":
    keep(torch.randn(2, 3), 3)
    keep_known(torch.randn(2, 3), 2)
    grow(4, 3)
    grow(4, 0)
    rank_change(3, 0)
    rank_change(3, 2)
    while_halve(torch.randn(5) * 10)
