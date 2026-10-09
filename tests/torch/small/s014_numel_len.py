# numel(), len(x) and len(x.shape): products of dims when the rank is known, dim 0 / the rank otherwise
import torch


def counts(x):
    B, T, C = x.shape
    #@requires x.dim() == 3
    n = x.numel()  #@dim n: x.shape[0] * x.shape[1] * x.shape[2]
    L = len(x)  #@dim L: x.shape[0]
    k = len(x.shape)  #@dim k: 3
    m = torch.zeros(B * T, C).numel()  #@dim m: x.shape[0] * x.shape[1] * x.shape[2]
    return n, L, k, m


def unknown_rank(x):
    L = len(x)  #@dim L: x.shape[0]
    r = len(x.shape)  #@dim r: x.dim()
    return L, r


def concrete():
    n = torch.zeros(2, 3, 4).numel()  #@dim n: 24
    L = len(torch.zeros(5, 2))  #@dim L: 5
    e = torch.zeros(()).numel()  #@dim e: 1
    return n, L, e


if __name__ == "__main__":
    counts(torch.randn(2, 3, 4))
    unknown_rank(torch.randn(7, 2))
    concrete()
